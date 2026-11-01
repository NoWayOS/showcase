// SPDX-License-Identifier: MIT OR Apache-2.0
//! The synthetic form the integration tests share: a generic four-step
//! service request, invented for these tests.
//!
//! 1. About you: a name (required) and an optional membership number.
//! 2. Your request: a topic (one of four, required) and optional details.
//! 3. A notice to read, with no fields.
//! 4. Confirm: consent to process the request (required) and an optional
//!    printed receipt.

// Each test file uses a different part of this module.
#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "shared by several test binaries, each of which uses only part of it; test code: a failed check panics"
)]

use showcase_kiosk_flow::{
    Charset, Field, FieldId, FlowConfig, Form, KioskFlow, Phase, Step, StepId, TextRule, Tick,
    Timeouts,
};

pub const NAME: FieldId = FieldId(1);
pub const MEMBER_NUMBER: FieldId = FieldId(2);
pub const TOPIC: FieldId = FieldId(3);
pub const DETAILS: FieldId = FieldId(4);
pub const CONSENT: FieldId = FieldId(5);
pub const RECEIPT: FieldId = FieldId(6);
/// Not in the form.
pub const NO_SUCH_FIELD: FieldId = FieldId(99);

pub const NAME_RULE: TextRule = TextRule::new(Charset::Text, 2, 32);
pub const MEMBER_NUMBER_RULE: TextRule = TextRule::new(Charset::Digits, 6, 10);
pub const DETAILS_RULE: TextRule = TextRule::new(Charset::Text, 0, 48);

pub const ABOUT_YOU: &[Field] = &[
    Field::text(NAME, NAME_RULE).required(),
    Field::text(MEMBER_NUMBER, MEMBER_NUMBER_RULE),
];
pub const REQUEST: &[Field] = &[
    Field::choice(TOPIC, 4).required(),
    Field::text(DETAILS, DETAILS_RULE),
];
pub const CONFIRM: &[Field] = &[
    Field::checkbox(CONSENT).required(),
    Field::checkbox(RECEIPT),
];
pub const STEPS: &[Step<'static>] = &[
    Step::new(StepId(10), ABOUT_YOU),
    Step::new(StepId(20), REQUEST),
    Step::new(StepId(30), &[]),
    Step::new(StepId(40), CONFIRM),
];
pub static FORM: Form<'static> = Form::new(STEPS);

/// Ticks are milliseconds in these tests.
pub const IDLE_RESET: Tick = 60_000;
pub const IDLE_WARNING: Tick = 15_000;
pub const SUBMIT: Tick = 10_000;
pub const CONFIRMATION: Tick = 8_000;
pub const MAX_ATTEMPTS: u8 = 3;
/// Twenty minutes, however active the person is.
pub const MAX_SESSION: Tick = 20 * 60_000;
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
/// The flow instance's epoch, as a boot counter or a random number would give it.
pub const EPOCH: u32 = 1;

/// Room for 8 fields of up to 48 bytes each.
pub type Flow = KioskFlow<'static, 8, 48>;

pub fn new_flow() -> Flow {
    Flow::new(&FORM, CONFIG, EPOCH).unwrap()
}

/// The same flow after the application restarted: a new epoch.
pub fn restarted_flow() -> Flow {
    Flow::new(&FORM, CONFIG, EPOCH + 1).unwrap()
}

/// Starts a session at `t` and answers every field, one tick per action,
/// ending at Review. Returns the tick of the last action.
pub fn fill_all(flow: &mut Flow, t: Tick) -> Tick {
    flow.start(t).unwrap();
    flow.set_text(NAME, "Sam Sample", t + 1).unwrap();
    flow.set_text(MEMBER_NUMBER, "12345678", t + 2).unwrap();
    flow.next(t + 3).unwrap();
    flow.select(TOPIC, 2, t + 4).unwrap();
    flow.set_text(DETAILS, "Replacement card, please", t + 5)
        .unwrap();
    flow.next(t + 6).unwrap();
    flow.next(t + 7).unwrap(); // the notice
    flow.set_checked(CONSENT, true, t + 8).unwrap();
    flow.set_checked(RECEIPT, true, t + 9).unwrap();
    flow.next(t + 10).unwrap();
    assert_eq!(flow.phase(), Phase::Review);
    assert!(flow.values().all(|(_, value)| !value.is_empty()));
    t + 10
}

/// Asserts that the flow holds no answer at all.
pub fn assert_wiped(flow: &Flow) {
    assert!(!flow.has_entered_data(), "answers survived: {flow:?}");
    assert_eq!(flow.values().count(), 6, "every field still has storage");
    for (field, value) in flow.values() {
        assert!(value.is_empty(), "{field:?} still holds {value:?}");
    }
}
