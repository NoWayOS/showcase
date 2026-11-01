// SPDX-License-Identifier: MIT OR Apache-2.0
//! [`KioskFlow`]: the state machine, its settings and its timed events.

use core::fmt;

use nowayos_bounded::BoundedVec;

use crate::error::{FieldProblem, FlowError, Issue, SetupError};
use crate::form::{Field, FieldId, FieldKind, Form, Step};
use crate::value::{Slot, Value};

/// A point in time, as a count of the caller's clock ticks.
///
/// The flow has no clock of its own: every call that can depend on time
/// takes the current tick, and the caller chooses the unit (milliseconds
/// from a timer protection domain, say). Read the tick **at the moment of
/// each call**, from one monotonic clock that does not wrap: a 64-bit count
/// from boot, not a 32-bit one.
///
/// A reading earlier than one already seen means the clock cannot be
/// trusted, so the flow fails closed: it wipes any session it holds
/// ([`TickEvent::IdleReset`] from [`KioskFlow::tick`],
/// [`FlowError::Expired`] from any other call) and carries on from the new
/// reading. A reading ahead of the real time ends the session too: at once
/// if it passes a deadline, otherwise at the next correct reading, which is
/// earlier. Either way the flow then carries on from the correct time.
/// Deadlines saturate at `u64::MAX`, so nothing can panic or wrap.
pub type Tick = u64;

/// How long each timed part of the flow lasts, in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// Ticks without activity after which a session waiting for the person
    /// ([`Phase::awaits_person`]) is wiped and the flow returns to
    /// [`Phase::Welcome`]. Greater than zero.
    pub idle_reset: Tick,
    /// How long before the reset the "are you still there?" warning starts.
    /// Zero means no warning; otherwise less than `idle_reset`.
    pub idle_warning: Tick,
    /// How long to wait for a submission result before treating the attempt
    /// as failed ([`FailureReason::TimedOut`]). Greater than zero.
    pub submit: Tick,
    /// How long the confirmation stays on screen after a successful
    /// submission before the flow returns to [`Phase::Welcome`]. Greater
    /// than zero.
    pub confirmation: Tick,
    /// The longest a session may hold answers, from [`KioskFlow::start`],
    /// however active the person is: activity restarts the idle timeout but
    /// never this one. It stops stuck input (a key held down, an object
    /// resting on the touchscreen, a misbehaving UI) from keeping one
    /// person's answers on a shared device. When it is reached the session
    /// is wiped ([`TickEvent::SessionLimit`]). Longer than both `idle_reset`
    /// and `submit`; choose it with room for the slowest person to finish,
    /// twenty minutes say.
    pub max_session: Tick,
}

/// Settings for a [`KioskFlow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowConfig {
    /// The timeouts.
    pub timeouts: Timeouts,
    /// Submission attempts allowed per session, first attempt and retries
    /// together. At least 1.
    pub max_attempts: u8,
}

impl Timeouts {
    /// These timeouts, or, in one of Showcase's QEMU test images, the short
    /// ones the tests wait out: the idle warning 5 s after the last
    /// activity and the reset 3 s after it, a submission's timeout after
    /// 2 s and the confirmation for 4 s; `max_session` is kept. A test image
    /// is one compiled with `SHOWCASE_TEST_TIMINGS` set, which Showcase's
    /// Makefile sets for `make qemu-test` and `make qemu-test-all` only, so
    /// the Pi's images, `make qemu` and the host tests keep these.
    #[must_use]
    pub const fn or_test(self) -> Self {
        if option_env!("SHOWCASE_TEST_TIMINGS").is_some() {
            Self {
                idle_reset: 8_000,
                idle_warning: 3_000,
                submit: 2_000,
                confirmation: 4_000,
                max_session: self.max_session,
            }
        } else {
            self
        }
    }
}

impl FlowConfig {
    /// Checks that the settings are usable: every timeout above zero, the
    /// idle warning shorter than the idle timeout, and the idle and submit
    /// timeouts both shorter than the maximum session length, so that no
    /// setting can switch the privacy reset off.
    ///
    /// # Errors
    ///
    /// [`SetupError::BadTimeouts`] or [`SetupError::NoAttempts`].
    pub fn check(&self) -> Result<(), SetupError> {
        let t = &self.timeouts;
        if t.idle_reset == 0
            || t.idle_warning >= t.idle_reset
            || t.submit == 0
            || t.confirmation == 0
            || t.idle_reset >= t.max_session
            || t.submit >= t.max_session
        {
            return Err(SetupError::BadTimeouts);
        }
        if self.max_attempts == 0 {
            return Err(SetupError::NoAttempts);
        }
        Ok(())
    }
}

/// Why a submission attempt failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    /// No result arrived within [`Timeouts::submit`]. The flow sets this
    /// itself; the caller may also report it.
    TimedOut,
    /// The service could not be reached or was not ready. Worth retrying.
    Unavailable,
    /// The service refused the submission. Sending the same answers again
    /// would not help: the person can go back and change them, or cancel.
    /// Until an answer changes, [`KioskFlow::retry`] and
    /// [`KioskFlow::submit`] both refuse with [`FlowError::NotRetryable`].
    Rejected,
}

impl FailureReason {
    /// Whether [`KioskFlow::retry`] may send the same answers again.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        !matches!(self, Self::Rejected)
    }
}

/// Where the flow is. The UI draws the screen for the current phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No session: the welcome screen. Holds no answers.
    Welcome,
    /// The person is on a step of the form.
    Step {
        /// Which step, counting from 0.
        index: usize,
    },
    /// Every step is complete; the person checks their answers before
    /// submitting them.
    Review,
    /// The answers are on their way to the service, and frozen until the
    /// result.
    Submitting {
        /// Which attempt this is, counting from 1.
        attempt: u8,
    },
    /// The service accepted the answers. They have already been cleared from
    /// the flow.
    Submitted,
    /// The submission failed. The answers are kept for a retry or a
    /// correction, until the idle timeout.
    Failed {
        /// Why it failed.
        reason: FailureReason,
        /// Which attempt failed, counting from 1.
        attempt: u8,
    },
}

impl Phase {
    /// Whether the flow is waiting for the person while holding their
    /// session: the phases the idle timeout applies to.
    #[must_use]
    pub const fn awaits_person(self) -> bool {
        matches!(self, Self::Step { .. } | Self::Review | Self::Failed { .. })
    }

    /// Whether the flow holds a session's answers: while it waits for the
    /// person, and while the answers are on their way to the service. The
    /// maximum session length applies in these phases. Leaving them ends
    /// the person's session: every answer has then been cleared.
    #[must_use]
    pub const fn holds_answers(self) -> bool {
        self.awaits_person() || matches!(self, Self::Submitting { .. })
    }
}

/// Identifies one submission attempt.
///
/// [`KioskFlow::submit`] and [`KioskFlow::retry`] hand one out; the result
/// must quote it back. A result whose ticket is not the one in flight (a
/// late answer to an attempt that already timed out, one meant for an
/// earlier person's session, or one for a submission made before the
/// application restarted) is refused with [`FlowError::StaleTicket`]
/// instead of being applied to the wrong submission.
///
/// Tickets identify attempts within one kiosk, for matching results to
/// submissions. They are not secret, and are not meant as a service-side
/// idempotency key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmissionTicket {
    /// The flow instance that made the submission ([`KioskFlow::epoch`]):
    /// different after every restart of the application.
    pub epoch: u32,
    /// The session the answers belong to ([`KioskFlow::session`]).
    pub session: u32,
    /// The attempt within that session, counting from 1.
    pub attempt: u8,
}

/// Something that happened because time passed, reported by
/// [`KioskFlow::tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickEvent {
    /// Nobody has touched the kiosk for a while: time to ask "are you still
    /// there?". Unless there is activity first, the session is wiped at
    /// `reset_at`. (If the session reaches its maximum length first,
    /// `reset_at` is that moment, which activity does not postpone.)
    IdleWarning {
        /// When the session will be wiped.
        reset_at: Tick,
    },
    /// Nobody touched the kiosk for the whole idle timeout, or the clock
    /// went backwards so the idle time could not be measured (see [`Tick`]).
    /// Every answer has been cleared and the flow is back at
    /// [`Phase::Welcome`]. Copies held elsewhere, such as the UI's text
    /// inputs, must be cleared now too.
    IdleReset,
    /// The session reached its maximum length ([`Timeouts::max_session`]),
    /// however active the person was. As for [`TickEvent::IdleReset`],
    /// every answer has been cleared, the flow is back at
    /// [`Phase::Welcome`], and copies held elsewhere must be cleared now too.
    SessionLimit,
    /// No submission result arrived in time. The flow is now
    /// [`Phase::Failed`] with [`FailureReason::TimedOut`].
    SubmitTimedOut,
    /// The confirmation has been shown for its full time. The flow is back
    /// at [`Phase::Welcome`].
    ConfirmationEnded,
}

impl TickEvent {
    /// Whether the session is over and the flow is back at
    /// [`Phase::Welcome`]: show the welcome screen, and clear every copy of
    /// the answers held outside the flow. True for
    /// [`TickEvent::IdleReset`], [`TickEvent::SessionLimit`] and
    /// [`TickEvent::ConfirmationEnded`].
    #[must_use]
    pub const fn ends_session(self) -> bool {
        matches!(
            self,
            Self::IdleReset | Self::SessionLimit | Self::ConfirmationEnded
        )
    }
}

/// What catching up with the clock did (see `KioskFlow::catch_up`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaughtUp {
    /// Nothing was due.
    Nothing,
    /// The submission in flight timed out, as of its deadline.
    SubmitTimedOut,
    /// The session was wiped: idle for the whole timeout, or the clock went
    /// backwards.
    IdleReset,
    /// The session was wiped: it reached its maximum length.
    SessionLimit,
}

impl CaughtUp {
    /// Whether the session was wiped.
    const fn wiped(self) -> bool {
        matches!(self, Self::IdleReset | Self::SessionLimit)
    }
}

/// Where a field sits: its storage slot and its step.
#[derive(Clone, Copy)]
struct Located<'a> {
    slot: usize,
    step: usize,
    field: &'a Field,
}

/// The state machine for a multi-step form on a shared public kiosk.
///
/// `F` is the most fields a form may have, and `V` the most bytes a text
/// value may hold. Storage for both is inline and fixed, about
/// `F * (V + 16)` bytes. In a protection domain, keep the flow in static
/// or handler state rather than on a small stack.
///
/// Every method that takes a [`Tick`] is either something the person did
/// (`start`, the edits, `next`, `back`, `submit`, `retry`, `cancel`,
/// `finish`, `note_activity`), a result from the service
/// (`submission_succeeded`, `submission_failed`) or the passage of time
/// (`tick`). Each first applies whatever fell due before its tick, so the
/// timing of the calls never extends a session. After any of them, draw the
/// screen from [`phase`](Self::phase).
///
/// See the [crate documentation](crate) for an example and for what the
/// privacy reset covers.
pub struct KioskFlow<'a, const F: usize, const V: usize> {
    form: &'a Form<'a>,
    config: FlowConfig,
    phase: Phase,
    /// One slot per field of the form, in form order.
    slots: BoundedVec<Slot<V>, F>,
    /// Differs after every restart of the application; tickets carry it.
    epoch: u32,
    session: u32,
    /// Submission attempts made in this session.
    attempts: u8,
    /// Whether the service rejected the answers as they stand: set by a
    /// [`FailureReason::Rejected`] result, cleared when an answer changes.
    rejected: bool,
    /// The latest tick seen. An earlier one is a clock fault.
    clock: Tick,
    /// When the current session started: its maximum length runs from here.
    session_since: Tick,
    /// When the current phase began.
    phase_since: Tick,
    /// Idle time is measured from here: the person's last action, or the
    /// start of the phase if that came later.
    idle_since: Tick,
    /// Whether the idle warning has been given in this idle period.
    warned: bool,
}

impl<'a, const F: usize, const V: usize> KioskFlow<'a, F, V> {
    /// A flow for `form`, waiting at [`Phase::Welcome`].
    ///
    /// `epoch` must differ each time the application starts, so that a
    /// result for a submission made before a restart is never applied to a
    /// session after it (see [`SubmissionTicket`]). It is **the backend's
    /// count of check-ins**: at each start
    /// the application checks in, and an approved kiosk is given the
    /// backend's next count, committed to the backend's storage before it
    /// is given, so no start - after a reboot or a restart of the backend -
    /// ever gets one again; `showcase_kiosk_contract::AppStart` makes the
    /// flow only then. It is never 0, and not secret. A demonstration whose
    /// service is a stand-in inside the application, restarting with it,
    /// may keep a fixed epoch.
    ///
    /// # Errors
    ///
    /// A [`SetupError`] if the form or settings cannot run: see
    /// [`Form::check`] and [`FlowConfig::check`].
    pub fn new(form: &'a Form<'a>, config: FlowConfig, epoch: u32) -> Result<Self, SetupError> {
        form.check(F, V)?;
        config.check()?;
        let mut slots = BoundedVec::new();
        for field in form.fields() {
            slots
                .push(Slot::for_kind(field.kind))
                .map_err(|_| SetupError::TooManyFields { capacity: F })?;
        }
        Ok(Self {
            form,
            config,
            phase: Phase::Welcome,
            slots,
            epoch,
            session: 0,
            attempts: 0,
            rejected: false,
            clock: 0,
            session_since: 0,
            phase_since: 0,
            idle_since: 0,
            warned: false,
        })
    }

    // ---- Reading the state ---------------------------------------------

    /// Where the flow is.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// The form being filled in.
    #[must_use]
    pub const fn form(&self) -> &'a Form<'a> {
        self.form
    }

    /// The settings.
    #[must_use]
    pub const fn config(&self) -> &FlowConfig {
        &self.config
    }

    /// The step on screen, in [`Phase::Step`].
    #[must_use]
    pub fn current_step(&self) -> Option<&'a Step<'a>> {
        match self.phase {
            Phase::Step { index } => self.form.steps.get(index),
            _ => None,
        }
    }

    /// The epoch given to [`new`](Self::new). Submission tickets carry it.
    #[must_use]
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// The session number: 0 before the first [`start`](Self::start), then
    /// one more for each session (wrapping after `u32::MAX`). Submission
    /// tickets carry it.
    #[must_use]
    pub const fn session(&self) -> u32 {
        self.session
    }

    /// Submission attempts made in this session.
    #[must_use]
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    /// Submission attempts this session has left.
    #[must_use]
    pub const fn attempts_left(&self) -> u8 {
        self.config.max_attempts.saturating_sub(self.attempts)
    }

    /// A field's current value, or `None` if the form has no such field.
    #[must_use]
    pub fn value(&self, field: FieldId) -> Option<Value<'_>> {
        let found = self.locate(field)?;
        self.slots.get(found.slot)?.view()
    }

    /// Every field's current value with its id, in form order. This is what
    /// the application sends to the service while [`Phase::Submitting`].
    pub fn values(&self) -> impl Iterator<Item = (FieldId, Value<'_>)> {
        self.form
            .fields()
            .zip(self.slots.iter())
            .filter_map(|(field, slot)| Some((field.id, slot.view()?)))
    }

    /// Whether any answer is held. Always `false` in [`Phase::Welcome`] and
    /// [`Phase::Submitted`].
    #[must_use]
    pub fn has_entered_data(&self) -> bool {
        self.slots.iter().any(|slot| !slot.is_empty())
    }

    /// Every field on the current step that stops [`next`](Self::next), in
    /// order. Empty outside [`Phase::Step`].
    pub fn problems(&self) -> impl Iterator<Item = FieldProblem> {
        let current = match self.phase {
            Phase::Step { index } => Some(index),
            _ => None,
        };
        self.located()
            .filter(move |found| Some(found.step) == current)
            .filter_map(|found| self.problem_at(found))
    }

    /// When the session will be wiped if nobody touches the kiosk, in the
    /// phases the idle timeout applies to: one idle timeout after the last
    /// action, or at the session's maximum length if that comes first.
    #[must_use]
    pub const fn idle_deadline(&self) -> Option<Tick> {
        if self.phase.awaits_person() {
            Some(self.reset_at())
        } else {
            None
        }
    }

    /// When the session will be wiped however active the person is
    /// ([`Timeouts::max_session`] after [`start`](Self::start)), in the
    /// phases that hold answers: [`Phase::Step`], [`Phase::Review`],
    /// [`Phase::Submitting`] and [`Phase::Failed`].
    #[must_use]
    pub const fn session_deadline(&self) -> Option<Tick> {
        if self.phase.holds_answers() {
            Some(self.session_deadline_at())
        } else {
            None
        }
    }

    /// While the "are you still there?" warning is showing: when the session
    /// will be wiped. Any action by the person withdraws the warning.
    #[must_use]
    pub const fn idle_warning(&self) -> Option<Tick> {
        if self.warned && self.phase.awaits_person() {
            Some(self.reset_at())
        } else {
            None
        }
    }

    /// The next tick at which [`tick`](Self::tick) would do something, or
    /// `None` if nothing is timed (at [`Phase::Welcome`]). An event-driven
    /// caller sets a timer for this instead of ticking continuously.
    ///
    /// A late call does no harm to privacy: every call first applies
    /// whatever fell due, as of when it was due (see the
    /// [crate documentation](crate)).
    #[must_use]
    pub const fn next_deadline(&self) -> Option<Tick> {
        let due = match self.phase {
            Phase::Welcome => return None,
            Phase::Submitted => {
                return Some(
                    self.phase_since
                        .saturating_add(self.config.timeouts.confirmation),
                );
            }
            Phase::Step { .. } | Phase::Review | Phase::Failed { .. } => {
                if !self.warned && self.config.timeouts.idle_warning > 0 {
                    self.warning_at()
                } else {
                    self.idle_deadline_at()
                }
            }
            Phase::Submitting { .. } => self.submit_deadline_at(),
        };
        // The session's maximum length applies too, whichever comes first.
        let limit = self.session_deadline_at();
        Some(if limit < due { limit } else { due })
    }

    // ---- What the person does -------------------------------------------

    /// Starts a new session on the first step, from [`Phase::Welcome`].
    ///
    /// # Errors
    ///
    /// [`FlowError::NotAllowed`] in any other phase; [`FlowError::Expired`]
    /// if an earlier session was still open but had ended (it has now been
    /// wiped, so starting again works).
    pub fn start(&mut self, now: Tick) -> Result<(), FlowError> {
        let now = self.begin(now)?;
        if self.phase != Phase::Welcome {
            return Err(FlowError::NotAllowed(self.phase));
        }
        self.session = self.session.wrapping_add(1);
        self.attempts = 0;
        self.rejected = false;
        // Already empty at Welcome; cleared again so a session can only
        // ever begin empty.
        self.clear_answers();
        self.session_since = now;
        self.enter(Phase::Step { index: 0 }, now);
        Ok(())
    }

    /// Sets a text field on the current step, replacing its value. Empty
    /// text clears it.
    ///
    /// # Errors
    ///
    /// [`FlowError::InvalidValue`] if the text breaks the field's rule (the
    /// field keeps its old value); [`FlowError::NotAllowed`] outside
    /// [`Phase::Step`]; [`FlowError::UnknownField`],
    /// [`FlowError::NotOnThisStep`] or [`FlowError::WrongKind`] for the wrong
    /// field; [`FlowError::Expired`] if the session had already timed out.
    pub fn set_text(&mut self, field: FieldId, text: &str, now: Tick) -> Result<(), FlowError> {
        self.begin(now)?;
        let found = self.editable(field)?;
        let FieldKind::Text(rule) = found.field.kind else {
            return Err(FlowError::WrongKind(field));
        };
        let refused = |issue| FlowError::InvalidValue(FieldProblem { field, issue });
        if let Some(issue) = rule.entry_issue(text) {
            return Err(refused(issue));
        }
        match self.slots.get_mut(found.slot) {
            Some(Slot::Text(value)) => {
                // Checked before clearing, so a refusal keeps the old value.
                if text.len() > value.capacity() {
                    return Err(refused(Issue::TooLong));
                }
                if value.as_str() != text {
                    self.rejected = false;
                }
                value.clear();
                value.push_str(text).map_err(|_| refused(Issue::TooLong))
            }
            _ => Err(FlowError::WrongKind(field)),
        }
    }

    /// Chooses option `option` (from 0) for a choice field on the current
    /// step.
    ///
    /// # Errors
    ///
    /// As [`set_text`](Self::set_text); [`FlowError::InvalidValue`] with
    /// [`Issue::NoSuchOption`] if the field has no such option.
    pub fn select(&mut self, field: FieldId, option: u8, now: Tick) -> Result<(), FlowError> {
        self.begin(now)?;
        let found = self.editable(field)?;
        let FieldKind::Choice { options } = found.field.kind else {
            return Err(FlowError::WrongKind(field));
        };
        if option >= options {
            return Err(FlowError::InvalidValue(FieldProblem {
                field,
                issue: Issue::NoSuchOption,
            }));
        }
        match self.slots.get_mut(found.slot) {
            Some(Slot::Choice(choice)) => {
                if *choice != Some(option) {
                    self.rejected = false;
                }
                *choice = Some(option);
                Ok(())
            }
            _ => Err(FlowError::WrongKind(field)),
        }
    }

    /// Checks or unchecks a checkbox on the current step.
    ///
    /// # Errors
    ///
    /// As [`set_text`](Self::set_text).
    pub fn set_checked(
        &mut self,
        field: FieldId,
        checked: bool,
        now: Tick,
    ) -> Result<(), FlowError> {
        self.begin(now)?;
        let found = self.editable(field)?;
        if found.field.kind != FieldKind::Checkbox {
            return Err(FlowError::WrongKind(field));
        }
        match self.slots.get_mut(found.slot) {
            Some(Slot::Checkbox(value)) => {
                if *value != checked {
                    self.rejected = false;
                }
                *value = checked;
                Ok(())
            }
            _ => Err(FlowError::WrongKind(field)),
        }
    }

    /// Clears one field on the current step, whatever its kind.
    ///
    /// # Errors
    ///
    /// As [`set_text`](Self::set_text), apart from the value checks.
    pub fn clear_field(&mut self, field: FieldId, now: Tick) -> Result<(), FlowError> {
        self.begin(now)?;
        let found = self.editable(field)?;
        let slot = self
            .slots
            .get_mut(found.slot)
            .ok_or(FlowError::UnknownField(field))?;
        if !slot.is_empty() {
            self.rejected = false;
        }
        slot.clear();
        Ok(())
    }

    /// Moves on from a complete step: to the next step, or after the last
    /// one to [`Phase::Review`].
    ///
    /// # Errors
    ///
    /// [`FlowError::Incomplete`] naming the first field that needs attention
    /// ([`problems`](Self::problems) lists them all);
    /// [`FlowError::NotAllowed`] outside [`Phase::Step`];
    /// [`FlowError::Expired`].
    pub fn next(&mut self, now: Tick) -> Result<(), FlowError> {
        let now = self.begin(now)?;
        let Phase::Step { index } = self.phase else {
            return Err(FlowError::NotAllowed(self.phase));
        };
        if let Some(problem) = self.problems().next() {
            return Err(FlowError::Incomplete(problem));
        }
        let following = index.saturating_add(1);
        if following < self.form.steps.len() {
            self.enter(Phase::Step { index: following }, now);
        } else {
            self.enter(Phase::Review, now);
        }
        Ok(())
    }

    /// Goes back one screen, keeping every answer: from a step to the one
    /// before it, from [`Phase::Review`] to the last step, and from
    /// [`Phase::Failed`] to [`Phase::Review`] so the answers can be
    /// corrected.
    ///
    /// # Errors
    ///
    /// [`FlowError::AtFirstStep`] on the first step;
    /// [`FlowError::NotAllowed`] in other phases; [`FlowError::Expired`].
    pub fn back(&mut self, now: Tick) -> Result<(), FlowError> {
        let now = self.begin(now)?;
        match self.phase {
            Phase::Step { index } => {
                let previous = index.checked_sub(1).ok_or(FlowError::AtFirstStep)?;
                self.enter(Phase::Step { index: previous }, now);
            }
            Phase::Review => {
                let last = self.form.steps.len().saturating_sub(1);
                self.enter(Phase::Step { index: last }, now);
            }
            Phase::Failed { .. } => self.enter(Phase::Review, now),
            other => return Err(FlowError::NotAllowed(other)),
        }
        Ok(())
    }

    /// Sends the answers: from [`Phase::Review`] to [`Phase::Submitting`].
    ///
    /// Hand the returned ticket to whatever talks to the service, with the
    /// answers from [`values`](Self::values), and quote it back in
    /// [`submission_succeeded`](Self::submission_succeeded) or
    /// [`submission_failed`](Self::submission_failed).
    ///
    /// # Errors
    ///
    /// [`FlowError::NoAttemptsLeft`] once this session has used every
    /// attempt; [`FlowError::Incomplete`] if any step is incomplete (checked
    /// again here, whatever happened before); [`FlowError::NotRetryable`]
    /// if the service rejected these answers and none has changed since;
    /// [`FlowError::NotAllowed`] outside [`Phase::Review`];
    /// [`FlowError::Expired`].
    pub fn submit(&mut self, now: Tick) -> Result<SubmissionTicket, FlowError> {
        let now = self.begin(now)?;
        if self.phase != Phase::Review {
            return Err(FlowError::NotAllowed(self.phase));
        }
        if let Some(problem) = self.located().find_map(|found| self.problem_at(found)) {
            return Err(FlowError::Incomplete(problem));
        }
        if self.rejected {
            return Err(FlowError::NotRetryable);
        }
        self.begin_attempt(now)
    }

    /// Sends the same answers again after a failure: from [`Phase::Failed`]
    /// to [`Phase::Submitting`], with a new ticket.
    ///
    /// # Errors
    ///
    /// [`FlowError::NotRetryable`] after [`FailureReason::Rejected`];
    /// [`FlowError::NoAttemptsLeft`]; [`FlowError::NotAllowed`] outside
    /// [`Phase::Failed`]; [`FlowError::Expired`].
    pub fn retry(&mut self, now: Tick) -> Result<SubmissionTicket, FlowError> {
        let now = self.begin(now)?;
        let Phase::Failed { reason, .. } = self.phase else {
            return Err(FlowError::NotAllowed(self.phase));
        };
        if !reason.is_retryable() {
            return Err(FlowError::NotRetryable);
        }
        self.begin_attempt(now)
    }

    /// Abandons the session: clears every answer and returns to
    /// [`Phase::Welcome`]. Does nothing at [`Phase::Welcome`], or if the
    /// session had already expired (it has been wiped either way).
    ///
    /// # Errors
    ///
    /// [`FlowError::NotAllowed`] while [`Phase::Submitting`], because the
    /// answers may already have reached the service (wait for the result or
    /// the submit timeout; once that has passed, cancel works even if
    /// [`tick`](Self::tick) has not noticed it), and at [`Phase::Submitted`]
    /// (use [`finish`](Self::finish)).
    pub fn cancel(&mut self, now: Tick) -> Result<(), FlowError> {
        let now = match self.begin(now) {
            Ok(now) => now,
            Err(FlowError::Expired) => return Ok(()),
            Err(other) => return Err(other),
        };
        match self.phase {
            Phase::Welcome => Ok(()),
            Phase::Step { .. } | Phase::Review | Phase::Failed { .. } => {
                self.wipe(now);
                Ok(())
            }
            other => Err(FlowError::NotAllowed(other)),
        }
    }

    /// Ends the confirmation early and returns to [`Phase::Welcome`]. Does
    /// nothing at [`Phase::Welcome`].
    ///
    /// # Errors
    ///
    /// [`FlowError::NotAllowed`] in other phases.
    pub fn finish(&mut self, now: Tick) -> Result<(), FlowError> {
        let now = self.begin(now)?;
        match self.phase {
            Phase::Welcome => Ok(()),
            Phase::Submitted => {
                self.wipe(now);
                Ok(())
            }
            other => Err(FlowError::NotAllowed(other)),
        }
    }

    /// Ends the session at once, whatever the phase, for a reason outside
    /// the flow: a malformed or unexpected message from the UI, say, which
    /// the application treats as a fault and fails closed on. Clears every
    /// answer and returns to [`Phase::Welcome`], from any phase. Unlike
    /// [`cancel`](Self::cancel) it also ends a submission in flight (its
    /// result is then refused with [`FlowError::StaleTicket`]) and a
    /// confirmation. Does nothing at [`Phase::Welcome`].
    pub fn end_session(&mut self, now: Tick) {
        let (now, _) = self.catch_up(now, false);
        if self.phase != Phase::Welcome {
            self.wipe(now);
        }
    }

    /// Records that the person is still there without changing anything,
    /// for input the flow does not otherwise see: scrolling, moving focus,
    /// typing before a value is committed, or "I'm still here" on the idle
    /// warning. Restarts the idle timer and withdraws the warning.
    ///
    /// # Errors
    ///
    /// [`FlowError::Expired`] if the session had already timed out.
    pub fn note_activity(&mut self, now: Tick) -> Result<(), FlowError> {
        self.begin(now).map(|_| ())
    }

    // ---- What the service answers ----------------------------------------

    /// The service accepted the submission with this ticket. The answers
    /// are cleared at once, since the kiosk no longer needs them, and the
    /// flow shows [`Phase::Submitted`] for [`Timeouts::confirmation`] ticks.
    ///
    /// A result counts if the flow is still submitting that attempt, even
    /// when the submit timeout has passed, until a call notices the timeout
    /// ([`tick`](Self::tick), or any action by the person): telling the
    /// person their answers were accepted beats inviting them to send them
    /// twice. That grace ends one idle timeout after the submit deadline,
    /// when on-time ticks would have wiped the session.
    ///
    /// # Errors
    ///
    /// [`FlowError::StaleTicket`] if the ticket is not the submission in
    /// flight; nothing changes. [`FlowError::Expired`] if the session had
    /// already ended by `now` (see [`FlowError::Expired`]): it has been
    /// wiped, and the result ignored.
    pub fn submission_succeeded(
        &mut self,
        ticket: SubmissionTicket,
        now: Tick,
    ) -> Result<(), FlowError> {
        let now = self.begin_result(now)?;
        self.ticket_in_flight(ticket)?;
        self.clear_answers();
        self.enter(Phase::Submitted, now);
        Ok(())
    }

    /// The submission with this ticket failed. The flow shows
    /// [`Phase::Failed`], keeping the answers for a retry or a correction;
    /// the idle timeout runs again from now, or from the submit deadline if
    /// the result came after it.
    ///
    /// # Errors
    ///
    /// As [`submission_succeeded`](Self::submission_succeeded).
    pub fn submission_failed(
        &mut self,
        ticket: SubmissionTicket,
        reason: FailureReason,
        now: Tick,
    ) -> Result<(), FlowError> {
        let now = self.begin_result(now)?;
        let attempt = self.ticket_in_flight(ticket)?;
        // A late failure dates from the deadline, as a timeout would have.
        let deadline = self.submit_deadline_at();
        let since = if now < deadline { now } else { deadline };
        self.enter(Phase::Failed { reason, attempt }, since);
        if !reason.is_retryable() {
            self.rejected = true;
        }
        Ok(())
    }

    // ---- Time --------------------------------------------------------------

    /// Lets time pass: gives the idle warning, wipes an idle session or one
    /// that reached its maximum length, times out a submission, or ends the
    /// confirmation, whichever is due. Call it at
    /// [`next_deadline`](Self::next_deadline), or regularly.
    ///
    /// Returns what happened, if anything. At most one event is reported per
    /// call. A late call applies everything that fell due in between, as of
    /// when it was due, and reports the outcome: a submission that timed out
    /// and was then left for a whole idle timeout is reported as
    /// [`TickEvent::IdleReset`], and a jump straight past both the warning
    /// and the reset skips the warning.
    #[must_use = "an event means the screen must change; when it ends the session, clear every copy of the answers"]
    pub fn tick(&mut self, now: Tick) -> Option<TickEvent> {
        let (now, caught_up) = self.catch_up(now, false);
        match caught_up {
            CaughtUp::IdleReset => return Some(TickEvent::IdleReset),
            CaughtUp::SessionLimit => return Some(TickEvent::SessionLimit),
            CaughtUp::SubmitTimedOut => return Some(TickEvent::SubmitTimedOut),
            CaughtUp::Nothing => {}
        }
        match self.phase {
            Phase::Welcome | Phase::Submitting { .. } => None,
            Phase::Step { .. } | Phase::Review | Phase::Failed { .. } => {
                let warn = !self.warned
                    && self.config.timeouts.idle_warning > 0
                    && now >= self.warning_at();
                if warn {
                    self.warned = true;
                    Some(TickEvent::IdleWarning {
                        reset_at: self.reset_at(),
                    })
                } else {
                    None
                }
            }
            Phase::Submitted => {
                let ends = self
                    .phase_since
                    .saturating_add(self.config.timeouts.confirmation);
                if now >= ends {
                    self.wipe(now);
                    Some(TickEvent::ConfirmationEnded)
                } else {
                    None
                }
            }
        }
    }

    // ---- Internals ---------------------------------------------------------

    /// When the idle timeout runs out, from the last action.
    const fn idle_deadline_at(&self) -> Tick {
        self.idle_since
            .saturating_add(self.config.timeouts.idle_reset)
    }

    /// When the session reaches its maximum length.
    const fn session_deadline_at(&self) -> Tick {
        self.session_since
            .saturating_add(self.config.timeouts.max_session)
    }

    /// When an idle session will be wiped: the idle deadline, or the
    /// session's maximum length if that comes first.
    const fn reset_at(&self) -> Tick {
        let idle = self.idle_deadline_at();
        let limit = self.session_deadline_at();
        if limit < idle { limit } else { idle }
    }

    /// When the submission in flight times out (meaningful while
    /// [`Phase::Submitting`]).
    const fn submit_deadline_at(&self) -> Tick {
        self.phase_since.saturating_add(self.config.timeouts.submit)
    }

    const fn warning_at(&self) -> Tick {
        let timeouts = &self.config.timeouts;
        self.idle_since
            .saturating_add(timeouts.idle_reset.saturating_sub(timeouts.idle_warning))
    }

    /// Brings the flow up to `now` before anything else happens. Whatever an
    /// on-time [`tick`](Self::tick) would already have done is done now, as
    /// of the moment it fell due, so late or missing ticks can never give a
    /// session more time:
    ///
    /// 1. a reading earlier than one already seen is a clock fault: any
    ///    session is wiped (fail closed) and the clock restarts from `now`;
    /// 2. an overdue submission times out at its deadline, not at `now`;
    /// 3. a session idle for the whole timeout, or at its maximum length, is
    ///    wiped.
    ///
    /// For a service result (`for_result`), an overdue submission stays in
    /// flight so that its result still counts, unless the idle timeout after
    /// its deadline has passed too.
    fn catch_up(&mut self, now: Tick, for_result: bool) -> (Tick, CaughtUp) {
        if now < self.clock {
            self.clock = now;
            if self.phase == Phase::Welcome {
                return (now, CaughtUp::Nothing);
            }
            self.wipe(now);
            return (now, CaughtUp::IdleReset);
        }
        self.clock = now;

        let mut caught_up = CaughtUp::Nothing;
        if let Phase::Submitting { attempt } = self.phase {
            let deadline = self.submit_deadline_at();
            let abandoned = now >= deadline.saturating_add(self.config.timeouts.idle_reset);
            if now >= deadline && (!for_result || abandoned) {
                let reason = FailureReason::TimedOut;
                self.enter(Phase::Failed { reason, attempt }, deadline);
                caught_up = CaughtUp::SubmitTimedOut;
            }
        }

        let idle_at = self.idle_deadline_at();
        let limit_at = self.session_deadline_at();
        let idle_due = self.phase.awaits_person() && now >= idle_at;
        let limit_due = self.phase.holds_answers() && now >= limit_at;
        // If both fell due, report the one that came first.
        if limit_due && !(idle_due && idle_at <= limit_at) {
            self.wipe(now);
            return (now, CaughtUp::SessionLimit);
        }
        if idle_due {
            self.wipe(now);
            return (now, CaughtUp::IdleReset);
        }
        (now, caught_up)
    }

    /// The start of everything the person does. The flow first catches up
    /// with the clock; if that ended the session, the action is refused, so
    /// it can never reach a session that has expired, however late `tick`
    /// was. Otherwise the action counts as activity.
    fn begin(&mut self, now: Tick) -> Result<Tick, FlowError> {
        let (now, caught_up) = self.catch_up(now, false);
        if caught_up.wiped() {
            return Err(FlowError::Expired);
        }
        if self.phase.awaits_person() {
            self.idle_since = now;
            self.warned = false;
        }
        Ok(now)
    }

    /// The start of every result from the service: as [`begin`](Self::begin),
    /// but a result is not activity, and an overdue submission stays in
    /// flight while its result can still count.
    fn begin_result(&mut self, now: Tick) -> Result<Tick, FlowError> {
        let (now, caught_up) = self.catch_up(now, true);
        if caught_up.wiped() {
            return Err(FlowError::Expired);
        }
        Ok(now)
    }

    fn enter(&mut self, phase: Phase, now: Tick) {
        self.phase = phase;
        self.phase_since = now;
        self.idle_since = now;
        self.warned = false;
    }

    /// Clears every answer, in place.
    fn clear_answers(&mut self) {
        for slot in self.slots.iter_mut() {
            slot.clear();
        }
    }

    /// Ends the session: clears every answer and returns to Welcome.
    fn wipe(&mut self, now: Tick) {
        self.clear_answers();
        self.attempts = 0;
        self.rejected = false;
        self.enter(Phase::Welcome, now);
    }

    fn begin_attempt(&mut self, now: Tick) -> Result<SubmissionTicket, FlowError> {
        if self.attempts >= self.config.max_attempts {
            return Err(FlowError::NoAttemptsLeft);
        }
        let attempt = self.attempts.saturating_add(1);
        self.attempts = attempt;
        self.enter(Phase::Submitting { attempt }, now);
        Ok(SubmissionTicket {
            epoch: self.epoch,
            session: self.session,
            attempt,
        })
    }

    /// The attempt number, if `ticket` is the submission in flight.
    fn ticket_in_flight(&self, ticket: SubmissionTicket) -> Result<u8, FlowError> {
        match self.phase {
            Phase::Submitting { attempt }
                if ticket.epoch == self.epoch
                    && ticket.session == self.session
                    && ticket.attempt == attempt =>
            {
                Ok(attempt)
            }
            _ => Err(FlowError::StaleTicket),
        }
    }

    /// Every field with its slot and step, in form order.
    fn located(&self) -> impl Iterator<Item = Located<'a>> {
        self.form
            .fields_by_step()
            .enumerate()
            .map(|(slot, (step, field))| Located { slot, step, field })
    }

    fn locate(&self, id: FieldId) -> Option<Located<'a>> {
        self.located().find(|found| found.field.id == id)
    }

    /// A field the person may edit now: on the step on screen.
    fn editable(&self, id: FieldId) -> Result<Located<'a>, FlowError> {
        let Phase::Step { index } = self.phase else {
            return Err(FlowError::NotAllowed(self.phase));
        };
        let found = self.locate(id).ok_or(FlowError::UnknownField(id))?;
        if found.step != index {
            return Err(FlowError::NotOnThisStep(id));
        }
        Ok(found)
    }

    /// What stops this field being complete, if anything.
    fn problem_at(&self, found: Located<'a>) -> Option<FieldProblem> {
        let field = found.field;
        let issue = match (field.kind, self.slots.get(found.slot)) {
            (FieldKind::Text(rule), Some(Slot::Text(text))) => {
                rule.completion_issue(text.as_str(), field.required)
            }
            (FieldKind::Choice { .. }, Some(Slot::Choice(choice))) => {
                (field.required && choice.is_none()).then_some(Issue::Missing)
            }
            (FieldKind::Checkbox, Some(Slot::Checkbox(checked))) => {
                (field.required && !*checked).then_some(Issue::Missing)
            }
            // Storage that does not match its field cannot arise (slots are
            // made from the fields). Should it ever, refuse to go on.
            _ => Some(Issue::Missing),
        }?;
        Some(FieldProblem {
            field: field.id,
            issue,
        })
    }
}

/// Shows the phase, session and how many fields hold a value, never the
/// values themselves.
impl<const F: usize, const V: usize> fmt::Debug for KioskFlow<'_, F, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filled = self.slots.iter().filter(|slot| !slot.is_empty()).count();
        f.debug_struct("KioskFlow")
            .field("phase", &self.phase)
            .field("epoch", &self.epoch)
            .field("session", &self.session)
            .field("attempts", &self.attempts)
            .field("fields", &self.slots.len())
            .field("fields_filled", &filled)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]
mod tests {
    use super::*;
    use crate::form::{Charset, StepId, TextRule};

    const A: FieldId = FieldId(1);
    const B: FieldId = FieldId(2);
    const C: FieldId = FieldId(3);
    const FIRST: &[Field] = &[Field::text(A, TextRule::new(Charset::Text, 0, 8))];
    const SECOND: &[Field] = &[
        Field::choice(B, 3),
        Field::text(C, TextRule::new(Charset::Text, 0, 8)),
    ];
    const STEPS: &[Step<'static>] = &[Step::new(StepId(1), FIRST), Step::new(StepId(2), SECOND)];
    static FORM: Form<'static> = Form::new(STEPS);
    const CONFIG: FlowConfig = FlowConfig {
        timeouts: Timeouts {
            idle_reset: 100,
            idle_warning: 0,
            submit: 10,
            confirmation: 10,
            max_session: 1_000,
        },
        max_attempts: 2,
    };
    const EPOCH: u32 = 7;

    fn new_flow() -> KioskFlow<'static, 4, 8> {
        KioskFlow::new(&FORM, CONFIG, EPOCH).unwrap()
    }

    /// Every text value keeps its storage, and every byte of it is zero -
    /// not merely empty (`BoundedStr::is_zeroed`).
    fn text_slots_are_zeroed(flow: &KioskFlow<'_, 4, 8>) -> bool {
        flow.slots.iter().all(|slot| match slot {
            Slot::Text(text) => text.is_zeroed() && text.capacity() == 8,
            _ => true,
        })
    }

    #[test]
    fn one_slot_per_field_of_the_matching_kind() {
        let flow = new_flow();
        assert_eq!(flow.slots.len(), 3);
        assert!(matches!(flow.slots.first(), Some(Slot::Text(_))));
        assert!(matches!(flow.slots.get(1), Some(Slot::Choice(None))));
        assert!(matches!(flow.slots.get(2), Some(Slot::Text(_))));
    }

    #[test]
    fn wiping_empties_every_slot_including_unvisited_steps() {
        let mut flow = new_flow();
        flow.start(0).unwrap();
        flow.set_text(A, "abc", 1).unwrap();
        // Fill a slot on a step the person has not reached, directly, to
        // prove the wipe does not depend on where they got to.
        if let Some(Slot::Text(text)) = flow.slots.get_mut(2) {
            text.push_str("xyz").unwrap();
        }
        assert!(!text_slots_are_zeroed(&flow));
        flow.wipe(2);
        assert!(text_slots_are_zeroed(&flow));
        assert!(!flow.has_entered_data());
        assert_eq!(flow.phase, Phase::Welcome);
    }

    proptest::proptest! {
        /// Whatever was typed and retyped, through the API or straight into
        /// a step the person has not reached, cancelling leaves every byte
        /// of every text value zero.
        #[test]
        fn cancelling_zeroes_every_text_value_whatever_was_typed(
            typed in proptest::collection::vec(("[ -~]{0,10}", proptest::bool::ANY), 1..12),
        ) {
            let mut flow = new_flow();
            flow.start(0).unwrap();
            let mut now: Tick = 0;
            for (text, unreached) in &typed {
                now = now.saturating_add(1);
                if *unreached {
                    if let Some(Slot::Text(value)) = flow.slots.get_mut(2) {
                        value.clear();
                        let _ = value.push_str(text);
                    }
                } else {
                    // Refused entries keep the old value; that is fine here.
                    let _ = flow.set_text(A, text, now);
                }
            }
            flow.cancel(now.saturating_add(1)).unwrap();
            proptest::prop_assert!(text_slots_are_zeroed(&flow));
            proptest::prop_assert!(!flow.has_entered_data());
        }
    }

    #[test]
    fn catching_up_fails_closed_when_the_clock_goes_backwards() {
        let mut flow = new_flow();
        // At Welcome there is nothing to wipe: the clock simply restarts.
        assert_eq!(flow.catch_up(50, false), (50, CaughtUp::Nothing));
        assert_eq!(flow.catch_up(10, false), (10, CaughtUp::Nothing));
        assert_eq!(flow.clock, 10);
        // With a session, an earlier reading wipes it.
        flow.start(20).unwrap();
        flow.set_text(A, "abc", 21).unwrap();
        assert_eq!(flow.catch_up(21, false), (21, CaughtUp::Nothing));
        assert_eq!(flow.catch_up(20, false), (20, CaughtUp::IdleReset));
        assert_eq!(flow.phase, Phase::Welcome);
        assert!(text_slots_are_zeroed(&flow));
        // Extreme readings saturate rather than wrap.
        assert_eq!(
            flow.catch_up(u64::MAX, false),
            (u64::MAX, CaughtUp::Nothing)
        );
        assert_eq!(flow.catch_up(0, false), (0, CaughtUp::Nothing));
    }

    #[test]
    fn catching_up_dates_a_late_submit_timeout_from_its_deadline() {
        let mut flow = new_flow();
        flow.start(0).unwrap();
        flow.set_text(A, "abc", 1).unwrap();
        flow.next(2).unwrap();
        flow.next(3).unwrap();
        flow.submit(4).unwrap();
        // For a result, the submission stays in flight past its deadline...
        assert_eq!(flow.catch_up(50, true), (50, CaughtUp::Nothing));
        assert_eq!(flow.phase, Phase::Submitting { attempt: 1 });
        // ...but not for anything else, and the failure dates from 4 + 10.
        assert_eq!(flow.catch_up(60, false), (60, CaughtUp::SubmitTimedOut));
        assert_eq!(flow.phase_since, 14);
        assert_eq!(flow.idle_since, 14);
        // One idle timeout after the deadline, the session is wiped.
        assert_eq!(flow.catch_up(114, true), (114, CaughtUp::IdleReset));
        assert_eq!(flow.phase, Phase::Welcome);
    }

    #[test]
    fn catching_up_reports_whichever_end_came_first() {
        // Active until 950, maximum length at 1_000: the limit came first.
        let mut flow = new_flow();
        flow.start(0).unwrap();
        for t in (50..=950).step_by(50) {
            flow.note_activity(t).unwrap();
        }
        assert_eq!(flow.catch_up(5_000, false), (5_000, CaughtUp::SessionLimit));
        // Idle since 10, so idle at 110, long before the limit.
        let mut flow = new_flow();
        flow.start(0).unwrap();
        flow.note_activity(10).unwrap();
        assert_eq!(flow.catch_up(5_000, false), (5_000, CaughtUp::IdleReset));
    }

    #[test]
    fn bad_settings_are_refused() {
        let mut config = CONFIG;
        config.max_attempts = 0;
        assert_eq!(config.check(), Err(SetupError::NoAttempts));
        for timeouts in [
            Timeouts {
                idle_reset: 0,
                ..CONFIG.timeouts
            },
            Timeouts {
                idle_warning: 100,
                ..CONFIG.timeouts
            },
            Timeouts {
                submit: 0,
                ..CONFIG.timeouts
            },
            Timeouts {
                confirmation: 0,
                ..CONFIG.timeouts
            },
            Timeouts {
                max_session: 0,
                ..CONFIG.timeouts
            },
            // Neither timeout may reach the maximum session length.
            Timeouts {
                max_session: 100,
                ..CONFIG.timeouts
            },
            Timeouts {
                submit: 1_000,
                ..CONFIG.timeouts
            },
        ] {
            let config = FlowConfig { timeouts, ..CONFIG };
            assert_eq!(config.check(), Err(SetupError::BadTimeouts));
            assert_eq!(
                KioskFlow::<'_, 4, 8>::new(&FORM, config, EPOCH).err(),
                Some(SetupError::BadTimeouts)
            );
        }
        assert_eq!(
            KioskFlow::<'_, 2, 8>::new(&FORM, CONFIG, EPOCH).err(),
            Some(SetupError::TooManyFields { capacity: 2 })
        );
    }

    #[test]
    fn events_that_end_the_session() {
        assert!(TickEvent::IdleReset.ends_session());
        assert!(TickEvent::SessionLimit.ends_session());
        assert!(TickEvent::ConfirmationEnded.ends_session());
        assert!(!TickEvent::SubmitTimedOut.ends_session());
        assert!(!TickEvent::IdleWarning { reset_at: 1 }.ends_session());
    }
}
