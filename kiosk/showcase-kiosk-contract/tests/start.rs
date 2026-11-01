// SPDX-License-Identifier: MIT OR Apache-2.0
//! The App's start: no session opens before a check-in gives the
//! App its epoch; the flow is made with exactly that epoch, which never
//! changes within a start; answers that do not belong change nothing; a
//! stale result is refused; and a kiosk whose key is revoked serves out the
//! session open, then stays out of service.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use proptest::prelude::*;
use showcase_kiosk_contract::{
    APPROVAL_CODE_LEN, Action, AppStart, Cancel, CheckIn, CheckInAnswer, CheckInTimes, CheckedIn,
    EndReason, Next, OutOfService, OutOfServiceReason, Refusal, Refused, Replies, Report,
    SessionEnded, SetChecked, ShowFailed, ShowWelcome, Start, StartError, StartSetupError,
    SubmissionOutcome, SubmissionResult, Submit,
};
use showcase_kiosk_flow::{FailureReason, FlowError, Phase, SubmissionTicket, Tick};

mod common;

use common::*;

const KEY_CODE: [u8; APPROVAL_CODE_LEN] = [0x3F, 0xA1, 0x07, 0x5C, 0xE2, 0x19, 0x88, 0x40];
const TIMES: CheckInTimes = CheckInTimes {
    retry: 5_000,
    timeout: 10_000,
};

type Start8 = AppStart<'static, 8, 64>;

fn new_start() -> Start8 {
    AppStart::new(&FORM, CONFIG, KEY_CODE, TIMES).unwrap()
}

fn awaiting() -> Report {
    Report::OutOfService(OutOfService {
        reason: OutOfServiceReason::AwaitingApproval { code: KEY_CODE },
    })
}

fn no_service() -> Report {
    Report::OutOfService(OutOfService {
        reason: OutOfServiceReason::NoService,
    })
}

fn answer(seq: u32, answer: CheckInAnswer) -> CheckedIn {
    CheckedIn { seq, answer }
}

/// Checks in at `now` and is approved with `epoch`.
fn approve(app: &mut Start8, epoch: u32, now: Tick) {
    let asked = app.check_in(now).unwrap();
    let replies = app
        .checked_in(&answer(asked.seq, CheckInAnswer::Approved { epoch }), now)
        .unwrap();
    assert_eq!(replies.reports(), &[Report::ShowWelcome(ShowWelcome)]);
}

/// Starts a session at `t` and answers every field, ending at the review.
fn fill(app: &mut Start8, t: Tick) -> Tick {
    let actions = fill_actions();
    for (i, action) in actions.iter().enumerate() {
        let _ = app.apply(action, t + i as Tick);
    }
    assert_eq!(app.flow().unwrap().phase(), Phase::Review);
    t + actions.len() as Tick
}

/// Start, then every field answered, ending at the review.
fn fill_actions() -> [Action; 7] {
    [
        Action::Start(Start { seq: 1 }),
        text(2, NAME, "Alex Example"),
        Action::Next(Next { seq: 3 }),
        Action::Select(showcase_kiosk_contract::Select {
            seq: 4,
            field: TOPIC,
            option: 1,
        }),
        Action::Next(Next { seq: 5 }),
        Action::SetChecked(SetChecked {
            seq: 6,
            field: AGREE,
            checked: true,
        }),
        Action::Next(Next { seq: 7 }),
    ]
}

#[test]
fn before_the_epoch_no_session_opens_and_the_ui_is_told_why() {
    let mut app = new_start();
    assert_eq!(app.start_reports().reports(), &[awaiting()]);
    assert_eq!(app.epoch(), None);
    assert!(app.flow().is_none());

    let start = app.apply(&Action::Start(Start { seq: 4 }), 0);
    assert_eq!(
        start.reports(),
        &[
            Report::Refused(Refused {
                seq: 4,
                refusal: Refusal::NotAllowed
            }),
            awaiting(),
        ]
    );
    assert_eq!(start.refused(), Some(FlowError::NotAllowed(Phase::Welcome)));

    // A check-in that fails: no service, until a later one.
    let asked = app.check_in(0).unwrap();
    let failed = app
        .checked_in(&answer(asked.seq, CheckInAnswer::Failed), 10)
        .unwrap();
    assert_eq!(failed.reports(), &[no_service()]);
    let start = send_start(&mut app, 20);
    assert_eq!(start.reports()[1], no_service());
    assert!(app.flow().is_none());
}

fn send_start(app: &mut Start8, now: Tick) -> Replies {
    let mut slot = [0u8; showcase_kiosk_contract::MAX_RECORD_LEN];
    let len = Action::Start(Start { seq: 9 }).encode(&mut slot).unwrap();
    app.receive(&slot[..len], now).unwrap()
}

#[test]
fn a_record_from_the_ui_that_cannot_be_decoded_opens_nothing_either() {
    let mut app = new_start();
    let faulted = app.receive(&[0xFF; 3], 0).unwrap_err();
    assert_eq!(faulted.replies.reports(), &[awaiting()]);
    assert!(app.out_of_service().is_some());
}

#[test]
fn it_checks_in_at_once_then_again_after_each_answer_that_is_no_approval() {
    let mut app = new_start();
    assert_eq!(app.next_tick(), Some(0));
    let first = app.check_in(0).unwrap();
    // One check-in at a time.
    assert_eq!(app.check_in(1), None);
    assert_eq!(app.next_tick(), Some(TIMES.timeout));

    let not_approved = app
        .checked_in(&answer(first.seq, CheckInAnswer::NotApproved), 100)
        .unwrap();
    // Still awaiting approval: nothing new to say.
    assert!(not_approved.reports().is_empty());
    assert_eq!(app.next_tick(), Some(100 + TIMES.retry));
    assert_eq!(app.check_in(100 + TIMES.retry - 1), None);
    let second = app.check_in(100 + TIMES.retry).unwrap();
    assert_ne!(second.seq, first.seq);

    // Unanswered by its deadline: failed.
    let deadline = 100 + TIMES.retry + TIMES.timeout;
    assert!(app.tick(deadline - 1).reports().is_empty());
    assert_eq!(app.tick(deadline).reports(), &[no_service()]);
    // Its answer, late, finds nothing waiting.
    assert_eq!(
        app.checked_in(
            &answer(second.seq, CheckInAnswer::Approved { epoch: 3 }),
            deadline + 1
        ),
        Err(StartError::NotAsked { seq: second.seq })
    );
    assert_eq!(app.epoch(), None);

    let third = app.check_in(deadline + TIMES.retry).unwrap();
    let approved = app
        .checked_in(
            &answer(third.seq, CheckInAnswer::Approved { epoch: 41 }),
            deadline + TIMES.retry,
        )
        .unwrap();
    assert_eq!(approved.reports(), &[Report::ShowWelcome(ShowWelcome)]);
    assert_eq!(app.epoch(), Some(41));
    assert_eq!(app.out_of_service(), None);
    // No more check-ins in this start.
    assert_eq!(app.check_in(u64::MAX), None);
}

#[test]
fn the_flow_is_made_with_exactly_the_epoch_and_its_tickets_carry_it() {
    let mut app = new_start();
    approve(&mut app, 0x0BAD_CAFE, 0);
    let t = fill(&mut app, 10);
    let sending = app.apply(&Action::Submit(Submit { seq: 30 }), t);
    let ticket = sending.submission().unwrap();
    assert_eq!(ticket.epoch, 0x0BAD_CAFE);
}

#[test]
fn answers_that_do_not_belong_change_nothing() {
    let mut app = new_start();
    // Nothing asked yet.
    assert_eq!(
        app.checked_in(&answer(1, CheckInAnswer::Approved { epoch: 5 }), 0),
        Err(StartError::NotAsked { seq: 1 })
    );
    let asked = app.check_in(0).unwrap();
    // Another check-in's answer.
    assert_eq!(
        app.checked_in(
            &answer(asked.seq + 1, CheckInAnswer::Approved { epoch: 5 }),
            1
        ),
        Err(StartError::NotAsked { seq: asked.seq + 1 })
    );
    // An epoch of 0: refused, and the check-in still waits for its answer.
    assert_eq!(
        app.checked_in(&answer(asked.seq, CheckInAnswer::Approved { epoch: 0 }), 2),
        Err(StartError::ZeroEpoch)
    );
    assert_eq!(app.next_tick(), Some(TIMES.timeout));
    let _ = app
        .checked_in(&answer(asked.seq, CheckInAnswer::Approved { epoch: 5 }), 3)
        .unwrap();
    // A second answer in the same start.
    for second in [
        CheckInAnswer::Approved { epoch: 6 },
        CheckInAnswer::NotApproved,
        CheckInAnswer::Failed,
    ] {
        assert_eq!(
            app.checked_in(&answer(asked.seq, second), 4),
            Err(StartError::AlreadyStarted)
        );
    }
    assert_eq!(app.epoch(), Some(5));
    assert_eq!(app.out_of_service(), None);
}

#[test]
fn a_result_before_the_epoch_is_refused() {
    let mut app = new_start();
    let result = SubmissionResult {
        ticket: SubmissionTicket {
            epoch: 1,
            session: 1,
            attempt: 1,
        },
        outcome: SubmissionOutcome::Accepted { issued: None },
    };
    assert_eq!(
        app.submission_result(&result, 0).unwrap_err(),
        StartError::NotStarted
    );
}

#[test]
fn a_fixed_epoch_of_0_is_refused() {
    assert_eq!(
        Start8::started(&FORM, CONFIG, 0).unwrap_err(),
        StartSetupError::ZeroEpoch
    );
    let app = Start8::started(&FORM, CONFIG, 3).unwrap();
    assert_eq!(app.epoch(), Some(3));
    assert_eq!(
        app.start_reports().reports(),
        &[Report::ShowWelcome(ShowWelcome)]
    );
}

#[test]
fn a_revoked_kiosk_serves_out_the_session_then_stays_out_of_service() {
    let mut app = new_start();
    approve(&mut app, 2, 0);
    let t = fill(&mut app, 10);
    let ticket = app
        .apply(&Action::Submit(Submit { seq: 30 }), t)
        .submission()
        .unwrap();
    let revoked = app
        .submission_result(
            &SubmissionResult {
                ticket,
                outcome: SubmissionOutcome::NotApproved,
            },
            t + 1,
        )
        .unwrap();
    // The person sees the service unavailable, and the session goes on.
    assert!(matches!(
        revoked.reports(),
        [Report::ShowFailed(ShowFailed {
            reason: FailureReason::Unavailable,
            ..
        })]
    ));
    assert!(app.is_revoked());
    assert!(app.flow().unwrap().has_entered_data());
    assert_eq!(app.out_of_service(), None);

    let cancel = app.apply(&Action::Cancel(Cancel { seq: 31 }), t + 2);
    assert_eq!(
        cancel.reports(),
        &[
            Report::SessionEnded(SessionEnded {
                session: 1,
                reason: EndReason::Cancelled
            }),
            no_service(),
        ]
    );
    // No session opens again, and no check-in is made, in this start.
    let start = app.apply(&Action::Start(Start { seq: 32 }), t + 3);
    assert_eq!(start.refused(), Some(FlowError::NotAllowed(Phase::Welcome)));
    assert_eq!(app.check_in(t + 1_000_000), None);
    assert!(app.tick(t + 1_000_000).reports().is_empty());
    assert_eq!(app.out_of_service(), Some(OutOfServiceReason::NoService));
}

// The property test: random check-in answers, submission results, actions
// and ticks.

#[derive(Debug, Clone)]
enum Op {
    /// Send the check-in due, if any.
    CheckIn,
    /// An answer to the last check-in, or to another `seq`.
    Answer {
        own: bool,
        answer: CheckInAnswer,
    },
    Act(Action),
    /// Start a session and answer every field, as a person would.
    Fill,
    Tick,
    /// A result for the last submission, or one with its epoch, session or
    /// attempt changed.
    Result {
        change: Change,
        outcome: SubmissionOutcome,
    },
    Garbage(Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
enum Change {
    None,
    Epoch(u32),
    Session(u32),
    Attempt(u8),
}

fn op() -> impl Strategy<Value = Op> {
    let check_in_answer = prop_oneof![
        4 => (0u32..4).prop_map(|epoch| CheckInAnswer::Approved { epoch }),
        1 => any::<u32>().prop_map(|epoch| CheckInAnswer::Approved { epoch }),
        2 => Just(CheckInAnswer::NotApproved),
        1 => Just(CheckInAnswer::Failed),
    ];
    let action = prop_oneof![
        3 => Just(Action::Start(Start { seq: 0 })),
        3 => Just(text(0, NAME, "Alex Example")),
        2 => Just(Action::Select(showcase_kiosk_contract::Select { seq: 0, field: TOPIC, option: 1 })),
        2 => Just(Action::SetChecked(SetChecked { seq: 0, field: AGREE, checked: true })),
        5 => Just(Action::Next(Next { seq: 0 })),
        3 => Just(Action::Submit(Submit { seq: 0 })),
        2 => Just(Action::Retry(showcase_kiosk_contract::Retry { seq: 0 })),
        1 => Just(Action::Cancel(Cancel { seq: 0 })),
        1 => Just(Action::Finish(showcase_kiosk_contract::Finish { seq: 0 })),
    ];
    let change = prop_oneof![
        4 => Just(Change::None),
        1 => any::<u32>().prop_map(Change::Epoch),
        1 => any::<u32>().prop_map(Change::Session),
        1 => (1..=u8::MAX).prop_map(Change::Attempt),
    ];
    let outcome = prop_oneof![
        1 => Just(SubmissionOutcome::Accepted { issued: None }),
        1 => Just(SubmissionOutcome::Rejected),
        1 => Just(SubmissionOutcome::Unavailable),
        2 => Just(SubmissionOutcome::NotApproved),
    ];
    prop_oneof![
        4 => Just(Op::CheckIn),
        4 => (prop::bool::weighted(0.9), check_in_answer).prop_map(|(own, answer)| Op::Answer { own, answer }),
        8 => action.prop_map(Op::Act),
        3 => Just(Op::Fill),
        3 => Just(Op::Tick),
        4 => (change, outcome).prop_map(|(change, outcome)| Op::Result { change, outcome }),
        1 => prop::collection::vec(any::<u8>(), 0..=16).prop_map(Op::Garbage),
    ]
}

fn step() -> impl Strategy<Value = Tick> {
    prop_oneof![
        6 => 0u64..2_000,
        3 => 2_000u64..12_000,
        1 => 12_000u64..120_000,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn no_session_before_the_epoch_one_epoch_a_start_and_stale_results_refused(
        ops in prop::collection::vec((op(), step()), 10..80),
    ) {
        let mut app = new_start();
        let mut now: Tick = 0;
        let mut last_seq: Option<u32> = None;
        let mut epoch: Option<u32> = None;
        let mut ticket: Option<SubmissionTicket> = None;
        let mut seq = 0u32;
        for (op, step) in &ops {
            now += step;
            seq += 1;
            let revoked_before = app.is_revoked();
            let replies = match op {
                Op::CheckIn => match app.check_in(now) {
                    Some(CheckIn { seq }) => {
                        prop_assert!(epoch.is_none(), "a check-in after the epoch");
                        last_seq = Some(seq);
                        continue;
                    }
                    None => continue,
                },
                Op::Answer { own, answer: given } => {
                    let asked = if *own { last_seq.unwrap_or(0) } else { last_seq.unwrap_or(0).wrapping_add(7) };
                    let before = (app.epoch(), app.out_of_service(), app.next_tick());
                    match app.checked_in(&answer(asked, *given), now) {
                        Ok(replies) => {
                            if let CheckInAnswer::Approved { epoch: given } = given {
                                prop_assert_eq!(app.epoch(), Some(*given));
                                prop_assert_eq!(replies.reports(), &[Report::ShowWelcome(ShowWelcome)]);
                            }
                            replies
                        }
                        Err(error) => {
                            // Refused: nothing changed.
                            prop_assert_eq!((app.epoch(), app.out_of_service(), app.next_tick()), before);
                            // After the epoch, every answer is a second one.
                            prop_assert_eq!(epoch.is_some(), error == StartError::AlreadyStarted);
                            continue;
                        }
                    }
                }
                Op::Act(action) => {
                    let mut action = action.clone();
                    set_seq(&mut action, seq);
                    let replies = app.apply(&action, now);
                    if epoch.is_none() && app.epoch().is_none() {
                        prop_assert_eq!(replies.refused(), Some(FlowError::NotAllowed(Phase::Welcome)));
                    }
                    if let Some(started) = replies.submission() {
                        ticket = Some(started);
                    }
                    replies
                }
                Op::Fill => {
                    // The replies of each action are checked as one call's
                    // would be: only the last is kept.
                    let mut replies = app.tick(now);
                    for mut action in fill_actions() {
                        set_seq(&mut action, seq);
                        replies = app.apply(&action, now);
                        if epoch.is_none() && app.epoch().is_none() {
                            prop_assert_eq!(replies.refused(), Some(FlowError::NotAllowed(Phase::Welcome)));
                        }
                    }
                    replies
                }
                Op::Tick => app.tick(now),
                Op::Result { change, outcome } => {
                    let Some(live) = ticket else {
                        let any = SubmissionTicket { epoch: 1, session: 1, attempt: 1 };
                        let result = SubmissionResult { ticket: any, outcome: *outcome };
                        let refused = app.submission_result(&result, now).is_err();
                        prop_assert!(refused || app.epoch().is_some());
                        continue;
                    };
                    let mut sent = live;
                    match change {
                        Change::None => {}
                        Change::Epoch(e) => sent.epoch = *e,
                        Change::Session(s) => sent.session = *s,
                        Change::Attempt(a) => sent.attempt = *a,
                    }
                    let other = sent != live;
                    let replies = app.submission_result(&SubmissionResult { ticket: sent, outcome: *outcome }, now).unwrap();
                    if other {
                        // Another epoch, session or attempt: stale, and
                        // nothing changes - a revocation included. (A
                        // session that has run out of time first is
                        // refused as expired, and its end reported.)
                        let refused = replies.refused();
                        prop_assert!(
                            matches!(refused, Some(FlowError::StaleTicket | FlowError::Expired)),
                            "{:?}", refused
                        );
                        if refused == Some(FlowError::StaleTicket) {
                            // At most a countdown's next second, as any
                            // call reports it.
                            let only_countdown = replies
                                .reports()
                                .iter()
                                .all(|r| matches!(r, Report::Countdown(_)));
                            prop_assert!(only_countdown);
                        }
                        prop_assert_eq!(app.is_revoked(), revoked_before);
                    }
                    if *outcome == SubmissionOutcome::NotApproved && replies.refused().is_none() {
                        prop_assert!(app.is_revoked());
                        // Failed as unavailable, the session not cut short.
                        let ended = replies.session_ended().is_some();
                        prop_assert!(!ended);
                    }
                    replies
                }
                Op::Garbage(bytes) => match app.receive(bytes, now) {
                    Ok(replies) => replies,
                    Err(faulted) => faulted.replies,
                },
            };

            // The epoch, once given, never changes within the start.
            match (epoch, app.epoch()) {
                (Some(before), after) => prop_assert_eq!(after, Some(before)),
                (None, Some(given)) => {
                    prop_assert_ne!(given, 0);
                    epoch = Some(given);
                }
                (None, None) => {
                    // No session before the epoch.
                    prop_assert!(app.flow().is_none());
                    prop_assert!(app.out_of_service().is_some());
                    let only_out_of_service = replies
                        .reports()
                        .iter()
                        .all(|r| matches!(r, Report::Refused(_) | Report::OutOfService(_)));
                    prop_assert!(only_out_of_service);
                }
            }
            // Every ticket carries this start's epoch.
            if let Some(started) = replies.submission() {
                prop_assert_eq!(Some(started.epoch), epoch);
            }
            // Revoked, once no session is open the kiosk is out of service
            // for good in this start, and opens no session.
            if app.is_revoked() && app.flow().is_some_and(|flow| flow.phase() == Phase::Welcome) {
                prop_assert_eq!(app.out_of_service(), Some(OutOfServiceReason::NoService));
            }
            if revoked_before && app.out_of_service().is_some() {
                prop_assert!(!app.flow().unwrap().phase().holds_answers());
            }
        }
    }
}

fn set_seq(action: &mut Action, seq: u32) {
    match action {
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
}
