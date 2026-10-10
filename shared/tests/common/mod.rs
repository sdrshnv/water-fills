#![allow(dead_code)]

use rusqlite::Connection;
use shared::logic::{state_from_aggregates, validate_fill, Error};
use shared::sql;
use shared::State;

/// The real SQL statements run against in-memory SQLite, mirroring `Ledger`'s API.
pub struct SqlLedger {
    pub conn: Connection,
}

impl SqlLedger {
    pub fn new() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(sql::MIGRATION).unwrap();
        Self { conn }
    }

    pub fn state(&self) -> State {
        self.conn
            .query_row(sql::SELECT_STATE, [], |r| {
                let total: u32 = r.get(0)?;
                let pays: u32 = r.get(1)?;
                let kind: Option<String> = r.get(2)?;
                let oz: Option<u32> = r.get(3)?;
                Ok(state_from_aggregates(
                    total,
                    pays,
                    sql::last_from_columns(kind.as_deref(), oz),
                ))
            })
            .unwrap()
    }

    pub fn fill(&self, oz: u32) -> Result<State, Error> {
        let oz = validate_fill(oz)?;
        self.conn.execute(sql::INSERT_FILL, [oz]).unwrap();
        Ok(self.state())
    }

    pub fn pay(&self) -> Result<State, Error> {
        match self.conn.execute(sql::INSERT_PAY, []).unwrap() {
            0 => Err(Error::NothingOwed),
            _ => Ok(self.state()),
        }
    }

    pub fn undo(&self) -> Result<State, Error> {
        match self.conn.execute(sql::DELETE_LAST, []).unwrap() {
            0 => Err(Error::NothingToUndo),
            _ => Ok(self.state()),
        }
    }

    pub fn row_count(&self) -> u32 {
        self.conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap()
    }
}
