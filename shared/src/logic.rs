use crate::{LastAction, State, BOTTLE_SIZES, UNIT_OZ};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The fill amount is not one of the bottle sizes.
    InvalidFill,
    /// Pay was requested while nothing is owed.
    NothingOwed,
    /// Undo was requested on an empty log.
    NothingToUndo,
}

/// Builds the API state from the aggregates the SQL query returns.
pub fn state_from_aggregates(total: u32, pays: u32, last: Option<LastAction>) -> State {
    State {
        oz: total % UNIT_OZ,
        owed: (total / UNIT_OZ).saturating_sub(pays),
        last,
    }
}

pub fn validate_fill(oz: u32) -> Result<u32, Error> {
    if BOTTLE_SIZES.contains(&oz) {
        Ok(oz)
    } else {
        Err(Error::InvalidFill)
    }
}

/// Checks an `Authorization` header value against the expected key.
/// Only the exact form `Bearer <key>` passes; the comparison is constant-time.
pub fn check_auth(header: Option<&str>, key: &str) -> bool {
    let Some(token) = header.and_then(|h| h.strip_prefix("Bearer ")) else {
        return false;
    };
    constant_time_eq(token.as_bytes(), key.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= (x ^ y) as usize;
    }
    diff == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Fill(u32),
    Pay,
}

/// In-memory reference model of the event log. The SQL implementation must
/// agree with it on every sequence of actions.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    events: Vec<Event>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(&mut self, oz: u32) -> Result<State, Error> {
        self.events.push(Event::Fill(validate_fill(oz)?));
        Ok(self.state())
    }

    pub fn pay(&mut self) -> Result<State, Error> {
        if self.state().owed == 0 {
            return Err(Error::NothingOwed);
        }
        self.events.push(Event::Pay);
        Ok(self.state())
    }

    pub fn undo(&mut self) -> Result<State, Error> {
        self.events.pop().ok_or(Error::NothingToUndo)?;
        Ok(self.state())
    }

    pub fn state(&self) -> State {
        let total = self
            .events
            .iter()
            .map(|e| if let Event::Fill(oz) = e { *oz } else { 0 })
            .sum();
        let pays = self.events.iter().filter(|e| **e == Event::Pay).count() as u32;
        let last = self.events.last().map(|e| match e {
            Event::Fill(oz) => LastAction::Fill(*oz),
            Event::Pay => LastAction::Pay,
        });
        state_from_aggregates(total, pays, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_split_into_oz_and_owed() {
        let cases = [
            (0, 0, 0, 0),
            (36, 0, 36, 0),
            (40, 0, 40, 0),
            (116, 0, 116, 0),
            (127, 0, 127, 0),
            (128, 0, 0, 1),
            (156, 0, 28, 1),
            (255, 0, 127, 1),
            (256, 0, 0, 2),
            (1_000_000, 0, 1_000_000 % 128, 1_000_000 / 128),
        ];
        for (total, pays, oz, owed) in cases {
            let s = state_from_aggregates(total, pays, None);
            assert_eq!((s.oz, s.owed), (oz, owed), "total={total}");
        }
    }

    #[test]
    fn pays_reduce_owed() {
        assert_eq!(state_from_aggregates(156, 1, None).owed, 0);
        assert_eq!(state_from_aggregates(300, 1, None).owed, 1);
        assert_eq!(state_from_aggregates(300, 2, None).owed, 0);
    }

    #[test]
    fn owed_never_underflows() {
        assert_eq!(state_from_aggregates(100, 3, None).owed, 0);
    }

    #[test]
    fn aggregates_keep_last() {
        let s = state_from_aggregates(40, 0, Some(LastAction::Fill(40)));
        assert_eq!(s.last, Some(LastAction::Fill(40)));
    }

    #[test]
    fn fill_validation() {
        assert_eq!(validate_fill(36), Ok(36));
        assert_eq!(validate_fill(40), Ok(40));
        for bad in [0, 1, 35, 37, 39, 41, 76, 128, u32::MAX] {
            assert_eq!(validate_fill(bad), Err(Error::InvalidFill), "oz={bad}");
        }
    }

    #[test]
    fn auth_accepts_exact_bearer() {
        assert!(check_auth(Some("Bearer s3cret"), "s3cret"));
    }

    #[test]
    fn auth_rejects_everything_else() {
        let key = "s3cret";
        let bad: [Option<&str>; 12] = [
            None,
            Some(""),
            Some("Bearer "),
            Some("Bearer"),
            Some("Basic s3cret"),
            Some("s3cret"),
            Some("Bearer wrong"),
            Some("Bearer s3cre"),
            Some("Bearer s3crets"),
            Some("Bearer  s3cret"),
            Some("Bearer s3cret "),
            Some("Bearer S3CRET"),
        ];
        for h in bad {
            assert!(!check_auth(h, key), "header={h:?}");
        }
        assert!(!check_auth(Some("bearer s3cret"), key));
    }

    #[test]
    fn auth_with_empty_key_only_matches_empty_token() {
        assert!(check_auth(Some("Bearer "), ""));
        assert!(!check_auth(Some("Bearer x"), ""));
        assert!(!check_auth(None, ""));
    }

    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"ab", b"abc"));
    }
}
