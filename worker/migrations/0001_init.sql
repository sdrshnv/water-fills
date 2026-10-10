CREATE TABLE events (
  id   INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL CHECK (kind IN ('fill','pay')),
  oz   INTEGER NOT NULL DEFAULT 0,
  at   TEXT NOT NULL DEFAULT (datetime('now'))
);
