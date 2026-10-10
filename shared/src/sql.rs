//! SQL used by the worker. Each write is a single atomic statement.
//! D1 is SQLite, so these are tested against an in-memory SQLite database.

use crate::LastAction;

pub const MIGRATION: &str = include_str!("../../worker/migrations/0001_init.sql");

pub const INSERT_FILL: &str = "INSERT INTO events (kind, oz) VALUES ('fill', ?1)";

/// Inserts a pay event only when at least one payment is owed.
/// Zero changed rows means nothing was owed.
pub const INSERT_PAY: &str = "INSERT INTO events (kind) \
     SELECT 'pay' \
     WHERE (SELECT COALESCE(SUM(oz), 0) / 128 - COALESCE(SUM(kind = 'pay'), 0) FROM events) > 0";

/// Removes only the newest event. Zero changed rows means the log was empty.
pub const DELETE_LAST: &str = "DELETE FROM events WHERE id = (SELECT MAX(id) FROM events)";

/// Columns: total, pays, last_kind, last_oz.
pub const SELECT_STATE: &str = "SELECT \
     COALESCE(SUM(oz), 0) AS total, \
     COALESCE(SUM(kind = 'pay'), 0) AS pays, \
     (SELECT kind FROM events ORDER BY id DESC LIMIT 1) AS last_kind, \
     (SELECT oz FROM events ORDER BY id DESC LIMIT 1) AS last_oz \
     FROM events";

pub fn last_from_columns(kind: Option<&str>, oz: Option<u32>) -> Option<LastAction> {
    match kind? {
        "fill" => Some(LastAction::Fill(oz.unwrap_or(0))),
        "pay" => Some(LastAction::Pay),
        _ => None,
    }
}
