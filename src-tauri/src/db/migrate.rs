use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use super::schema::{get_row_count, get_table_schema};

/// Migration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOptions {
    pub create_table_if_not_exists: bool,
    pub truncate_before_insert: bool,
    pub disable_constraints: bool,
    pub batch_size: usize,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            create_table_if_not_exists: true,
            truncate_before_insert: false,
            disable_constraints: true,
            batch_size: 1000,
        }
    }
}

/// Migration progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub table_name: String,
    pub current_table: usize,
    pub total_tables: usize,
    pub rows_transferred: i64,
    pub total_rows: i64,
    pub status: String,
    pub error: Option<String>,
}

/// Migration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub success: bool,
    pub tables_migrated: usize,
    pub total_rows: i64,
    pub errors: Vec<String>,
    pub elapsed_ms: u64,
}

/// Cancellation token for migrations
pub type CancellationToken = Arc<AtomicBool>;

pub fn create_cancellation_token() -> CancellationToken {
    Arc::new(AtomicBool::new(false))
}

/// Migrate tables from source to target
pub async fn migrate_tables(
    app_handle: AppHandle,
    source_pool: &PgPool,
    target_pool: &PgPool,
    tables: Vec<(String, String)>, // (schema, table)
    options: MigrationOptions,
    cancel_token: CancellationToken,
    target_schema_override: Option<String>,
) -> MigrationResult {
    let start = std::time::Instant::now();
    let mut tables_migrated = 0;
    let mut total_rows: i64 = 0;
    let mut errors = Vec::new();
    let total_tables = tables.len();

    for (idx, (schema, table)) in tables.iter().enumerate() {
        if cancel_token.load(Ordering::Relaxed) {
            errors.push("Migration cancelled by user".to_string());
            break;
        }

        let progress = MigrationProgress {
            table_name: table.clone(),
            current_table: idx + 1,
            total_tables,
            rows_transferred: 0,
            total_rows: 0,
            status: "Starting".to_string(),
            error: None,
        };
        let _ = app_handle.emit("migration-progress", &progress);

        match migrate_single_table(
            &app_handle,
            source_pool,
            target_pool,
            schema,
            table,
            &options,
            &cancel_token,
            idx + 1,
            total_tables,
            target_schema_override.as_deref(),
        )
        .await
        {
            Ok(rows) => {
                tables_migrated += 1;
                total_rows += rows;
            }
            Err(e) => {
                errors.push(format!("{}.{}: {}", schema, table, e));
            }
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;

    MigrationResult {
        success: errors.is_empty(),
        tables_migrated,
        total_rows,
        errors,
        elapsed_ms: elapsed,
    }
}

/// Migrate a single table
async fn migrate_single_table(
    app_handle: &AppHandle,
    source_pool: &PgPool,
    target_pool: &PgPool,
    schema: &str,
    table: &str,
    options: &MigrationOptions,
    cancel_token: &CancellationToken,
    current_table: usize,
    total_tables: usize,
    target_schema_override: Option<&str>,
) -> Result<i64, String> {
    let target_schema = target_schema_override.unwrap_or(schema);
    let source_full_table = format!("\"{}\".\"{}\"", schema, table);
    let target_full_table = format!("\"{}\".\"{}\"", target_schema, table);

    // Get source table schema and row count
    let table_schema = get_table_schema(source_pool, schema, table).await?;
    let total_rows = get_row_count(source_pool, schema, table).await?;

    // Emit initial progress
    let progress = MigrationProgress {
        table_name: table.to_string(),
        current_table,
        total_tables,
        rows_transferred: 0,
        total_rows,
        status: "Preparing".to_string(),
        error: None,
    };
    let _ = app_handle.emit("migration-progress", &progress);

    // Ensure target schema exists
    let schema_query = format!("CREATE SCHEMA IF NOT EXISTS \"{}\"", target_schema);
    let _ = sqlx::query(&schema_query)
        .execute(target_pool)
        .await;

    // Create table if needed
    if options.create_table_if_not_exists {
        // Modify create statement to handle schema change and IF NOT EXISTS
        let create_stmt = table_schema.create_statement
            .replace(
                &format!("CREATE TABLE \"{}\".\"{}\"", schema, table),
                &format!("CREATE TABLE IF NOT EXISTS \"{}\".\"{}\"", target_schema, table)
            );
        
        sqlx::query(&create_stmt)
            .execute(target_pool)
            .await
            .map_err(|e| format!("Failed to create table: {}", e))?;
    }

    // Truncate if needed
    if options.truncate_before_insert {
        sqlx::query(&format!("TRUNCATE TABLE {} CASCADE", target_full_table))
            .execute(target_pool)
            .await
            .map_err(|e| format!("Failed to truncate: {}", e))?;
    }

    // Disable constraints if needed
    if options.disable_constraints {
        let _ = sqlx::query(&format!("ALTER TABLE {} DISABLE TRIGGER ALL", target_full_table))
            .execute(target_pool)
            .await;
    }

    // Build column list
    let columns: Vec<String> = table_schema
        .columns
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect();
    let column_list = columns.join(", ");

    // Stream data in batches
    let mut rows_transferred: i64 = 0;
    let batch_size = options.batch_size as i64;
    
    // For Keyset Pagination (much faster than OFFSET)
    let pk_col = table_schema.primary_key_columns.first().cloned();
    let mut last_pk_value: Option<String> = None;

    loop {
        if cancel_token.load(Ordering::Relaxed) {
            return Err("Migration cancelled".to_string());
        }

        // Build Fetch Query with Keyset Pagination if possible (on SOURCE)
        let select_query = if let Some(ref pk) = pk_col {
            let where_clause = if let Some(ref last_val) = last_pk_value {
                format!("WHERE \"{}\" > {}", pk, last_val)
            } else {
                "".to_string()
            };
            format!(
                "SELECT {} FROM {} {} ORDER BY \"{}\" LIMIT {}",
                column_list, source_full_table, where_clause, pk, batch_size
            )
        } else {
            // Fallback to OFFSET if no PK
            format!(
                "SELECT {} FROM {} ORDER BY 1 LIMIT {} OFFSET {}",
                column_list, source_full_table, batch_size, rows_transferred
            )
        };

        let rows: Vec<PgRow> = sqlx::query(&select_query)
            .fetch_all(source_pool)
            .await
            .map_err(|e| format!("Failed to fetch data: {}", e))?;

        if rows.is_empty() {
            break;
        }

        let batch_count = rows.len() as i64;

        // Build a single Multi-Row INSERT statement (Turbo Mode)
        let mut row_values = Vec::new();
        for row in &rows {
            let values = build_insert_values(row, &table_schema.columns)?;
            row_values.push(format!("({})", values));
            
            // Track last PK for next batch
            if let Some(ref pk) = pk_col {
                if let Ok(val) = get_column_value_as_sql(row, pk, "text") {
                    last_pk_value = Some(val);
                }
            }
        }

        // INSERT into TARGET
        let insert_query = format!(
            "INSERT INTO {} ({}) VALUES {} ON CONFLICT DO NOTHING",
            target_full_table,
            column_list,
            row_values.join(", ")
        );

        sqlx::query(&insert_query)
            .execute(target_pool)
            .await
            .map_err(|e| format!("Turbo Insert failed: {}", e))?;

        rows_transferred += batch_count;

        // Emit progress
        let progress = MigrationProgress {
            table_name: table.to_string(),
            current_table,
            total_tables,
            rows_transferred,
            total_rows,
            status: "Migrating".to_string(),
            error: None,
        };
        let _ = app_handle.emit("migration-progress", &progress);

        if batch_count < batch_size {
            break;
        }
    }

    // Re-enable constraints
    if options.disable_constraints {
        let _ = sqlx::query(&format!("ALTER TABLE {} ENABLE TRIGGER ALL", target_full_table))
            .execute(target_pool)
            .await;
    }

    // Sync sequences after migration (on TARGET)
    let _ = sync_sequences(target_pool, target_schema, table).await;

    // Emit completion progress
    let progress = MigrationProgress {
        table_name: table.to_string(),
        current_table,
        total_tables,
        rows_transferred,
        total_rows,
        status: "Complete".to_string(),
        error: None,
    };
    let _ = app_handle.emit("migration-progress", &progress);

    Ok(rows_transferred)
}

/// Reset sequences to max value + 1
async fn sync_sequences(pool: &PgPool, schema: &str, table: &str) -> Result<(), String> {
    let query = r#"
        DO $$
        DECLARE
            r RECORD;
            max_val bigint;
        BEGIN
            FOR r IN (
                SELECT 
                    quote_ident(n.nspname) || '.' || quote_ident(s.relname) AS seq_fqn,
                    quote_ident(a.attname) AS col_name,
                    quote_ident(n.nspname) || '.' || quote_ident(t.relname) AS table_fqn
                FROM pg_class s
                JOIN pg_namespace n ON n.oid = s.relnamespace
                JOIN pg_depend d ON d.objid = s.oid AND d.deptype = 'a'
                JOIN pg_class t ON t.oid = d.refobjid
                JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = d.refobjsubid
                WHERE s.relkind = 'S'
                AND n.nspname = $1
                AND t.relname = $2
            ) LOOP
                EXECUTE format('SELECT setval(%L, COALESCE((SELECT MAX(%s) FROM %s), 0) + 1, false)', 
                    r.seq_fqn, r.col_name, r.table_fqn);
            END LOOP;
        END $$;
    "#;

    sqlx::query(query)
        .bind(schema)
        .bind(table)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to sync sequences: {}", e))?;

    Ok(())
}

/// Build insert values from a row
fn build_insert_values(row: &PgRow, columns: &[super::schema::ColumnInfo]) -> Result<String, String> {
    let mut values = Vec::new();

    for col in columns {
        let value = get_column_value_as_sql(row, &col.name, &col.data_type)?;
        values.push(value);
    }

    Ok(values.join(", "))
}

/// Get column value as SQL string
fn get_column_value_as_sql(row: &PgRow, column: &str, data_type: &str) -> Result<String, String> {
    let dt = data_type.to_lowercase();
    
    // Handle Numeric Types
    if dt == "integer" || dt == "int4" {
        let val: Result<Option<i32>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as i32 failed: {}", column, e))
        };
    }
    
    if dt == "bigint" || dt == "int8" {
        let val: Result<Option<i64>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as i64 failed: {}", column, e))
        };
    }
    
    if dt == "smallint" || dt == "int2" {
        let val: Result<Option<i16>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as i16 failed: {}", column, e))
        };
    }

    if dt == "numeric" || dt == "decimal" {
        let val: Result<Option<bigdecimal::BigDecimal>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as numeric failed: {}", column, e))
        };
    }

    if dt == "real" || dt == "float4" {
        let val: Result<Option<f32>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as f32 failed: {}", column, e))
        };
    }

    if dt == "double precision" || dt == "float8" {
        let val: Result<Option<f64>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(v.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as f64 failed: {}", column, e))
        };
    }
    
    if dt == "boolean" || dt == "bool" {
        let val: Result<Option<bool>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(if v { "TRUE" } else { "FALSE" }.to_string()),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as bool failed: {}", column, e))
        };
    }

    // Handle Temporal Types
    if dt == "timestamp" || dt == "timestamp without time zone" {
        let val: Result<Option<chrono::NaiveDateTime>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v.format("%Y-%m-%d %H:%M:%S%.f"))),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as timestamp failed: {}", column, e))
        };
    }

    if dt == "timestamp with time zone" || dt == "timestamptz" {
        let val: Result<Option<chrono::DateTime<chrono::Utc>>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v.to_rfc3339())),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as timestamptz failed: {}", column, e))
        };
    }

    if dt == "date" {
        let val: Result<Option<chrono::NaiveDate>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v.format("%Y-%m-%d"))),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as date failed: {}", column, e))
        };
    }

    if dt == "time" || dt == "time without time zone" {
        let val: Result<Option<chrono::NaiveTime>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v.format("%H:%M:%S%.f"))),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as time failed: {}", column, e))
        };
    }

    // Handle Network Types
    if dt == "inet" || dt == "cidr" {
        let val: Result<Option<ipnetwork::IpNetwork>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v)),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as inet failed: {}", column, e))
        };
    }

    // Handle JSON Types
    if dt == "json" || dt == "jsonb" {
        let val: Result<Option<serde_json::Value>, _> = row.try_get(column);
        return match val {
            Ok(Some(v)) => Ok(format!("'{}'", v.to_string().replace('\'', "''"))),
            Ok(None) => Ok("NULL".to_string()),
            Err(e) => Err(format!("Col {} as json failed: {}", column, e))
        };
    }

    // Handle String-like types (and fallback)
    let val: Result<Option<String>, _> = row.try_get(column);
    match val {
        Ok(Some(v)) => Ok(format!("'{}'", v.replace('\'', "''"))),
        Ok(None) => Ok("NULL".to_string()),
        Err(_) => {
            // Last resort fallback
            let int_res: Result<Option<i64>, _> = row.try_get(column);
            if let Ok(Some(v)) = int_res { return Ok(v.to_string()); }
            
            let float_res: Result<Option<f64>, _> = row.try_get(column);
            if let Ok(Some(v)) = float_res { return Ok(v.to_string()); }

            let bool_res: Result<Option<bool>, _> = row.try_get(column);
            if let Ok(Some(v)) = bool_res { return Ok(if v { "TRUE" } else { "FALSE" }.to_string()); }

            // Check if it's actually NULL
            let is_null = row.try_get::<Option<sqlx::types::JsonValue>, _>(column).is_ok_and(|v| v.is_none());
            if is_null {
                Ok("NULL".to_string())
            } else {
                Err(format!("Unsupported or unreadable data type '{}' for column '{}'", data_type, column))
            }
        }
    }
}
