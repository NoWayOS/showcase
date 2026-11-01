// SPDX-License-Identifier: MIT OR Apache-2.0
//! Property tests: whatever a person, the service and the clock do, in any
//! order, the flow never panics and keeps its promises.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

mod common;

use common::*;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use showcase_kiosk_flow::{
    FailureReason, FieldId, FlowConfig, FlowError, Phase, SubmissionTicket, Tick, TickEvent,
    Timeouts, Value,
};

/// The form's fields, plus one it does not have.
const FIELDS: [FieldId; 7] = [
    NAME,
    MEMBER_NUMBER,
    TOPIC,
    DETAILS,
    CONSENT,
    RECEIPT,
    NO_SUCH_FIELD,
];

/// A maximum session length short enough for generated sequences to reach
/// often (it must stay above the idle and submit timeouts).
const LIMIT: Tick = 75_000;
const PROPERTY_CONFIG: FlowConfig = FlowConfig {
    timeouts: Timeouts {
        max_session: LIMIT,
        ..CONFIG.timeouts
    },
    ..CONFIG
};

/// However the calls are timed, no call leaves answers in place this long
/// after the person's last action.
const BOUND: Tick = SUBMIT + IDLE_RESET;

fn new_property_flow() -> Flow {
    Flow::new(&FORM, PROPERTY_CONFIG, EPOCH).unwrap()
}

#[derive(Debug, Clone)]
enum Op {
    Start,
    /// What a person who knows the form does: valid answers for every field
    /// on the current step. Elsewhere, or on a step with no fields, just a
    /// touch. Without it, random input would rarely get past the first step.
    Answer,
    SetText(usize, String),
    Select(usize, u8),
    SetChecked(usize, bool),
    Clear(usize),
    Next,
    Back,
    Submit,
    Retry,
    Cancel,
    Finish,
    Activity,
    /// A result for the latest ticket, or for a stale one.
    Succeed {
        stale: Stale,
    },
    Fail {
        stale: Stale,
        reason: FailureReason,
    },
    /// The caller's clock moves on (no call is made).
    Wait(Tick),
    /// The caller's clock goes back (no call is made): a clock fault.
    Rewind(Tick),
    /// `tick` is called.
    Clock,
}

/// Which ticket a result quotes.
#[derive(Debug, Clone, Copy)]
enum Stale {
    /// The latest one handed out.
    No,
    /// Another attempt of the same session.
    Attempt,
    /// Another session.
    Session,
    /// The same numbers from before the application restarted.
    Epoch,
}

impl Op {
    /// Whether the person did this (rather than the service or the clock).
    fn by_person(&self) -> bool {
        !matches!(
            self,
            Op::Succeed { .. } | Op::Fail { .. } | Op::Wait(_) | Op::Rewind(_) | Op::Clock
        )
    }
}

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => "[a-zA-Z ]{0,40}",
        2 => "[0-9]{0,12}",
        // Anything at all: control characters, any script, too long.
        2 => any::<String>(),
    ]
}

fn reason() -> impl Strategy<Value = FailureReason> {
    prop_oneof![
        Just(FailureReason::TimedOut),
        Just(FailureReason::Unavailable),
        Just(FailureReason::Rejected),
    ]
}

fn stale() -> impl Strategy<Value = Stale> {
    prop_oneof![
        6 => Just(Stale::No),
        1 => Just(Stale::Attempt),
        1 => Just(Stale::Session),
        1 => Just(Stale::Epoch),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let field = 0..FIELDS.len();
    prop_oneof![
        2 => Just(Op::Start),
        5 => Just(Op::Answer),
        4 => (field.clone(), text()).prop_map(|(f, s)| Op::SetText(f, s)),
        2 => (field.clone(), 0u8..6).prop_map(|(f, o)| Op::Select(f, o)),
        2 => (field.clone(), any::<bool>()).prop_map(|(f, b)| Op::SetChecked(f, b)),
        1 => field.prop_map(Op::Clear),
        5 => Just(Op::Next),
        1 => Just(Op::Back),
        3 => Just(Op::Submit),
        1 => Just(Op::Retry),
        1 => Just(Op::Cancel),
        1 => Just(Op::Finish),
        1 => Just(Op::Activity),
        2 => stale().prop_map(|stale| Op::Succeed { stale }),
        2 => (stale(), reason()).prop_map(|(stale, reason)| Op::Fail { stale, reason }),
        // Mostly a few seconds; sometimes long enough for any timeout.
        4 => (0..=5_000u64).prop_map(Op::Wait),
        1 => (0..=2 * IDLE_RESET).prop_map(Op::Wait),
        1 => (1..=IDLE_RESET).prop_map(Op::Rewind),
        3 => Just(Op::Clock),
    ]
}

/// What one call returned.
struct Outcome {
    result: Result<(), FlowError>,
    /// What `tick` reported, for [`Op::Clock`].
    event: Option<TickEvent>,
    /// The ticket a result quoted, for [`Op::Succeed`] and [`Op::Fail`].
    quoted: Option<SubmissionTicket>,
}

/// Makes the call `op` stands for, at `now`. Not for [`Op::Wait`] or
/// [`Op::Rewind`], which make no call.
fn apply(flow: &mut Flow, op: &Op, now: Tick, ticket: &mut Option<SubmissionTicket>) -> Outcome {
    let mut event = None;
    let mut quoted = None;
    let result = match op {
        Op::Start => flow.start(now),
        Op::Answer => match flow.phase() {
            Phase::Step { index } if !STEPS[index].fields.is_empty() => STEPS[index]
                .fields
                .iter()
                .try_for_each(|field| match field.id {
                    NAME => flow.set_text(NAME, "Sam Sample", now),
                    MEMBER_NUMBER => flow.set_text(MEMBER_NUMBER, "123456", now),
                    TOPIC => flow.select(TOPIC, 1, now),
                    DETAILS => flow.set_text(DETAILS, "Replacement card", now),
                    other => flow.set_checked(other, true, now),
                }),
            _ => flow.note_activity(now),
        },
        Op::SetText(f, text) => flow.set_text(FIELDS[*f], text, now),
        Op::Select(f, option) => flow.select(FIELDS[*f], *option, now),
        Op::SetChecked(f, checked) => flow.set_checked(FIELDS[*f], *checked, now),
        Op::Clear(f) => flow.clear_field(FIELDS[*f], now),
        Op::Next => flow.next(now),
        Op::Back => flow.back(now),
        Op::Submit | Op::Retry => {
            let sent = if matches!(op, Op::Submit) {
                flow.submit(now)
            } else {
                flow.retry(now)
            };
            sent.map(|sent| {
                *ticket = Some(sent);
            })
        }
        Op::Cancel => flow.cancel(now),
        Op::Finish => flow.finish(now),
        Op::Activity => flow.note_activity(now),
        Op::Succeed { stale } | Op::Fail { stale, .. } => {
            let latest = ticket.unwrap_or(SubmissionTicket {
                epoch: EPOCH,
                session: flow.session(),
                attempt: 1,
            });
            let quote = match stale {
                Stale::No => latest,
                Stale::Attempt => SubmissionTicket {
                    attempt: latest.attempt.wrapping_add(1),
                    ..latest
                },
                Stale::Session => SubmissionTicket {
                    session: latest.session.wrapping_add(1),
                    ..latest
                },
                Stale::Epoch => SubmissionTicket {
                    epoch: latest.epoch.wrapping_add(1),
                    ..latest
                },
            };
            quoted = Some(quote);
            match op {
                Op::Fail { reason, .. } => flow.submission_failed(quote, *reason, now),
                _ => flow.submission_succeeded(quote, now),
            }
        }
        Op::Clock => {
            event = flow.tick(now);
            Ok(())
        }
        Op::Wait(_) | Op::Rewind(_) => panic!("{op:?} makes no call"),
    };
    Outcome {
        result,
        event,
        quoted,
    }
}

/// An owned copy of a value, to compare before and after a call.
#[derive(Debug, Clone, PartialEq)]
enum Owned {
    Text(String),
    Choice(Option<u8>),
    Checkbox(bool),
}

fn owned(value: Option<Value<'_>>) -> Option<Owned> {
    value.map(|value| match value {
        Value::Text(text) => Owned::Text(text.to_owned()),
        Value::Choice(choice) => Owned::Choice(choice),
        Value::Checkbox(checked) => Owned::Checkbox(checked),
    })
}

fn snapshot(flow: &Flow) -> Vec<(FieldId, Option<Owned>)> {
    FIELDS
        .iter()
        .map(|&field| (field, owned(flow.value(field))))
        .collect()
}

/// When answers held at `now` would break a promise: the person's last
/// action was too long ago, or the session has reached its maximum length.
fn check_bound(
    flow: &Flow,
    now: Tick,
    last_person: Tick,
    session_start: Tick,
) -> Result<(), TestCaseError> {
    if flow.has_entered_data() {
        prop_assert!(
            now < last_person.saturating_add(BOUND),
            "answers held at {} ({:?}), last person action at {}",
            now,
            flow.phase(),
            last_person
        );
        prop_assert!(
            now < session_start.saturating_add(LIMIT),
            "answers held at {} ({:?}), session started at {}",
            now,
            flow.phase(),
            session_start
        );
    }
    Ok(())
}

/// The promises that hold after every call.
fn check_invariants(flow: &Flow) -> Result<(), TestCaseError> {
    let phase = flow.phase();
    // No answers outside a session, nor once they have been sent.
    if matches!(phase, Phase::Welcome | Phase::Submitted) {
        prop_assert!(!flow.has_entered_data(), "answers held at {phase:?}");
    }
    if let Phase::Step { index } = phase {
        prop_assert!(index < STEPS.len());
    }
    prop_assert!(flow.attempts() <= MAX_ATTEMPTS);
    if let Phase::Submitting { attempt } | Phase::Failed { attempt, .. } = phase {
        prop_assert!(attempt >= 1);
        prop_assert_eq!(attempt, flow.attempts());
    }
    // Stored answers always obey their rules.
    for (field, value) in flow.values() {
        match (field, value) {
            (NAME, Value::Text(text)) => prop_assert_eq!(NAME_RULE.entry_issue(text), None),
            (MEMBER_NUMBER, Value::Text(text)) => {
                prop_assert_eq!(MEMBER_NUMBER_RULE.entry_issue(text), None);
            }
            (DETAILS, Value::Text(text)) => {
                prop_assert_eq!(DETAILS_RULE.entry_issue(text), None);
            }
            (TOPIC, Value::Choice(choice)) => prop_assert!(choice.is_none_or(|c| c < 4)),
            (CONSENT | RECEIPT, Value::Checkbox(_)) => {}
            other => prop_assert!(false, "unexpected field or kind {other:?}"),
        }
    }
    // Nothing reaches the review, or the service, incomplete.
    if matches!(
        phase,
        Phase::Review | Phase::Submitting { .. } | Phase::Failed { .. }
    ) {
        let name = flow.value(NAME).and_then(|v| v.as_text()).unwrap_or("");
        prop_assert!(
            !name.trim().is_empty() && name.len() >= 2,
            "name incomplete"
        );
        let member = flow
            .value(MEMBER_NUMBER)
            .and_then(|v| v.as_text())
            .unwrap_or("");
        prop_assert!(
            member.trim().is_empty() || member.len() >= 6,
            "number too short"
        );
        prop_assert!(flow.value(TOPIC).and_then(|v| v.as_choice()).is_some());
        prop_assert!(flow.value(CONSENT).is_some_and(|v| v.is_checked()));
    }
    Ok(())
}

/// Starts a case: half of them from a completed review, so that submission,
/// its timeouts and its retries are exercised as much as the steps. Returns
/// the flow, the time, and the times of the last person action and of the
/// session's start.
fn begin_case(from_review: bool) -> (Flow, Tick, Tick, Tick) {
    let mut flow = new_property_flow();
    let start: Tick = 1_000_000;
    if from_review {
        let last = fill_all(&mut flow, start);
        (flow, last, last, start)
    } else {
        (flow, start, start, start)
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn the_flow_keeps_its_promises_whatever_happens(
        from_review in any::<bool>(),
        ops in prop::collection::vec(op(), 1..120),
    ) {
        let (mut flow, mut now, mut last_person, mut session_start) = begin_case(from_review);
        // The latest reading the flow has seen.
        let mut seen = if from_review { now } else { 0 };
        let mut ticket: Option<SubmissionTicket> = None;

        for op in ops {
            match op {
                Op::Wait(ticks) => {
                    now = now.saturating_add(ticks);
                    continue;
                }
                Op::Rewind(ticks) => {
                    now = now.saturating_sub(ticks);
                    continue;
                }
                _ => {}
            }
            let phase = flow.phase();
            let session = flow.session();
            let values = snapshot(&flow);
            let clock_fault = now < seen;
            let Outcome { result, event, quoted } = apply(&mut flow, &op, now, &mut ticket);
            seen = now;

            // A reading earlier than one already seen wipes any session.
            if clock_fault && phase != Phase::Welcome {
                match op {
                    Op::Clock => prop_assert_eq!(event, Some(TickEvent::IdleReset)),
                    // Cancel ends the session either way, so it succeeds.
                    Op::Cancel => prop_assert_eq!(result, Ok(())),
                    _ => prop_assert_eq!(result, Err(FlowError::Expired)),
                }
                prop_assert_eq!(flow.phase(), Phase::Welcome);
            }
            // A call that finds the session over wipes it and does nothing else.
            if result == Err(FlowError::Expired) {
                prop_assert_ne!(phase, Phase::Welcome);
                prop_assert_eq!(flow.phase(), Phase::Welcome);
            }
            if let Some(event) = event {
                prop_assert_eq!(event.ends_session(), flow.phase() == Phase::Welcome);
                if event == TickEvent::SubmitTimedOut {
                    prop_assert!(
                        matches!(
                            flow.phase(),
                            Phase::Failed { reason: FailureReason::TimedOut, .. }
                        ),
                        "phase after SubmitTimedOut: {:?}",
                        flow.phase()
                    );
                }
            }
            if matches!(op, Op::Start) && result.is_ok() {
                session_start = now;
                prop_assert_eq!(flow.session(), session.wrapping_add(1));
                prop_assert_eq!(flow.phase(), Phase::Step { index: 0 });
            }
            if op.by_person() && result != Err(FlowError::Expired) {
                last_person = now;
                // Any action by the person restarts the idle timer, and
                // withdraws the warning.
                if flow.phase().awaits_person() {
                    let idle = now.saturating_add(IDLE_RESET);
                    let limit = session_start.saturating_add(LIMIT);
                    prop_assert_eq!(flow.idle_deadline(), Some(idle.min(limit)));
                    prop_assert_eq!(flow.idle_warning(), None);
                }
            }
            // A result is accepted exactly when it is for the submission in
            // flight, unless the session had already ended.
            if let Some(quoted) = quoted {
                let in_flight = matches!(phase, Phase::Submitting { attempt }
                    if quoted == SubmissionTicket { epoch: EPOCH, session, attempt });
                match result {
                    Err(FlowError::Expired) => {}
                    Ok(()) => {
                        prop_assert!(in_flight);
                        if let Op::Fail { reason, .. } = op {
                            let attempt = quoted.attempt;
                            prop_assert_eq!(flow.phase(), Phase::Failed { reason, attempt });
                        } else {
                            prop_assert_eq!(flow.phase(), Phase::Submitted);
                        }
                    }
                    Err(FlowError::StaleTicket) => {
                        prop_assert!(!in_flight);
                        prop_assert_eq!(flow.phase(), phase);
                        prop_assert_eq!(snapshot(&flow), values.clone());
                    }
                    Err(other) => prop_assert!(false, "unexpected {other:?}"),
                }
            }
            // A refused edit leaves every answer as it was.
            if matches!(op, Op::SetText(..) | Op::Select(..) | Op::SetChecked(..) | Op::Clear(..)) {
                match result {
                    Ok(()) => {
                        if let Op::SetText(f, text) = &op {
                            prop_assert_eq!(
                                owned(flow.value(FIELDS[*f])),
                                Some(Owned::Text(text.clone()))
                            );
                        }
                    }
                    Err(FlowError::Expired) => {}
                    Err(_) => prop_assert_eq!(snapshot(&flow), values.clone()),
                }
            }
            check_bound(&flow, now, last_person, session_start)?;
            check_invariants(&flow)?;
        }
    }

    /// Calls may come late, or not at all (the clock only moves forward
    /// here): no call reaches answers past the privacy bound.
    #[test]
    fn late_calls_never_reach_answers_past_the_bound(
        from_review in any::<bool>(),
        ops in prop::collection::vec(op(), 1..120),
    ) {
        let (mut flow, mut now, mut last_person, mut session_start) = begin_case(from_review);
        let mut ticket: Option<SubmissionTicket> = None;
        for op in ops {
            match op {
                Op::Wait(ticks) => {
                    now = now.saturating_add(ticks);
                    continue;
                }
                Op::Rewind(_) => continue,
                _ => {}
            }
            let Outcome { result, .. } = apply(&mut flow, &op, now, &mut ticket);
            if matches!(op, Op::Start) && result.is_ok() {
                session_start = now;
            }
            if op.by_person() && result != Err(FlowError::Expired) {
                last_person = now;
            }
            check_bound(&flow, now, last_person, session_start)?;
        }
    }

    /// An event-driven caller that ticks at every `next_deadline`: answers
    /// are wiped on time, so they are never held past the bound at any
    /// moment, between calls included.
    #[test]
    fn on_time_ticks_wipe_answers_on_time(
        from_review in any::<bool>(),
        ops in prop::collection::vec(op(), 1..120),
    ) {
        let (mut flow, mut now, mut last_person, mut session_start) = begin_case(from_review);
        let mut ticket: Option<SubmissionTicket> = None;
        for op in ops {
            match op {
                Op::Wait(ticks) => {
                    let until = now.saturating_add(ticks);
                    let mut timers = 0;
                    while let Some(deadline) = flow.next_deadline() {
                        if deadline > until {
                            break;
                        }
                        timers += 1;
                        prop_assert!(timers <= 8, "next_deadline does not move on: {}", deadline);
                        now = deadline.max(now);
                        let _ = flow.tick(now);
                        check_bound(&flow, now, last_person, session_start)?;
                    }
                    now = until;
                    check_bound(&flow, now, last_person, session_start)?;
                    continue;
                }
                // Monotonic time; ticks are the caller's timers, above.
                Op::Rewind(_) | Op::Clock => continue,
                _ => {}
            }
            let Outcome { result, .. } = apply(&mut flow, &op, now, &mut ticket);
            if matches!(op, Op::Start) && result.is_ok() {
                session_start = now;
            }
            if op.by_person() && result != Err(FlowError::Expired) {
                last_person = now;
            }
            check_bound(&flow, now, last_person, session_start)?;
        }
    }

    #[test]
    fn any_text_is_either_stored_exactly_or_refused_without_change(
        first in "[a-z]{1,8}",
        attempt in any::<String>(),
    ) {
        let mut flow = new_flow();
        flow.start(0).unwrap();
        flow.set_text(NAME, &first, 1).unwrap();
        let before = owned(flow.value(NAME));
        match flow.set_text(NAME, &attempt, 2) {
            Ok(()) => prop_assert_eq!(flow.value(NAME), Some(Value::Text(attempt.as_str()))),
            Err(FlowError::InvalidValue(problem)) => {
                prop_assert_eq!(problem.field, NAME);
                prop_assert_eq!(owned(flow.value(NAME)), before);
            }
            Err(other) => prop_assert!(false, "unexpected {other:?}"),
        }
    }
}
