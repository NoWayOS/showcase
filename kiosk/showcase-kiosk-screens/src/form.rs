// SPDX-License-Identifier: MIT OR Apache-2.0
//! The reference form: the steps and fields the reference screens
//! ask for, as the kiosk flow (`showcase-kiosk-flow`) holds them, and the
//! timeouts the reference kiosk runs with. The application PD runs the flow
//! on this form; the UI ([`crate::kiosk`]) shows each step on its screen and
//! sends each answer under these field ids.
//!
//! | Step | Screen | Fields |
//! | --- | --- | --- |
//! | 0 | `step-text` | [`NAME`] (required text), [`REFERENCE`] (optional digits) |
//! | 1 | `step-choice` | [`TOPIC`] (required choice of three), [`DETAILS`] (optional text) |
//! | 2 | `step-checkbox` | [`AGREE`] (required checkbox), [`RECEIPT`] (optional checkbox) |
//! | 3 | `step-notice` | none: a notice |
//!
//! Each text field's longest value is its screen's: the screen's input
//! refuses more as it is typed, and the flow refuses more again.

use showcase_kiosk_flow::{
    Charset, Field, FieldId, FlowConfig, Form, Step, StepId, TextRule, Timeouts,
};

/// The name: required, one line of text, at most 32 bytes.
pub const NAME: FieldId = FieldId(1);
/// A reference number: optional, digits only, at most 10.
pub const REFERENCE: FieldId = FieldId(2);
/// What the question is about: required, one of [`TOPICS`].
pub const TOPIC: FieldId = FieldId(3);
/// Details: optional, one line of text, at most 48 bytes.
pub const DETAILS: FieldId = FieldId(4);
/// The agreement to send the answers: required, ticked.
pub const AGREE: FieldId = FieldId(5);
/// Whether to print a receipt: optional.
pub const RECEIPT: FieldId = FieldId(6);

/// The topics [`TOPIC`] offers, as `step-choice`'s radio group shows them.
pub const TOPICS: [&str; 3] = ["Opening hours", "An order", "Something else"];

/// The most fields the form has: the flow's room for answers.
pub const FIELDS: usize = 6;

/// The longest text value of any field, in bytes: the flow's room for each.
pub const VALUE_LEN: usize = 48;

/// The form's steps, in order.
pub static FORM: Form<'static> = Form::new(&[
    Step::new(
        StepId(1),
        &[
            Field::text(NAME, TextRule::new(Charset::Text, 1, 32)).required(),
            Field::text(REFERENCE, TextRule::new(Charset::Digits, 1, 10)),
        ],
    ),
    Step::new(
        StepId(2),
        &[
            Field::choice(TOPIC, 3).required(),
            Field::text(DETAILS, TextRule::new(Charset::Text, 1, 48)),
        ],
    ),
    Step::new(
        StepId(3),
        &[Field::checkbox(AGREE).required(), Field::checkbox(RECEIPT)],
    ),
    Step::new(StepId(4), &[]),
]);

/// How long the reference kiosk waits, in milliseconds (the flow's ticks);
/// shorter in QEMU's test images ([`Timeouts::or_test`]).
///
/// A demonstration's timeouts, short enough that the idle warning and the
/// privacy reset can be watched, and tested in QEMU within its 60 s: the
/// warning after 15 s with no input, the reset 15 s later; a submission
/// times out after 5 s; the confirmation shows for 10 s; a session lasts
/// at most 20 minutes. Three attempts to send.
pub const CONFIG: FlowConfig = FlowConfig {
    timeouts: Timeouts {
        idle_reset: 30_000,
        idle_warning: 15_000,
        submit: 5_000,
        confirmation: 10_000,
        max_session: 20 * 60_000,
    }
    .or_test(),
    max_attempts: 3,
};
