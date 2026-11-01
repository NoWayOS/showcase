// SPDX-License-Identifier: MIT OR Apache-2.0
//! The App's start: see [`AppStart`].

use core::fmt;

use showcase_kiosk_flow::{
    FailureReason, FlowConfig, FlowError, Form, KioskFlow, Phase, SetupError as FlowSetupError,
    Tick,
};

use crate::{
    APPROVAL_CODE_LEN, Action, AppEnd, CheckIn, CheckInAnswer, CheckedIn, Faulted, OutOfService,
    OutOfServiceReason, Refusal, Refused, Replies, Report, SetupError, ShowWelcome,
    SubmissionOutcome, SubmissionResult, decode_action,
};

/// When the App checks in, in ticks (milliseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckInTimes {
    /// How long after an answer of *not approved* or *failed* it checks
    /// in again.
    pub retry: Tick,
    /// How long it waits for an answer before it takes the check-in as
    /// failed.
    pub timeout: Tick,
}

/// Why a form cannot run behind [`AppStart`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartSetupError {
    /// The flow refused the form or its settings.
    Flow(FlowSetupError),
    /// The app-UI contract refused the form.
    Contract(SetupError),
    /// A fixed epoch of 0 ([`AppStart::started`]).
    ZeroEpoch,
}

impl fmt::Display for StartSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flow(error) => write!(f, "the flow refused the form: {error}"),
            Self::Contract(error) => write!(f, "the contract refused the form: {error}"),
            Self::ZeroEpoch => f.write_str("an epoch of 0"),
        }
    }
}

impl core::error::Error for StartSetupError {}

/// A record from the network path that [`AppStart`] refused. It changed
/// nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    /// The App has its epoch for this start already: a second answer
    /// cannot change it.
    AlreadyStarted,
    /// No check-in with this `seq` is waiting for its answer.
    NotAsked {
        /// The `seq` the answer quoted.
        seq: u32,
    },
    /// An approval with epoch 0, which no backend gives.
    ZeroEpoch,
    /// A submission's result before the App has an epoch: it made no
    /// submission.
    NotStarted,
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => f.write_str("a check-in answer after the epoch was given"),
            Self::NotAsked { seq } => write!(f, "an answer to check-in {seq}, not waiting"),
            Self::ZeroEpoch => f.write_str("an approval with epoch 0"),
            Self::NotStarted => f.write_str("a submission's result before the epoch"),
        }
    }
}

impl core::error::Error for StartError {}

/// The App's start: no session opens until a check-in gives the
/// App its epoch.
///
/// [`AppStart`] wraps [`AppEnd`]. At each start it checks in with the
/// backend through the network path ([`CheckIn`]) and, until an answer
/// approves the kiosk with an epoch, is **out of service**: the UI is
/// shown the [`OutOfService`] report - awaiting approval, with the key's
/// public approval code, or no service after a check-in that failed - and
/// every action is refused, a start among them, with
/// [`Refusal::NotAllowed`]. It checks in again a while after each answer
/// that is not an approval, and takes a check-in unanswered by its deadline
/// as failed. Approved, it makes its [`KioskFlow`] with exactly that epoch,
/// and shows the welcome screen; from then on it is [`AppEnd`], and the
/// epoch never changes until the next start.
///
/// **A revoked kiosk.** A submission answered *not approved* fails, for
/// the person, as *unavailable*; the session runs on to its own end, never
/// cut short, and the kiosk is then out of service - *no service* - until
/// it restarts. No session opens again in this start, and no check-in is
/// made: on the Pi and in QEMU a restart makes a new key, which a person
/// approves at the backend. (The story's proposal,
/// awaiting the Product Owner's decision.)
pub struct AppStart<'a, const F: usize, const V: usize> {
    form: &'a Form<'a>,
    config: FlowConfig,
    code: [u8; APPROVAL_CODE_LEN],
    times: CheckInTimes,
    /// The last check-in's `seq`.
    seq: u32,
    /// Why the kiosk is out of service until it has its epoch.
    reason: OutOfServiceReason,
    /// The check-in waiting for its answer, and its deadline.
    asked: Option<(u32, Tick)>,
    /// When to check in next, while none is waiting.
    next: Tick,
    /// A submission was answered *not approved*.
    revoked: bool,
    /// Once the epoch is given.
    app: Option<AppEnd<'a, F, V>>,
}

impl<'a, const F: usize, const V: usize> AppStart<'a, F, V> {
    /// The App at its start, out of service and awaiting approval, its
    /// key's approval code `code`, to check in at once and then as `times`
    /// says.
    ///
    /// # Errors
    ///
    /// A [`StartSetupError`] if the form cannot run: checked now, so it
    /// cannot fail when the epoch comes.
    pub fn new(
        form: &'a Form<'a>,
        config: FlowConfig,
        code: [u8; APPROVAL_CODE_LEN],
        times: CheckInTimes,
    ) -> Result<Self, StartSetupError> {
        // Any epoch will do to check the form: the flow does not look at it.
        let _ = make::<F, V>(form, config, 1)?;
        Ok(Self {
            form,
            config,
            code,
            times,
            seq: 0,
            reason: OutOfServiceReason::AwaitingApproval { code },
            asked: None,
            next: 0,
            revoked: false,
            app: None,
        })
    }

    /// The App started with a fixed `epoch`, making no check-in: for a
    /// demonstration whose service is a stand-in in the same protection
    /// domain, which restarts with it, and for tests.
    ///
    /// # Errors
    ///
    /// A [`StartSetupError`] if the form cannot run, or `epoch` is 0.
    pub fn started(
        form: &'a Form<'a>,
        config: FlowConfig,
        epoch: u32,
    ) -> Result<Self, StartSetupError> {
        if epoch == 0 {
            return Err(StartSetupError::ZeroEpoch);
        }
        let app = make::<F, V>(form, config, epoch)?;
        Ok(Self {
            form,
            config,
            code: [0; APPROVAL_CODE_LEN],
            times: CheckInTimes {
                retry: 0,
                timeout: 0,
            },
            seq: 0,
            reason: OutOfServiceReason::NoService,
            asked: None,
            next: 0,
            revoked: false,
            app: Some(app),
        })
    }

    /// What to tell the UI as the App starts: out of service, and why,
    /// until it has its epoch; the welcome screen after.
    pub fn start_reports(&self) -> Replies {
        let mut replies = Replies::new();
        replies.push(match self.out_of_service() {
            Some(reason) => Report::OutOfService(OutOfService { reason }),
            None => Report::ShowWelcome(ShowWelcome),
        });
        replies
    }

    /// This start's epoch, once given.
    #[must_use]
    pub fn epoch(&self) -> Option<u32> {
        self.flow().map(KioskFlow::epoch)
    }

    /// The application end, once the epoch is given.
    #[must_use]
    pub const fn app(&self) -> Option<&AppEnd<'a, F, V>> {
        self.app.as_ref()
    }

    /// The flow, once the epoch is given: to read the answers to send with
    /// a submission ([`KioskFlow::values`]).
    #[must_use]
    pub fn flow(&self) -> Option<&KioskFlow<'a, F, V>> {
        self.app.as_ref().map(AppEnd::flow)
    }

    /// Why the kiosk is out of service, if it is: until the epoch, and
    /// after a revocation once the session open then has ended.
    #[must_use]
    pub fn out_of_service(&self) -> Option<OutOfServiceReason> {
        match &self.app {
            Some(app) => app.out_of_service(),
            None => Some(self.reason),
        }
    }

    /// Whether a submission was answered *not approved* in this start.
    #[must_use]
    pub const fn is_revoked(&self) -> bool {
        self.revoked
    }

    /// The check-in to send to the network path at tick `now`, if one is
    /// due: before the epoch, with none waiting for its answer, at once at
    /// the start and then [`CheckInTimes::retry`] after each answer that
    /// was no approval.
    pub fn check_in(&mut self, now: Tick) -> Option<CheckIn> {
        if self.app.is_some() || self.asked.is_some() || now < self.next {
            return None;
        }
        self.seq = self.seq.wrapping_add(1);
        self.asked = Some((self.seq, now.saturating_add(self.times.timeout)));
        Some(CheckIn { seq: self.seq })
    }

    /// The network path's answer to a check-in, at tick `now`. Approved,
    /// the App makes its flow with that epoch and shows the welcome
    /// screen; otherwise it stays out of service, saying why if that
    /// changed, and checks in again later.
    ///
    /// # Errors
    ///
    /// A [`StartError`], having changed nothing, for an answer after the
    /// epoch was given, one to no check-in waiting, or an epoch of 0.
    pub fn checked_in(&mut self, answer: &CheckedIn, now: Tick) -> Result<Replies, StartError> {
        if self.app.is_some() {
            return Err(StartError::AlreadyStarted);
        }
        if self.asked.is_none_or(|(seq, _)| seq != answer.seq) {
            return Err(StartError::NotAsked { seq: answer.seq });
        }
        let reason = match answer.answer {
            CheckInAnswer::Approved { epoch: 0 } => return Err(StartError::ZeroEpoch),
            CheckInAnswer::Approved { epoch } => {
                // Cannot fail: `new` checked the form, and the flow does
                // not look at the epoch. Were it ever to, the kiosk stays
                // out of service, as for a check-in that failed.
                if let Ok(app) = make::<F, V>(self.form, self.config, epoch) {
                    self.asked = None;
                    self.app = Some(app);
                    let mut replies = Replies::new();
                    replies.push(Report::ShowWelcome(ShowWelcome));
                    return Ok(replies);
                }
                OutOfServiceReason::NoService
            }
            CheckInAnswer::NotApproved => OutOfServiceReason::AwaitingApproval { code: self.code },
            CheckInAnswer::Failed => OutOfServiceReason::NoService,
        };
        Ok(self.wait_again(reason, now))
    }

    /// Takes one record from the UI, as [`AppEnd::receive`]; before the
    /// epoch, as [`apply`](Self::apply).
    ///
    /// # Errors
    ///
    /// [`Faulted`] for a record that cannot be decoded: with no session
    /// open before the epoch, its replies only say the kiosk is out of
    /// service.
    pub fn receive(&mut self, record: &[u8], now: Tick) -> Result<Replies, Faulted> {
        if let Some(app) = &mut self.app {
            return app.receive(record, now);
        }
        match decode_action(record) {
            Ok(action) => Ok(self.apply(&action, now)),
            Err(error) => Err(Faulted {
                error,
                replies: self.start_reports(),
            }),
        }
    }

    /// Applies one decoded action, as [`AppEnd::apply`]. Before the epoch
    /// no session opens: every action is refused with
    /// [`Refusal::NotAllowed`], and the UI is shown why the kiosk is out of
    /// service.
    pub fn apply(&mut self, action: &Action, now: Tick) -> Replies {
        if let Some(app) = &mut self.app {
            return app.apply(action, now);
        }
        let mut replies = Replies::new();
        replies.push(Report::Refused(Refused {
            seq: action.seq(),
            refusal: Refusal::NotAllowed,
        }));
        replies.push(Report::OutOfService(OutOfService {
            reason: self.reason,
        }));
        replies.refused = Some(FlowError::NotAllowed(Phase::Welcome));
        replies
    }

    /// Lets time pass, at tick `now`, as [`AppEnd::tick`]. Before the
    /// epoch, a check-in unanswered by its deadline has failed: the kiosk
    /// has no service, and checks in again later.
    pub fn tick(&mut self, now: Tick) -> Replies {
        if let Some(app) = &mut self.app {
            return app.tick(now);
        }
        match self.asked {
            Some((_, deadline)) if now >= deadline => {
                self.wait_again(OutOfServiceReason::NoService, now)
            }
            _ => Replies::new(),
        }
    }

    /// When to call [`tick`](Self::tick) or [`check_in`](Self::check_in)
    /// next: before the epoch, the waiting check-in's deadline or the
    /// next check-in; then as [`AppEnd::next_tick`].
    #[must_use]
    pub fn next_tick(&self) -> Option<Tick> {
        match &self.app {
            Some(app) => app.next_tick(),
            None => Some(self.asked.map_or(self.next, |(_, deadline)| deadline)),
        }
    }

    /// A submission's result from the network path, at tick `now`, applied
    /// as [`AppEnd::submission_issued`] or [`AppEnd::submission_failed`]:
    /// a result whose ticket names another epoch, session or attempt
    /// changes nothing ([`Replies::refused`],
    /// [`FlowError::StaleTicket`]). *Not approved* fails the attempt as
    /// *unavailable*, and the kiosk is out of service once the session
    /// ends, until it restarts (see [`AppStart`]).
    ///
    /// # Errors
    ///
    /// [`StartError::NotStarted`] before the epoch.
    pub fn submission_result(
        &mut self,
        result: &SubmissionResult,
        now: Tick,
    ) -> Result<Replies, StartError> {
        let Some(app) = &mut self.app else {
            return Err(StartError::NotStarted);
        };
        let ticket = result.ticket;
        let replies = match result.outcome {
            SubmissionOutcome::Accepted { issued } => app.submission_issued(ticket, issued, now),
            SubmissionOutcome::Rejected => {
                app.submission_failed(ticket, FailureReason::Rejected, now)
            }
            SubmissionOutcome::Unavailable => {
                app.submission_failed(ticket, FailureReason::Unavailable, now)
            }
            SubmissionOutcome::NotApproved => {
                let mut replies = app.submission_failed(ticket, FailureReason::Unavailable, now);
                if replies.refused().is_none() {
                    self.revoked = true;
                    // The session is open: the stop waits for its end, and
                    // says nothing now. Were it not, its one report follows.
                    for report in app.stop_service(OutOfServiceReason::NoService).reports() {
                        replies.push(*report);
                    }
                }
                replies
            }
        };
        Ok(replies)
    }

    /// Ends the session at once for a fault, as [`AppEnd::end_session`];
    /// before the epoch there is none, and nothing to say.
    pub fn end_session(&mut self, now: Tick) -> Replies {
        match &mut self.app {
            Some(app) => app.end_session(now),
            None => Replies::new(),
        }
    }

    /// No answer approved the kiosk: out of service for `reason`, saying
    /// so if it changed, with the next check-in a retry away.
    fn wait_again(&mut self, reason: OutOfServiceReason, now: Tick) -> Replies {
        self.asked = None;
        self.next = now.saturating_add(self.times.retry);
        let mut replies = Replies::new();
        if reason != self.reason {
            self.reason = reason;
            replies.push(Report::OutOfService(OutOfService { reason }));
        }
        replies
    }
}

impl<const F: usize, const V: usize> fmt::Debug for AppStart<'_, F, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppStart")
            .field("epoch", &self.epoch())
            .field("out_of_service", &self.out_of_service())
            .field("asked", &self.asked)
            .field("revoked", &self.revoked)
            .finish_non_exhaustive()
    }
}

/// The application end for `form`, its flow made with `epoch`.
fn make<'a, const F: usize, const V: usize>(
    form: &'a Form<'a>,
    config: FlowConfig,
    epoch: u32,
) -> Result<AppEnd<'a, F, V>, StartSetupError> {
    let flow = KioskFlow::new(form, config, epoch).map_err(StartSetupError::Flow)?;
    AppEnd::new(flow).map_err(StartSetupError::Contract)
}
