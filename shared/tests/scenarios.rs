//! Behavioural scenarios. Each runs against the in-memory `Ledger` model and
//! against real SQL (via `SqlLedger`), and both must give identical results.

mod common;

use common::SqlLedger;
use shared::logic::{Error, Ledger};
use shared::{LastAction, State};

#[derive(Clone, Copy, Debug)]
enum Act {
    Fill(u32),
    Pay,
    Undo,
}

fn st(oz: u32, owed: u32, last: Option<LastAction>) -> State {
    State { oz, owed, last }
}

/// Runs the actions on both backends, asserting they agree at every step.
/// Returns the results from the model.
fn run(actions: &[Act]) -> Vec<Result<State, Error>> {
    let mut model = Ledger::new();
    let sql = SqlLedger::new();
    let mut out = Vec::new();
    for (i, a) in actions.iter().enumerate() {
        let (m, s) = match *a {
            Act::Fill(oz) => (model.fill(oz), sql.fill(oz)),
            Act::Pay => (model.pay(), sql.pay()),
            Act::Undo => (model.undo(), sql.undo()),
        };
        assert_eq!(m, s, "model/sql disagree at step {i} ({a:?})");
        assert_eq!(model.state(), sql.state(), "state mismatch at step {i}");
        out.push(m);
    }
    out
}

fn final_state(actions: &[Act]) -> State {
    run(actions).into_iter().rev().find_map(|r| r.ok()).unwrap()
}

use Act::*;

#[test]
fn starts_empty() {
    assert_eq!(Ledger::new().state(), st(0, 0, None));
    assert_eq!(SqlLedger::new().state(), st(0, 0, None));
}

#[test]
fn headline_example() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(40), Pay]);
    assert_eq!(r[2], Ok(st(116, 0, Some(LastAction::Fill(36)))));
    assert_eq!(r[3], Ok(st(28, 1, Some(LastAction::Fill(40)))));
    assert_eq!(r[4], Ok(st(28, 0, Some(LastAction::Pay))));
}

#[test]
fn single_fills() {
    assert_eq!(final_state(&[Fill(40)]), st(40, 0, Some(LastAction::Fill(40))));
    assert_eq!(final_state(&[Fill(36)]), st(36, 0, Some(LastAction::Fill(36))));
}

#[test]
fn pay_rejected_when_nothing_owed() {
    let r = run(&[Pay]);
    assert_eq!(r[0], Err(Error::NothingOwed));
    let r = run(&[Fill(40), Fill(40), Fill(36), Pay]);
    assert_eq!(r[3], Err(Error::NothingOwed));
}

#[test]
fn rejected_pay_leaves_state_unchanged() {
    let mut m = Ledger::new();
    m.fill(40).unwrap();
    let before = m.state();
    assert_eq!(m.pay(), Err(Error::NothingOwed));
    assert_eq!(m.state(), before);

    let s = SqlLedger::new();
    s.fill(40).unwrap();
    let rows = s.row_count();
    assert_eq!(s.pay(), Err(Error::NothingOwed));
    assert_eq!(s.row_count(), rows);
}

#[test]
fn exact_multiple_of_128_leaves_zero_remainder() {
    // 128 itself is unreachable with 36s and 40s; 16 * 40 = 640 = 5 * 128 is.
    let fills = vec![Fill(40); 16];
    let s = final_state(&fills);
    assert_eq!((s.oz, s.owed), (0, 5));
}

#[test]
fn hits_boundary_exactly_with_mixed_bottles() {
    // Search for a mix of 36s and 40s whose total is a multiple of 128.
    let mut found = None;
    'o: for a in 0..40u32 {
        for b in 0..40u32 {
            let t = 36 * a + 40 * b;
            if t > 0 && t % 128 == 0 {
                found = Some((a, b));
                break 'o;
            }
        }
    }
    let (a, b) = found.expect("some mix reaches a multiple of 128");
    let mut acts = vec![Fill(36); a as usize];
    acts.extend(vec![Fill(40); b as usize]);
    let s = final_state(&acts);
    assert_eq!(s.oz, 0);
    assert_eq!(s.owed, (36 * a + 40 * b) / 128);
}

#[test]
fn crossing_boundary_enables_pay() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(36)]);
    assert_eq!(r[3], Ok(st(24, 1, Some(LastAction::Fill(36)))));
}

#[test]
fn multiple_payments_accumulate_and_pay_down_one_at_a_time() {
    let mut acts = vec![Fill(40); 16]; // 640 oz = 5 payments
    acts.extend([Pay, Pay]);
    let r = run(&acts);
    assert_eq!(r[15], Ok(st(0, 5, Some(LastAction::Fill(40)))));
    assert_eq!(r[16], Ok(st(0, 4, Some(LastAction::Pay))));
    assert_eq!(r[17], Ok(st(0, 3, Some(LastAction::Pay))));
}

#[test]
fn can_pay_all_owed_then_next_pay_rejected() {
    let mut acts = vec![Fill(40); 16];
    acts.extend(vec![Pay; 5]);
    acts.push(Pay);
    let r = run(&acts);
    assert_eq!(r[20], Ok(st(0, 0, Some(LastAction::Pay))));
    assert_eq!(r[21], Err(Error::NothingOwed));
}

#[test]
fn remainder_survives_payments() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(40), Pay, Fill(36)]);
    assert_eq!(r[5], Ok(st(64, 0, Some(LastAction::Fill(36)))));
}

#[test]
fn undo_fill_that_crossed_boundary_restores_previous_state() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(40), Undo]);
    assert_eq!(r[3], Ok(st(28, 1, Some(LastAction::Fill(40)))));
    assert_eq!(r[4], Ok(st(116, 0, Some(LastAction::Fill(36)))));
}

#[test]
fn undo_pay_restores_owed() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(40), Pay, Undo]);
    assert_eq!(r[5], Ok(st(28, 1, Some(LastAction::Fill(40)))));
}

#[test]
fn undo_walks_back_to_empty_then_rejects() {
    let r = run(&[Fill(40), Fill(36), Pay, Undo, Undo, Undo]);
    assert_eq!(r[2], Err(Error::NothingOwed));
    assert_eq!(r[3], Ok(st(40, 0, Some(LastAction::Fill(40)))));
    assert_eq!(r[4], Ok(st(0, 0, None)));
    assert_eq!(r[5], Err(Error::NothingToUndo));
}

#[test]
fn undo_on_empty_rejected() {
    assert_eq!(run(&[Undo])[0], Err(Error::NothingToUndo));
}

#[test]
fn undo_then_fill_again() {
    let r = run(&[Fill(40), Undo, Fill(36)]);
    assert_eq!(r[1], Ok(st(0, 0, None)));
    assert_eq!(r[2], Ok(st(36, 0, Some(LastAction::Fill(36)))));
}

#[test]
fn undo_is_a_stack_through_a_pay() {
    let r = run(&[Fill(40), Fill(40), Fill(40), Fill(40), Pay, Undo, Undo]);
    // total 160 -> 32 oz, 1 owed; pay -> 0 owed; undo pay -> 1 owed; undo fill -> 120 oz, 0 owed
    assert_eq!(r[4], Ok(st(32, 0, Some(LastAction::Pay))));
    assert_eq!(r[5], Ok(st(32, 1, Some(LastAction::Fill(40)))));
    assert_eq!(r[6], Ok(st(120, 0, Some(LastAction::Fill(40)))));
}

#[test]
fn pay_after_undo_of_boundary_fill_is_rejected() {
    let r = run(&[Fill(40), Fill(40), Fill(36), Fill(40), Undo, Pay]);
    assert_eq!(r[5], Err(Error::NothingOwed));
}

#[test]
fn last_label_always_tracks_newest_event() {
    let acts = [Fill(40), Fill(36), Fill(40), Fill(40), Pay, Fill(36), Undo, Undo];
    let expect = [
        Some(LastAction::Fill(40)),
        Some(LastAction::Fill(36)),
        Some(LastAction::Fill(40)),
        Some(LastAction::Fill(40)),
        Some(LastAction::Pay),
        Some(LastAction::Fill(36)),
        Some(LastAction::Pay),
        Some(LastAction::Fill(40)),
    ];
    for (r, e) in run(&acts).iter().zip(expect) {
        assert_eq!(r.unwrap().last, e);
    }
}

#[test]
fn invalid_fill_rejected_and_does_not_change_state() {
    for bad in [0, 35, 37, 41, 128] {
        let mut m = Ledger::new();
        m.fill(40).unwrap();
        let before = m.state();
        assert_eq!(m.fill(bad), Err(Error::InvalidFill));
        assert_eq!(m.state(), before);

        let s = SqlLedger::new();
        s.fill(40).unwrap();
        assert_eq!(s.fill(bad), Err(Error::InvalidFill));
        assert_eq!(s.row_count(), 1);
    }
}

#[test]
fn long_run_over_many_days() {
    let mut acts = Vec::new();
    for i in 0..200 {
        acts.push(Fill(if i % 3 == 0 { 36 } else { 40 }));
        if i % 7 == 0 {
            acts.push(Pay);
        }
    }
    let results = run(&acts);
    for r in results.into_iter().flatten() {
        assert!(r.oz < 128);
    }
}

#[test]
fn state_json_shape() {
    let s = st(28, 1, Some(LastAction::Fill(40)));
    let json = serde_json::to_value(s).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "oz": 28, "owed": 1, "last": { "Fill": 40 } })
    );
    assert_eq!(serde_json::from_value::<State>(json).unwrap(), s);

    let none = st(0, 0, None);
    let json = serde_json::to_value(none).unwrap();
    assert_eq!(json, serde_json::json!({ "oz": 0, "owed": 0, "last": null }));
    assert_eq!(serde_json::from_value::<State>(json).unwrap(), none);

    let pay = st(0, 0, Some(LastAction::Pay));
    assert_eq!(serde_json::to_value(pay).unwrap()["last"], "Pay");
}

#[test]
fn fill_request_round_trip() {
    let req = shared::FillRequest { oz: 36 };
    let json = serde_json::to_string(&req).unwrap();
    assert_eq!(json, r#"{"oz":36}"#);
    assert_eq!(serde_json::from_str::<shared::FillRequest>(&json).unwrap(), req);
    assert!(serde_json::from_str::<shared::FillRequest>(r#"{"oz":-1}"#).is_err());
    assert!(serde_json::from_str::<shared::FillRequest>(r#"{}"#).is_err());
}
