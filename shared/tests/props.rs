//! Property tests: random action sequences must keep the model and SQL in
//! lockstep and satisfy the accounting invariants.

mod common;

use common::SqlLedger;
use proptest::prelude::*;
use shared::logic::{Error, Ledger};

#[derive(Clone, Copy, Debug)]
enum Act {
    Fill36,
    Fill40,
    Pay,
    Undo,
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        3 => Just(Act::Fill36),
        3 => Just(Act::Fill40),
        2 => Just(Act::Pay),
        2 => Just(Act::Undo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn sql_matches_model_and_invariants_hold(acts in prop::collection::vec(act(), 0..200)) {
        let mut model = Ledger::new();
        let sql = SqlLedger::new();
        // Independent bookkeeping, a stack of applied events: Some(oz) = fill, None = pay.
        let mut stack: Vec<Option<u32>> = Vec::new();

        for a in acts {
            let before = model.state();
            let (m, s) = match a {
                Act::Fill36 => (model.fill(36), sql.fill(36)),
                Act::Fill40 => (model.fill(40), sql.fill(40)),
                Act::Pay => (model.pay(), sql.pay()),
                Act::Undo => (model.undo(), sql.undo()),
            };
            prop_assert_eq!(m, s);
            prop_assert_eq!(model.state(), sql.state());

            match (a, m) {
                (Act::Fill36, Ok(_)) => stack.push(Some(36)),
                (Act::Fill40, Ok(_)) => stack.push(Some(40)),
                (Act::Pay, Ok(_)) => stack.push(None),
                (Act::Undo, Ok(_)) => { stack.pop(); }
                _ => {}
            }

            // A pay is accepted iff something was owed; an undo iff the log was non-empty.
            if let Act::Pay = a {
                prop_assert_eq!(m.is_ok(), before.owed > 0);
                if m.is_err() { prop_assert_eq!(m, Err(Error::NothingOwed)); }
            }
            if let Act::Undo = a {
                prop_assert_eq!(m.is_ok(), before.last.is_some());
            }

            let st = model.state();
            let total: u32 = stack.iter().flatten().sum();
            let pays = stack.iter().filter(|e| e.is_none()).count() as u32;
            prop_assert!(st.oz < 128);
            prop_assert_eq!(st.owed * 128 + st.oz + pays * 128, total);
            prop_assert_eq!(sql.row_count() as usize, stack.len());
        }
    }

    #[test]
    fn fill_then_undo_is_identity(
        prefix in prop::collection::vec(act(), 0..60),
        oz in prop::sample::select(vec![36u32, 40]),
    ) {
        let mut model = Ledger::new();
        let sql = SqlLedger::new();
        for a in prefix {
            match a {
                Act::Fill36 => { let _ = (model.fill(36), sql.fill(36)); }
                Act::Fill40 => { let _ = (model.fill(40), sql.fill(40)); }
                Act::Pay => { let _ = (model.pay(), sql.pay()); }
                Act::Undo => { let _ = (model.undo(), sql.undo()); }
            }
        }
        let before = model.state();
        model.fill(oz).unwrap();
        sql.fill(oz).unwrap();
        prop_assert_eq!(model.undo().unwrap(), before);
        prop_assert_eq!(sql.undo().unwrap(), before);
    }

    #[test]
    fn pay_then_undo_is_identity_when_payable(prefix in prop::collection::vec(act(), 0..80)) {
        let mut model = Ledger::new();
        for a in prefix {
            match a {
                Act::Fill36 => { let _ = model.fill(36); }
                Act::Fill40 => { let _ = model.fill(40); }
                Act::Pay => { let _ = model.pay(); }
                Act::Undo => { let _ = model.undo(); }
            }
        }
        let before = model.state();
        if before.owed > 0 {
            model.pay().unwrap();
            prop_assert_eq!(model.undo().unwrap(), before);
        }
    }

    #[test]
    fn invalid_fills_always_rejected(oz in 0u32..1000) {
        prop_assume!(oz != 36 && oz != 40);
        prop_assert_eq!(Ledger::new().fill(oz), Err(Error::InvalidFill));
    }

    #[test]
    fn auth_only_accepts_exact_key(key in "[a-zA-Z0-9]{1,24}", other in "[a-zA-Z0-9]{0,24}") {
        let ok = format!("Bearer {key}");
        prop_assert!(shared::logic::check_auth(Some(&ok), &key));
        let attempt = format!("Bearer {other}");
        prop_assert_eq!(shared::logic::check_auth(Some(&attempt), &key), other == key);
    }
}
