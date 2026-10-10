//! Everything the UI decides about labels and enabled/disabled buttons.

use crate::{LastAction, State};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewModel {
    pub oz_text: String,
    /// `None` when nothing is owed.
    pub owed_text: Option<String>,
    pub pay_enabled: bool,
    pub undo_enabled: bool,
    pub undo_label: String,
}

impl From<&State> for ViewModel {
    fn from(s: &State) -> Self {
        let owed_text = match s.owed {
            0 => None,
            1 => Some("1 payment owed".to_string()),
            n => Some(format!("{n} payments owed")),
        };
        let undo_label = match s.last {
            None => "Undo".to_string(),
            Some(LastAction::Fill(oz)) => format!("Undo +{oz} oz"),
            Some(LastAction::Pay) => "Undo Pay".to_string(),
        };
        ViewModel {
            oz_text: format!("{} oz", s.oz),
            owed_text,
            pay_enabled: s.owed > 0,
            undo_enabled: s.last.is_some(),
            undo_label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(oz: u32, owed: u32, last: Option<LastAction>) -> State {
        State { oz, owed, last }
    }

    #[test]
    fn pay_enabled_only_when_owed() {
        assert!(!ViewModel::from(&st(0, 0, None)).pay_enabled);
        assert!(!ViewModel::from(&st(116, 0, None)).pay_enabled);
        assert!(ViewModel::from(&st(28, 1, None)).pay_enabled);
        assert!(ViewModel::from(&st(0, 5, None)).pay_enabled);
    }

    #[test]
    fn owed_text() {
        assert_eq!(ViewModel::from(&st(0, 0, None)).owed_text, None);
        assert_eq!(
            ViewModel::from(&st(0, 1, None)).owed_text.as_deref(),
            Some("1 payment owed")
        );
        assert_eq!(
            ViewModel::from(&st(0, 2, None)).owed_text.as_deref(),
            Some("2 payments owed")
        );
    }

    #[test]
    fn undo_state_and_label() {
        let v = ViewModel::from(&st(0, 0, None));
        assert!(!v.undo_enabled);
        assert_eq!(v.undo_label, "Undo");

        let v = ViewModel::from(&st(40, 0, Some(LastAction::Fill(40))));
        assert!(v.undo_enabled);
        assert_eq!(v.undo_label, "Undo +40 oz");

        let v = ViewModel::from(&st(36, 0, Some(LastAction::Fill(36))));
        assert_eq!(v.undo_label, "Undo +36 oz");

        let v = ViewModel::from(&st(28, 0, Some(LastAction::Pay)));
        assert!(v.undo_enabled);
        assert_eq!(v.undo_label, "Undo Pay");
    }

    #[test]
    fn oz_text_formatting() {
        assert_eq!(ViewModel::from(&st(0, 0, None)).oz_text, "0 oz");
        assert_eq!(ViewModel::from(&st(127, 0, None)).oz_text, "127 oz");
    }
}
