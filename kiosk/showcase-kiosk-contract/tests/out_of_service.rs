// SPDX-License-Identifier: MIT OR Apache-2.0
//! Out of service: with no session open the UI is told at once and
//! nothing else, and a start is refused until service resumes; a session
//! open at the stop - or its confirmation - runs to its own end, and the
//! out-of-service report follows its end in place of the welcome screen.
//! The property test in `session_end.rs` stops and resumes at random too.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use showcase_kiosk_contract::{
    Action, Cancel, EndReason, OutOfService, OutOfServiceReason, Refusal, Refused, Report,
    SessionEnded, ShowSubmitted, ShowWelcome, Start, Submit,
};
use showcase_kiosk_flow::{FlowError, Phase};

mod common;

use common::*;

const REASONS: [OutOfServiceReason; 3] = [
    OutOfServiceReason::AwaitingApproval {
        code: [0x3F, 0xA1, 0x07, 0x5C, 0xE2, 0x19, 0x88, 0x40],
    },
    OutOfServiceReason::NoService,
    OutOfServiceReason::Updating,
];

fn out_of_service(reason: OutOfServiceReason) -> Report {
    Report::OutOfService(OutOfService { reason })
}

#[test]
fn with_no_session_open_the_ui_is_told_at_once_and_a_start_is_refused_until_service_resumes() {
    for reason in REASONS {
        let mut app = new_app();
        let stop = app.stop_service(reason);
        assert_eq!(stop.reports(), &[out_of_service(reason)]);
        assert_eq!(app.out_of_service(), Some(reason));

        let start = send(&mut app, &Action::Start(Start { seq: 1 }), 10);
        assert_eq!(start.refused(), Some(FlowError::NotAllowed(Phase::Welcome)));
        assert_eq!(
            start.reports(),
            &[
                Report::Refused(Refused {
                    seq: 1,
                    refusal: Refusal::NotAllowed
                }),
                out_of_service(reason),
            ]
        );
        assert_eq!(app.flow().phase(), Phase::Welcome);
        assert!(app.tick(IDLE_RESET * 2).reports().is_empty());

        let resume = app.resume_service();
        assert_eq!(resume.reports(), &[Report::ShowWelcome(ShowWelcome)]);
        assert_eq!(app.out_of_service(), None);
        let _ = fill_all(&mut app, IDLE_RESET * 3);
    }
}

#[test]
fn stopping_again_changes_the_reason_and_resuming_twice_says_nothing_more() {
    let mut app = new_app();
    let _ = app.stop_service(REASONS[0]);
    let again = app.stop_service(REASONS[2]);
    assert_eq!(again.reports(), &[out_of_service(REASONS[2])]);
    assert_eq!(app.resume_service().reports().len(), 1);
    assert!(app.resume_service().reports().is_empty());
}

#[test]
fn a_session_open_at_the_stop_runs_to_its_own_end_then_the_ui_is_told() {
    for reason in REASONS {
        let mut app = new_app();
        let t = fill_all(&mut app, 0);
        // Nothing is said and nothing is cut short.
        assert!(app.stop_service(reason).reports().is_empty());
        assert_eq!(app.out_of_service(), None);
        assert_eq!(app.flow().phase(), Phase::Review);
        assert!(app.flow().has_entered_data());

        let cancel = send(&mut app, &Action::Cancel(Cancel { seq: 20 }), t + 1);
        assert_eq!(
            cancel.reports(),
            &[
                Report::SessionEnded(SessionEnded {
                    session: 1,
                    reason: EndReason::Cancelled
                }),
                out_of_service(reason),
            ]
        );
        assert_eq!(app.out_of_service(), Some(reason));
    }
}

#[test]
fn a_session_ended_by_the_idle_timeout_is_followed_by_the_report_too() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let _ = app.stop_service(OutOfServiceReason::NoService);
    let _ = app.tick(t + IDLE_RESET - IDLE_WARNING);
    let replies = app.tick(t + IDLE_RESET);
    assert_eq!(
        ended(&replies),
        &[Report::SessionEnded(SessionEnded {
            session: 1,
            reason: EndReason::Idle
        })]
    );
    assert_eq!(
        replies.reports().last(),
        Some(&out_of_service(OutOfServiceReason::NoService))
    );
}

#[test]
fn a_submission_accepted_after_the_stop_keeps_its_confirmation() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let sending = send(&mut app, &Action::Submit(Submit { seq: 30 }), t + 1);
    let ticket = sending.submission().unwrap();
    let _ = app.stop_service(OutOfServiceReason::Updating);

    let accepted = app.submission_succeeded(ticket, t + 2);
    assert_eq!(
        accepted.reports()[..2],
        [
            Report::SessionEnded(SessionEnded {
                session: 1,
                reason: EndReason::Submitted
            }),
            Report::ShowSubmitted(ShowSubmitted),
        ]
    );
    assert_eq!(app.out_of_service(), None, "the confirmation still shows");

    let over = app.tick(t + 2 + CONFIRMATION);
    assert_eq!(
        over.reports(),
        &[out_of_service(OutOfServiceReason::Updating)]
    );
}

#[test]
fn resuming_before_the_session_ends_withdraws_the_stop() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let _ = app.stop_service(OutOfServiceReason::NoService);
    assert!(app.resume_service().reports().is_empty());
    let cancel = send(&mut app, &Action::Cancel(Cancel { seq: 20 }), t + 1);
    assert_eq!(
        cancel.reports().last(),
        Some(&Report::ShowWelcome(ShowWelcome))
    );
    assert_eq!(app.out_of_service(), None);
}
