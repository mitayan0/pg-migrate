use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

/// Table information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub row_count: i64,
    pub size_bytes: i64,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
    pub is_primary_key: bool,
    pub ordinal_position: i32,
}

/// Foreign key dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub constraint_name: String,
    pub table_schema: String,
    pub table_name: String,
    pub column_name: String,
    pub foreign_table_schema: String,
    pub foreign_table_name: String,
    pub foreign_column_name: String,
}

/// Table dependency info for sorting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDependency {
    pub schema: String,
    pub name: String,
    pub depends_on: Vec<(String, String)>, // (schema, table)
}

/// Full table schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub schema_name: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key_columns: Vec<String>,
    pub create_statement: String,
}

/// List all tables in the database
pub async fn list_tables(pool: &PgPool) -> Result<Vec<TableInfo>, String> {
    let query = r#"
        SELECT 
            t.table_name,
            t.table_schema,
            COALESCE(pg_total_relation_size(c.oid), 0) as size_bytes
        FROM information_schema.tables t
        LEFT JOIN pg_catalog.pg_namespace n ON n.nspname = t.table_schema
        LEFT JOIN pg_catalog.pg_class c ON c.relname = t.table_name AND c.relnamespace = n.oid
        WHERE t.table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
            AND t.table_type = 'BASE TABLE'
        ORDER BY t.table_schema, t.table_name
    "#;

    let rows = sqlx::query(query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list tables: {}", e))?;

    let mut tables = Vec::new();

    for row in rows {
        let name: String = row.get("table_name");
        let schema: String = row.get("table_schema");
        let size_bytes: i64 = row.get("size_bytes");

        // Fetch EXACT row count for each table
        let count_query = format!("SELECT COUNT(*) FROM \"{}\".\"{}\"", schema, name);
        let row_count: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        tables.push(TableInfo {
            name,
            schema,
            row_count,
            size_bytes,
        });
    }

    Ok(tables)
}

/// Get exact row count for a table
pub async fn get_row_count(pool: &PgPool, schema: &str, table: &str) -> Result<i64, String> {
    let query = format!(
        "SELECT COUNT(*) as count FROM {}.{}",
        quote_ident(schema),
        quote_ident(table)
    );

    let row = sqlx::query(&query)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count rows: {}", e))?;

    Ok(row.get::<i64, _>("count"))
}

/// Get table schema (columns, types, constraints)
pub async fn get_table_schema(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<TableSchema, String> {
    // Get columns
    let columns_query = r#"
        SELECT 
            c.column_name,
            c.data_type,
            c.is_nullable = 'YES' as is_nullable,
            c.column_default,
            c.ordinal_position,
            CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END as is_primary_key
        FROM information_schema.columns c
        LEFT JOIN (
            SELECT kcu.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu 
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            WHERE tc.table_schema = $1 
                AND tc.table_name = $2 
                AND tc.constraint_type = 'PRIMARY KEY'
        ) pk ON c.column_name = pk.column_name
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position
    "#;

    let rows = sqlx::query(columns_query)
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get columns: {}", e))?;

    let columns: Vec<ColumnInfo> = rows
        .iter()
        .map(|row| ColumnInfo {
            name: row.get("column_name"),
            data_type: row.get("data_type"),
            is_nullable: row.get("is_nullable"),
            column_default: row.get("column_default"),
            ordinal_position: row.get("ordinal_position"),
            is_primary_key: row.get("is_primary_key"),
        })
        .collect();

    let primary_key_columns: Vec<String> = columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.clone())
        .collect();

    // Generate CREATE TABLE statement
    let create_statement =
        generate_create_table_statement(schema, table, &columns, &primary_key_columns);

    Ok(TableSchema {
        table_name: table.to_string(),
        schema_name: schema.to_string(),
        columns,
        primary_key_columns,
        create_statement,
    })
}

/// Generate CREATE TABLE statement from schema info
fn generate_create_table_statement(
    schema: &str,
    table: &str,
    columns: &[ColumnInfo],
    primary_keys: &[String],
) -> String {
    let mut sql = format!(
        "CREATE TABLE {}.{} (\n",
        quote_ident(schema),
        quote_ident(table)
    );

    let column_defs: Vec<String> = columns
        .iter()
        .map(|col| {
            let mut data_type = col.data_type.clone();
            let mut default_clause = String::new();

            // Detect SERIAL/BIGSERIAL patterns to avoid "sequence does not exist" errors
            let is_sequence = col
                .column_default
                .as_ref()
                .map_or(false, |d| d.contains("nextval"));

            if is_sequence {
                if data_type.to_lowercase() == "integer" {
                    data_type = "SERIAL".to_string();
                } else if data_type.to_lowercase() == "bigint" {
                    data_type = "BIGSERIAL".to_string();
                } else if data_type.to_lowercase() == "smallint" {
                    data_type = "SMALLSERIAL".to_string();
                } else {
                    // Fallback to original if we don't know the serial type
                    if let Some(ref default) = col.column_default {
                        default_clause = format!(" DEFAULT {}", default);
                    }
                }
            } else if let Some(ref default) = col.column_default {
                default_clause = format!(" DEFAULT {}", default);
            }

            let mut def = format!("    {} {}", quote_ident(&col.name), data_type);
            if !col.is_nullable && !is_sequence {
                // SERIAL implies NOT NULL
                def.push_str(" NOT NULL");
            }

            def.push_str(&default_clause);
            def
        })
        .collect();

    sql.push_str(&column_defs.join(",\n"));

    if !primary_keys.is_empty() {
        let pk_cols: Vec<String> = primary_keys.iter().map(|c| quote_ident(c)).collect();
        sql.push_str(&format!(",\n    PRIMARY KEY ({})", pk_cols.join(", ")));
    }

    sql.push_str("\n);");
    sql
}

/// List all schemas in the database (excluding system schemas)
pub async fn list_schemas(pool: &PgPool) -> Result<Vec<String>, String> {
    let query = r#"
        SELECT schema_name 
        FROM information_schema.schemata 
        WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast') 
          AND schema_name NOT LIKE 'pg_temp_%' 
          AND schema_name NOT LIKE 'pg_toast_temp_%'
        ORDER BY schema_name
    "#;

    let rows = sqlx::query(query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list schemas: {}", e))?;

    Ok(rows.iter().map(|r| r.get("schema_name")).collect())
}

/// Quote an identifier for PostgreSQL
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Get all table dependencies (Foreign Keys)
pub async fn get_all_dependencies(pool: &PgPool) -> Result<Vec<TableDependency>, String> {
    let query = r#"
        SELECT
            tc.table_schema,
            tc.table_name,
            ccu.table_schema AS foreign_table_schema,
            ccu.table_name AS foreign_table_name
        FROM information_schema.table_constraints AS tc
        JOIN information_schema.key_column_usage AS kcu
            ON tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage AS ccu
            ON ccu.constraint_name = tc.constraint_name
            AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
            AND tc.table_schema NOT IN ('pg_catalog', 'information_schema')
    "#;

    let rows = sqlx::query(query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get dependencies: {}", e))?;

    let mut map: std::collections::HashMap<(String, String), Vec<(String, String)>> =
        std::collections::HashMap::new();

    for row in rows {
        let schema: String = row.get("table_schema");
        let table: String = row.get("table_name");
        let f_schema: String = row.get("foreign_table_schema");
        let f_table: String = row.get("foreign_table_name");

        // Avoid self-references causing issues (though they are valid dependencies, simple sorts might strictly fail or we handle them specially)
        if schema != f_schema || table != f_table {
            map.entry((schema, table))
                .or_default()
                .push((f_schema, f_table));
        }
    }

    // Convert map to vector
    let dependencies = map
        .into_iter()
        .map(|((schema, name), depends_on)| TableDependency {
            schema,
            name,
            depends_on,
        })
        .collect();

    Ok(dependencies)
}
