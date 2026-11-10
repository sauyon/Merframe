CREATE TABLE arbitration_runs (
    id TEXT PRIMARY KEY,
    started_at INTEGER,
    recorded_at INTEGER NOT NULL,
    run_json TEXT NOT NULL
);

CREATE INDEX arbitration_runs_started_at ON arbitration_runs (started_at);
