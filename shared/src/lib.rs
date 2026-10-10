//! Types and pure logic shared by the worker and the frontend.
//! Everything here is plain Rust so it can be tested natively.

use serde::{Deserialize, Serialize};

pub mod logic;
pub mod sql;
pub mod view;

/// The store sells water in units of this many ounces.
pub const UNIT_OZ: u32 = 128;
/// The bottle sizes the user fills.
pub const BOTTLE_SIZES: [u32; 2] = [40, 36];

/// The newest event in the log, used to label the Undo button.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastAction {
    Fill(u32),
    Pay,
}

/// What the API returns after every request.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// Ounces accumulated toward the next payment (always below 128).
    pub oz: u32,
    /// Payments that are due but not yet marked paid.
    pub owed: u32,
    pub last: Option<LastAction>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FillRequest {
    pub oz: u32,
}
