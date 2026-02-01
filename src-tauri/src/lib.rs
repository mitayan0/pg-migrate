mod commands;
mod db;

use std::sync::Arc;

use commands::{
    cancel_migration, connect_database, disconnect_database, get_schemas, get_table_schema,
    get_tables, start_migration, test_connection, AppState,
};
use db::create_connection_manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn_manager = create_connection_manager();
    let app_state = Arc::new(AppState::new(conn_manager));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            connect_database,
            disconnect_database,
            get_tables,
            get_schemas,
            get_table_schema,
            start_migration,
            cancel_migration,
            test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
