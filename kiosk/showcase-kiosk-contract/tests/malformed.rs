// SPDX-License-Identifier: MIT OR Apache-2.0
//! Malformed records, records of an unknown kind and records of the wrong
//! version are typed errors, never a panic, and a bad record from the UI
//! ends the session.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use nowayos_ipc::{DecodeError, HEADER_LEN};
use proptest::prelude::*;
use showcase_kiosk_contract::{
    ACTION_KINDS, Action, ContractError, EndReason, MAX_RECORD_LEN, MAX_TEXT_LEN, REPORT_KINDS,
    Report, SessionEnded, SetText, ShowWelcome, decode_action, decode_report,
};
use showcase_kiosk_flow::{FieldId, Phase};

mod common;

use common::*;

fn action_bytes(action: &Action) -> Vec<u8> {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = action.encode(&mut slot).unwrap();
    slot[..len].to_vec()
}

fn report_bytes(report: &Report) -> Vec<u8> {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = report.encode(&mut slot).unwrap();
    slot[..len].to_vec()
}

/// A kind that is neither an action nor a report.
fn unknown_kind() -> impl Strategy<Value = u16> {
    any::<u16>().prop_filter("not a kind of the contract", |kind| {
        !ACTION_KINDS.contains(kind) && !REPORT_KINDS.contains(kind)
    })
}

fn set_kind(bytes: &mut [u8], kind: u16) {
    bytes[..2].copy_from_slice(&kind.to_le_bytes());
}

fn set_version(bytes: &mut [u8], version: u16) {
    bytes[2..4].copy_from_slice(&version.to_le_bytes());
}

fn set_payload_len(bytes: &mut [u8], len: u16) {
    bytes[4..6].copy_from_slice(&len.to_le_bytes());
}

fn decode_error(result: Result<impl std::fmt::Debug, ContractError>) -> DecodeError {
    match result {
        Err(ContractError::Decode(err)) => err,
        other => panic!("expected a decode error, got {other:?}"),
    }
}

proptest! {
    #[test]
    fn any_bytes_decode_or_fail_without_panicking(bytes in prop::collection::vec(any::<u8>(), 0..=96)) {
        // Whatever the UI or the application sends, each end returns a
        // value or a typed error.
        let action = decode_action(&bytes);
        let report = decode_report(&bytes);
        if let Ok(action) = &action {
            prop_assert!(ACTION_KINDS.contains(&action.kind()));
        }
        if let Ok(report) = &report {
            prop_assert!(REPORT_KINDS.contains(&report.kind()));
        }
        // No record is both.
        prop_assert!(action.is_err() || report.is_err());
    }

    #[test]
    fn an_unknown_kind_is_refused_by_both_ends(
        action in any_action(), report in any_report(), kind in unknown_kind(),
    ) {
        let mut bytes = action_bytes(&action);
        set_kind(&mut bytes, kind);
        prop_assert_eq!(decode_action(&bytes), Err(ContractError::UnexpectedKind { kind }));
        let mut bytes = report_bytes(&report);
        set_kind(&mut bytes, kind);
        prop_assert_eq!(decode_report(&bytes), Err(ContractError::UnexpectedKind { kind }));
    }

    #[test]
    fn a_record_on_the_wrong_ring_is_refused(action in any_action(), report in any_report()) {
        prop_assert_eq!(
            decode_action(&report_bytes(&report)),
            Err(ContractError::UnexpectedKind { kind: report.kind() })
        );
        prop_assert_eq!(
            decode_report(&action_bytes(&action)),
            Err(ContractError::UnexpectedKind { kind: action.kind() })
        );
    }

    #[test]
    fn a_wrong_version_is_refused(
        action in any_action(), report in any_report(), version in any::<u16>(),
    ) {
        prop_assume!(version != showcase_kiosk_contract::VERSION);
        let expected = DecodeError::UnsupportedVersion {
            expected: showcase_kiosk_contract::VERSION,
            found: version,
        };
        let mut bytes = action_bytes(&action);
        set_version(&mut bytes, version);
        prop_assert_eq!(decode_action(&bytes), Err(ContractError::Decode(expected)));
        let mut bytes = report_bytes(&report);
        set_version(&mut bytes, version);
        prop_assert_eq!(decode_report(&bytes), Err(ContractError::Decode(expected)));
    }

    #[test]
    fn a_wrong_length_is_refused(action in any_action(), len in any::<u16>()) {
        let mut bytes = action_bytes(&action);
        let real = bytes.len() - HEADER_LEN;
        prop_assume!(usize::from(len) != real);
        set_payload_len(&mut bytes, len);
        prop_assert_eq!(
            decode_error(decode_action(&bytes)),
            DecodeError::WrongLength { expected: real, found: usize::from(len) }
        );
    }

    #[test]
    fn a_truncated_record_is_refused(action in any_action(), report in any_report(), cut in any::<prop::sample::Index>()) {
        let bytes = action_bytes(&action);
        let short = &bytes[..cut.index(bytes.len())];
        let truncated = matches!(decode_error(decode_action(short)), DecodeError::Truncated { .. });
        prop_assert!(truncated);
        let bytes = report_bytes(&report);
        let short = &bytes[..cut.index(bytes.len())];
        let truncated = matches!(decode_error(decode_report(short)), DecodeError::Truncated { .. });
        prop_assert!(truncated);
    }

    #[test]
    fn any_change_to_a_record_decodes_to_a_record_or_a_typed_error(
        action in any_action(), report in any_report(),
        at in any::<prop::sample::Index>(), value in any::<u8>(),
    ) {
        let mut bytes = action_bytes(&action);
        let i = at.index(bytes.len());
        bytes[i] = value;
        let _ = decode_action(&bytes);
        let mut bytes = report_bytes(&report);
        let i = at.index(bytes.len());
        bytes[i] = value;
        let _ = decode_report(&bytes);
    }

    #[test]
    fn a_bad_record_ends_the_session_at_any_point(
        backs in 0u32..=3, garbage in prop::collection::vec(any::<u8>(), 0..=96),
    ) {
        prop_assume!(decode_action(&garbage).is_err());
        let mut app = new_app();
        let mut t = fill_all(&mut app, 1_000);
        // Go back some way, so the fault arrives on a step or the review.
        for seq in 0..backs {
            t += 1;
            let _ = send(&mut app, &Action::Back(showcase_kiosk_contract::Back { seq: 100 + seq }), t);
        }
        prop_assert!(app.flow().has_entered_data());
        let session = app.flow().session();

        let fault = app.receive(&garbage, t + 1).unwrap_err();
        prop_assert_eq!(
            fault.replies.reports(),
            &[
                Report::SessionEnded(SessionEnded { session, reason: EndReason::Fault }),
                Report::ShowWelcome(ShowWelcome),
            ]
        );
        prop_assert_eq!(app.flow().phase(), Phase::Welcome);
        prop_assert!(!app.flow().has_entered_data());

        // With no session open there is nothing to end, and nothing to report.
        let again = app.receive(&garbage, t + 2).unwrap_err();
        prop_assert!(again.replies.reports().is_empty());
    }
}

#[test]
fn each_malformed_field_is_named() {
    let set_text = action_bytes(&Action::SetText(
        SetText::new(1, FieldId(2), "abc").unwrap(),
    ));
    // The payload starts at byte 8: seq 8..12, field 12..14, length 14..16,
    // text 16..80.
    let mut bytes = set_text.clone();
    bytes[14..16].copy_from_slice(&u16::try_from(MAX_TEXT_LEN + 1).unwrap().to_le_bytes());
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::InvalidValue { field: "len" }
    );
    let mut bytes = set_text.clone();
    bytes[16] = 0xFF; // not UTF-8
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::InvalidValue { field: "text" }
    );
    let mut bytes = set_text.clone();
    bytes[79] = b'x'; // text after the stated length
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::NonZeroPadding
    );
    let mut bytes = set_text;
    bytes[6] = 1; // the header's reserved field
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::ReservedNotZero
    );

    let check = action_bytes(&Action::SetChecked(showcase_kiosk_contract::SetChecked {
        seq: 1,
        field: FieldId(2),
        checked: true,
    }));
    let mut bytes = check.clone();
    bytes[14] = 2;
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::InvalidValue { field: "bool" }
    );
    let mut bytes = check;
    bytes[15] = 1;
    assert_eq!(
        decode_error(decode_action(&bytes)),
        DecodeError::NonZeroPadding
    );

    // Reports: every code is checked, and so is every padding byte.
    let cases: [(Report, usize, u8, DecodeError); 11] = [
        (
            Report::SessionEnded(SessionEnded {
                session: 1,
                reason: EndReason::Cancelled,
            }),
            12,
            0,
            DecodeError::InvalidValue { field: "reason" },
        ),
        (
            Report::SessionEnded(SessionEnded {
                session: 1,
                reason: EndReason::Cancelled,
            }),
            15,
            9,
            DecodeError::NonZeroPadding,
        ),
        (
            Report::ShowSubmitting(showcase_kiosk_contract::ShowSubmitting { attempt: 1 }),
            8,
            0,
            DecodeError::InvalidValue { field: "attempt" },
        ),
        (
            Report::ShowFailed(showcase_kiosk_contract::ShowFailed {
                reason: showcase_kiosk_flow::FailureReason::Rejected,
                attempt: 1,
                attempts_left: 2,
            }),
            8,
            4,
            DecodeError::InvalidValue { field: "reason" },
        ),
        (
            Report::ValueRefused(showcase_kiosk_contract::ValueRefused {
                seq: 1,
                field: FieldId(1),
                issue: showcase_kiosk_flow::Issue::TooLong,
            }),
            14,
            6,
            DecodeError::InvalidValue { field: "issue" },
        ),
        (
            Report::Incomplete(showcase_kiosk_contract::Incomplete {
                seq: 1,
                field: FieldId(1),
                issue: showcase_kiosk_flow::Issue::Missing,
            }),
            15,
            1,
            DecodeError::NonZeroPadding,
        ),
        (
            Report::Refused(showcase_kiosk_contract::Refused {
                seq: 1,
                refusal: showcase_kiosk_contract::Refusal::NotAllowed,
            }),
            12,
            0,
            DecodeError::InvalidValue { field: "refusal" },
        ),
        // Out of service: an unknown reason, either side of the
        // three; a code where none belongs; padding.
        (
            Report::OutOfService(showcase_kiosk_contract::OutOfService {
                reason: showcase_kiosk_contract::OutOfServiceReason::NoService,
            }),
            8,
            0,
            DecodeError::InvalidValue { field: "reason" },
        ),
        (
            Report::OutOfService(showcase_kiosk_contract::OutOfService {
                reason: showcase_kiosk_contract::OutOfServiceReason::Updating,
            }),
            8,
            4,
            DecodeError::InvalidValue { field: "reason" },
        ),
        (
            Report::OutOfService(showcase_kiosk_contract::OutOfService {
                reason: showcase_kiosk_contract::OutOfServiceReason::Updating,
            }),
            19,
            1,
            DecodeError::InvalidValue { field: "code" },
        ),
        (
            Report::OutOfService(showcase_kiosk_contract::OutOfService {
                reason: showcase_kiosk_contract::OutOfServiceReason::AwaitingApproval {
                    code: [0xFF; 8],
                },
            }),
            10,
            1,
            DecodeError::NonZeroPadding,
        ),
    ];
    for (report, at, value, expected) in cases {
        let mut bytes = report_bytes(&report);
        bytes[at] = value;
        assert_eq!(decode_error(decode_report(&bytes)), expected, "{report:?}");
    }
}

#[test]
fn a_bad_record_during_a_submission_or_confirmation_ends_it_too() {
    use showcase_kiosk_contract::Submit;

    // While the answers are on their way to the service, which `cancel`
    // would refuse.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let replies = send(&mut app, &Action::Submit(Submit { seq: 50 }), t + 1);
    let ticket = replies.submission().unwrap();
    let session = app.flow().session();
    let fault = app.receive(&[0xFF; 4], t + 2).unwrap_err();
    assert_eq!(
        fault.error,
        ContractError::Decode(DecodeError::Truncated {
            needed: 6,
            available: 4
        })
    );
    assert_eq!(
        fault.replies.session_ended(),
        Some(SessionEnded {
            session,
            reason: EndReason::Fault
        })
    );
    assert!(!app.flow().has_entered_data());
    // The result that arrives later belongs to a session that has ended.
    let late = app.submission_succeeded(ticket, t + 3);
    assert_eq!(
        late.refused(),
        Some(showcase_kiosk_flow::FlowError::StaleTicket)
    );
    assert!(late.reports().is_empty());

    // On the confirmation the session has already ended and been reported:
    // the fault only returns to the welcome screen.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 50 }), t + 1)
        .submission()
        .unwrap();
    let accepted = app.submission_succeeded(ticket, t + 2);
    assert_eq!(ended(&accepted).len(), 1);
    let fault = app.receive(&[0u8; 8], t + 3).unwrap_err();
    assert_eq!(fault.error, ContractError::UnexpectedKind { kind: 0 });
    assert_eq!(fault.replies.reports(), &[Report::ShowWelcome(ShowWelcome)]);
}

#[test]
fn an_unknown_kind_or_a_wrong_version_from_the_ui_ends_the_session() {
    use showcase_kiosk_contract::Start;

    let start = action_bytes(&Action::Start(Start { seq: 1 }));
    // A version this end does not read, a report sent on the action ring,
    // and a kind nobody defined.
    let mut newer = start.clone();
    set_version(&mut newer, 2);
    let report = report_bytes(&Report::SessionEnded(SessionEnded {
        session: 1,
        reason: EndReason::Cancelled,
    }));
    let mut unknown = start;
    set_kind(&mut unknown, 0x7777);
    let cases = [
        (
            newer,
            ContractError::Decode(DecodeError::UnsupportedVersion {
                expected: 1,
                found: 2,
            }),
        ),
        (
            report,
            ContractError::UnexpectedKind {
                kind: showcase_kiosk_contract::SESSION_ENDED_KIND,
            },
        ),
        (unknown, ContractError::UnexpectedKind { kind: 0x7777 }),
    ];
    for (bytes, expected) in cases {
        let mut app = new_app();
        let t = fill_all(&mut app, 0);
        let fault = app.receive(&bytes, t + 1).unwrap_err();
        assert_eq!(fault.error, expected);
        assert_eq!(
            fault.replies.session_ended(),
            Some(SessionEnded {
                session: 1,
                reason: EndReason::Fault
            })
        );
        assert!(!app.flow().has_entered_data());
    }
}
