//! Tests of the SQL statements and migration themselves.

mod common;

use common::SqlLedger;
use shared::logic::Error;
use shared::{sql, LastAction};

#[test]
fn migration_applies_cleanly() {
    let _ = SqlLedger::new();
}

#[test]
fn migration_cannot_be_applied_twice() {
    let s = SqlLedger::new();
    assert!(s.conn.execute_batch(sql::MIGRATION).is_err());
}

#[test]
fn empty_table_state_handles_null_sums() {
    let s = SqlLedger::new();
    let (total, pays, kind, oz): (u32, u32, Option<String>, Option<u32>) = s
        .conn
        .query_row(sql::SELECT_STATE, [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .unwrap();
    assert_eq!((total, pays, kind, oz), (0, 0, None, None));
}

#[test]
fn fill_inserts_exactly_one_row() {
    let s = SqlLedger::new();
    s.fill(40).unwrap();
    assert_eq!(s.row_count(), 1);
    s.fill(36).unwrap();
    assert_eq!(s.row_count(), 2);
}

#[test]
fn invalid_fill_never_reaches_the_table() {
    let s = SqlLedger::new();
    assert_eq!(s.fill(37), Err(Error::InvalidFill));
    assert_eq!(s.row_count(), 0);
}

#[test]
fn pay_guard_inserts_only_when_owed() {
    let s = SqlLedger::new();
    assert_eq!(s.conn.execute(sql::INSERT_PAY, []).unwrap(), 0);
    for _ in 0..4 {
        s.fill(40).unwrap(); // 160 oz
    }
    assert_eq!(s.conn.execute(sql::INSERT_PAY, []).unwrap(), 1);
    assert_eq!(s.conn.execute(sql::INSERT_PAY, []).unwrap(), 0);
    assert_eq!(s.row_count(), 5);
}

#[test]
fn pay_row_has_zero_oz() {
    let s = SqlLedger::new();
    for _ in 0..4 {
        s.fill(40).unwrap();
    }
    s.pay().unwrap();
    let oz: u32 = s
        .conn
        .query_row("SELECT oz FROM events WHERE kind = 'pay'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(oz, 0);
}

#[test]
fn undo_deletes_only_the_newest_row() {
    let s = SqlLedger::new();
    s.fill(40).unwrap();
    s.fill(36).unwrap();
    s.fill(40).unwrap();
    assert_eq!(s.conn.execute(sql::DELETE_LAST, []).unwrap(), 1);
    assert_eq!(s.row_count(), 2);
    let ozs: Vec<u32> = s
        .conn
        .prepare("SELECT oz FROM events ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(ozs, vec![40, 36]);
}

#[test]
fn undo_on_empty_table_changes_nothing() {
    let s = SqlLedger::new();
    assert_eq!(s.conn.execute(sql::DELETE_LAST, []).unwrap(), 0);
    assert_eq!(s.undo(), Err(Error::NothingToUndo));
}

#[test]
fn undo_still_works_after_ids_are_reused_by_autoincrement() {
    let s = SqlLedger::new();
    s.fill(40).unwrap();
    s.undo().unwrap();
    s.fill(36).unwrap();
    assert_eq!(s.state().last, Some(LastAction::Fill(36)));
    s.undo().unwrap();
    assert_eq!(s.row_count(), 0);
}

#[test]
fn check_constraint_rejects_unknown_kind() {
    let s = SqlLedger::new();
    let r = s
        .conn
        .execute("INSERT INTO events (kind, oz) VALUES ('refund', 5)", []);
    assert!(r.is_err());
    assert_eq!(s.row_count(), 0);
}

#[test]
fn kind_is_required() {
    let s = SqlLedger::new();
    assert!(s.conn.execute("INSERT INTO events (oz) VALUES (40)", []).is_err());
}

#[test]
fn timestamp_defaults_are_populated() {
    let s = SqlLedger::new();
    s.fill(40).unwrap();
    let at: String = s
        .conn
        .query_row("SELECT at FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(at.len(), 19); // "YYYY-MM-DD HH:MM:SS"
}

#[test]
fn last_from_columns_mapping() {
    assert_eq!(sql::last_from_columns(None, None), None);
    assert_eq!(
        sql::last_from_columns(Some("fill"), Some(40)),
        Some(LastAction::Fill(40))
    );
    assert_eq!(sql::last_from_columns(Some("pay"), Some(0)), Some(LastAction::Pay));
    assert_eq!(sql::last_from_columns(Some("pay"), None), Some(LastAction::Pay));
    assert_eq!(sql::last_from_columns(Some("bogus"), Some(1)), None);
}

#[test]
fn state_is_stable_across_repeated_reads() {
    let s = SqlLedger::new();
    s.fill(40).unwrap();
    assert_eq!(s.state(), s.state());
}
