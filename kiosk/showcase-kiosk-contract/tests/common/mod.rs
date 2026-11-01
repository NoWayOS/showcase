// SPDX-License-Identifier: MIT OR Apache-2.0
//! The synthetic form the contract's tests share: a generic three-step
//! request, invented for these tests.
//!
//! 1. A name (required) and an optional reference code (digits).
//! 2. A topic (one of three, required) and an optional note.
//! 3. Agreement (required) and an optional copy by post.

// Each test file uses a different part of this module.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "shared by several test binaries, each of which uses only part of it; test code: a failed check panics"
)]

use proptest::prelude::*;
use showcase_kiosk_contract::{
    Abandon, Action, AppEnd, Back, Cancel, ClearField, Countdown, EndReason, Finish, IdleWarning,
    Incomplete, Issued, MAX_RECORD_LEN, Next, OutOfService, OutOfServiceReason, Refusal, Refused,
    Replies, Report, Retry, Select, SessionEnded, SetChecked, SetText, ShowFailed, ShowReview,
    ShowStep, ShowSubmitted, ShowSubmitting, ShowWelcome, SimulateFailure, Start, StillHere,
    Submit, ValueRefused,
};
use showcase_kiosk_flow::{
    Charset, FailureReason, Field, FieldId, FlowConfig, Form, Issue, KioskFlow, Phase, Step,
    StepId, TextRule, Tick, Timeouts,
};

pub const NAME: FieldId = FieldId(1);
pub const CODE: FieldId = FieldId(2);
pub const TOPIC: FieldId = FieldId(3);
pub const NOTE: FieldId = FieldId(4);
pub const AGREE: FieldId = FieldId(5);
pub const BY_POST: FieldId = FieldId(6);
/// Every field of the form, in order.
pub const FIELDS: [FieldId; 6] = [NAME, CODE, TOPIC, NOTE, AGREE, BY_POST];

pub const STEPS: &[Step<'static>] = &[
    Step::new(
        StepId(1),
        &[
            Field::text(NAME, TextRule::new(Charset::Text, 2, 32)).required(),
            Field::text(CODE, TextRule::new(Charset::Digits, 4, 8)),
        ],
    ),
    Step::new(
        StepId(2),
        &[
            Field::choice(TOPIC, 3).required(),
            Field::text(NOTE, TextRule::new(Charset::Text, 0, 64)),
        ],
    ),
    Step::new(
        StepId(3),
        &[Field::checkbox(AGREE).required(), Field::checkbox(BY_POST)],
    ),
];
pub static FORM: Form<'static> = Form::new(STEPS);

/// Ticks are milliseconds in these tests.
pub const IDLE_RESET: Tick = 60_000;
pub const IDLE_WARNING: Tick = 15_000;
pub const SUBMIT: Tick = 10_000;
pub const CONFIRMATION: Tick = 8_000;
pub const MAX_SESSION: Tick = 20 * 60_000;
pub const MAX_ATTEMPTS: u8 = 3;
pub const CONFIG: FlowConfig = FlowConfig {
    timeouts: Timeouts {
        idle_reset: IDLE_RESET,
        idle_warning: IDLE_WARNING,
        submit: SUBMIT,
        confirmation: CONFIRMATION,
        max_session: MAX_SESSION,
    },
    max_attempts: MAX_ATTEMPTS,
};

/// Room for 8 fields of up to 64 bytes each.
pub type App = AppEnd<'static, 8, 64>;

pub fn new_app() -> App {
    let flow: KioskFlow<'static, 8, 64> = KioskFlow::new(&FORM, CONFIG, 7).unwrap();
    AppEnd::new(flow).unwrap()
}

/// The action as a ring slot would hold it.
pub fn record(action: &Action) -> [u8; MAX_RECORD_LEN] {
    let mut slot = [0u8; MAX_RECORD_LEN];
    action.encode(&mut slot).unwrap();
    slot
}

/// Sends an action through the wire, as the UI would.
pub fn send(app: &mut App, action: &Action, now: Tick) -> Replies {
    app.receive(&record(action), now).unwrap()
}

pub fn text(seq: u32, field: FieldId, value: &str) -> Action {
    Action::SetText(SetText::new(seq, field, value).unwrap())
}

/// Starts a session at `t` and answers every field, one tick per action,
/// ending at the review. Returns the tick of the last action.
pub fn fill_all(app: &mut App, t: Tick) -> Tick {
    let _ = send(app, &Action::Start(Start { seq: 1 }), t);
    let _ = send(app, &text(2, NAME, "Alex Example"), t + 1);
    let _ = send(app, &text(3, CODE, "4321"), t + 2);
    let _ = send(app, &Action::Next(Next { seq: 4 }), t + 3);
    let pick = Select {
        seq: 5,
        field: TOPIC,
        option: 1,
    };
    let _ = send(app, &Action::Select(pick), t + 4);
    let _ = send(app, &text(6, NOTE, "Please call back"), t + 5);
    let _ = send(app, &Action::Next(Next { seq: 7 }), t + 6);
    let tick = SetChecked {
        seq: 8,
        field: AGREE,
        checked: true,
    };
    let _ = send(app, &Action::SetChecked(tick), t + 7);
    let _ = send(app, &Action::Next(Next { seq: 9 }), t + 8);
    assert_eq!(app.flow().phase(), Phase::Review);
    assert!(app.flow().has_entered_data());
    t + 8
}

/// The session-ended reports among `replies`.
pub fn ended(replies: &Replies) -> Vec<Report> {
    replies
        .reports()
        .iter()
        .copied()
        .filter(|report| matches!(report, Report::SessionEnded(_)))
        .collect()
}

pub fn issue() -> impl Strategy<Value = Issue> {
    prop_oneof![
        Just(Issue::Missing),
        Just(Issue::TooShort),
        Just(Issue::TooLong),
        Just(Issue::BadCharacter),
        Just(Issue::NoSuchOption),
    ]
}

pub fn failure() -> impl Strategy<Value = FailureReason> {
    prop_oneof![
        Just(FailureReason::TimedOut),
        Just(FailureReason::Unavailable),
        Just(FailureReason::Rejected),
    ]
}

pub fn refusal() -> impl Strategy<Value = Refusal> {
    prop_oneof![
        Just(Refusal::NotAllowed),
        Just(Refusal::UnknownField),
        Just(Refusal::NotOnThisStep),
        Just(Refusal::WrongKind),
        Just(Refusal::AtFirstStep),
        Just(Refusal::NoAttemptsLeft),
        Just(Refusal::NotRetryable),
        Just(Refusal::Expired),
    ]
}

pub fn end_reason() -> impl Strategy<Value = EndReason> {
    prop_oneof![
        Just(EndReason::Cancelled),
        Just(EndReason::Idle),
        Just(EndReason::TimeLimit),
        Just(EndReason::Submitted),
        Just(EndReason::Fault),
        Just(EndReason::Abandoned),
    ]
}

/// Any action, with any values a record can hold.
pub fn any_action() -> impl Strategy<Value = Action> {
    let field = any::<u16>().prop_map(FieldId);
    prop_oneof![
        any::<u32>().prop_map(|seq| Action::Start(Start { seq })),
        (any::<u32>(), field.clone(), "\\PC{0,16}").prop_filter_map(
            "fits a record",
            |(seq, field, text)| SetText::new(seq, field, &text).ok().map(Action::SetText)
        ),
        (any::<u32>(), field.clone(), any::<u8>())
            .prop_map(|(seq, field, option)| Action::Select(Select { seq, field, option })),
        (any::<u32>(), field.clone(), any::<bool>()).prop_map(|(seq, field, checked)| {
            Action::SetChecked(SetChecked {
                seq,
                field,
                checked,
            })
        }),
        (any::<u32>(), field)
            .prop_map(|(seq, field)| Action::ClearField(ClearField { seq, field })),
        any::<u32>().prop_map(|seq| Action::Next(Next { seq })),
        any::<u32>().prop_map(|seq| Action::Back(Back { seq })),
        any::<u32>().prop_map(|seq| Action::Submit(Submit { seq })),
        any::<u32>().prop_map(|seq| Action::Retry(Retry { seq })),
        any::<u32>().prop_map(|seq| Action::Cancel(Cancel { seq })),
        any::<u32>().prop_map(|seq| Action::Finish(Finish { seq })),
        any::<u32>().prop_map(|seq| Action::StillHere(StillHere { seq })),
        any::<u32>().prop_map(|seq| Action::Abandon(Abandon { seq })),
        any::<u32>().prop_map(|seq| Action::SimulateFailure(SimulateFailure { seq })),
    ]
}

/// Any report, with any values a record can hold.
pub fn any_report() -> impl Strategy<Value = Report> {
    let field = any::<u16>().prop_map(FieldId);
    prop_oneof![
        Just(Report::ShowWelcome(ShowWelcome)),
        any::<u16>().prop_map(|index| Report::ShowStep(ShowStep { index })),
        Just(Report::ShowReview(ShowReview)),
        (1..=u8::MAX).prop_map(|attempt| Report::ShowSubmitting(ShowSubmitting { attempt })),
        Just(Report::ShowSubmitted(ShowSubmitted)),
        (failure(), 1..=u8::MAX, any::<u8>()).prop_map(|(reason, attempt, attempts_left)| {
            Report::ShowFailed(ShowFailed {
                reason,
                attempt,
                attempts_left,
            })
        }),
        any::<u32>().prop_map(|remaining| Report::IdleWarning(IdleWarning { remaining })),
        (any::<u32>(), field.clone(), issue()).prop_map(|(seq, field, issue)| {
            Report::ValueRefused(ValueRefused { seq, field, issue })
        }),
        (any::<u32>(), field, issue())
            .prop_map(|(seq, field, issue)| Report::Incomplete(Incomplete { seq, field, issue })),
        (any::<u32>(), refusal())
            .prop_map(|(seq, refusal)| Report::Refused(Refused { seq, refusal })),
        (any::<u32>(), end_reason())
            .prop_map(|(session, reason)| Report::SessionEnded(SessionEnded { session, reason })),
        any::<u32>().prop_map(|remaining| Report::Countdown(Countdown { remaining })),
        "[A-Z0-9]{1,8}".prop_map(|code| Report::Issued(Issued::new(&code).unwrap())),
        out_of_service_reason().prop_map(|reason| Report::OutOfService(OutOfService { reason })),
    ]
}

/// Any reason to be out of service, with any approval code.
pub fn out_of_service_reason() -> impl Strategy<Value = OutOfServiceReason> {
    prop_oneof![
        any::<[u8; 8]>().prop_map(|code| OutOfServiceReason::AwaitingApproval { code }),
        Just(OutOfServiceReason::NoService),
        Just(OutOfServiceReason::Updating),
    ]
}
