// SPDX-License-Identifier: MIT OR Apache-2.0
//! Errors: forms or settings that cannot run, and actions the flow refuses.
//!
//! No error carries an entered value, only field ids and reason codes, so
//! reporting or logging an error cannot leak what a person typed.

use core::fmt;

use crate::flow::Phase;
use crate::form::{FieldId, StepId};

/// What is wrong with a field's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// A required field is empty or only spaces, a required choice has no
    /// option chosen, or a required checkbox is not checked.
    Missing,
    /// Shorter than the field's minimum length.
    TooShort,
    /// Longer than the field's maximum length.
    TooLong,
    /// Contains a character the field does not accept.
    BadCharacter,
    /// Not one of the field's options.
    NoSuchOption,
}

/// A field and what is wrong with it. Never includes the value itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldProblem {
    /// The field concerned.
    pub field: FieldId,
    /// What is wrong.
    pub issue: Issue,
}

/// An action that [`KioskFlow`](crate::KioskFlow) refused.
///
/// A refused action changes nothing, with two exceptions. An action taken by
/// the person still counts as activity, so it restarts the idle timer. And
/// [`FlowError::Expired`] reports that the session was wiped just before the
/// action could apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowError {
    /// The action does not apply in the current phase, given here.
    NotAllowed(Phase),
    /// The session had ended before this call arrived: nobody had touched
    /// the kiosk for the whole idle timeout, the session had reached its
    /// maximum length ([`Timeouts::max_session`](crate::Timeouts::max_session)),
    /// or the clock had gone backwards (see [`Tick`](crate::Tick)). The
    /// session has now been wiped, every entered value cleared, and the flow
    /// is back at [`Phase::Welcome`]. The action or result was not applied:
    /// it was meant for a session that no longer exists. Copies of the
    /// answers held elsewhere must be cleared now too.
    Expired,
    /// No field in the form has this id.
    UnknownField(FieldId),
    /// The field is on a different step from the one on screen.
    NotOnThisStep(FieldId),
    /// The action does not suit the field's kind, for example text for a
    /// checkbox.
    WrongKind(FieldId),
    /// The value was refused. The field keeps its previous value.
    InvalidValue(FieldProblem),
    /// The step, or at submission the whole form, is not complete. This is
    /// the first field that needs attention;
    /// [`KioskFlow::problems`](crate::KioskFlow::problems) lists every one on
    /// the current step.
    Incomplete(FieldProblem),
    /// Back was pressed on the first step. There is no earlier step; use
    /// [`KioskFlow::cancel`](crate::KioskFlow::cancel) to leave the form.
    AtFirstStep,
    /// Every submission attempt this session allows has been used.
    NoAttemptsLeft,
    /// The service rejected the submission, so sending the same answers again
    /// would not help. The person can go back and change them, or cancel.
    /// [`KioskFlow::retry`](crate::KioskFlow::retry) refuses after a
    /// rejection, and [`KioskFlow::submit`](crate::KioskFlow::submit) until
    /// an answer has changed.
    NotRetryable,
    /// The ticket does not match the submission in flight: the result is
    /// late, duplicated, or for another attempt or an earlier session. It
    /// has been ignored.
    StaleTicket,
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAllowed(phase) => write!(f, "not allowed in phase {phase:?}"),
            Self::Expired => f.write_str("the session had expired and has been wiped"),
            Self::UnknownField(field) => write!(f, "no field {}", field.0),
            Self::NotOnThisStep(field) => write!(f, "field {} is not on the current step", field.0),
            Self::WrongKind(field) => write!(f, "wrong kind of value for field {}", field.0),
            Self::InvalidValue(problem) => {
                write!(
                    f,
                    "value refused for field {}: {:?}",
                    problem.field.0, problem.issue
                )
            }
            Self::Incomplete(problem) => {
                write!(
                    f,
                    "field {} needs attention: {:?}",
                    problem.field.0, problem.issue
                )
            }
            Self::AtFirstStep => f.write_str("already on the first step"),
            Self::NoAttemptsLeft => f.write_str("no submission attempts left"),
            Self::NotRetryable => {
                f.write_str("the submission was rejected; retrying would not help")
            }
            Self::StaleTicket => {
                f.write_str("result ignored: it is not for the submission in flight")
            }
        }
    }
}

impl core::error::Error for FlowError {}

/// Why a [`Form`](crate::Form) or [`FlowConfig`](crate::FlowConfig) cannot
/// be run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    /// The form has no steps.
    NoSteps,
    /// The form has more fields than the flow has room for.
    TooManyFields {
        /// How many fields the flow can hold.
        capacity: usize,
    },
    /// Two steps share this id.
    DuplicateStep(StepId),
    /// Two fields share this id.
    DuplicateField(FieldId),
    /// A text rule allows nothing: its maximum length is zero, or its minimum
    /// is above its maximum.
    BadTextRule(FieldId),
    /// A text field may be longer than the flow can store.
    TextExceedsStorage {
        /// The field concerned.
        field: FieldId,
        /// The field's maximum length in bytes.
        max_len: u16,
        /// The flow's storage per value, in bytes.
        capacity: usize,
    },
    /// A choice field has no options.
    NoOptions(FieldId),
    /// A timeout is zero, the idle warning does not start before the idle
    /// reset, or the idle or submit timeout is not shorter than the maximum
    /// session length.
    BadTimeouts,
    /// The settings allow no submission attempts.
    NoAttempts,
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSteps => f.write_str("the form has no steps"),
            Self::TooManyFields { capacity } => {
                write!(f, "the form has more than {capacity} fields")
            }
            Self::DuplicateStep(step) => write!(f, "two steps have id {}", step.0),
            Self::DuplicateField(field) => write!(f, "two fields have id {}", field.0),
            Self::BadTextRule(field) => write!(f, "field {} has an impossible text rule", field.0),
            Self::TextExceedsStorage {
                field,
                max_len,
                capacity,
            } => write!(
                f,
                "field {} allows {max_len} bytes but values hold at most {capacity}",
                field.0
            ),
            Self::NoOptions(field) => write!(f, "choice field {} has no options", field.0),
            Self::BadTimeouts => {
                f.write_str("a timeout is zero, or too long for the timeouts that bound it")
            }
            Self::NoAttempts => f.write_str("at least one submission attempt is needed"),
        }
    }
}

impl core::error::Error for SetupError {}
