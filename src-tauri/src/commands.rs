use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::RwLock;

use crate::db::{
    create_cancellation_token, list_schemas, list_tables, migrate_tables, CancellationToken,
    ConnectionConfig, ConnectionManagerHandle, ConnectionStatus, MigrationOptions, MigrationResult,
    TableInfo, TableSchema,
};

/// Application state holding connection manager and cancellation token
pub struct AppState {
    pub conn_manager: ConnectionManagerHandle,
    pub cancel_token: RwLock<Option<CancellationToken>>,
}

impl AppState {
    pub fn new(conn_manager: ConnectionManagerHandle) -> Self {
        Self {
            conn_manager,
            cancel_token: RwLock::new(None),
        }
    }
}

/// Connect to a PostgreSQL database
#[tauri::command]
pub async fn connect_database(
    state: State<'_, Arc<AppState>>,
    config: ConnectionConfig,
) -> Result<ConnectionStatus, String> {
    state.conn_manager.connect(config).await
}

/// Disconnect from a database
#[tauri::command]
pub async fn disconnect_database(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<(), String> {
    state.conn_manager.disconnect(&connection_id).await
}

/// List all tables in a database
#[tauri::command]
pub async fn get_tables(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<Vec<TableInfo>, String> {
    let pool = state
        .conn_manager
        .get_pool(&connection_id)
        .await
        .ok_or("Connection not found")?;

    list_tables(&pool).await
}

/// List all schemas in a database
#[tauri::command]
pub async fn get_schemas(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let pool = state
        .conn_manager
        .get_pool(&connection_id)
        .await
        .ok_or("Connection not found")?;

    list_schemas(&pool).await
}

/// Get table schema details
#[tauri::command]
pub async fn get_table_schema(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: String,
    table: String,
) -> Result<TableSchema, String> {
    let pool = state
        .conn_manager
        .get_pool(&connection_id)
        .await
        .ok_or("Connection not found")?;

    crate::db::get_table_schema(&pool, &schema, &table).await
}

/// Request to migrate tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateTablesRequest {
    pub source_connection_id: String,
    pub target_connection_id: String,
    pub tables: Vec<TableSelection>,
    pub options: MigrationOptions,
    pub target_schema_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSelection {
    pub schema: String,
    pub name: String,
}

/// Start table migration
#[tauri::command]
pub async fn start_migration(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: MigrateTablesRequest,
) -> Result<MigrationResult, String> {
    let source_pool = state
        .conn_manager
        .get_pool(&request.source_connection_id)
        .await
        .ok_or("Source connection not found")?;

    let target_pool = state
        .conn_manager
        .get_pool(&request.target_connection_id)
        .await
        .ok_or("Target connection not found")?;

    // Create cancellation token
    let cancel_token = create_cancellation_token();
    {
        let mut token = state.cancel_token.write().await;
        *token = Some(cancel_token.clone());
    }

    let tables: Vec<(String, String)> = request
        .tables
        .iter()
        .map(|t| (t.schema.clone(), t.name.clone()))
        .collect();

    let result = migrate_tables(
        app_handle,
        &source_pool,
        &target_pool,
        tables,
        request.options,
        cancel_token,
        request.target_schema_override,
    )
    .await;

    // Clear cancellation token
    {
        let mut token = state.cancel_token.write().await;
        *token = None;
    }

    Ok(result)
}

/// Cancel ongoing migration
#[tauri::command]
pub async fn cancel_migration(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let token = state.cancel_token.read().await;
    if let Some(ref t) = *token {
        t.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    } else {
        Err("No migration in progress".to_string())
    }
}

/// Test database connection without storing it
#[tauri::command]
pub async fn test_connection(config: ConnectionConfig) -> Result<bool, String> {
    use sqlx::postgres::PgPoolOptions;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.connection_string())
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| format!("Query failed: {}", e))?;

    pool.close().await;
    Ok(true)
}

/// Schema comparison result for a single table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub schema: String,
    pub table: String,
    pub status: String, // "MATCH", "MISSING_IN_TARGET", "COLUMNS_MISMATCH"
    pub details: Option<String>,
}

/// Compare source and target schemas
#[tauri::command]
pub async fn analyze_schema(
    state: State<'_, Arc<AppState>>,
    source_connection_id: String,
    target_connection_id: String,
    tables: Vec<TableSelection>,
) -> Result<Vec<SchemaDiff>, String> {
    let source_pool = state
        .conn_manager
        .get_pool(&source_connection_id)
        .await
        .ok_or("Source connection not found")?;

    let target_pool = state
        .conn_manager
        .get_pool(&target_connection_id)
        .await
        .ok_or("Target connection not found")?;

    let mut diffs = Vec::new();

    for t in tables {
        let source_schema = crate::db::get_table_schema(&source_pool, &t.schema, &t.name).await;

        match source_schema {
            Ok(s_schema) => {
                // Check if exists in target
                // Note: We might want to handle target_schema_override logic here too eventually
                let target_schema =
                    crate::db::get_table_schema(&target_pool, &t.schema, &t.name).await;

                match target_schema {
                    Ok(t_schema) => {
                        // Compare columns
                        let mut mismatch_details = Vec::new();

                        // Check for missing columns in target
                        for s_col in &s_schema.columns {
                            let t_col = t_schema.columns.iter().find(|c| c.name == s_col.name);
                            match t_col {
                                Some(tc) => {
                                    if s_col.data_type != tc.data_type {
                                        mismatch_details.push(format!(
                                            "Column '{}' type mismatch: {} vs {}",
                                            s_col.name, s_col.data_type, tc.data_type
                                        ));
                                    }
                                    if s_col.is_nullable != tc.is_nullable {
                                        // Warning only?
                                    }
                                }
                                None => {
                                    mismatch_details
                                        .push(format!("Column '{}' missing in target", s_col.name));
                                }
                            }
                        }

                        if mismatch_details.is_empty() {
                            diffs.push(SchemaDiff {
                                schema: t.schema,
                                table: t.name,
                                status: "MATCH".to_string(),
                                details: None,
                            });
                        } else {
                            diffs.push(SchemaDiff {
                                schema: t.schema,
                                table: t.name,
                                status: "COLUMNS_MISMATCH".to_string(),
                                details: Some(mismatch_details.join(", ")),
                            });
                        }
                    }
                    Err(_) => {
                        diffs.push(SchemaDiff {
                            schema: t.schema,
                            table: t.name,
                            status: "MISSING_IN_TARGET".to_string(),
                            details: Some("Table does not exist in target database".to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                diffs.push(SchemaDiff {
                    schema: t.schema,
                    table: t.name,
                    status: "ERROR".to_string(),
                    details: Some(format!("Failed to read source schema: {}", e)),
                });
            }
        }
    }

    Ok(diffs)
}

/// Sort tables based on Foreign Key dependencies
#[tauri::command]
pub async fn sort_tables_by_dependency(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    tables: Vec<TableSelection>,
) -> Result<Vec<TableSelection>, String> {
    let pool = state
        .conn_manager
        .get_pool(&connection_id)
        .await
        .ok_or("Connection not found")?;

    let all_deps = crate::db::get_all_dependencies(&pool).await?;

    // Filter deps to only include selected tables
    // We only care if Table A depends on Table B AND both are in the selection list.

    // Build Graph: Adjacency List
    // key: (schema, table), value: list of dependencies (parents)
    let mut graph: std::collections::HashMap<(String, String), Vec<(String, String)>> =
        std::collections::HashMap::new();
    let selected_set: std::collections::HashSet<(String, String)> = tables
        .iter()
        .map(|t| (t.schema.clone(), t.name.clone()))
        .collect();

    // Initialize graph with all selected tables
    for t in &tables {
        graph.insert((t.schema.clone(), t.name.clone()), Vec::new());
    }

    // Populate edges
    for dep in all_deps {
        if selected_set.contains(&(dep.schema.clone(), dep.name.clone())) {
            for parent in dep.depends_on {
                if selected_set.contains(&parent) {
                    // Add edge: Node -> Parent
                    if let Some(deps) = graph.get_mut(&(dep.schema.clone(), dep.name.clone())) {
                        deps.push(parent);
                    }
                }
            }
        }
    }

    // Topological Sort (Kahn's Algorithm adaptation or simple DFS)
    // We want to migrate PARENTS first.
    // So if A depends on B, B comes before A.

    let mut sorted_tables = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut temp_visited = std::collections::HashSet::new(); // for cycle detection

    // Recursive Visit function
    fn visit(
        node: &(String, String),
        graph: &std::collections::HashMap<(String, String), Vec<(String, String)>>,
        visited: &mut std::collections::HashSet<(String, String)>,
        temp_visited: &mut std::collections::HashSet<(String, String)>,
        sorted: &mut Vec<TableSelection>,
    ) {
        if visited.contains(node) {
            return;
        }
        if temp_visited.contains(node) {
            // Cycle detected! Just treat as visited to break loop,
            // but ideally we should warn. For migration, we just output one.
            return;
        }

        temp_visited.insert(node.clone());

        if let Some(parents) = graph.get(node) {
            for parent in parents {
                visit(parent, graph, visited, temp_visited, sorted);
            }
        }

        temp_visited.remove(node);
        visited.insert(node.clone());
        sorted.push(TableSelection {
            schema: node.0.clone(),
            name: node.1.clone(),
        });
    }

    // The generic Topological Sort usually gives parents last if we do post-order traversal?
    // Wait: Post-order DFS gives: [Leaf, ..., Root].
    // If A depends on B (A -> B), we want B then A.
    // My graph is: A has edge to B.
    // visiting A -> visit B -> B has no deps -> push B. Then push A.
    // So Result is [B, A]. This is CORRECT for migration (B created first).

    // Make sure ordering is deterministic for non-dependent tables (alphabetical)
    let mut nodes: Vec<(String, String)> = tables
        .iter()
        .map(|t| (t.schema.clone(), t.name.clone()))
        .collect();
    nodes.sort();

    for node in nodes {
        visit(
            &node,
            &graph,
            &mut visited,
            &mut temp_visited,
            &mut sorted_tables,
        );
    }

    Ok(sorted_tables)
}
