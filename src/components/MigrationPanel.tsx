import type { MigrationOptions } from "../App";

interface MigrationPanelProps {
    options: MigrationOptions;
    onOptionsChange: (options: MigrationOptions) => void;
    targetSchema: string;
    onTargetSchemaChange: (val: string) => void;
    schemas: string[];
    onMigrate: () => void;
    onCancel: () => void;
    isMigrating: boolean;
    canMigrate: boolean;
    selectedCount: number;
}

export function MigrationPanel({
    options,
    onOptionsChange,
    targetSchema,
    onTargetSchemaChange,
    schemas,
    onMigrate,
    onCancel,
    isMigrating,
    canMigrate,
    selectedCount,
}: MigrationPanelProps) {
    const handleOptionChange = (key: keyof MigrationOptions, value: boolean | number) => {
        onOptionsChange({ ...options, [key]: value });
    };

    return (
        <div className="g-card p-4">
            <div className="flex flex-wrap items-end gap-6">
                {/* Options Section */}
                <div className="flex-1">
                    <h3 className="text-sm font-medium text-[var(--on-surface)] mb-3">
                        Migration options
                    </h3>
                    <div className="flex flex-wrap items-center gap-4">
                        {/* Target Schema Override */}
                        <div className="g-field" style={{ width: '180px', flex: 'none' }}>
                            <input
                                type="text"
                                value={targetSchema}
                                onChange={(e) => onTargetSchemaChange(e.target.value)}
                                placeholder="Original Schema"
                                className="g-input h-9"
                                list="target-schemas"
                            />
                            <label className="text-[10px] top-[-8px]">Target Schema (Optional)</label>
                            <datalist id="target-schemas">
                                {schemas.map(s => (
                                    <option key={s} value={s} />
                                ))}
                            </datalist>
                        </div>

                        {/* ... rest of existing options ... */}

                        {/* Create Table Option */}
                        <label className="flex items-center gap-2 cursor-pointer group">
                            <div
                                className={`g-checkbox ${options.create_table_if_not_exists ? 'checked' : ''}`}
                                onClick={() => handleOptionChange("create_table_if_not_exists", !options.create_table_if_not_exists)}
                            >
                                {options.create_table_if_not_exists && (
                                    <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                                    </svg>
                                )}
                            </div>
                            <span className="text-sm text-[var(--on-surface-variant)] group-hover:text-[var(--on-surface)]">
                                Create
                            </span>
                        </label>

                        {/* Truncate Option */}
                        <label className="flex items-center gap-2 cursor-pointer group">
                            <div
                                className={`g-checkbox ${options.truncate_before_insert ? 'checked' : ''}`}
                                style={options.truncate_before_insert ? { background: 'var(--google-yellow)', borderColor: 'var(--google-yellow)' } : {}}
                                onClick={() => handleOptionChange("truncate_before_insert", !options.truncate_before_insert)}
                            >
                                {options.truncate_before_insert && (
                                    <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                                    </svg>
                                )}
                            </div>
                            <span className="text-sm text-[var(--on-surface-variant)] group-hover:text-[var(--on-surface)]">
                                Truncate
                            </span>
                        </label>

                        {/* Batch Size */}
                        <div className="flex items-center gap-2 border-l border-[var(--outline-variant)] pl-4">
                            <span className="text-xs font-medium text-[var(--on-surface-variant)]">Batch:</span>
                            <select
                                value={options.batch_size}
                                onChange={(e) => handleOptionChange("batch_size", parseInt(e.target.value))}
                                className="g-select py-1 h-8 text-xs"
                            >
                                <option value={100}>100</option>
                                <option value={1000}>1k</option>
                                <option value={10000}>10k</option>
                                <option value={50000}>50k</option>
                            </select>
                        </div>
                    </div>
                </div>

                {/* Action Buttons */}
                <div className="flex items-center gap-2">
                    {isMigrating ? (
                        <button onClick={onCancel} className="g-btn-outlined" style={{ color: 'var(--google-red)', borderColor: 'var(--google-red)' }}>
                            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                            </svg>
                            Cancel
                        </button>
                    ) : (
                        <button
                            onClick={onMigrate}
                            disabled={!canMigrate}
                            className="g-btn-filled g-btn-filled-green"
                            style={{
                                backgroundColor: canMigrate ? '#1e8e3e' : '#dadce0',
                                minWidth: '140px'
                            }}
                        >
                            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14 5l7 7m0 0l-7 7m7-7H3" />
                            </svg>
                            Migrate {selectedCount > 0 ? selectedCount : ''}
                        </button>
                    )}
                </div>
            </div>

            {/* Warning for Truncate */}
            {options.truncate_before_insert && (
                <div className="g-alert g-alert-warning mt-4">
                    <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                    </svg>
                    <span><strong>Warning:</strong> Truncate will delete existing data in target tables.</span>
                </div>
            )}
        </div>
    );
}
