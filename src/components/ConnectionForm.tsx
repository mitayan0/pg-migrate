import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConnectionConfig, ConnectionStatus, SavedConnection } from "../App";

interface ConnectionFormProps {
    label: string;
    config: ConnectionConfig;
    onConfigChange: (config: ConnectionConfig) => void;
    connection: ConnectionStatus | null;
    onConnect: (connection: ConnectionStatus) => void;
    onDisconnect: () => void;
    savedConnections: SavedConnection[];
    onSaveConnection: (conn: SavedConnection) => void;
    onDeleteConnection: (name: string) => void;
}

export function ConnectionForm({
    label,
    config,
    onConfigChange,
    connection,
    onConnect,
    onDisconnect,
    savedConnections,
    onSaveConnection,
    onDeleteConnection,
}: ConnectionFormProps) {
    const [isConnecting, setIsConnecting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [showPassword, setShowPassword] = useState(false);
    const [showSaveDialog, setShowSaveDialog] = useState(false);
    const [connectionName, setConnectionName] = useState("");
    const [showSavedList, setShowSavedList] = useState(false);

    const handleConnect = async () => {
        setIsConnecting(true);
        setError(null);

        try {
            const finalConfig = {
                ...config,
                port: config.port || 5432
            };
            const status = await invoke<ConnectionStatus>("connect_database", { config: finalConfig });
            onConnect(status);

            // Auto-save connection if it's new
            const exists = savedConnections.some(c =>
                c.config.host === config.host &&
                c.config.database === config.database &&
                c.config.username === config.username
            );

            if (!exists) {
                const name = `${config.database} @ ${config.host}`;
                onSaveConnection({
                    name,
                    config: { ...config },
                });
            }
        } catch (err) {
            setError(String(err));
        } finally {
            setIsConnecting(false);
        }
    };

    const handleDisconnect = async () => {
        if (!connection) return;

        try {
            await invoke("disconnect_database", { connectionId: connection.id });
            onDisconnect();
        } catch (err) {
            console.error("Disconnect error:", err);
            onDisconnect();
        }
    };

    const handleSaveConnection = () => {
        if (!connectionName.trim()) return;

        onSaveConnection({
            name: connectionName.trim(),
            config: { ...config },
        });

        setShowSaveDialog(false);
        setConnectionName("");
    };

    const handleLoadConnection = (saved: SavedConnection) => {
        onConfigChange({ ...saved.config });
        setShowSavedList(false);
    };

    const handleDeleteConnection = (e: React.MouseEvent, name: string) => {
        e.stopPropagation();
        if (window.confirm(`Are you sure you want to delete the connection "${name}"?`)) {
            onDeleteConnection(name);
        }
    };

    const isConnected = connection?.connected ?? false;

    return (
        <div className="g-card p-4">
            {/* Header */}
            <div className="flex items-center justify-between mb-4 border-b border-[var(--outline-variant)] pb-3">
                <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                        <h2 className="text-sm font-bold text-[var(--on-surface)] uppercase tracking-tight">
                            {label}
                        </h2>
                        {isConnected && (
                            <span className="g-chip g-chip-success py-0 px-2 text-[9px] h-4">Active</span>
                        )}
                    </div>
                    <div className="flex items-center gap-2">
                        <div className={`g-status-dot ${isConnected ? 'online' : 'offline'}`} />
                        <span className={`text-[10px] font-medium ${isConnected ? 'text-[var(--google-green)]' : 'text-[var(--on-surface-variant)]'}`}>
                            {isConnected ? "Connected" : "Disconnected"}
                        </span>
                    </div>
                </div>

                {/* Saved Connections Selector in Header */}
                {savedConnections.length > 0 && (
                    <div className="relative min-w-[160px]">
                        <button
                            onClick={() => setShowSavedList(!showSavedList)}
                            className="w-full flex items-center justify-between px-3 py-1.5 bg-[var(--surface-variant)] hover:bg-[var(--outline-variant)] text-[11px] font-medium text-[var(--on-surface)] rounded transition-colors group"
                        >
                            <span className="truncate mr-2">Saved Connections</span>
                            <svg className={`w-3.5 h-3.5 text-[var(--on-surface-variant)] group-hover:text-[var(--google-blue)] transition-transform ${showSavedList ? 'rotate-180' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.6} d="M19 9l-7 7-7-7" />
                            </svg>
                        </button>

                        {showSavedList && (
                            <div className="absolute top-full right-0 mt-1 z-20 bg-white border border-[var(--outline)] rounded shadow-xl w-64 max-h-64 overflow-y-auto">
                                <div className="p-2 border-b border-[var(--outline-variant)] bg-[var(--surface-dim)] text-[10px] font-bold text-[var(--on-surface-variant)] uppercase tracking-wider">
                                    Load saved connection
                                </div>
                                {savedConnections.map((conn) => (
                                    <div
                                        key={conn.name}
                                        onClick={() => handleLoadConnection(conn)}
                                        className="flex items-center justify-between px-4 py-2.5 hover:bg-[var(--google-blue-light)] cursor-pointer group transition-colors border-b last:border-0 border-[var(--outline-variant)]"
                                    >
                                        <div className="flex flex-col">
                                            <span className="text-sm font-medium text-[var(--on-surface)] group-hover:text-[var(--google-blue)]">
                                                {conn.name}
                                            </span>
                                            <span className="text-[10px] text-[var(--on-surface-variant)]">
                                                {conn.config.database}@{conn.config.host}
                                            </span>
                                        </div>
                                        <button
                                            onClick={(e) => handleDeleteConnection(e, conn.name)}
                                            className="p-1.5 hover:bg-[var(--google-red-light)] rounded-full text-[var(--google-red)] opacity-0 group-hover:opacity-100 transition-all font-bold"
                                            title="Delete saved connection"
                                        >
                                            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                                            </svg>
                                        </button>
                                    </div>
                                ))}
                            </div>
                        )}
                        {showSavedList && <div className="fixed inset-0 z-10" onClick={() => setShowSavedList(false)} />}
                    </div>
                )}
            </div>

            {/* Form Fields */}
            <div className="space-y-3">
                {/* Host & Port */}
                <div className="flex gap-3">
                    <div className="g-field flex-1">
                        <input
                            type="text"
                            value={config.host}
                            onChange={(e) => onConfigChange({ ...config, host: e.target.value })}
                            disabled={isConnected}
                            placeholder=" "
                            className="g-input"
                        />
                        <label>Host</label>
                    </div>
                    <div className="g-field" style={{ width: '90px', flex: 'none' }}>
                        <input
                            type="number"
                            value={config.port || ""}
                            onChange={(e) => onConfigChange({ ...config, port: e.target.value === "" ? 0 : parseInt(e.target.value) })}
                            disabled={isConnected}
                            placeholder="5432"
                            className="g-input"
                        />
                        <label>Port</label>
                    </div>
                </div>

                {/* Database & Username */}
                <div className="flex gap-3">
                    <div className="g-field flex-1">
                        <input
                            type="text"
                            value={config.database}
                            onChange={(e) => onConfigChange({ ...config, database: e.target.value })}
                            disabled={isConnected}
                            placeholder=" "
                            className="g-input"
                        />
                        <label>Database</label>
                    </div>
                    <div className="g-field flex-1">
                        <input
                            type="text"
                            value={config.username}
                            onChange={(e) => onConfigChange({ ...config, username: e.target.value })}
                            disabled={isConnected}
                            placeholder=" "
                            className="g-input"
                        />
                        <label>Username</label>
                    </div>
                </div>

                {/* Password */}
                <div className="g-field">
                    <input
                        type={showPassword ? "text" : "password"}
                        value={config.password}
                        onChange={(e) => onConfigChange({ ...config, password: e.target.value })}
                        disabled={isConnected}
                        placeholder=" "
                        className="g-input pr-10"
                    />
                    <label>Password</label>
                    <button
                        type="button"
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--on-surface-variant)] hover:text-[var(--on-surface)]"
                        style={{ zIndex: 2 }}
                    >
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            {showPassword ? (
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
                            ) : (
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                            )}
                        </svg>
                    </button>
                </div>
            </div>

            {/* Error */}
            {error && (
                <div className="g-alert g-alert-error mt-3">
                    <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                    </svg>
                    <span className="text-sm">{error}</span>
                </div>
            )}

            {/* Buttons */}
            <div className="mt-4 flex gap-2">
                <button
                    onClick={isConnected ? handleDisconnect : handleConnect}
                    disabled={isConnecting || (!isConnected && !config.database)}
                    className={`flex-1 ${isConnected ? 'g-btn-outlined' : 'g-btn-filled'}`}
                >
                    {isConnecting && <div className="g-spinner w-4 h-4" />}
                    {isConnected ? "Disconnect" : (isConnecting ? "Connecting..." : "Connect")}
                </button>

                <button
                    onClick={() => {
                        setConnectionName(config.database || "");
                        setShowSaveDialog(true);
                    }}
                    className="g-btn-outlined px-3"
                    title="Save to list"
                >
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                    </svg>
                </button>
            </div>

            {/* Save Dialog */}
            {showSaveDialog && (
                <div className="mt-3 p-3 bg-[var(--surface-variant)] rounded space-y-3">
                    <div className="g-field">
                        <input
                            type="text"
                            value={connectionName}
                            onChange={(e) => setConnectionName(e.target.value)}
                            placeholder=" "
                            className="g-input"
                            autoFocus
                        />
                        <label>Connection name</label>
                    </div>
                    <div className="flex gap-2">
                        <button onClick={handleSaveConnection} disabled={!connectionName.trim()} className="g-btn-filled flex-1 py-2">
                            Save
                        </button>
                        <button onClick={() => setShowSaveDialog(false)} className="g-btn-outlined flex-1 py-2">
                            Cancel
                        </button>
                    </div>
                </div>
            )}

            {/* Connected Info */}
            {isConnected && connection && (
                <div className="mt-3 g-chip g-chip-success">
                    <svg className="w-4 h-4 mr-1" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                    </svg>
                    {connection.database}@{connection.host}
                </div>
            )}
        </div>
    );
}
