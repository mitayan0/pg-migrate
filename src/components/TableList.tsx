import { useState, useMemo } from "react";
import type { TableInfo } from "../App";

interface TableListProps {
    label: string;
    tables: TableInfo[];
    selectedTables: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    selectable: boolean;
    onAnalyze?: () => void;
    onSort?: () => void;
    isAnalyzing?: boolean;
    isSorting?: boolean;
}

export function TableList({
    label,
    tables,
    selectedTables,
    onSelectionChange,
    selectable,
    onAnalyze,
    onSort,
    isAnalyzing,
    isSorting,
}: TableListProps) {
    const [searchQuery, setSearchQuery] = useState("");
    const [selectedSchema, setSelectedSchema] = useState("all");

    const schemas = useMemo(() => {
        const s = new Set(tables.map(t => t.schema));
        return ["all", ...Array.from(s).sort()];
    }, [tables]);

    const filteredTables = useMemo(() => {
        return tables.filter(t => {
            const matchesSearch = !searchQuery.trim() ||
                t.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                t.schema.toLowerCase().includes(searchQuery.toLowerCase());

            const matchesSchema = selectedSchema === "all" || t.schema === selectedSchema;

            return matchesSearch && matchesSchema;
        });
    }, [tables, searchQuery, selectedSchema]);

    const handleSelectAll = () => {
        if (selectedTables.size === filteredTables.length) {
            onSelectionChange(new Set());
        } else {
            const newSelected = new Set(selectedTables);
            filteredTables.forEach(t => newSelected.add(`${t.schema}.${t.name}`));
            onSelectionChange(newSelected);
        }
    };

    const handleToggle = (table: TableInfo) => {
        const key = `${table.schema}.${table.name}`;
        const newSelected = new Set(selectedTables);
        if (newSelected.has(key)) {
            newSelected.delete(key);
        } else {
            newSelected.add(key);
        }
        onSelectionChange(newSelected);
    };

    const formatBytes = (bytes: number): string => {
        if (bytes === 0) return "0 B";
        const k = 1024;
        const sizes = ["B", "KB", "MB", "GB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
    };

    const formatNumber = (num: number): string => {
        return num.toLocaleString();
    };

    const getStatusIcon = (status?: string, details?: string) => {
        if (!status) return null;
        switch (status) {
            case "MATCH":
                return <span title="Schema matches" className="text-green-500">✓</span>;
            case "MISSING_IN_TARGET":
                return <span title="Missing in target" className="text-blue-500 text-xs px-1 border border-blue-500 rounded">NEW</span>;
            case "COLUMNS_MISMATCH":
                return <span title={details || "Column mismatch"} className="text-orange-500">⚠️</span>;
            case "ERROR":
                return <span title={details || "Error checking schema"} className="text-red-500">!</span>;
            default:
                return null;
        }
    };

    return (
        <div className="g-card p-4 h-96 flex flex-col">
            {/* Header */}
            <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-medium text-[var(--on-surface)]">
                    {label}
                </h3>
                <div className="flex items-center gap-2">
                    {onSort && (
                        <button
                            onClick={onSort}
                            disabled={isSorting || selectedTables.size === 0}
                            title="Sort by Dependencies"
                            className="p-1 rounded hover:bg-[var(--surface-variant)] disabled:opacity-30 transition-colors"
                        >
                            <svg className={`w-4 h-4 text-[var(--google-blue)] ${isSorting ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 4h13M3 8h9m-9 4h6m4 0l4-4m0 0l4 4m-4-4v12" />
                            </svg>
                        </button>
                    )}
                    {onAnalyze && (
                        <button
                            onClick={onAnalyze}
                            disabled={isAnalyzing}
                            title="Analyze Schema Differences"
                            className="p-1 rounded hover:bg-[var(--surface-variant)] disabled:opacity-30 transition-colors"
                        >
                            <svg className={`w-4 h-4 text-[var(--google-blue)] ${isAnalyzing ? 'animate-pulse' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                            </svg>
                        </button>
                    )}
                    <span className="g-chip text-xs">
                        {tables.length} tables
                    </span>
                </div>
            </div>

            {/* Search & Filter */}
            <div className="flex gap-2 mb-3">
                <div className="relative flex-1">
                    <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                        <svg className="h-4 w-4 text-[var(--on-surface-variant)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                        </svg>
                    </div>
                    <input
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        placeholder="Search tables..."
                        className="g-input h-9"
                        style={{ paddingLeft: '40px' }}
                    />
                </div>
                <select
                    value={selectedSchema}
                    onChange={(e) => setSelectedSchema(e.target.value)}
                    className="g-select h-9 text-xs min-w-[120px]"
                >
                    {schemas.map(s => (
                        <option key={s} value={s}>{s === 'all' ? 'All schemas' : s}</option>
                    ))}
                </select>
            </div>

            {/* Select All */}
            {selectable && filteredTables.length > 0 && (
                <div className="flex items-center justify-between mb-2">
                    <button
                        onClick={handleSelectAll}
                        className="g-btn-text py-1 px-2 text-xs h-auto"
                    >
                        {selectedTables.size === filteredTables.length ? "Deselect all" : "Select all filtered"}
                    </button>
                    <span className="text-xs text-[var(--on-surface-variant)]">
                        {selectedTables.size} selected
                    </span>
                </div>
            )}

            {/* Table List */}
            <div className="flex-1 overflow-y-auto space-y-1">
                {filteredTables.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full py-8 text-[var(--on-surface-variant)]">
                        <div className="w-16 h-16 rounded-full bg-[var(--surface-variant)] flex items-center justify-center mb-4">
                            <svg className="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
                            </svg>
                        </div>
                        <p className="text-sm font-medium">
                            {tables.length === 0 ? "No tables found" : "No matching tables"}
                        </p>
                    </div>
                ) : (
                    filteredTables.map((table) => {
                        const key = `${table.schema}.${table.name}`;
                        const isSelected = selectedTables.has(key);

                        return (
                            <div
                                key={key}
                                onClick={() => selectable && handleToggle(table)}
                                className={`flex items-center gap-3 p-2 rounded transition-colors ${selectable ? "cursor-pointer hover:bg-[var(--surface-variant)]" : ""
                                    } ${isSelected ? "bg-[var(--google-blue-light)]" : ""}`}
                            >
                                {selectable && (
                                    <div className={`g-checkbox ${isSelected ? 'checked' : ''}`}>
                                        {isSelected && (
                                            <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                                            </svg>
                                        )}
                                    </div>
                                )}

                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-2">
                                        <span className="text-sm font-medium text-[var(--on-surface)] truncate">
                                            {table.name}
                                        </span>
                                        <span className="text-[10px] font-bold text-[var(--google-blue)] uppercase tracking-tight opacity-70">
                                            {table.schema}
                                        </span>
                                        {selectable && getStatusIcon(table.status, table.statusDetails)}
                                    </div>
                                    <div className="flex items-center gap-3 mt-0.5">
                                        <span className="text-[10px] text-[var(--on-surface-variant)]">
                                            {formatNumber(table.row_count)} rows
                                        </span>
                                        <span className="text-[10px] text-[var(--on-surface-variant)]">
                                            {formatBytes(table.size_bytes)}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        );
                    })
                )}
            </div>
        </div>
    );
}
