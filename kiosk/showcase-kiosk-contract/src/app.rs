// SPDX-License-Identifier: MIT OR Apache-2.0
//! [`AppEnd`]: the application's end of the contract. It drives a
//! [`KioskFlow`] with the UI's actions and the service's results, and says
//! what to report to the UI after each call.

use core::fmt;

use nowayos_bounded::BoundedVec;
use showcase_kiosk_flow::{
    FailureReason, FieldKind, FlowError, KioskFlow, Phase, SubmissionTicket, Tick, TickEvent,
};

use crate::{
    Action, ContractError, Countdown, EndReason, IdleWarning, Issued, MAX_TEXT_LEN, OutOfService,
    OutOfServiceReason, Refusal, Report, SessionEnded, SetupError, ShowFailed, ShowReview,
    ShowStep, ShowSubmitted, ShowSubmitting, ShowWelcome, decode_action,
};

/// The most reports one call can produce: a [`SessionEnded`], the answer
/// to an action, and the screen to show - or a [`SessionEnded`], the
/// confirmation, what the service issued ([`Issued`]) and the
/// confirmation's first [`Countdown`].
pub const MAX_REPLIES: usize = 4;

/// A second, in the kiosk's ticks (milliseconds): how often a screen that
/// counts down is told its time left ([`Countdown`]).
pub const SECOND: Tick = 1_000;

/// What the application must do after a call to [`AppEnd`]: send
/// [`reports`](Self::reports) to the UI, in order, and send the answers to
/// the service if a [`submission`](Self::submission) began.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "the reports must reach the UI: one may end a session"]
pub struct Replies {
    reports: BoundedVec<Report, MAX_REPLIES>,
    submission: Option<SubmissionTicket>,
    pub(crate) refused: Option<FlowError>,
    simulate_failure: bool,
}

impl Replies {
    pub(crate) fn new() -> Self {
        Self {
            reports: BoundedVec::new(),
            submission: None,
            refused: None,
            simulate_failure: false,
        }
    }

    /// The reports to send to the UI, in order. A [`SessionEnded`] always
    /// comes before the screen that follows it.
    #[must_use]
    pub fn reports(&self) -> &[Report] {
        self.reports.as_slice()
    }

    /// The session-ended report among them, if the call ended a session.
    #[must_use]
    pub fn session_ended(&self) -> Option<SessionEnded> {
        self.reports.iter().find_map(|report| match report {
            Report::SessionEnded(ended) => Some(*ended),
            _ => None,
        })
    }

    /// A submission began: send the answers
    /// ([`KioskFlow::values`]) to the service with this ticket, and give its
    /// result to [`AppEnd::submission_succeeded`] or
    /// [`AppEnd::submission_failed`].
    #[must_use]
    pub const fn submission(&self) -> Option<SubmissionTicket> {
        self.submission
    }

    /// The person asked a demonstration to fail this session's
    /// submissions ([`SimulateFailure`](crate::SimulateFailure)), in a
    /// session that is open: a demonstration's application tells its
    /// stand-in for the service; a product's ignores it.
    #[must_use]
    pub const fn simulate_failure(&self) -> bool {
        self.simulate_failure
    }

    /// What the flow refused, if anything, for the application's own
    /// records: an action (also reported to the UI) or a service result.
    /// Never contains entered text.
    #[must_use]
    pub const fn refused(&self) -> Option<FlowError> {
        self.refused
    }

    /// Adds a report. A call makes at most [`MAX_REPLIES`], so there is
    /// always room (the tests check every path).
    pub(crate) fn push(&mut self, report: Report) {
        // Cannot fail: see above. Were it ever to, the reports that matter
        // most come first: the end of a session before anything else.
        let _ = self.reports.push(report);
    }
}

/// A record from the UI that the application refused: malformed, of an
/// unknown kind or the wrong version. The application has ended the
/// session, if one was open (fail closed): send [`replies`](Self::replies)
/// to the UI, and record [`error`](Self::error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faulted {
    /// Why the record was refused. Never contains entered text.
    pub error: ContractError,
    /// What to send to the UI: the end of the session and the welcome
    /// screen, if a session was open.
    pub replies: Replies,
}

impl fmt::Display for Faulted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "record from the UI refused, and any open session ended: {}",
            self.error
        )
    }
}

impl core::error::Error for Faulted {}

/// What the UI is showing, as far as the application knows: changes are
/// reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Screen {
    phase: Phase,
    warning: bool,
}

/// The application protection domain's end of the contract.
///
/// It owns the [`KioskFlow`], takes every action from the UI through
/// [`receive`](Self::receive), every timer through [`tick`](Self::tick) and
/// every service result through
/// [`submission_succeeded`](Self::submission_succeeded) and
/// [`submission_failed`](Self::submission_failed), and returns the
/// [`Replies`] for each. Whatever ends a session, the replies carry exactly
/// one [`SessionEnded`] for it, naming the reason and nothing about the
/// answers.
///
/// ```
/// use showcase_kiosk_contract::{
///     Action, AppEnd, EndReason, Report, SessionEnded, SetText, ShowStep, ShowWelcome, Start,
///     decode_report, MAX_RECORD_LEN,
/// };
/// use showcase_kiosk_flow::{
///     Charset, Field, FieldId, FlowConfig, Form, KioskFlow, Step, StepId, TextRule, Timeouts,
/// };
///
/// const NAME: FieldId = FieldId(1);
/// static FORM: Form<'static> = Form::new(&[Step::new(
///     StepId(1),
///     &[Field::text(NAME, TextRule::new(Charset::Text, 1, 32)).required()],
/// )]);
/// const CONFIG: FlowConfig = FlowConfig {
///     timeouts: Timeouts {
///         idle_reset: 60_000,
///         idle_warning: 0,
///         submit: 10_000,
///         confirmation: 8_000,
///         max_session: 20 * 60_000,
///     },
///     max_attempts: 3,
/// };
/// let flow: KioskFlow<'_, 4, 32> = KioskFlow::new(&FORM, CONFIG, 1).unwrap();
/// let mut app = AppEnd::new(flow).unwrap();
///
/// // The UI's records arrive through the ring, copied into private memory.
/// let mut slot = [0u8; MAX_RECORD_LEN];
/// Action::Start(Start { seq: 1 }).encode(&mut slot).unwrap();
/// let replies = app.receive(&slot, 0).unwrap();
/// assert_eq!(replies.reports(), &[Report::ShowStep(ShowStep { index: 0 })]);
///
/// let typed = SetText::new(2, NAME, "Sam Sample").unwrap();
/// Action::SetText(typed).encode(&mut slot).unwrap();
/// assert!(app.receive(&slot, 1_000).unwrap().reports().is_empty());
///
/// // A minute later, nobody has touched it: one session-ended report, then
/// // the welcome screen. Neither carries the answer.
/// let replies = app.tick(61_000);
/// assert_eq!(
///     replies.reports(),
///     &[
///         Report::SessionEnded(SessionEnded { session: 1, reason: EndReason::Idle }),
///         Report::ShowWelcome(ShowWelcome),
///     ]
/// );
/// assert!(!app.flow().has_entered_data());
///
/// // The UI decodes each report and must wipe its copies on the first.
/// replies.reports()[0].encode(&mut slot).unwrap();
/// assert!(matches!(decode_report(&slot), Ok(Report::SessionEnded(_))));
/// ```
#[derive(Debug)]
pub struct AppEnd<'a, const F: usize, const V: usize> {
    flow: KioskFlow<'a, F, V>,
    /// A session is open: it started, and its end has not been reported.
    open: bool,
    /// The screen counting down, and the whole seconds left last reported
    /// for it.
    counted: Option<(Screen, Tick)>,
    /// Out of service, and why: no session opens.
    stopped: Option<OutOfServiceReason>,
    /// Asked to stop while a session was open or its confirmation showed:
    /// it stops once the welcome screen would show.
    stopping: Option<OutOfServiceReason>,
}

impl<'a, const F: usize, const V: usize> AppEnd<'a, F, V> {
    /// The application's end, driving `flow`.
    ///
    /// # Errors
    ///
    /// [`SetupError`] if the form has more steps than the contract can
    /// number, or a text field longer than a [`SetText`](crate::SetText)
    /// carries.
    pub fn new(flow: KioskFlow<'a, F, V>) -> Result<Self, SetupError> {
        let form = flow.form();
        if form.steps.len() > usize::from(u16::MAX) {
            return Err(SetupError::TooManySteps {
                steps: form.steps.len(),
            });
        }
        for field in form.fields() {
            if let FieldKind::Text(rule) = field.kind
                && usize::from(rule.max_len) > MAX_TEXT_LEN
            {
                return Err(SetupError::TextTooLong {
                    field: field.id,
                    max_len: rule.max_len,
                });
            }
        }
        let open = flow.phase().holds_answers();
        Ok(Self {
            flow,
            open,
            counted: None,
            stopped: None,
            stopping: None,
        })
    }

    /// Stops opening sessions, for `reason`: the kiosk is not yet
    /// approved, cannot reach its backend, or is being updated. With no
    /// session open and the welcome screen showing, the UI is told at once
    /// with [`OutOfService`], and nothing else. A session already open, or
    /// its confirmation, runs to its own end first - never cut short - and
    /// its [`SessionEnded`] is followed by [`OutOfService`] in place of the
    /// welcome screen. Called again, the reason changes. Until
    /// [`resume_service`](Self::resume_service), a start is refused with
    /// [`FlowError::NotAllowed`].
    pub fn stop_service(&mut self, reason: OutOfServiceReason) -> Replies {
        let mut replies = Replies::new();
        if self.flow.phase() == Phase::Welcome {
            self.stopping = None;
            self.stopped = Some(reason);
            replies.push(Report::OutOfService(OutOfService { reason }));
        } else {
            self.stopping = Some(reason);
        }
        replies
    }

    /// Opens sessions again: out of service, the UI is shown the welcome
    /// screen; a stop still waiting for a session's end is withdrawn.
    pub fn resume_service(&mut self) -> Replies {
        let mut replies = Replies::new();
        self.stopping = None;
        if self.stopped.take().is_some() {
            replies.push(Report::ShowWelcome(ShowWelcome));
        }
        replies
    }

    /// Why the kiosk is out of service, if it is.
    #[must_use]
    pub const fn out_of_service(&self) -> Option<OutOfServiceReason> {
        self.stopped
    }

    /// The flow, to read: the answers to send with a submission
    /// ([`KioskFlow::values`]).
    #[must_use]
    pub const fn flow(&self) -> &KioskFlow<'a, F, V> {
        &self.flow
    }

    /// When the screen showing ends by itself, if it counts down:
    /// the privacy reset while the idle warning shows, or the end of the
    /// confirmation.
    #[must_use]
    pub const fn countdown_deadline(&self) -> Option<Tick> {
        match self.flow.phase() {
            Phase::Submitted => self.flow.next_deadline(),
            _ => self.flow.idle_warning(),
        }
    }

    /// When to call [`tick`](Self::tick) next: the flow's next deadline
    /// ([`KioskFlow::next_deadline`]), or, while a screen counts down, the
    /// moment its whole seconds left go down by one - whichever comes
    /// first. `None` if nothing is timed.
    #[must_use]
    pub fn next_tick(&self) -> Option<Tick> {
        let step = self
            .countdown_deadline()
            .map(|deadline| match self.counted {
                // The seconds left go down when the time left reaches one
                // second fewer than were shown.
                Some((screen, shown)) if screen == self.screen() => {
                    deadline.saturating_sub(shown.saturating_sub(1).saturating_mul(SECOND))
                }
                // Not reported yet: at once.
                _ => 0,
            });
        [self.flow.next_deadline(), step]
            .into_iter()
            .flatten()
            .min()
    }

    /// Takes one record from the UI, already copied out of the ring, at
    /// tick `now`.
    ///
    /// # Errors
    ///
    /// [`Faulted`] if the record is malformed, of an unknown kind or the
    /// wrong version: the application treats the UI as faulty or hostile,
    /// ends the session if one is open, and the error carries the replies
    /// that say so. An action the flow refuses is not an error here: it is
    /// reported to the UI ([`Refusal::report`]).
    pub fn receive(&mut self, record: &[u8], now: Tick) -> Result<Replies, Faulted> {
        match decode_action(record) {
            Ok(action) => Ok(self.apply(&action, now)),
            Err(error) => Err(Faulted {
                error,
                replies: self.end_session(now),
            }),
        }
    }

    /// Applies one decoded action at tick `now`.
    ///
    /// Whatever fell due before it is applied first, so an action never
    /// reaches a session that has ended, and the session's end is reported
    /// with its true reason. Then the action: a refusal is answered with
    /// [`ValueRefused`](crate::ValueRefused),
    /// [`Incomplete`](crate::Incomplete) or [`Refused`](crate::Refused),
    /// quoting the action's `seq`; a refused value also clears the field,
    /// so the application's value and the UI's copy stay the same. Last,
    /// the screen, if it changed or the UI may be out of step.
    pub fn apply(&mut self, action: &Action, now: Tick) -> Replies {
        let mut replies = Replies::new();
        let before = self.screen();
        // A warning that fell due is withdrawn by the action itself.
        let due = self.flow.tick(now);
        self.ended_by_tick(due, &mut replies);

        let result = self.act(action, now);
        let answer = result
            .err()
            .map(|error| Refusal::report(action.seq(), error));
        if let Err(FlowError::InvalidValue(problem)) = result {
            // The field keeps no value the UI does not have: both clear it.
            let _ = self.flow.clear_field(problem.field, now);
        }
        if let Some(answer) = answer {
            replies.push(answer);
        }
        let reason = match (action, result) {
            // Cannot happen after the catch-up above.
            (_, Err(FlowError::Expired)) => EndReason::Idle,
            (Action::Cancel(_), _) => EndReason::Cancelled,
            (Action::Abandon(_), _) => EndReason::Abandoned,
            // No other action ends a session.
            _ => EndReason::Fault,
        };
        self.note_end(reason, &mut replies);
        self.note_start();
        let resync = matches!(answer, Some(Report::Refused(_)));
        self.show_if_changed(before, resync, &mut replies);
        self.count_down(now, &mut replies);
        replies.submission = result.ok().flatten();
        replies.refused = result.err();
        replies.simulate_failure =
            matches!(action, Action::SimulateFailure(_)) && self.flow.phase().holds_answers();
        replies
    }

    /// Lets time pass, at tick `now`: call it at
    /// [`next_tick`](Self::next_tick). Reports the idle warning, the end of
    /// a session that was idle or reached its maximum length, a submission
    /// that timed out, the end of the confirmation, or - once a second
    /// while a screen counts down - its time left ([`Countdown`]).
    pub fn tick(&mut self, now: Tick) -> Replies {
        let mut replies = Replies::new();
        let before = self.screen();
        let due = self.flow.tick(now);
        if let Some(TickEvent::IdleWarning { reset_at }) = due {
            let left = reset_at.saturating_sub(now);
            let remaining = u32::try_from(left).unwrap_or(u32::MAX);
            replies.push(Report::IdleWarning(IdleWarning { remaining }));
            self.counted = Some((self.screen(), left.div_ceil(SECOND)));
            return replies;
        }
        self.ended_by_tick(due, &mut replies);
        self.show_if_changed(before, false, &mut replies);
        self.count_down(now, &mut replies);
        replies
    }

    /// The service accepted the submission with `ticket`, at tick `now`.
    /// The session ends: [`SessionEnded`] with [`EndReason::Submitted`],
    /// then the confirmation. A stale ticket changes nothing
    /// ([`Replies::refused`]).
    pub fn submission_succeeded(&mut self, ticket: SubmissionTicket, now: Tick) -> Replies {
        self.submission_issued(ticket, None, now)
    }

    /// As [`submission_succeeded`](Self::submission_succeeded), the
    /// service having issued `issued` for the answers - a pass or queue
    /// number - which follows the confirmation ([`Issued`]). The
    /// answers are wiped before the confirmation is reported.
    pub fn submission_issued(
        &mut self,
        ticket: SubmissionTicket,
        issued: Option<Issued>,
        now: Tick,
    ) -> Replies {
        let before = self.screen();
        let expired = self.expiry_reason(now);
        let result = self.flow.submission_succeeded(ticket, now);
        let issued = issued.filter(|_| result.is_ok());
        self.after_result(before, result, EndReason::Submitted, expired, issued, now)
    }

    /// The submission with `ticket` failed, at tick `now`: the failure is
    /// shown, with the attempts left. A stale ticket changes nothing
    /// ([`Replies::refused`]).
    pub fn submission_failed(
        &mut self,
        ticket: SubmissionTicket,
        reason: FailureReason,
        now: Tick,
    ) -> Replies {
        let before = self.screen();
        let expired = self.expiry_reason(now);
        let result = self.flow.submission_failed(ticket, reason, now);
        self.after_result(before, result, EndReason::Fault, expired, None, now)
    }

    /// Ends the session at once for a fault, at tick `now`, whatever the
    /// phase ([`KioskFlow::end_session`]): every answer is cleared, and the
    /// UI is told with [`SessionEnded`] and [`EndReason::Fault`] if a session
    /// was open. [`receive`](Self::receive) calls it for a record it cannot
    /// decode; the application calls it for faults it sees elsewhere.
    pub fn end_session(&mut self, now: Tick) -> Replies {
        let mut replies = Replies::new();
        let before = self.screen();
        self.flow.end_session(now);
        self.note_end(EndReason::Fault, &mut replies);
        self.show_if_changed(before, false, &mut replies);
        self.count_down(now, &mut replies);
        replies
    }

    fn act(&mut self, action: &Action, now: Tick) -> Result<Option<SubmissionTicket>, FlowError> {
        let flow = &mut self.flow;
        match action {
            // Out of service, no session opens.
            Action::Start(_) if self.stopped.is_some() => Err(FlowError::NotAllowed(flow.phase())),
            Action::Start(_) => flow.start(now).map(|()| None),
            Action::SetText(a) => flow.set_text(a.field, a.text(), now).map(|()| None),
            Action::Select(a) => flow.select(a.field, a.option, now).map(|()| None),
            Action::SetChecked(a) => flow.set_checked(a.field, a.checked, now).map(|()| None),
            Action::ClearField(a) => flow.clear_field(a.field, now).map(|()| None),
            Action::Next(_) => flow.next(now).map(|()| None),
            Action::Back(_) => flow.back(now).map(|()| None),
            Action::Submit(_) => flow.submit(now).map(Some),
            Action::Retry(_) => flow.retry(now).map(Some),
            Action::Cancel(_) => flow.cancel(now).map(|()| None),
            Action::Finish(_) => flow.finish(now).map(|()| None),
            Action::StillHere(_) => flow.note_activity(now).map(|()| None),
            Action::Abandon(_) => {
                flow.end_session(now);
                Ok(None)
            }
            // Nothing in the flow: the replies say so to the application.
            Action::SimulateFailure(_) => Ok(None),
        }
    }

    /// Reports the end of a session that a timed event ended.
    fn ended_by_tick(&mut self, due: Option<TickEvent>, replies: &mut Replies) {
        let reason = match due {
            Some(TickEvent::IdleReset) => EndReason::Idle,
            Some(TickEvent::SessionLimit) => EndReason::TimeLimit,
            // No other event ends a session that holds answers.
            _ => EndReason::Fault,
        };
        self.note_end(reason, replies);
    }

    fn after_result(
        &mut self,
        before: Screen,
        result: Result<(), FlowError>,
        applied: EndReason,
        expired: EndReason,
        issued: Option<Issued>,
        now: Tick,
    ) -> Replies {
        let mut replies = Replies::new();
        let reason = if result.is_ok() { applied } else { expired };
        self.note_end(reason, &mut replies);
        self.show_if_changed(before, false, &mut replies);
        if let Some(issued) = issued {
            replies.push(Report::Issued(issued));
        }
        self.count_down(now, &mut replies);
        replies.refused = result.err();
        replies
    }

    /// Why a session that a service result finds ended had ended: its
    /// maximum length, if that had passed by `now`, else the idle timeout.
    /// Worked out before the call, which wipes the deadlines.
    fn expiry_reason(&self, now: Tick) -> EndReason {
        match self.flow.session_deadline() {
            Some(limit) if now >= limit => EndReason::TimeLimit,
            _ => EndReason::Idle,
        }
    }

    /// If the open session has just ended, reports it: exactly once per
    /// session, whatever ended it.
    fn note_end(&mut self, reason: EndReason, replies: &mut Replies) {
        if self.open && !self.flow.phase().holds_answers() {
            self.open = false;
            replies.push(Report::SessionEnded(SessionEnded {
                session: self.flow.session(),
                reason,
            }));
        }
    }

    fn note_start(&mut self) {
        if !self.open && self.flow.phase().holds_answers() {
            self.open = true;
        }
    }

    fn screen(&self) -> Screen {
        Screen {
            phase: self.flow.phase(),
            warning: self.flow.idle_warning().is_some(),
        }
    }

    /// Reports the time left of the screen counting down, if its whole
    /// seconds changed since last reported for it - after the screen, so a
    /// [`Countdown`] always follows the screen it counts down. Last of a
    /// call's reports, so it is the one left out were there ever no room.
    fn count_down(&mut self, now: Tick, replies: &mut Replies) {
        let screen = self.screen();
        let Some(deadline) = self.countdown_deadline() else {
            self.counted = None;
            return;
        };
        let left = deadline.saturating_sub(now);
        let seconds = left.div_ceil(SECOND);
        if seconds > 0 && self.counted != Some((screen, seconds)) {
            let remaining = u32::try_from(left).unwrap_or(u32::MAX);
            replies.push(Report::Countdown(Countdown { remaining }));
            self.counted = Some((screen, seconds));
        }
    }

    /// Reports the screen if it changed, or if `resync` asks for it. A stop
    /// waiting for the session's end takes effect as the welcome screen
    /// would show.
    fn show_if_changed(&mut self, before: Screen, resync: bool, replies: &mut Replies) {
        if self.flow.phase() == Phase::Welcome
            && let Some(reason) = self.stopping.take()
        {
            self.stopped = Some(reason);
        }
        if resync || self.screen() != before {
            replies.push(self.show());
        }
    }

    /// The report naming the screen for the flow's phase - out of service,
    /// the out-of-service screen in place of the welcome screen.
    fn show(&self) -> Report {
        match self.flow.phase() {
            Phase::Welcome => match self.stopped {
                Some(reason) => Report::OutOfService(OutOfService { reason }),
                None => Report::ShowWelcome(ShowWelcome),
            },
            Phase::Step { index } => Report::ShowStep(ShowStep {
                // Fits: `new` refuses forms with more steps.
                index: u16::try_from(index).unwrap_or(u16::MAX),
            }),
            Phase::Review => Report::ShowReview(ShowReview),
            Phase::Submitting { attempt } => Report::ShowSubmitting(ShowSubmitting { attempt }),
            Phase::Submitted => Report::ShowSubmitted(ShowSubmitted),
            Phase::Failed { reason, attempt } => Report::ShowFailed(ShowFailed {
                reason,
                attempt,
                attempts_left: self.flow.attempts_left(),
            }),
        }
    }
}
