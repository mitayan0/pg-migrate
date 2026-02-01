use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl ConnectionConfig {
    pub fn connection_string(&self) -> String {
        // URL-encode username and password to handle special characters
        let encoded_username = urlencoding::encode(&self.username);
        let encoded_password = urlencoding::encode(&self.password);
        format!(
            "postgres://{}:{}@{}:{}/{}?sslmode=require",
            encoded_username, encoded_password, self.host, self.port, self.database
        )
    }
}

/// Connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub id: String,
    pub connected: bool,
    pub database: String,
    pub host: String,
    pub error: Option<String>,
}

/// Holds active database connections
pub struct ConnectionManager {
    connections: RwLock<HashMap<String, PgPool>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Connect to a PostgreSQL database
    pub async fn connect(&self, config: ConnectionConfig) -> Result<ConnectionStatus, String> {
        let conn_string = config.connection_string();
        let id = Uuid::new_v4().to_string();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&conn_string)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        // Test the connection
        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .map_err(|e| format!("Connection test failed: {}", e))?;

        let mut connections = self.connections.write().await;
        connections.insert(id.clone(), pool);

        Ok(ConnectionStatus {
            id,
            connected: true,
            database: config.database,
            host: config.host,
            error: None,
        })
    }

    /// Disconnect from a database
    pub async fn disconnect(&self, id: &str) -> Result<(), String> {
        let mut connections = self.connections.write().await;
        if let Some(pool) = connections.remove(id) {
            pool.close().await;
            Ok(())
        } else {
            Err(format!("Connection {} not found", id))
        }
    }

    /// Get a connection pool by ID
    pub async fn get_pool(&self, id: &str) -> Option<PgPool> {
        let connections = self.connections.read().await;
        connections.get(id).cloned()
    }

    /// Disconnect all connections (internal use)
    #[allow(dead_code)]
    pub async fn disconnect_all(&self) {
        let mut connections = self.connections.write().await;
        for (_, pool) in connections.drain() {
            pool.close().await;
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe handle to ConnectionManager
pub type ConnectionManagerHandle = Arc<ConnectionManager>;

pub fn create_connection_manager() -> ConnectionManagerHandle {
    Arc::new(ConnectionManager::new())
}
