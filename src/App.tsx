import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./index.css";
import { ConnectionForm } from "./components/ConnectionForm";
import { TableList } from "./components/TableList";
import { MigrationPanel } from "./components/MigrationPanel";
import { ProgressBar } from "./components/ProgressBar";

// Types
export interface ConnectionConfig {
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
}

export interface ConnectionStatus {
  id: string;
  connected: boolean;
  database: string;
  host: string;
  error?: string;
}

export interface TableInfo {
  name: string;
  schema: string;
  row_count: number;
  size_bytes: number;
}

export interface MigrationProgress {
  table_name: string;
  current_table: number;
  total_tables: number;
  rows_transferred: number;
  total_rows: number;
  status: string;
  error?: string;
}

export interface MigrationOptions {
  create_table_if_not_exists: boolean;
  truncate_before_insert: boolean;
  disable_constraints: boolean;
  batch_size: number;
}

export interface MigrationResult {
  success: boolean;
  tables_migrated: number;
  total_rows: number;
  errors: string[];
  elapsed_ms: number;
}

export interface SavedConnection {
  name: string;
  config: ConnectionConfig;
}

const STORAGE_KEY = "pg_migrate_saved_connections";

function loadSavedConnections(): SavedConnection[] {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? JSON.parse(saved) : [];
  } catch {
    return [];
  }
}

function persistConnections(connections: SavedConnection[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(connections));
}

function App() {
  // Connection states
  const [sourceConnection, setSourceConnection] = useState<ConnectionStatus | null>(null);
  const [targetConnection, setTargetConnection] = useState<ConnectionStatus | null>(null);

  const [sourceConfig, setSourceConfig] = useState<ConnectionConfig>({
    host: "localhost",
    port: 5432,
    database: "",
    username: "postgres",
    password: "",
  });

  const [targetConfig, setTargetConfig] = useState<ConnectionConfig>({
    host: "localhost",
    port: 5432,
    database: "",
    username: "postgres",
    password: "",
  });

  // Shared Saved Connections
  const [savedConnections, setSavedConnections] = useState<SavedConnection[]>(loadSavedConnections());

  const handleSaveConnection = (newConnection: SavedConnection) => {
    setSavedConnections(prev => {
      const updated = [...prev.filter(c => c.name !== newConnection.name), newConnection];
      persistConnections(updated);
      return updated;
    });
  };

  const handleDeleteConnection = (name: string) => {
    setSavedConnections(prev => {
      const updated = prev.filter(c => c.name !== name);
      persistConnections(updated);
      return updated;
    });
  };

  // Table states
  const [sourceTables, setSourceTables] = useState<TableInfo[]>([]);
  const [targetTables, setTargetTables] = useState<TableInfo[]>([]);
  const [targetSchemas, setTargetSchemas] = useState<string[]>([]);
  const [selectedTables, setSelectedTables] = useState<Set<string>>(new Set());

  // Migration states
  const [isMigrating, setIsMigrating] = useState(false);
  const [progress, setProgress] = useState<MigrationProgress | null>(null);
  const [lastResult, setLastResult] = useState<MigrationResult | null>(null);
  const [targetSchema, setTargetSchema] = useState("");

  // Migration options
  const [options, setOptions] = useState<MigrationOptions>({
    create_table_if_not_exists: true,
    truncate_before_insert: false,
    disable_constraints: true,
    batch_size: 1000,
  });

  // Listen for migration progress events
  useEffect(() => {
    const unlisten = listen<MigrationProgress>("migration-progress", (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Fetch tables and schemas
  useEffect(() => {
    if (sourceConnection?.connected) {
      fetchTables(sourceConnection.id, setSourceTables);
    } else {
      setSourceTables([]);
    }
  }, [sourceConnection]);

  useEffect(() => {
    if (targetConnection?.connected) {
      fetchTables(targetConnection.id, setTargetTables);
      invoke<string[]>("get_schemas", { connectionId: targetConnection.id })
        .then(setTargetSchemas)
        .catch(console.error);
    } else {
      setTargetTables([]);
      setTargetSchemas([]);
    }
  }, [targetConnection]);

  const fetchTables = async (connectionId: string, setter: (tables: TableInfo[]) => void) => {
    try {
      const tables = await invoke<TableInfo[]>("get_tables", { connectionId });
      setter(tables);
    } catch (error) {
      console.error("Failed to fetch tables:", error);
      setter([]);
    }
  };

  const handleSwap = () => {
    // Swap connections
    const tempConn = sourceConnection;
    setSourceConnection(targetConnection);
    setTargetConnection(tempConn);

    // Swap configurations
    const tempConfig = sourceConfig;
    setSourceConfig(targetConfig);
    setTargetConfig(tempConfig);

    // Swap tables
    const tempTables = sourceTables;
    setSourceTables(targetTables);
    setTargetTables(tempTables);

    // Clear selection
    setSelectedTables(new Set());
  };

  const handleMigrate = async () => {
    if (!sourceConnection || !targetConnection) return;
    if (selectedTables.size === 0) return;

    setIsMigrating(true);
    setProgress(null);
    setLastResult(null);

    try {
      const tablesToMigrate = sourceTables
        .filter((t) => selectedTables.has(`${t.schema}.${t.name}`))
        .map((t) => ({ schema: t.schema, name: t.name }));

      const result = await invoke<MigrationResult>("start_migration", {
        request: {
          source_connection_id: sourceConnection.id,
          target_connection_id: targetConnection.id,
          tables: tablesToMigrate,
          options,
          target_schema_override: targetSchema.trim() || null,
        },
      });

      setLastResult(result);

      // Refresh target tables
      if (targetConnection) {
        fetchTables(targetConnection.id, setTargetTables);
      }
    } catch (error) {
      console.error("Migration failed:", error);
      setLastResult({
        success: false,
        tables_migrated: 0,
        total_rows: 0,
        errors: [String(error)],
        elapsed_ms: 0,
      });
    } finally {
      setIsMigrating(false);
      setProgress(null);
    }
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_migration");
    } catch (error) {
      console.error("Failed to cancel:", error);
    }
  };

  return (
    <div className="min-h-screen p-6" style={{ background: 'var(--surface-dim)' }}>
      {/* Header */}
      <header className="mb-6">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg flex items-center justify-center" style={{ background: 'var(--google-blue)' }}>
            <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
            </svg>
          </div>
          <div>
            <h1 className="text-xl font-medium" style={{ color: 'var(--on-surface)', fontFamily: "'Google Sans', sans-serif" }}>
              PostgreSQL Table Migrator
            </h1>
            <p className="text-sm" style={{ color: 'var(--on-surface-variant)' }}>
              Bidirectional table migration
            </p>
          </div>
        </div>
      </header>

      {/* Connection Panels */}
      <div className="flex items-center gap-4 mb-6">
        {/* Source Connection */}
        <div className="flex-1">
          <ConnectionForm
            label="Source database"
            config={sourceConfig}
            onConfigChange={setSourceConfig}
            connection={sourceConnection}
            onConnect={setSourceConnection}
            onDisconnect={() => setSourceConnection(null)}
            savedConnections={savedConnections}
            onSaveConnection={handleSaveConnection}
            onDeleteConnection={handleDeleteConnection}
          />
        </div>

        {/* Swap Button - Elevated Circle */}
        <div className="flex-none -mx-2 z-10">
          <button
            onClick={handleSwap}
            disabled={isMigrating}
            className="w-10 h-10 rounded-full bg-white border border-[var(--outline)] shadow-sm hover:shadow-md hover:bg-[var(--surface-variant)] flex items-center justify-center transition-all group disabled:opacity-50"
            title="Swap Source and Target"
          >
            <svg
              className="w-5 h-5 text-[var(--google-blue)] transform group-hover:rotate-180 transition-transform duration-500"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
              />
            </svg>
          </button>
        </div>

        {/* Target Connection */}
        <div className="flex-1">
          <ConnectionForm
            label="Target database"
            config={targetConfig}
            onConfigChange={setTargetConfig}
            connection={targetConnection}
            onConnect={setTargetConnection}
            onDisconnect={() => setTargetConnection(null)}
            savedConnections={savedConnections}
            onSaveConnection={handleSaveConnection}
            onDeleteConnection={handleDeleteConnection}
          />
        </div>
      </div>

      {/* Table Selection */}
      <div className="flex gap-4 mb-6">
        <div className="flex-1">
          <TableList
            label="Source tables"
            tables={sourceTables}
            selectedTables={selectedTables}
            onSelectionChange={setSelectedTables}
            selectable={true}
          />
        </div>

        {/* Arrow indicator - Minimalist */}
        <div className="flex items-center justify-center flex-none">
          <div className="w-10 h-10 rounded-full bg-[var(--google-blue-light)] flex items-center justify-center">
            <svg
              className="w-6 h-6 text-[var(--google-blue)]"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M13 7l5 5m0 0l-5 5m5-5H6"
              />
            </svg>
          </div>
        </div>

        <div className="flex-1">
          <TableList
            label="Target tables"
            tables={targetTables}
            selectedTables={new Set()}
            onSelectionChange={() => { }}
            selectable={false}
          />
        </div>
      </div>

      {/* Migration Panel */}
      <MigrationPanel
        options={options}
        onOptionsChange={setOptions}
        targetSchema={targetSchema}
        onTargetSchemaChange={setTargetSchema}
        schemas={targetSchemas}
        onMigrate={handleMigrate}
        onCancel={handleCancel}
        isMigrating={isMigrating}
        canMigrate={
          !!sourceConnection?.connected &&
          !!targetConnection?.connected &&
          selectedTables.size > 0
        }
        selectedCount={selectedTables.size}
      />

      {/* Progress */}
      {(isMigrating || progress) && (
        <div className="mt-4">
          <ProgressBar progress={progress} />
        </div>
      )}

      {/* Result */}
      {lastResult && !isMigrating && (
        <div className={`mt-4 g-alert ${lastResult.success ? 'g-alert-success' : 'g-alert-error'}`}>
          {lastResult.success ? (
            <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
            </svg>
          ) : (
            <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
            </svg>
          )}
          <span>
            {lastResult.success
              ? `Successfully migrated ${lastResult.tables_migrated} tables (${lastResult.total_rows.toLocaleString()} rows) in ${(lastResult.elapsed_ms / 1000).toFixed(2)}s`
              : `Migration failed: ${lastResult.errors.join(", ")}`}
          </span>
        </div>
      )}
    </div>
  );
}

export default App;
