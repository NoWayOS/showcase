// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reports: the records the application sends the UI, one kind for each
//! thing the UI must show or do.
//!
//! No report carries an answer: no entered text, no option chosen, no box
//! ticked. Reports name fields only by id, and say what is wrong with one
//! by an [`Issue`] code.

use nowayos_ipc::{DecodeError, EncodeError, Header, Message, Reader, Writer, decode, encode};
use showcase_kiosk_flow::{FailureReason, FieldId, FlowError, Issue};

use crate::{ContractError, VERSION};

/// Kind of [`ShowWelcome`].
pub const SHOW_WELCOME_KIND: u16 = 0x4B81;
/// Kind of [`ShowStep`].
pub const SHOW_STEP_KIND: u16 = 0x4B82;
/// Kind of [`ShowReview`].
pub const SHOW_REVIEW_KIND: u16 = 0x4B83;
/// Kind of [`ShowSubmitting`].
pub const SHOW_SUBMITTING_KIND: u16 = 0x4B84;
/// Kind of [`ShowSubmitted`].
pub const SHOW_SUBMITTED_KIND: u16 = 0x4B85;
/// Kind of [`ShowFailed`].
pub const SHOW_FAILED_KIND: u16 = 0x4B86;
/// Kind of [`IdleWarning`].
pub const IDLE_WARNING_KIND: u16 = 0x4B87;
/// Kind of [`ValueRefused`].
pub const VALUE_REFUSED_KIND: u16 = 0x4B88;
/// Kind of [`Incomplete`].
pub const INCOMPLETE_KIND: u16 = 0x4B89;
/// Kind of [`Refused`].
pub const REFUSED_KIND: u16 = 0x4B8A;
/// Kind of [`SessionEnded`].
pub const SESSION_ENDED_KIND: u16 = 0x4B8B;
/// Kind of [`Countdown`].
pub const COUNTDOWN_KIND: u16 = 0x4B8C;
/// Kind of [`Issued`].
pub const ISSUED_KIND: u16 = 0x4B8D;
/// Kind of [`OutOfService`]: the first kind after [`Issued`], so
/// the kinds before it stay as they are.
pub const OUT_OF_SERVICE_KIND: u16 = 0x4B8E;

/// Every report kind, in the order of [`Report`]'s variants.
pub const REPORT_KINDS: [u16; 14] = [
    SHOW_WELCOME_KIND,
    SHOW_STEP_KIND,
    SHOW_REVIEW_KIND,
    SHOW_SUBMITTING_KIND,
    SHOW_SUBMITTED_KIND,
    SHOW_FAILED_KIND,
    IDLE_WARNING_KIND,
    VALUE_REFUSED_KIND,
    INCOMPLETE_KIND,
    REFUSED_KIND,
    SESSION_ENDED_KIND,
    COUNTDOWN_KIND,
    ISSUED_KIND,
    OUT_OF_SERVICE_KIND,
];

/// A report with no payload: the record is its header.
macro_rules! empty_report {
    ($(#[$doc:meta])* $name:ident = $kind:ident) => {
        $(#[$doc])*
        ///
        /// Wire layout: no payload.
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct $name;

        impl Message for $name {
            const KIND: u16 = $kind;
            const VERSION: u16 = VERSION;
            const PAYLOAD_LEN: usize = 0;

            fn encode_payload(&self, _out: &mut Writer<'_>) -> Result<(), EncodeError> {
                Ok(())
            }

            fn decode_payload(_input: &mut Reader<'_>) -> Result<Self, DecodeError> {
                Ok(Self)
            }
        }
    };
}

empty_report!(
    /// Show the welcome screen: no session is open.
    ShowWelcome = SHOW_WELCOME_KIND
);
empty_report!(
    /// Show the review screen: every step is complete, and the person
    /// checks their answers (the UI's own copies: see the
    /// [crate documentation](crate)) before sending them.
    ShowReview = SHOW_REVIEW_KIND
);
empty_report!(
    /// Show the confirmation: the service accepted the answers. A
    /// [`SessionEnded`] has already said so, and every copy of them is gone.
    ShowSubmitted = SHOW_SUBMITTED_KIND
);

/// Show a step of the form.
///
/// Wire layout, 2 bytes: the step's index, from 0 (`u16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShowStep {
    /// Which step, counting from 0.
    pub index: u16,
}

impl Message for ShowStep {
    const KIND: u16 = SHOW_STEP_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 2;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u16(self.index)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            index: input.read_u16()?,
        })
    }
}

/// Show that the answers are on their way to the service.
///
/// Wire layout, 1 byte: the attempt, from 1 (`u8`, not zero).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShowSubmitting {
    /// Which attempt this is, from 1.
    pub attempt: u8,
}

impl Message for ShowSubmitting {
    const KIND: u16 = SHOW_SUBMITTING_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 1;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u8(attempt_to_wire(self.attempt).map_err(encode_error)?)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            attempt: attempt_to_wire(input.read_u8()?)?,
        })
    }
}

/// Show that the submission failed, and whether the person may retry.
///
/// Wire layout, 4 bytes: the reason (`1` timed out, `2` unavailable, `3`
/// rejected), the attempt that failed (from 1, not zero), the attempts left
/// (`u8` each), then one zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowFailed {
    /// Why it failed. After [`FailureReason::Rejected`], sending the same
    /// answers again is refused.
    pub reason: FailureReason,
    /// Which attempt failed, from 1.
    pub attempt: u8,
    /// Attempts this session has left.
    pub attempts_left: u8,
}

impl Message for ShowFailed {
    const KIND: u16 = SHOW_FAILED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 4;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u8(failure_to_wire(self.reason))?;
        out.write_u8(attempt_to_wire(self.attempt).map_err(encode_error)?)?;
        out.write_u8(self.attempts_left)?;
        out.write_zeros(1)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let reason = failure_from_wire(input.read_u8()?)?;
        let attempt = attempt_to_wire(input.read_u8()?)?;
        let attempts_left = input.read_u8()?;
        input.read_zeros(1)?;
        Ok(Self {
            reason,
            attempt,
            attempts_left,
        })
    }
}

/// Ask "are you still there?": unless the person does something first,
/// the session ends after `remaining` more ticks. Any later `Show…` report
/// withdraws the warning.
///
/// Wire layout, 4 bytes: `remaining` (`u32`), in the application's ticks
/// (milliseconds in the kiosk), saturating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdleWarning {
    /// Ticks until the session ends.
    pub remaining: u32,
}

impl Message for IdleWarning {
    const KIND: u16 = IDLE_WARNING_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 4;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.remaining)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            remaining: input.read_u32()?,
        })
    }
}

/// How long the screen showing has left before it ends by itself:
/// sent once a second, as the whole seconds left change, while the idle
/// warning shows ([`IdleWarning`] starts it) and while the confirmation
/// does (sent after [`ShowSubmitted`]). The UI shows the seconds left from
/// it - `remaining` rounded up to whole seconds - and keeps no clock of its
/// own; the application ends the session when the time is up, whatever the
/// UI shows. A UI showing a screen with no countdown ignores it.
///
/// Wire layout, 4 bytes: `remaining` (`u32`), in the application's ticks
/// (milliseconds in the kiosk), saturating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Countdown {
    /// Ticks until the screen ends by itself.
    pub remaining: u32,
}

impl Message for Countdown {
    const KIND: u16 = COUNTDOWN_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 4;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.remaining)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            remaining: input.read_u32()?,
        })
    }
}

/// The longest code an [`Issued`] report carries.
pub const ISSUED_LEN: usize = 8;

/// What the service issued for the answers it accepted: a pass
/// number, a queue number - one to [`ISSUED_LEN`] capital letters and
/// digits, which the confirmation shows. It comes from the service, after
/// [`ShowSubmitted`] and before the confirmation's [`Countdown`]: never an
/// answer, nor anything made from one.
///
/// Wire layout, 8 bytes: the code's characters (`A`-`Z`, `0`-`9`), then
/// zero bytes to the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Issued {
    pub(crate) code: [u8; ISSUED_LEN],
}

impl Issued {
    /// The report for `code`.
    ///
    /// # Errors
    ///
    /// [`EncodeError::InvalidValue`] unless `code` is 1 to [`ISSUED_LEN`]
    /// capital letters and digits.
    pub fn new(code: &str) -> Result<Self, EncodeError> {
        let mut bytes = [0u8; ISSUED_LEN];
        if code.is_empty() || code.len() > ISSUED_LEN {
            return Err(EncodeError::InvalidValue { field: "code" });
        }
        for (slot, byte) in bytes.iter_mut().zip(code.bytes()) {
            *slot = byte;
        }
        Self::checked(bytes).map_err(encode_error)
    }

    /// The code: capital letters and digits.
    #[must_use]
    pub fn code(&self) -> &str {
        let len = self
            .code
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(ISSUED_LEN);
        self.code
            .get(..len)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
            .unwrap_or("")
    }

    /// `bytes` as a code, if it is one: characters then zero padding.
    pub(crate) fn checked(bytes: [u8; ISSUED_LEN]) -> Result<Self, DecodeError> {
        let len = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(ISSUED_LEN);
        let (code, padding) = bytes.split_at(len);
        let valid = !code.is_empty()
            && code
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            && padding.iter().all(|&byte| byte == 0);
        if valid {
            Ok(Self { code: bytes })
        } else {
            Err(DecodeError::InvalidValue { field: "code" })
        }
    }
}

impl Message for Issued {
    const KIND: u16 = ISSUED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = ISSUED_LEN;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        Self::checked(self.code).map_err(encode_error)?;
        out.write_bytes(&self.code)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Self::checked(input.read_array()?)
    }
}

/// The length of the code an [`OutOfService`] report carries while the
/// kiosk awaits approval: the first 64 bits of the SHA-256 of its public
/// key.
pub const APPROVAL_CODE_LEN: usize = 8;

/// Why the kiosk is out of service ([`OutOfService`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutOfServiceReason {
    /// Its key awaits approval at the backend: the code the person
    /// approving compares. Public: whoever reads it can
    /// do nothing with it.
    AwaitingApproval {
        /// The approval code.
        code: [u8; APPROVAL_CODE_LEN],
    },
    /// The backend cannot be reached, or turned the kiosk down.
    NoService,
    /// An update is being applied (the update hook).
    Updating,
}

/// The kiosk is out of service: the UI shows the fixed
/// out-of-service screen, and why - with the approval code while the kiosk
/// awaits approval - takes no input and opens no session. The application
/// sends it in place of the welcome screen, once no session is open
/// ([`AppEnd::stop_service`](crate::AppEnd::stop_service)), and refuses a
/// start until it resumes service with [`ShowWelcome`]. It carries no
/// answer.
///
/// Wire layout, 12 bytes: the reason (`u8`: `1` awaiting approval, `2` no
/// service, `3` updating), three zero bytes, then the approval code -
/// [`APPROVAL_CODE_LEN`] bytes while awaiting approval, zeros otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutOfService {
    /// Why.
    pub reason: OutOfServiceReason,
}

impl Message for OutOfService {
    const KIND: u16 = OUT_OF_SERVICE_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 4 + APPROVAL_CODE_LEN;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        let (reason, code) = match self.reason {
            OutOfServiceReason::AwaitingApproval { code } => (1, code),
            OutOfServiceReason::NoService => (2, [0; APPROVAL_CODE_LEN]),
            OutOfServiceReason::Updating => (3, [0; APPROVAL_CODE_LEN]),
        };
        out.write_u8(reason)?;
        out.write_zeros(3)?;
        out.write_bytes(&code)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let reason = input.read_u8()?;
        input.read_zeros(3)?;
        let code: [u8; APPROVAL_CODE_LEN] = input.read_array()?;
        let reason = match reason {
            1 => OutOfServiceReason::AwaitingApproval { code },
            2 | 3 if code != [0; APPROVAL_CODE_LEN] => {
                return Err(DecodeError::InvalidValue { field: "code" });
            }
            2 => OutOfServiceReason::NoService,
            3 => OutOfServiceReason::Updating,
            _ => return Err(DecodeError::InvalidValue { field: "reason" }),
        };
        Ok(Self { reason })
    }
}

/// The value an action set was refused, by the field's rule: the
/// application has cleared the field, and the UI clears its own copy of
/// it (if its last action for the field was this one) and shows `issue`.
///
/// Wire layout, 8 bytes: `seq` (`u32`), field (`u16`), issue (`u8`: `1`
/// missing, `2` too short, `3` too long, `4` bad character, `5` no such
/// option), then one zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRefused {
    /// The action refused.
    pub seq: u32,
    /// The field.
    pub field: FieldId,
    /// What is wrong with the value; never the value itself.
    pub issue: Issue,
}

impl Message for ValueRefused {
    const KIND: u16 = VALUE_REFUSED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        encode_problem(out, self.seq, self.field, self.issue)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let (seq, field, issue) = decode_problem(input)?;
        Ok(Self { seq, field, issue })
    }
}

/// The person cannot go on yet (`Next` or `Submit` refused): this is the
/// first field that needs attention. Nothing changed.
///
/// Wire layout, 8 bytes: as [`ValueRefused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Incomplete {
    /// The action refused.
    pub seq: u32,
    /// The first field that needs attention.
    pub field: FieldId,
    /// What it needs.
    pub issue: Issue,
}

impl Message for Incomplete {
    const KIND: u16 = INCOMPLETE_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        encode_problem(out, self.seq, self.field, self.issue)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let (seq, field, issue) = decode_problem(input)?;
        Ok(Self { seq, field, issue })
    }
}

/// Why the application refused an action, when it was not because of a
/// field's value ([`ValueRefused`]) or an incomplete step
/// ([`Incomplete`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// Not possible on the screen the application is showing; the current
    /// `Show…` report follows, so the UI catches up.
    NotAllowed,
    /// The form has no such field.
    UnknownField,
    /// The field is on another step.
    NotOnThisStep,
    /// The action does not suit the field's kind.
    WrongKind,
    /// Back on the first step: there is no earlier one.
    AtFirstStep,
    /// Every submission attempt this session allows has been used.
    NoAttemptsLeft,
    /// The service rejected these answers; sending them again unchanged
    /// would not help.
    NotRetryable,
    /// The session had already ended when the action arrived.
    Expired,
}

impl Refusal {
    /// The report answering action `seq`, refused by the flow with `error`:
    /// [`ValueRefused`], [`Incomplete`] or [`Refused`].
    #[must_use]
    pub const fn report(seq: u32, error: FlowError) -> Report {
        let refusal = match error {
            FlowError::InvalidValue(problem) => {
                return Report::ValueRefused(ValueRefused {
                    seq,
                    field: problem.field,
                    issue: problem.issue,
                });
            }
            FlowError::Incomplete(problem) => {
                return Report::Incomplete(Incomplete {
                    seq,
                    field: problem.field,
                    issue: problem.issue,
                });
            }
            // A ticket belongs to a service result, never to an action.
            FlowError::NotAllowed(_) | FlowError::StaleTicket => Self::NotAllowed,
            FlowError::Expired => Self::Expired,
            FlowError::UnknownField(_) => Self::UnknownField,
            FlowError::NotOnThisStep(_) => Self::NotOnThisStep,
            FlowError::WrongKind(_) => Self::WrongKind,
            FlowError::AtFirstStep => Self::AtFirstStep,
            FlowError::NoAttemptsLeft => Self::NoAttemptsLeft,
            FlowError::NotRetryable => Self::NotRetryable,
        };
        Report::Refused(Refused { seq, refusal })
    }

    const fn to_wire(self) -> u8 {
        match self {
            Self::NotAllowed => 1,
            Self::UnknownField => 2,
            Self::NotOnThisStep => 3,
            Self::WrongKind => 4,
            Self::AtFirstStep => 5,
            Self::NoAttemptsLeft => 6,
            Self::NotRetryable => 7,
            Self::Expired => 8,
        }
    }

    const fn from_wire(code: u8) -> Result<Self, DecodeError> {
        Ok(match code {
            1 => Self::NotAllowed,
            2 => Self::UnknownField,
            3 => Self::NotOnThisStep,
            4 => Self::WrongKind,
            5 => Self::AtFirstStep,
            6 => Self::NoAttemptsLeft,
            7 => Self::NotRetryable,
            8 => Self::Expired,
            _ => return Err(DecodeError::InvalidValue { field: "refusal" }),
        })
    }
}

/// The application refused an action, for a reason other than a value or
/// an incomplete step. Nothing changed.
///
/// Wire layout, 8 bytes: `seq` (`u32`), the refusal (`u8`: `1` not
/// allowed, `2` unknown field, `3` not on this step, `4` wrong kind, `5` at
/// the first step, `6` no attempts left, `7` not retryable, `8` expired),
/// then three zero bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Refused {
    /// The action refused.
    pub seq: u32,
    /// Why.
    pub refusal: Refusal,
}

impl Message for Refused {
    const KIND: u16 = REFUSED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.seq)?;
        out.write_u8(self.refusal.to_wire())?;
        out.write_zeros(3)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let refusal = Refusal::from_wire(input.read_u8()?)?;
        input.read_zeros(3)?;
        Ok(Self { seq, refusal })
    }
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndReason {
    /// The person cancelled.
    Cancelled,
    /// Nobody touched the kiosk for the whole idle timeout, or the clock
    /// went backwards so idle time could not be measured.
    Idle,
    /// The session reached its maximum length, however active the person
    /// was.
    TimeLimit,
    /// The service accepted the answers. The confirmation follows
    /// ([`ShowSubmitted`]).
    Submitted,
    /// The application ended it for a fault: a record from the UI it could
    /// not decode ([`AppEnd::receive`](crate::AppEnd::receive)), or a fault
    /// elsewhere ([`AppEnd::end_session`](crate::AppEnd::end_session)).
    Fault,
    /// The UI abandoned it ([`Abandon`](crate::Abandon)): the person chose
    /// Home in the showcase, or the UI failed closed on a report it
    /// could not trust.
    Abandoned,
}

impl EndReason {
    const fn to_wire(self) -> u8 {
        match self {
            Self::Cancelled => 1,
            Self::Idle => 2,
            Self::TimeLimit => 3,
            Self::Submitted => 4,
            Self::Fault => 5,
            Self::Abandoned => 6,
        }
    }

    const fn from_wire(code: u8) -> Result<Self, DecodeError> {
        Ok(match code {
            1 => Self::Cancelled,
            2 => Self::Idle,
            3 => Self::TimeLimit,
            4 => Self::Submitted,
            5 => Self::Fault,
            6 => Self::Abandoned,
            _ => return Err(DecodeError::InvalidValue { field: "reason" }),
        })
    }
}

/// **The session is over: wipe every copy of its answers now.**
///
/// The application sends exactly one for each session it ends, whatever
/// ends it, before anything else about what follows; the UI clears every
/// value it holds for the session (its text inputs, its copies of the
/// answers) in place, then draws the screen the next report names. It
/// carries the session's number and the reason, and nothing about the
/// answers.
///
/// Wire layout, 8 bytes: the session (`u32`), the reason (`u8`: `1`
/// cancelled, `2` idle, `3` time limit, `4` submitted, `5` fault, `6`
/// abandoned by the UI), then three zero bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionEnded {
    /// The session that ended ([`KioskFlow::session`](showcase_kiosk_flow::KioskFlow::session)).
    pub session: u32,
    /// Why it ended.
    pub reason: EndReason,
}

impl Message for SessionEnded {
    const KIND: u16 = SESSION_ENDED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.session)?;
        out.write_u8(self.reason.to_wire())?;
        out.write_zeros(3)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let session = input.read_u32()?;
        let reason = EndReason::from_wire(input.read_u8()?)?;
        input.read_zeros(3)?;
        Ok(Self { session, reason })
    }
}

/// Any report, as [`decode_report`] returns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// See [`ShowWelcome`].
    ShowWelcome(ShowWelcome),
    /// See [`ShowStep`].
    ShowStep(ShowStep),
    /// See [`ShowReview`].
    ShowReview(ShowReview),
    /// See [`ShowSubmitting`].
    ShowSubmitting(ShowSubmitting),
    /// See [`ShowSubmitted`].
    ShowSubmitted(ShowSubmitted),
    /// See [`ShowFailed`].
    ShowFailed(ShowFailed),
    /// See [`IdleWarning`].
    IdleWarning(IdleWarning),
    /// See [`ValueRefused`].
    ValueRefused(ValueRefused),
    /// See [`Incomplete`].
    Incomplete(Incomplete),
    /// See [`Refused`].
    Refused(Refused),
    /// See [`SessionEnded`].
    SessionEnded(SessionEnded),
    /// See [`Countdown`].
    Countdown(Countdown),
    /// See [`Issued`].
    Issued(Issued),
    /// See [`OutOfService`].
    OutOfService(OutOfService),
}

/// Only so a bounded list of reports can be made; never sent for its own
/// sake.
impl Default for Report {
    fn default() -> Self {
        Self::ShowWelcome(ShowWelcome)
    }
}

impl Report {
    /// The record kind.
    #[must_use]
    pub const fn kind(&self) -> u16 {
        match self {
            Self::ShowWelcome(_) => SHOW_WELCOME_KIND,
            Self::ShowStep(_) => SHOW_STEP_KIND,
            Self::ShowReview(_) => SHOW_REVIEW_KIND,
            Self::ShowSubmitting(_) => SHOW_SUBMITTING_KIND,
            Self::ShowSubmitted(_) => SHOW_SUBMITTED_KIND,
            Self::ShowFailed(_) => SHOW_FAILED_KIND,
            Self::IdleWarning(_) => IDLE_WARNING_KIND,
            Self::ValueRefused(_) => VALUE_REFUSED_KIND,
            Self::Incomplete(_) => INCOMPLETE_KIND,
            Self::Refused(_) => REFUSED_KIND,
            Self::SessionEnded(_) => SESSION_ENDED_KIND,
            Self::Countdown(_) => COUNTDOWN_KIND,
            Self::Issued(_) => ISSUED_KIND,
            Self::OutOfService(_) => OUT_OF_SERVICE_KIND,
        }
    }

    /// Whether this report names the screen to draw (a `Show…` report).
    #[must_use]
    pub const fn is_screen(&self) -> bool {
        matches!(
            self,
            Self::ShowWelcome(_)
                | Self::ShowStep(_)
                | Self::ShowReview(_)
                | Self::ShowSubmitting(_)
                | Self::ShowSubmitted(_)
                | Self::ShowFailed(_)
                | Self::OutOfService(_)
        )
    }

    /// Encodes the report as a record at the start of `out`, returning its
    /// length; [`MAX_RECORD_LEN`](crate::MAX_RECORD_LEN) bytes always
    /// suffice.
    ///
    /// # Errors
    ///
    /// As for [`nowayos_ipc::encode`]: [`EncodeError::BufferTooSmall`] if
    /// `out` is too short, [`EncodeError::InvalidValue`] for an attempt of 0.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        match self {
            Self::ShowWelcome(r) => encode(r, out),
            Self::ShowStep(r) => encode(r, out),
            Self::ShowReview(r) => encode(r, out),
            Self::ShowSubmitting(r) => encode(r, out),
            Self::ShowSubmitted(r) => encode(r, out),
            Self::ShowFailed(r) => encode(r, out),
            Self::IdleWarning(r) => encode(r, out),
            Self::ValueRefused(r) => encode(r, out),
            Self::Incomplete(r) => encode(r, out),
            Self::Refused(r) => encode(r, out),
            Self::SessionEnded(r) => encode(r, out),
            Self::Countdown(r) => encode(r, out),
            Self::Issued(r) => encode(r, out),
            Self::OutOfService(r) => encode(r, out),
        }
    }
}

/// Decodes one report record, already copied out of the ring into private
/// memory. Bytes after the record are ignored.
///
/// # Errors
///
/// [`ContractError::UnexpectedKind`] for a kind that is not a report
/// (including every action kind); [`ContractError::Decode`] for a malformed
/// record, a wrong version or length among them. Never panics, whatever the
/// bytes.
pub fn decode_report(record: &[u8]) -> Result<Report, ContractError> {
    let report = match Header::parse(record)?.kind {
        SHOW_WELCOME_KIND => Report::ShowWelcome(decode(record)?),
        SHOW_STEP_KIND => Report::ShowStep(decode(record)?),
        SHOW_REVIEW_KIND => Report::ShowReview(decode(record)?),
        SHOW_SUBMITTING_KIND => Report::ShowSubmitting(decode(record)?),
        SHOW_SUBMITTED_KIND => Report::ShowSubmitted(decode(record)?),
        SHOW_FAILED_KIND => Report::ShowFailed(decode(record)?),
        IDLE_WARNING_KIND => Report::IdleWarning(decode(record)?),
        VALUE_REFUSED_KIND => Report::ValueRefused(decode(record)?),
        INCOMPLETE_KIND => Report::Incomplete(decode(record)?),
        REFUSED_KIND => Report::Refused(decode(record)?),
        SESSION_ENDED_KIND => Report::SessionEnded(decode(record)?),
        COUNTDOWN_KIND => Report::Countdown(decode(record)?),
        ISSUED_KIND => Report::Issued(decode(record)?),
        OUT_OF_SERVICE_KIND => Report::OutOfService(decode(record)?),
        kind => return Err(ContractError::UnexpectedKind { kind }),
    };
    Ok(report)
}

/// An attempt number is counted from 1.
const fn attempt_to_wire(attempt: u8) -> Result<u8, DecodeError> {
    if attempt == 0 {
        Err(DecodeError::InvalidValue { field: "attempt" })
    } else {
        Ok(attempt)
    }
}

/// A field that cannot be encoded, from the check that refuses it on
/// receipt.
pub(crate) const fn encode_error(err: DecodeError) -> EncodeError {
    match err {
        DecodeError::InvalidValue { field } => EncodeError::InvalidValue { field },
        _ => EncodeError::InvalidValue { field: "value" },
    }
}

const fn failure_to_wire(reason: FailureReason) -> u8 {
    match reason {
        FailureReason::TimedOut => 1,
        FailureReason::Unavailable => 2,
        FailureReason::Rejected => 3,
    }
}

const fn failure_from_wire(code: u8) -> Result<FailureReason, DecodeError> {
    Ok(match code {
        1 => FailureReason::TimedOut,
        2 => FailureReason::Unavailable,
        3 => FailureReason::Rejected,
        _ => return Err(DecodeError::InvalidValue { field: "reason" }),
    })
}

const fn issue_to_wire(issue: Issue) -> u8 {
    match issue {
        Issue::Missing => 1,
        Issue::TooShort => 2,
        Issue::TooLong => 3,
        Issue::BadCharacter => 4,
        Issue::NoSuchOption => 5,
    }
}

const fn issue_from_wire(code: u8) -> Result<Issue, DecodeError> {
    Ok(match code {
        1 => Issue::Missing,
        2 => Issue::TooShort,
        3 => Issue::TooLong,
        4 => Issue::BadCharacter,
        5 => Issue::NoSuchOption,
        _ => return Err(DecodeError::InvalidValue { field: "issue" }),
    })
}

/// The layout [`ValueRefused`] and [`Incomplete`] share.
fn encode_problem(
    out: &mut Writer<'_>,
    seq: u32,
    field: FieldId,
    issue: Issue,
) -> Result<(), EncodeError> {
    out.write_u32(seq)?;
    out.write_u16(field.0)?;
    out.write_u8(issue_to_wire(issue))?;
    out.write_zeros(1)
}

fn decode_problem(input: &mut Reader<'_>) -> Result<(u32, FieldId, Issue), DecodeError> {
    let seq = input.read_u32()?;
    let field = FieldId(input.read_u16()?);
    let issue = issue_from_wire(input.read_u8()?)?;
    input.read_zeros(1)?;
    Ok((seq, field, issue))
}
