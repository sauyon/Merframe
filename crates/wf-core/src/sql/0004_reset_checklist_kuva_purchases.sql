CREATE TABLE reset_checklist_kuva_purchases (
    vendor TEXT NOT NULL,
    period_start INTEGER NOT NULL,
    bought INTEGER NOT NULL,
    PRIMARY KEY (vendor, period_start)
);
