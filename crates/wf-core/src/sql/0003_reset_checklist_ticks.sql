CREATE TABLE reset_checklist_ticks (
    manual_row TEXT NOT NULL,
    period_start INTEGER NOT NULL,
    PRIMARY KEY (manual_row, period_start)
);
