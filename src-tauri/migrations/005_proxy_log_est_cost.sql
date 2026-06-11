-- Add estimated cost column to proxy_log for persisted per-request cost estimation

ALTER TABLE proxy_log ADD COLUMN est_cost REAL NOT NULL DEFAULT 0;
