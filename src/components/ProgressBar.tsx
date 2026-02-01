import type { MigrationProgress } from "../App";

interface ProgressBarProps {
    progress: MigrationProgress | null;
}

export function ProgressBar({ progress }: ProgressBarProps) {
    if (!progress) {
        return (
            <div className="g-card p-4">
                <div className="flex items-center gap-3">
                    <div className="g-spinner" />
                    <span className="text-sm text-[var(--on-surface-variant)]">Preparing migration...</span>
                </div>
            </div>
        );
    }

    const overallPercent = progress.total_tables > 0
        ? Math.round((progress.current_table / progress.total_tables) * 100)
        : 0;

    const tablePercent = progress.total_rows > 0
        ? Math.round((progress.rows_transferred / progress.total_rows) * 100)
        : 0;

    const formatNumber = (num: number): string => num.toLocaleString();

    return (
        <div className="g-card p-4">
            {/* Header */}
            <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-3">
                    {progress.status === "Complete" ? (
                        <div className="w-6 h-6 rounded-full flex items-center justify-center" style={{ background: 'var(--google-green)' }}>
                            <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                            </svg>
                        </div>
                    ) : (
                        <div className="g-spinner" />
                    )}
                    <div>
                        <p className="text-sm font-medium text-[var(--on-surface)]">
                            {progress.status === "Complete" ? "Migration complete" : `Migrating: ${progress.table_name}`}
                        </p>
                        <p className="text-xs text-[var(--on-surface-variant)]">
                            Table {progress.current_table} of {progress.total_tables}
                        </p>
                    </div>
                </div>
                <div className="text-right">
                    <p className="text-2xl font-medium text-[var(--on-surface)]">{overallPercent}%</p>
                </div>
            </div>

            {/* Overall Progress Bar */}
            <div className="g-progress mb-4">
                <div className="g-progress-bar" style={{ width: `${overallPercent}%` }} />
            </div>

            {/* Current Table Progress */}
            <div className="p-3 rounded" style={{ background: 'var(--surface-variant)' }}>
                <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-[var(--on-surface)]">{progress.table_name}</span>
                    <span className="text-sm text-[var(--on-surface-variant)]">{tablePercent}%</span>
                </div>
                <div className="g-progress mb-2" style={{ background: 'var(--outline)' }}>
                    <div className="g-progress-bar" style={{ width: `${tablePercent}%`, background: 'var(--google-green)' }} />
                </div>
                <div className="flex items-center justify-between text-xs text-[var(--on-surface-variant)]">
                    <span>
                        {formatNumber(progress.rows_transferred)} / {formatNumber(progress.total_rows)} rows
                    </span>
                    <span className="capitalize">{progress.status}</span>
                </div>
            </div>

            {/* Error Display */}
            {progress.error && (
                <div className="g-alert g-alert-error mt-4">
                    <svg className="w-5 h-5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                    </svg>
                    <span>{progress.error}</span>
                </div>
            )}
        </div>
    );
}
