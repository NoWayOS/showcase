// SPDX-License-Identifier: MIT OR Apache-2.0
//! Whenever the flow ends a session, exactly one session-ended record goes
//! to the UI, with the reason and no answer data, before the screen that
//! follows.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use proptest::prelude::*;
use showcase_kiosk_contract::{
    Abandon, Action, Back, Cancel, Countdown, EndReason, Finish, IdleWarning, Incomplete, Issued,
    MAX_REPLIES, Next, OutOfService, OutOfServiceReason, Refusal, Refused, Replies, Report, Retry,
    Select, SessionEnded, SetChecked, ShowFailed, ShowReview, ShowStep, ShowSubmitted,
    ShowSubmitting, ShowWelcome, Start, StillHere, Submit, ValueRefused,
};
use showcase_kiosk_flow::{
    FailureReason, FieldId, FlowError, Issue, Phase, SubmissionTicket, Tick,
};

mod common;

use common::*;

fn session_ended(session: u32, reason: EndReason) -> Report {
    Report::SessionEnded(SessionEnded { session, reason })
}

#[test]
fn a_cancel_ends_the_session_once() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let replies = send(&mut app, &Action::Cancel(Cancel { seq: 20 }), t + 1);
    assert_eq!(
        replies.reports(),
        &[
            session_ended(1, EndReason::Cancelled),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
    assert!(!app.flow().has_entered_data());
    // Cancelling again, with nothing open, reports nothing.
    let again = send(&mut app, &Action::Cancel(Cancel { seq: 21 }), t + 2);
    assert!(again.reports().is_empty());
}

#[test]
fn abandon_ends_the_session_in_any_phase_a_submission_in_flight_included() {
    // The UI fails closed and asks the application to end the
    // session - where Cancel is refused, while the answers are on their
    // way, too.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let sending = send(&mut app, &Action::Submit(Submit { seq: 31 }), t + 2);
    assert!(sending.submission().is_some());
    let cancel = send(&mut app, &Action::Cancel(Cancel { seq: 32 }), t + 3);
    assert!(
        app.flow().has_entered_data(),
        "Cancel is refused while sending"
    );
    assert!(cancel.session_ended().is_none());
    let replies = send(&mut app, &Action::Abandon(Abandon { seq: 33 }), t + 4);
    assert_eq!(
        replies.reports(),
        &[
            session_ended(1, EndReason::Abandoned),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
    assert!(!app.flow().has_entered_data());
    // With nothing open, it reports nothing.
    let again = send(&mut app, &Action::Abandon(Abandon { seq: 34 }), t + 5);
    assert!(again.reports().is_empty());
}

#[test]
fn the_idle_warning_then_the_idle_reset() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    assert_eq!(
        app.tick(t + IDLE_RESET - IDLE_WARNING).reports(),
        &[Report::IdleWarning(IdleWarning {
            remaining: u32::try_from(IDLE_WARNING).unwrap()
        })]
    );
    // "I'm still here" withdraws the warning: the screen is shown again.
    let here = send(
        &mut app,
        &Action::StillHere(StillHere { seq: 30 }),
        t + 50_000,
    );
    assert_eq!(here.reports(), &[Report::ShowReview(ShowReview)]);
    // Then the person walks away.
    let replies = app.tick(t + 50_000 + IDLE_RESET);
    assert_eq!(
        replies.reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
    assert!(!app.flow().has_entered_data());
}

fn countdown(remaining: Tick) -> Report {
    Report::Countdown(Countdown {
        remaining: u32::try_from(remaining).unwrap(),
    })
}

/// Ticks at each of the application's next ticks until `until`, gathering
/// every report.
fn tick_until(app: &mut App, until: Tick) -> Vec<(Tick, Report)> {
    let mut reports = Vec::new();
    while let Some(at) = app.next_tick().filter(|&at| at <= until) {
        let replies = app.tick(at);
        reports.extend(replies.reports().iter().map(|report| (at, *report)));
    }
    reports
}

#[test]
fn the_idle_warning_counts_down_each_second_then_ends_the_session_once() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let warn_at = t + IDLE_RESET - IDLE_WARNING;
    assert_eq!(app.next_tick(), Some(warn_at));
    let reset_at = t + IDLE_RESET;
    let reports = tick_until(&mut app, reset_at);
    // The warning with its time, then one countdown a second - 14 s left,
    // 13 s ... 1 s - each sent when its second begins, then exactly one
    // end of the session and the welcome screen, at the reset.
    let mut expected = vec![(
        warn_at,
        Report::IdleWarning(IdleWarning {
            remaining: u32::try_from(IDLE_WARNING).unwrap(),
        }),
    )];
    for seconds in (1..IDLE_WARNING / 1_000).rev() {
        let at = reset_at - seconds * 1_000;
        expected.push((at, countdown(seconds * 1_000)));
    }
    expected.push((reset_at, session_ended(1, EndReason::Idle)));
    expected.push((reset_at, Report::ShowWelcome(ShowWelcome)));
    assert_eq!(reports, expected);
    assert!(!app.flow().has_entered_data());
    assert_eq!(app.next_tick(), None, "nothing timed at the welcome screen");
    assert_eq!(app.countdown_deadline(), None);
}

#[test]
fn a_tick_between_seconds_or_late_reports_the_seconds_left_once() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let reset_at = t + IDLE_RESET;
    assert_eq!(app.tick(reset_at - IDLE_WARNING).reports().len(), 1);
    // Half a second on: still 15 whole seconds left, nothing to say.
    assert!(app.tick(reset_at - IDLE_WARNING + 500).reports().is_empty());
    // A timer five seconds late: the time left now, once.
    assert_eq!(
        app.tick(reset_at - 9_700).reports(),
        &[countdown(9_700)],
        "10 whole seconds left"
    );
    assert_eq!(app.next_tick(), Some(reset_at - 9_000));
    // A late timer past the reset itself: the end, and no countdown.
    assert_eq!(
        app.tick(reset_at + 3_000).reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
}

#[test]
fn still_here_stops_the_countdown() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let reset_at = t + IDLE_RESET;
    let _warned = app.tick(reset_at - IDLE_WARNING);
    let _ticked = app.tick(reset_at - 14_000);
    let here = send(
        &mut app,
        &Action::StillHere(StillHere { seq: 31 }),
        reset_at - 13_500,
    );
    assert_eq!(here.reports(), &[Report::ShowReview(ShowReview)]);
    assert_eq!(app.countdown_deadline(), None);
    // The next tick is the next warning, a whole idle period on.
    let next = reset_at - 13_500 + IDLE_RESET - IDLE_WARNING;
    assert_eq!(app.next_tick(), Some(next));
    assert!(tick_until(&mut app, next - 1).is_empty());
}

#[test]
fn the_confirmation_counts_down_each_second_then_shows_the_welcome_screen() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 62 }), t + 1)
        .submission()
        .unwrap();
    let accepted_at = t + 2;
    let accepted = app.submission_succeeded(ticket, accepted_at);
    assert_eq!(accepted.reports().last(), Some(&countdown(CONFIRMATION)));
    let ends_at = accepted_at + CONFIRMATION;
    let reports = tick_until(&mut app, ends_at);
    let mut expected: Vec<(Tick, Report)> = (1..CONFIRMATION / 1_000)
        .rev()
        .map(|seconds| (ends_at - seconds * 1_000, countdown(seconds * 1_000)))
        .collect();
    // The session ended when the answers were accepted: no second end.
    expected.push((ends_at, Report::ShowWelcome(ShowWelcome)));
    assert_eq!(reports, expected);
}

#[test]
fn what_the_service_issued_follows_the_confirmation_with_the_answers_gone() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 63 }), t + 1)
        .submission()
        .unwrap();
    let pass = Issued::new("4821").unwrap();
    let accepted = app.submission_issued(ticket, Some(pass), t + 2);
    assert_eq!(
        accepted.reports(),
        &[
            session_ended(1, EndReason::Submitted),
            Report::ShowSubmitted(ShowSubmitted),
            Report::Issued(pass),
            countdown(CONFIRMATION),
        ]
    );
    assert!(!app.flow().has_entered_data());
    // A stale ticket: nothing issued, nothing shown.
    let stale = app.submission_issued(ticket, Some(pass), t + 3);
    assert!(stale.reports().is_empty());
    assert!(stale.refused().is_some());
}

#[test]
fn a_demonstration_s_failure_switch_says_so_only_in_a_session() {
    let mut app = new_app();
    let simulate = Action::SimulateFailure(showcase_kiosk_contract::SimulateFailure { seq: 1 });
    // At the welcome screen: nothing, and no session starts.
    let idle = send(&mut app, &simulate, 0);
    assert!(!idle.simulate_failure());
    assert!(idle.reports().is_empty());
    let t = fill_all(&mut app, 10);
    let replies = send(&mut app, &simulate, t + 1);
    assert!(replies.simulate_failure());
    // The flow is as it was: still a review to send, no report.
    assert!(replies.reports().is_empty());
    assert_eq!(app.flow().phase(), Phase::Review);
    assert!(app.flow().has_entered_data());
}

#[test]
fn the_maximum_length_ends_the_session_however_active_the_person_is() {
    let mut app = new_app();
    let mut t = fill_all(&mut app, 0);
    let mut seq = 40;
    let replies = loop {
        t += IDLE_RESET / 2;
        seq += 1;
        let replies = send(&mut app, &Action::StillHere(StillHere { seq }), t);
        if !replies.reports().is_empty() {
            break replies;
        }
    };
    // "Still here" is no refusal on the welcome screen either.
    assert_eq!(
        replies.reports(),
        &[
            session_ended(1, EndReason::TimeLimit),
            Report::ShowWelcome(ShowWelcome),
        ]
    );
    assert!(t >= MAX_SESSION);
    assert!(!app.flow().has_entered_data());
}

#[test]
fn an_accepted_submission_ends_the_session_and_the_confirmation_does_not_end_another() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let submitting = send(&mut app, &Action::Submit(Submit { seq: 60 }), t + 1);
    assert_eq!(
        submitting.reports(),
        &[Report::ShowSubmitting(ShowSubmitting { attempt: 1 })]
    );
    let ticket = submitting.submission().unwrap();
    let accepted = app.submission_succeeded(ticket, t + 2);
    assert_eq!(
        accepted.reports(),
        &[
            session_ended(1, EndReason::Submitted),
            Report::ShowSubmitted(ShowSubmitted),
            countdown(CONFIRMATION),
        ]
    );
    assert!(!app.flow().has_entered_data());
    // The confirmation ends by itself, or early: back to the welcome screen,
    // and no second session-ended record.
    let over = app.tick(t + 2 + CONFIRMATION);
    assert_eq!(over.reports(), &[Report::ShowWelcome(ShowWelcome)]);

    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 60 }), t + 1)
        .submission()
        .unwrap();
    assert_eq!(ended(&app.submission_succeeded(ticket, t + 2)).len(), 1);
    let finished = send(&mut app, &Action::Finish(Finish { seq: 61 }), t + 3);
    assert_eq!(finished.reports(), &[Report::ShowWelcome(ShowWelcome)]);
}

#[test]
fn a_failed_submission_keeps_the_session_and_says_what_is_left() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 70 }), t + 1)
        .submission()
        .unwrap();
    let failed = app.submission_failed(ticket, FailureReason::Unavailable, t + 2);
    assert_eq!(
        failed.reports(),
        &[Report::ShowFailed(ShowFailed {
            reason: FailureReason::Unavailable,
            attempt: 1,
            attempts_left: MAX_ATTEMPTS - 1,
        })]
    );
    assert!(app.flow().has_entered_data());
    let retry = send(&mut app, &Action::Retry(Retry { seq: 71 }), t + 3);
    assert_eq!(
        retry.reports(),
        &[Report::ShowSubmitting(ShowSubmitting { attempt: 2 })]
    );
    assert!(retry.submission().is_some());

    // No result in time: the attempt fails by itself.
    let timed_out = app.tick(t + 3 + SUBMIT);
    assert_eq!(
        timed_out.reports(),
        &[Report::ShowFailed(ShowFailed {
            reason: FailureReason::TimedOut,
            attempt: 2,
            attempts_left: MAX_ATTEMPTS - 2,
        })]
    );
}

#[test]
fn a_result_that_finds_the_session_over_reports_its_end() {
    // Submitted, then nothing heard for the submit timeout and a whole idle
    // timeout after it: the result arrives too late.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 80 }), t + 1)
        .submission()
        .unwrap();
    let late = app.submission_succeeded(ticket, t + 1 + SUBMIT + IDLE_RESET);
    assert_eq!(late.refused(), Some(FlowError::Expired));
    assert_eq!(
        late.reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::ShowWelcome(ShowWelcome)
        ]
    );

    // A result after the session's maximum length names that.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let ticket = send(&mut app, &Action::Submit(Submit { seq: 80 }), t + 1)
        .submission()
        .unwrap();
    let late = app.submission_failed(ticket, FailureReason::Unavailable, MAX_SESSION);
    assert_eq!(late.session_ended().unwrap().reason, EndReason::TimeLimit);
}

#[test]
fn an_action_after_the_timeout_finds_the_session_ended_first() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    // No tick arrived in time; the person's next touch comes far too late.
    let late = send(
        &mut app,
        &Action::Back(Back { seq: 90 }),
        t + 2 * IDLE_RESET,
    );
    assert_eq!(
        late.reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::Refused(Refused {
                seq: 90,
                refusal: Refusal::NotAllowed
            }),
            Report::ShowWelcome(ShowWelcome),
        ]
    );
    // Start, late too, ends the old session and begins the next in one call.
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    let restart = send(
        &mut app,
        &Action::Start(Start { seq: 91 }),
        t + 2 * IDLE_RESET,
    );
    assert_eq!(
        restart.reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::ShowStep(ShowStep { index: 0 })
        ]
    );
    assert_eq!(app.flow().session(), 2);
    assert!(!app.flow().has_entered_data());
}

#[test]
fn a_clock_that_goes_backwards_ends_the_session() {
    let mut app = new_app();
    let t = fill_all(&mut app, 10_000);
    let replies = app.tick(t - 5_000);
    assert_eq!(
        replies.reports(),
        &[
            session_ended(1, EndReason::Idle),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
}

#[test]
fn ending_for_a_fault_elsewhere() {
    let mut app = new_app();
    let t = fill_all(&mut app, 0);
    assert_eq!(
        app.end_session(t + 1).reports(),
        &[
            session_ended(1, EndReason::Fault),
            Report::ShowWelcome(ShowWelcome)
        ]
    );
    assert!(app.end_session(t + 2).reports().is_empty());
}

#[test]
fn refusals_quote_the_action_and_a_refused_value_is_cleared() {
    let mut app = new_app();
    let _ = send(&mut app, &Action::Start(Start { seq: 1 }), 0);
    let _ = send(&mut app, &text(2, CODE, "1234"), 1);
    assert!(!app.flow().value(CODE).unwrap().is_empty());
    // A letter in a digits-only field: refused, and the field is cleared, as
    // the UI clears its copy.
    let refused = send(&mut app, &text(3, CODE, "12a4"), 2);
    assert_eq!(
        refused.reports(),
        &[Report::ValueRefused(ValueRefused {
            seq: 3,
            field: CODE,
            issue: Issue::BadCharacter
        })]
    );
    assert!(app.flow().value(CODE).unwrap().is_empty());
    // Next without the required name: the first field that needs attention.
    let next = send(&mut app, &Action::Next(Next { seq: 4 }), 3);
    assert_eq!(
        next.reports(),
        &[Report::Incomplete(Incomplete {
            seq: 4,
            field: NAME,
            issue: Issue::Missing
        })]
    );
    // An option the field does not have.
    let pick = Select {
        seq: 5,
        field: TOPIC,
        option: 0,
    };
    let wrong_step = send(&mut app, &Action::Select(pick), 4);
    assert_eq!(
        wrong_step.reports(),
        &[
            Report::Refused(Refused {
                seq: 5,
                refusal: Refusal::NotOnThisStep
            }),
            Report::ShowStep(ShowStep { index: 0 })
        ]
    );
    let back = send(&mut app, &Action::Back(Back { seq: 6 }), 5);
    assert_eq!(
        back.reports(),
        &[
            Report::Refused(Refused {
                seq: 6,
                refusal: Refusal::AtFirstStep
            }),
            Report::ShowStep(ShowStep { index: 0 })
        ]
    );
    let unknown = SetChecked {
        seq: 7,
        field: FieldId(99),
        checked: true,
    };
    assert_eq!(
        send(&mut app, &Action::SetChecked(unknown), 6).reports()[0],
        Report::Refused(Refused {
            seq: 7,
            refusal: Refusal::UnknownField
        })
    );
    let wrong_kind = SetChecked {
        seq: 8,
        field: NAME,
        checked: true,
    };
    assert_eq!(
        send(&mut app, &Action::SetChecked(wrong_kind), 7).reports()[0],
        Report::Refused(Refused {
            seq: 8,
            refusal: Refusal::WrongKind
        })
    );
}

/// What the property test does next.
#[derive(Debug, Clone)]
enum Op {
    Act(Action),
    Tick,
    Succeeded,
    Failed(FailureReason),
    StaleResult,
    Garbage(Vec<u8>),
    EndSession,
    /// The application stops opening sessions.
    Stop(OutOfServiceReason),
    /// And opens them again.
    Resume,
}

fn op() -> impl Strategy<Value = Op> {
    let field = prop_oneof![
        4 => prop::sample::select(FIELDS.to_vec()),
        1 => any::<u16>().prop_map(FieldId),
    ];
    let value = prop_oneof![
        Just(String::new()),
        Just("Alex Example".to_owned()),
        Just("4321".to_owned()),
        "[a-z0-9 ]{0,40}",
        "\\PC{0,16}",
    ];
    let action = prop_oneof![
        3 => Just(Action::Start(Start { seq: 0 })),
        6 => (field.clone(), value).prop_map(|(field, value)| text(0, field, &value)),
        3 => (field.clone(), 0u8..4).prop_map(|(field, option)| Action::Select(Select { seq: 0, field, option })),
        3 => (field.clone(), any::<bool>()).prop_map(|(field, checked)| Action::SetChecked(SetChecked { seq: 0, field, checked })),
        1 => field.prop_map(|field| Action::ClearField(showcase_kiosk_contract::ClearField { seq: 0, field })),
        6 => Just(Action::Next(Next { seq: 0 })),
        2 => Just(Action::Back(Back { seq: 0 })),
        3 => Just(Action::Submit(Submit { seq: 0 })),
        2 => Just(Action::Retry(Retry { seq: 0 })),
        1 => Just(Action::Cancel(Cancel { seq: 0 })),
        1 => Just(Action::Finish(Finish { seq: 0 })),
        2 => Just(Action::StillHere(StillHere { seq: 0 })),
        1 => Just(Action::Abandon(Abandon { seq: 0 })),
        1 => Just(Action::SimulateFailure(showcase_kiosk_contract::SimulateFailure { seq: 0 })),
    ];
    prop_oneof![
        12 => action.prop_map(Op::Act),
        3 => Just(Op::Tick),
        2 => Just(Op::Succeeded),
        2 => prop_oneof![
            Just(FailureReason::TimedOut),
            Just(FailureReason::Unavailable),
            Just(FailureReason::Rejected),
        ].prop_map(Op::Failed),
        1 => Just(Op::StaleResult),
        1 => prop::collection::vec(any::<u8>(), 0..=24).prop_map(Op::Garbage),
        1 => Just(Op::EndSession),
        1 => out_of_service_reason().prop_map(Op::Stop),
        1 => Just(Op::Resume),
    ]
}

/// How far time moves before an operation: mostly a little, sometimes past
/// a timeout, sometimes backwards (a clock fault).
fn step() -> impl Strategy<Value = i64> {
    prop_oneof![
        8 => 0i64..5_000,
        3 => 5_000i64..80_000,
        1 => 80_000i64..400_000,
        1 => -5_000i64..0,
    ]
}

fn run(
    app: &mut App,
    op: &Op,
    now: Tick,
    ticket: &mut Option<SubmissionTicket>,
    seq: u32,
) -> Replies {
    match op {
        Op::Act(action) => {
            let mut action = action.clone();
            match &mut action {
                Action::Start(a) => a.seq = seq,
                Action::SetText(a) => a.seq = seq,
                Action::Select(a) => a.seq = seq,
                Action::SetChecked(a) => a.seq = seq,
                Action::ClearField(a) => a.seq = seq,
                Action::Next(a) => a.seq = seq,
                Action::Back(a) => a.seq = seq,
                Action::Submit(a) => a.seq = seq,
                Action::Retry(a) => a.seq = seq,
                Action::Cancel(a) => a.seq = seq,
                Action::Finish(a) => a.seq = seq,
                Action::StillHere(a) => a.seq = seq,
                Action::Abandon(a) => a.seq = seq,
                Action::SimulateFailure(a) => a.seq = seq,
            }
            let replies = send(app, &action, now);
            if let Some(started) = replies.submission() {
                *ticket = Some(started);
            }
            replies
        }
        Op::Tick => app.tick(now),
        Op::Succeeded => match *ticket {
            Some(t) => app.submission_succeeded(t, now),
            None => app.tick(now),
        },
        Op::Failed(reason) => match *ticket {
            Some(t) => app.submission_failed(t, *reason, now),
            None => app.tick(now),
        },
        Op::StaleResult => {
            let stale = SubmissionTicket {
                epoch: 99,
                session: 1,
                attempt: 1,
            };
            app.submission_succeeded(stale, now)
        }
        Op::Garbage(bytes) => match app.receive(bytes, now) {
            Ok(replies) => replies,
            Err(fault) => fault.replies,
        },
        Op::EndSession => app.end_session(now),
        Op::Stop(reason) => app.stop_service(*reason),
        Op::Resume => app.resume_service(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_end_of_a_session_is_reported_exactly_once(
        ops in prop::collection::vec((op(), step()), 1..60),
    ) {
        let mut app = new_app();
        let mut now: Tick = 1_000_000;
        let mut ticket = None;
        let mut sessions_ended = 0u32;
        let mut reports_ended = 0u32;
        // The last stop not since resumed.
        let mut stopped: Option<OutOfServiceReason> = None;
        for (index, (op, step)) in ops.iter().enumerate() {
            now = now.saturating_add_signed(*step);
            let session_before = app.flow().session();
            let open_before = app.flow().phase().holds_answers();
            let phase_before = app.flow().phase();
            let out_before = app.out_of_service();
            let data_before = app.flow().has_entered_data();
            let replies = run(&mut app, op, now, &mut ticket, u32::try_from(index).unwrap());
            let session_after = app.flow().session();
            let open_after = app.flow().phase().holds_answers();

            // A stop or a resume never cuts a session short, nor its
            // confirmation: with no session open, a stop is reported at
            // once and alone; a resume of a kiosk out of service shows the
            // welcome screen.
            match op {
                Op::Stop(reason) => {
                    stopped = Some(*reason);
                    let expected: &[Report] = if phase_before == Phase::Welcome {
                        &[Report::OutOfService(OutOfService { reason: *reason })]
                    } else {
                        &[]
                    };
                    prop_assert_eq!(replies.reports(), expected);
                }
                Op::Resume => {
                    stopped = None;
                    let expected: &[Report] = if out_before.is_some() {
                        &[Report::ShowWelcome(ShowWelcome)]
                    } else {
                        &[]
                    };
                    prop_assert_eq!(replies.reports(), expected);
                }
                _ => {}
            }
            if matches!(op, Op::Stop(_) | Op::Resume) {
                prop_assert_eq!(app.flow().phase(), phase_before);
                prop_assert_eq!(session_after, session_before);
                prop_assert_eq!(app.flow().has_entered_data(), data_before);
            }
            // Out of service, no session opens; at the welcome screen the
            // kiosk is out of service exactly while stopped.
            if app.out_of_service().is_some() {
                prop_assert_eq!(app.flow().phase(), Phase::Welcome);
                prop_assert!(!open_after);
            }
            if app.flow().phase() == Phase::Welcome {
                prop_assert_eq!(app.out_of_service(), stopped);
            }

            // The session open before the call ended in it if the flow
            // left the phases that hold answers, or moved on to a new
            // session.
            let ended_here = open_before && (!open_after || session_after != session_before);
            let reported: Vec<SessionEnded> = replies
                .reports()
                .iter()
                .filter_map(|report| match report {
                    Report::SessionEnded(ended) => Some(*ended),
                    _ => None,
                })
                .collect();
            prop_assert_eq!(reported.len(), usize::from(ended_here), "{:?} -> {:?}", op, replies);
            if let Some(ended) = reported.first() {
                sessions_ended += 1;
                prop_assert_eq!(ended.session, session_before);
                // Whatever follows, the session's answers are gone.
                prop_assert!(!app.flow().has_entered_data());
                // It comes before any screen.
                let at = replies.reports().iter().position(|r| matches!(r, Report::SessionEnded(_))).unwrap();
                prop_assert!(replies.reports()[..at].iter().all(|r| !r.is_screen()));
            }
            reports_ended += u32::try_from(reported.len()).unwrap();
            prop_assert!(replies.reports().len() <= MAX_REPLIES);
            // A countdown only while a screen counts down, with
            // its true time left, at most one a call, and last.
            let countdowns: Vec<Countdown> = replies
                .reports()
                .iter()
                .filter_map(|report| match report {
                    Report::Countdown(countdown) => Some(*countdown),
                    _ => None,
                })
                .collect();
            prop_assert!(countdowns.len() <= 1);
            if let Some(countdown) = countdowns.first() {
                let deadline = app.countdown_deadline();
                prop_assert!(deadline.is_some_and(|deadline| deadline > now), "{:?}", replies);
                let left = deadline.unwrap() - now;
                prop_assert_eq!(u64::from(countdown.remaining), left);
                prop_assert!(matches!(replies.reports().last(), Some(Report::Countdown(_))));
            }
            // At most one screen per call, and it is the flow's phase.
            let screens: Vec<&Report> = replies.reports().iter().filter(|r| r.is_screen()).collect();
            prop_assert!(screens.len() <= 1);
            if let Some(screen) = screens.first() {
                let matches_phase = match (screen, app.flow().phase()) {
                    (Report::ShowWelcome(_), Phase::Welcome) => app.out_of_service().is_none(),
                    (Report::OutOfService(shown), Phase::Welcome) => {
                        app.out_of_service() == Some(shown.reason)
                    }
                    (Report::ShowReview(_), Phase::Review)
                    | (Report::ShowSubmitted(_), Phase::Submitted) => true,
                    (Report::ShowStep(s), Phase::Step { index }) => usize::from(s.index) == index,
                    (Report::ShowSubmitting(s), Phase::Submitting { attempt }) => s.attempt == attempt,
                    (Report::ShowFailed(s), Phase::Failed { reason, attempt }) => {
                        s.reason == reason && s.attempt == attempt
                    }
                    _ => false,
                };
                prop_assert!(matches_phase, "{:?} shown in {:?}", screen, app.flow().phase());
            }
        }
        prop_assert_eq!(sessions_ended, reports_ended);
    }
}
