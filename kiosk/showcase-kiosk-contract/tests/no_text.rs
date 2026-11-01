// SPDX-License-Identifier: MIT OR Apache-2.0
//! No entered text appears in anything formatted or refused: not in a
//! record's `Debug` output, not in an error, not in a report.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use proptest::prelude::*;
use showcase_kiosk_contract::{
    Action, MAX_RECORD_LEN, MAX_TEXT_LEN, Next, Report, SetText, Start, decode_action,
};
use showcase_kiosk_flow::FieldId;

mod common;

use common::*;

/// Text that cannot turn up in formatted output by chance: an unusual
/// prefix, then letters. Formatted output is checked for it, and for its
/// bytes, which a `{:?}` of a byte array would show.
fn marked() -> impl Strategy<Value = String> {
    "[a-z]{6,40}".prop_map(|tail| format!("qzx{tail}"))
}

/// Whether `haystack` shows `text`, as text or as a list of its bytes.
fn shows(haystack: &str, text: &str) -> bool {
    let bytes = format!("{:?}", text.as_bytes());
    let inner = bytes.trim_start_matches('[').trim_end_matches(']');
    haystack.contains(text) || haystack.contains(inner)
}

/// Whether the encoded reports contain the text's bytes anywhere.
fn reports_carry(reports: &[Report], text: &str) -> bool {
    reports.iter().any(|report| {
        let mut slot = [0u8; MAX_RECORD_LEN];
        let len = report.encode(&mut slot).unwrap();
        slot[..len]
            .windows(text.len())
            .any(|window| window == text.as_bytes())
    })
}

proptest! {
    #[test]
    fn records_never_format_their_text(text in marked(), seq in any::<u32>(), field in any::<u16>()) {
        let record = SetText::new(seq, FieldId(field), &text).unwrap();
        let shown = format!("{record:?} {record:#?}");
        prop_assert!(!shows(&shown, &text), "{}", shown);
        let length = format!("text_len: {}", text.len());
        prop_assert!(shown.contains(&length));
        let action = Action::SetText(record);
        let shown = format!("{action:?} {action:#?}");
        prop_assert!(!shows(&shown, &text), "{}", shown);
    }

    #[test]
    fn a_malformed_text_record_is_refused_without_its_text(
        text in marked(), damage in 0usize..3,
    ) {
        let mut slot = [0u8; MAX_RECORD_LEN];
        Action::SetText(SetText::new(1, FieldId(2), &text).unwrap()).encode(&mut slot).unwrap();
        match damage {
            // Shorter than the text, so text sits in the padding.
            0 => slot[14..16].copy_from_slice(&3u16.to_le_bytes()),
            // A byte that is not UTF-8 in the middle of the text.
            1 => slot[18] = 0xFF,
            // A length beyond the record.
            _ => slot[14..16].copy_from_slice(&u16::try_from(MAX_TEXT_LEN + 1).unwrap().to_le_bytes()),
        }
        let error = decode_action(&slot).unwrap_err();
        let shown = format!("{error} {error:?}");
        prop_assert!(!shows(&shown, &text), "{}", shown);

        // The application refuses it and ends the session: nothing it
        // returns shows the text either.
        let mut app = new_app();
        let _ = send(&mut app, &Action::Start(Start { seq: 1 }), 0);
        let fault = app.receive(&slot, 1).unwrap_err();
        let shown = format!("{fault} {fault:?}");
        prop_assert!(!shows(&shown, &text), "{}", shown);
        prop_assert!(!reports_carry(fault.replies.reports(), &text));
    }

    #[test]
    fn a_value_the_form_refuses_comes_back_without_its_text(text in marked()) {
        let mut app = new_app();
        let _ = send(&mut app, &Action::Start(Start { seq: 1 }), 0);
        // Letters in a digits-only field, and too long for the name.
        let long: String = text.chars().cycle().take(40).collect();
        for (field, value) in [(CODE, text), (NAME, long)] {
            let replies = send(&mut app, &Action::SetText(SetText::new(2, field, &value).unwrap()), 1);
            prop_assert!(replies.refused().is_some());
            let shown = format!("{replies:?}");
            prop_assert!(!shows(&shown, &value), "{}", shown);
            prop_assert!(!reports_carry(replies.reports(), &value));
            prop_assert!(app.flow().value(field).unwrap().is_empty());
        }
    }

    #[test]
    fn no_report_ever_carries_an_answer(name in marked(), note in marked()) {
        // A whole session, every way it can go: the answers are entered,
        // checked, sent and accepted, and no report carries any of them.
        let mut app = new_app();
        let mut all = Vec::new();
        all.extend_from_slice(send(&mut app, &Action::Start(Start { seq: 1 }), 0).reports());
        all.extend_from_slice(send(&mut app, &text(2, NAME, &name[..name.len().min(32)]), 1).reports());
        all.extend_from_slice(send(&mut app, &Action::Next(Next { seq: 3 }), 2).reports());
        all.extend_from_slice(send(&mut app, &Action::Select(showcase_kiosk_contract::Select { seq: 4, field: TOPIC, option: 2 }), 3).reports());
        all.extend_from_slice(send(&mut app, &text(5, NOTE, &note), 4).reports());
        all.extend_from_slice(send(&mut app, &Action::Next(Next { seq: 6 }), 5).reports());
        all.extend_from_slice(send(&mut app, &Action::SetChecked(showcase_kiosk_contract::SetChecked { seq: 7, field: AGREE, checked: true }), 6).reports());
        all.extend_from_slice(send(&mut app, &Action::Next(Next { seq: 8 }), 7).reports());
        let submit = send(&mut app, &Action::Submit(showcase_kiosk_contract::Submit { seq: 9 }), 8);
        all.extend_from_slice(submit.reports());
        all.extend_from_slice(app.submission_succeeded(submit.submission().unwrap(), 9).reports());
        all.extend_from_slice(app.tick(9 + CONFIRMATION).reports());
        for answer in [&name[..name.len().min(32)], note.as_str()] {
            prop_assert!(!reports_carry(&all, answer));
            let shown = format!("{all:?}");
            prop_assert!(!shows(&shown, answer));
        }
    }
}

#[test]
fn wiping_an_action_empties_its_text_in_place() {
    use nowayos_bounded::Wipe;
    let mut set = SetText::new(9, FieldId(1), "qzxsample").unwrap();
    set.wipe();
    assert_eq!(set.text(), "");
    assert_eq!(
        (set.seq, set.field),
        (9, FieldId(1)),
        "only the text is wiped"
    );
    let mut action = Action::SetText(SetText::new(10, FieldId(2), "qzxother").unwrap());
    action.wipe();
    assert!(matches!(&action, Action::SetText(set) if set.text().is_empty()));
    let mut next = Action::Next(Next { seq: 11 });
    next.wipe();
    assert_eq!(
        next,
        Action::Next(Next { seq: 11 }),
        "no other action holds text"
    );
}
