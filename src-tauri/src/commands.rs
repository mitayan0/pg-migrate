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
