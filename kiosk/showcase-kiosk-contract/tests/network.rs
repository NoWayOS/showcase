// SPDX-License-Identifier: MIT OR Apache-2.0
//! The App's records with the network path: each round-trips with
//! one encoding, every code and padding byte is checked, an epoch or
//! attempt of 0 is refused, the kinds are apart from the app-UI
//! contract's, and no bytes make a decoder panic.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::collections::HashSet;

use nowayos_ipc::{DecodeError, EncodeError, HEADER_LEN};
use proptest::prelude::*;
use showcase_kiosk_contract::{
    ACTION_KINDS, CheckIn, CheckInAnswer, CheckedIn, ContractError, FROM_NETWORK_KINDS,
    FromNetwork, Issued, MAX_RECORD_LEN, REPORT_KINDS, Start, SubmissionOutcome, SubmissionResult,
    TO_NETWORK_KINDS, ToNetwork, VERSION, decode_action, decode_from_network, decode_report,
    decode_to_network,
};
use showcase_kiosk_flow::SubmissionTicket;

const TICKET: SubmissionTicket = SubmissionTicket {
    epoch: 7,
    session: 3,
    attempt: 2,
};

fn bytes_from(record: &FromNetwork) -> Vec<u8> {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = record.encode(&mut slot).unwrap();
    slot[..len].to_vec()
}

fn bytes_to(record: &ToNetwork) -> Vec<u8> {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let len = record.encode(&mut slot).unwrap();
    slot[..len].to_vec()
}

fn decode_error<T: std::fmt::Debug>(result: Result<T, ContractError>) -> DecodeError {
    match result {
        Err(ContractError::Decode(error)) => error,
        other => panic!("expected a decode error, got {other:?}"),
    }
}

fn every_answer() -> [CheckedIn; 3] {
    [
        CheckInAnswer::Approved { epoch: 2 },
        CheckInAnswer::NotApproved,
        CheckInAnswer::Failed,
    ]
    .map(|answer| CheckedIn { seq: 9, answer })
}

fn every_outcome() -> [SubmissionOutcome; 5] {
    [
        SubmissionOutcome::Accepted { issued: None },
        SubmissionOutcome::Accepted {
            issued: Some(Issued::new("Q12").unwrap()),
        },
        SubmissionOutcome::Rejected,
        SubmissionOutcome::Unavailable,
        SubmissionOutcome::NotApproved,
    ]
}

#[test]
fn every_record_round_trips_with_its_kind_and_version() {
    let check_in = ToNetwork::CheckIn(CheckIn { seq: 0x0102_0304 });
    let bytes = bytes_to(&check_in);
    assert_eq!(bytes, [0x41, 0x4B, 1, 0, 4, 0, 0, 0, 4, 3, 2, 1]);
    assert_eq!(decode_to_network(&bytes).unwrap(), check_in);

    let mut from: Vec<FromNetwork> = every_answer().map(FromNetwork::CheckedIn).to_vec();
    from.extend(every_outcome().map(|outcome| {
        FromNetwork::SubmissionResult(SubmissionResult {
            ticket: TICKET,
            outcome,
        })
    }));
    for record in from {
        let bytes = bytes_from(&record);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), record.kind());
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), VERSION);
        assert_eq!(decode_from_network(&bytes).unwrap(), record, "{record:?}");
    }
}

#[test]
fn layouts_match_their_tables() {
    let approved = bytes_from(&FromNetwork::CheckedIn(CheckedIn {
        seq: 5,
        answer: CheckInAnswer::Approved { epoch: 0x0A0B_0C0D },
    }));
    assert_eq!(approved[..8], [0xC1, 0x4B, 1, 0, 12, 0, 0, 0]);
    assert_eq!(
        approved[8..],
        [5, 0, 0, 0, 1, 0, 0, 0, 0x0D, 0x0C, 0x0B, 0x0A]
    );

    let result = bytes_from(&FromNetwork::SubmissionResult(SubmissionResult {
        ticket: TICKET,
        outcome: SubmissionOutcome::Accepted {
            issued: Some(Issued::new("Q12").unwrap()),
        },
    }));
    assert_eq!(result[..8], [0xC2, 0x4B, 1, 0, 20, 0, 0, 0]);
    assert_eq!(result[8..20], [7, 0, 0, 0, 3, 0, 0, 0, 2, 1, 0, 0]);
    assert_eq!(result[20..], *b"Q12\0\0\0\0\0");
}

#[test]
fn every_code_and_padding_byte_is_checked() {
    let approved = bytes_from(&FromNetwork::CheckedIn(every_answer()[0]));
    let failed = bytes_from(&FromNetwork::CheckedIn(every_answer()[2]));
    let rejected = bytes_from(&FromNetwork::SubmissionResult(SubmissionResult {
        ticket: TICKET,
        outcome: SubmissionOutcome::Rejected,
    }));
    let cases: [(&Vec<u8>, usize, u8, DecodeError); 11] = [
        // A check-in's answer: unknown, either side of the three.
        (
            &approved,
            12,
            0,
            DecodeError::InvalidValue { field: "answer" },
        ),
        (
            &approved,
            12,
            4,
            DecodeError::InvalidValue { field: "answer" },
        ),
        // Approved with epoch 0; an epoch with no approval.
        (
            &approved,
            16,
            0,
            DecodeError::InvalidValue { field: "epoch" },
        ),
        (&failed, 16, 1, DecodeError::InvalidValue { field: "epoch" }),
        (&approved, 13, 1, DecodeError::NonZeroPadding),
        // A result: a ticket's epoch or attempt of 0.
        (
            &rejected,
            8,
            0,
            DecodeError::InvalidValue { field: "epoch" },
        ),
        (
            &rejected,
            16,
            0,
            DecodeError::InvalidValue { field: "attempt" },
        ),
        // An unknown outcome, either side of the four.
        (
            &rejected,
            17,
            0,
            DecodeError::InvalidValue { field: "outcome" },
        ),
        (
            &rejected,
            17,
            5,
            DecodeError::InvalidValue { field: "outcome" },
        ),
        // A code with no acceptance; padding.
        (
            &rejected,
            20,
            b'A',
            DecodeError::InvalidValue { field: "code" },
        ),
        (&rejected, 19, 1, DecodeError::NonZeroPadding),
    ];
    for (record, at, value, expected) in cases {
        let mut bytes = record.clone();
        bytes[at] = value;
        assert_eq!(
            decode_error(decode_from_network(&bytes)),
            expected,
            "byte {at} = {value}"
        );
    }
    // An accepted result's code is an issued code, or none.
    let accepted = bytes_from(&FromNetwork::SubmissionResult(SubmissionResult {
        ticket: TICKET,
        outcome: SubmissionOutcome::Accepted { issued: None },
    }));
    let mut bytes = accepted;
    bytes[20] = b'q';
    assert_eq!(
        decode_error(decode_from_network(&bytes)),
        DecodeError::InvalidValue { field: "code" }
    );
}

#[test]
fn an_epoch_or_attempt_of_0_cannot_be_sent() {
    let mut slot = [0u8; MAX_RECORD_LEN];
    let zero = FromNetwork::CheckedIn(CheckedIn {
        seq: 1,
        answer: CheckInAnswer::Approved { epoch: 0 },
    });
    assert_eq!(
        zero.encode(&mut slot),
        Err(EncodeError::InvalidValue { field: "epoch" })
    );
    for (ticket, field) in [
        (SubmissionTicket { epoch: 0, ..TICKET }, "epoch"),
        (
            SubmissionTicket {
                attempt: 0,
                ..TICKET
            },
            "attempt",
        ),
    ] {
        let result = FromNetwork::SubmissionResult(SubmissionResult {
            ticket,
            outcome: SubmissionOutcome::Unavailable,
        });
        assert_eq!(
            result.encode(&mut slot),
            Err(EncodeError::InvalidValue { field })
        );
    }
}

#[test]
fn the_kinds_are_apart_and_a_record_on_the_wrong_ring_is_refused() {
    let mut kinds = HashSet::new();
    for kind in ACTION_KINDS
        .iter()
        .chain(&REPORT_KINDS)
        .chain(&TO_NETWORK_KINDS)
        .chain(&FROM_NETWORK_KINDS)
    {
        assert!(kinds.insert(*kind), "kind {kind:#06x} twice");
    }
    let check_in = bytes_to(&ToNetwork::CheckIn(CheckIn { seq: 1 }));
    let answer = bytes_from(&FromNetwork::CheckedIn(every_answer()[1]));
    let mut start = [0u8; MAX_RECORD_LEN];
    let len = showcase_kiosk_contract::Action::Start(Start { seq: 1 })
        .encode(&mut start)
        .unwrap();
    let start = &start[..len];
    assert!(matches!(
        decode_from_network(&check_in),
        Err(ContractError::UnexpectedKind { .. })
    ));
    assert!(matches!(
        decode_from_network(start),
        Err(ContractError::UnexpectedKind { .. })
    ));
    assert!(matches!(
        decode_to_network(&answer),
        Err(ContractError::UnexpectedKind { .. })
    ));
    assert!(matches!(
        decode_action(&answer),
        Err(ContractError::UnexpectedKind { .. })
    ));
    assert!(matches!(
        decode_report(&answer),
        Err(ContractError::UnexpectedKind { .. })
    ));
}

fn any_from_network() -> impl Strategy<Value = FromNetwork> {
    let answer = prop_oneof![
        (1..=u32::MAX).prop_map(|epoch| CheckInAnswer::Approved { epoch }),
        Just(CheckInAnswer::NotApproved),
        Just(CheckInAnswer::Failed),
    ];
    let outcome = prop_oneof![
        Just(SubmissionOutcome::Accepted { issued: None }),
        "[A-Z0-9]{1,8}".prop_map(|code| SubmissionOutcome::Accepted {
            issued: Some(Issued::new(&code).unwrap())
        }),
        Just(SubmissionOutcome::Rejected),
        Just(SubmissionOutcome::Unavailable),
        Just(SubmissionOutcome::NotApproved),
    ];
    let ticket = (1..=u32::MAX, any::<u32>(), 1..=u8::MAX).prop_map(|(epoch, session, attempt)| {
        SubmissionTicket {
            epoch,
            session,
            attempt,
        }
    });
    prop_oneof![
        (any::<u32>(), answer)
            .prop_map(|(seq, answer)| FromNetwork::CheckedIn(CheckedIn { seq, answer })),
        (ticket, outcome).prop_map(|(ticket, outcome)| FromNetwork::SubmissionResult(
            SubmissionResult { ticket, outcome }
        )),
    ]
}

proptest! {
    #[test]
    fn any_record_round_trips(record in any_from_network(), seq in any::<u32>()) {
        prop_assert_eq!(decode_from_network(&bytes_from(&record)).unwrap(), record);
        let check_in = ToNetwork::CheckIn(CheckIn { seq });
        prop_assert_eq!(decode_to_network(&bytes_to(&check_in)).unwrap(), check_in);
    }

    #[test]
    fn a_wrong_version_is_refused(record in any_from_network(), version in any::<u16>()) {
        prop_assume!(version != VERSION);
        let mut bytes = bytes_from(&record);
        bytes[2..4].copy_from_slice(&version.to_le_bytes());
        prop_assert_eq!(
            decode_from_network(&bytes),
            Err(ContractError::Decode(DecodeError::UnsupportedVersion {
                expected: VERSION,
                found: version
            }))
        );
    }

    #[test]
    fn no_bytes_make_a_decoder_panic(
        bytes in prop::collection::vec(any::<u8>(), 0..=MAX_RECORD_LEN + 1),
        kind in prop::sample::select(FROM_NETWORK_KINDS.iter().chain(&TO_NETWORK_KINDS).copied().collect::<Vec<_>>()),
    ) {
        let _ = decode_from_network(&bytes);
        let _ = decode_to_network(&bytes);
        // And with a header of one of the kinds, so the payload is read.
        let mut bytes = bytes;
        if bytes.len() >= HEADER_LEN {
            bytes[..2].copy_from_slice(&kind.to_le_bytes());
            bytes[2..4].copy_from_slice(&VERSION.to_le_bytes());
            bytes[6..8].fill(0);
            let _ = decode_from_network(&bytes);
            let _ = decode_to_network(&bytes);
        }
    }
}
