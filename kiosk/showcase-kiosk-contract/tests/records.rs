// SPDX-License-Identifier: MIT OR Apache-2.0
//! Every action and report is a fixed-size record with its own kind and
//! version, and comes back from its bytes exactly as it went in.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::collections::HashSet;

use nowayos_ipc::{HEADER_LEN, Header};
use proptest::prelude::*;
use showcase_kiosk_contract::{
    ACTION_KINDS, Abandon, Action, Back, Cancel, ClearField, Countdown, EndReason, Finish,
    IdleWarning, Incomplete, Issued, MAX_RECORD_LEN, MAX_TEXT_LEN, Next, OutOfService,
    OutOfServiceReason, REPORT_KINDS, Refusal, Refused, Report, Retry, Select, SessionEnded,
    SetChecked, SetText, ShowFailed, ShowReview, ShowStep, ShowSubmitted, ShowSubmitting,
    ShowWelcome, SimulateFailure, Start, StillHere, Submit, VERSION, ValueRefused, decode_action,
    decode_report,
};
use showcase_kiosk_flow::{FailureReason, FieldId, Issue};

mod common;

use common::{any_action, any_report};

/// One of every action.
fn every_action() -> Vec<Action> {
    vec![
        Action::Start(Start { seq: 1 }),
        Action::SetText(SetText::new(2, FieldId(7), "Synthetic answer, é ü").unwrap()),
        Action::Select(Select {
            seq: 3,
            field: FieldId(8),
            option: 2,
        }),
        Action::SetChecked(SetChecked {
            seq: 4,
            field: FieldId(9),
            checked: true,
        }),
        Action::ClearField(ClearField {
            seq: 5,
            field: FieldId(10),
        }),
        Action::Next(Next { seq: 6 }),
        Action::Back(Back { seq: 7 }),
        Action::Submit(Submit { seq: 8 }),
        Action::Retry(Retry { seq: 9 }),
        Action::Cancel(Cancel { seq: 10 }),
        Action::Finish(Finish { seq: 11 }),
        Action::StillHere(StillHere { seq: u32::MAX }),
        Action::Abandon(Abandon { seq: 12 }),
        Action::SimulateFailure(SimulateFailure { seq: 13 }),
    ]
}

/// One of every report.
fn every_report() -> Vec<Report> {
    vec![
        Report::ShowWelcome(ShowWelcome),
        Report::ShowStep(ShowStep { index: 2 }),
        Report::ShowReview(ShowReview),
        Report::ShowSubmitting(ShowSubmitting { attempt: 1 }),
        Report::ShowSubmitted(ShowSubmitted),
        Report::ShowFailed(ShowFailed {
            reason: FailureReason::Unavailable,
            attempt: 2,
            attempts_left: 1,
        }),
        Report::IdleWarning(IdleWarning { remaining: 15_000 }),
        Report::ValueRefused(ValueRefused {
            seq: 12,
            field: FieldId(3),
            issue: Issue::BadCharacter,
        }),
        Report::Incomplete(Incomplete {
            seq: 13,
            field: FieldId(4),
            issue: Issue::Missing,
        }),
        Report::Refused(Refused {
            seq: 14,
            refusal: Refusal::AtFirstStep,
        }),
        Report::SessionEnded(SessionEnded {
            session: 5,
            reason: EndReason::Idle,
        }),
        Report::Countdown(Countdown { remaining: 9_000 }),
        Report::Issued(Issued::new("A047").unwrap()),
        Report::OutOfService(OutOfService {
            reason: OutOfServiceReason::AwaitingApproval {
                code: [0x3F, 0xA1, 0x07, 0x5C, 0xE2, 0x19, 0x88, 0x40],
            },
        }),
    ]
}

fn encoded_action(action: &Action) -> ([u8; MAX_RECORD_LEN], usize) {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = action.encode(&mut slot).unwrap();
    (slot, len)
}

fn encoded_report(report: &Report) -> ([u8; MAX_RECORD_LEN], usize) {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = report.encode(&mut slot).unwrap();
    (slot, len)
}

#[test]
fn every_action_round_trips_with_its_own_kind_and_version() {
    let actions = every_action();
    assert_eq!(actions.len(), ACTION_KINDS.len());
    for (action, kind) in actions.iter().zip(ACTION_KINDS) {
        assert_eq!(action.kind(), kind);
        let (slot, len) = encoded_action(action);
        let header = Header::parse(&slot).unwrap();
        assert_eq!(header.kind, kind);
        assert_eq!(header.version, VERSION);
        assert_eq!(HEADER_LEN + usize::from(header.payload_len), len);
        assert!(len <= MAX_RECORD_LEN);
        // Exactly the record, and the record in a longer slot.
        assert_eq!(&decode_action(&slot[..len]).unwrap(), action);
        assert_eq!(&decode_action(&slot).unwrap(), action);
    }
}

#[test]
fn every_report_round_trips_with_its_own_kind_and_version() {
    let reports = every_report();
    assert_eq!(reports.len(), REPORT_KINDS.len());
    for (report, kind) in reports.iter().zip(REPORT_KINDS) {
        assert_eq!(report.kind(), kind);
        let (slot, len) = encoded_report(report);
        let header = Header::parse(&slot).unwrap();
        assert_eq!(header.kind, kind);
        assert_eq!(header.version, VERSION);
        assert_eq!(HEADER_LEN + usize::from(header.payload_len), len);
        assert!(len <= MAX_RECORD_LEN);
        assert_eq!(decode_report(&slot[..len]).unwrap(), *report);
        assert_eq!(decode_report(&slot).unwrap(), *report);
    }
}

#[test]
fn every_kind_is_distinct_and_the_rings_share_none() {
    let actions: HashSet<u16> = ACTION_KINDS.into_iter().collect();
    let reports: HashSet<u16> = REPORT_KINDS.into_iter().collect();
    assert_eq!(actions.len(), ACTION_KINDS.len());
    assert_eq!(reports.len(), REPORT_KINDS.len());
    assert!(actions.is_disjoint(&reports));
}

#[test]
fn records_are_fixed_size() {
    // A record's length depends on its kind only, never on its contents.
    for value in ["", "a", "Synthetic answer, é ü", &"x".repeat(MAX_TEXT_LEN)] {
        let (_, len) = encoded_action(&Action::SetText(
            SetText::new(1, FieldId(1), value).unwrap(),
        ));
        assert_eq!(len, MAX_RECORD_LEN);
    }
    assert_eq!(MAX_RECORD_LEN, 80);
}

#[test]
fn set_text_layout_matches_its_table() {
    let action = Action::SetText(SetText::new(0x0403_0201, FieldId(0x0605), "Hi é").unwrap());
    let (slot, len) = encoded_action(&action);
    assert_eq!(len, 80);
    // Header: kind 0x4B02, version 1, payload 72, reserved 0.
    assert_eq!(slot[..8], [0x02, 0x4B, 1, 0, 72, 0, 0, 0]);
    // seq, field, length 5 (é is two bytes), the text, then zeros.
    assert_eq!(slot[8..16], [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 5, 0]);
    assert_eq!(&slot[16..21], "Hi é".as_bytes());
    assert!(slot[21..80].iter().all(|&b| b == 0));
}

#[test]
fn session_ended_layout_matches_its_table() {
    let report = Report::SessionEnded(SessionEnded {
        session: 0x0403_0201,
        reason: EndReason::Submitted,
    });
    let (slot, len) = encoded_report(&report);
    assert_eq!(len, 16);
    assert_eq!(slot[..8], [0x8B, 0x4B, 1, 0, 8, 0, 0, 0]);
    assert_eq!(slot[8..16], [0x01, 0x02, 0x03, 0x04, 4, 0, 0, 0]);
}

#[test]
fn out_of_service_layout_matches_its_table() {
    let code = [1, 2, 3, 4, 5, 6, 7, 8];
    let cases = [
        (OutOfServiceReason::AwaitingApproval { code }, 1, code),
        (OutOfServiceReason::NoService, 2, [0; 8]),
        (OutOfServiceReason::Updating, 3, [0; 8]),
    ];
    for (reason, byte, code) in cases {
        let (slot, len) = encoded_report(&Report::OutOfService(OutOfService { reason }));
        assert_eq!(len, 20);
        assert_eq!(slot[..8], [0x8E, 0x4B, 1, 0, 12, 0, 0, 0]);
        assert_eq!(slot[8..12], [byte, 0, 0, 0]);
        assert_eq!(slot[12..20], code, "the code only while awaiting approval");
    }
}

#[test]
fn set_text_holds_at_most_its_limit() {
    let full = "x".repeat(MAX_TEXT_LEN);
    assert_eq!(SetText::new(1, FieldId(1), &full).unwrap().text(), full);
    let over = SetText::new(1, FieldId(1), &"x".repeat(MAX_TEXT_LEN + 1)).unwrap_err();
    assert_eq!(over.capacity, MAX_TEXT_LEN);
    // Two-byte letters count twice.
    assert!(SetText::new(1, FieldId(1), &"é".repeat(MAX_TEXT_LEN / 2)).is_ok());
    assert!(SetText::new(1, FieldId(1), &"é".repeat(MAX_TEXT_LEN / 2 + 1)).is_err());
}

#[test]
fn an_attempt_of_zero_cannot_be_sent() {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let zero = Report::ShowSubmitting(ShowSubmitting { attempt: 0 });
    assert!(zero.encode(&mut slot).is_err());
    let failed = Report::ShowFailed(ShowFailed {
        reason: FailureReason::TimedOut,
        attempt: 0,
        attempts_left: 0,
    });
    assert!(failed.encode(&mut slot).is_err());
}

#[test]
fn a_short_buffer_is_an_error() {
    let mut slot = [0u8; 12];
    let action = Action::SetText(SetText::new(1, FieldId(1), "abc").unwrap());
    assert!(action.encode(&mut slot).is_err());
    assert!(
        Report::ShowFailed(ShowFailed {
            reason: FailureReason::TimedOut,
            attempt: 1,
            attempts_left: 0,
        })
        .encode(&mut slot[..11])
        .is_err()
    );
}

proptest! {
    #[test]
    fn any_action_round_trips(action in any_action()) {
        let (slot, len) = encoded_action(&action);
        prop_assert_eq!(decode_action(&slot[..len]).unwrap(), action);
    }

    #[test]
    fn any_report_round_trips(report in any_report()) {
        let (slot, len) = encoded_report(&report);
        prop_assert_eq!(decode_report(&slot[..len]).unwrap(), report);
    }
}

#[test]
fn an_issued_code_is_capital_letters_and_digits_and_nothing_else() {
    for good in ["A047", "4821", "Z", "ABCDEFGH"] {
        assert_eq!(Issued::new(good).unwrap().code(), good);
    }
    for bad in ["", "ABCDEFGHI", "a047", "A 47", "A-47", "Ä047"] {
        assert!(Issued::new(bad).is_err(), "{bad:?}");
    }
    // On the wire: a character after the padding, or one outside the set,
    // is refused.
    let (mut slot, len) = encoded_report(&Report::Issued(Issued::new("A047").unwrap()));
    assert_eq!(
        decode_report(&slot[..len]).unwrap(),
        Report::Issued(Issued::new("A047").unwrap())
    );
    slot[HEADER_LEN + 5] = b'9';
    assert!(
        decode_report(&slot[..len]).is_err(),
        "a character after the padding"
    );
    slot[HEADER_LEN + 5] = 0;
    slot[HEADER_LEN] = b'a';
    assert!(decode_report(&slot[..len]).is_err(), "a small letter");
    slot[HEADER_LEN] = 0;
    assert!(decode_report(&slot[..len]).is_err(), "no code at all");
}
