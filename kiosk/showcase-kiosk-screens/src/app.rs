// SPDX-License-Identifier: MIT OR Apache-2.0
//! The reference kiosk's application: what its application PD
//! does, without seL4, so the host can test it joined to the UI
//! ([`crate::kiosk`]) - every path, the faults included.
//!
//! [`KioskApp`] runs the kiosk flow on the reference form ([`crate::form`])
//! behind the app-UI contract's application end
//! (`showcase_kiosk_contract::AppEnd`), with the service stand-in
//! ([`crate::stand_in`]). The PD around it moves records between its rings
//! and this type, asks the timer for [`KioskApp::next_wake`], and logs the
//! [`Note`]s - none of which holds an answer.
//!
//! **It checks in first**, made with
//! [`checking_in`](KioskApp::checking_in): out of service until the
//! backend's stand-in ([`CheckInStandIn`]) approves the kiosk with an
//! epoch (`showcase_kiosk_contract::AppStart`). The stand-in's records - the
//! check-in, its answer, each submission's result - go through their
//! encoding and the network path's decoders as a network PD's would. Made
//! [`with`](KioskApp::with) a fixed epoch, it starts in service: for tests,
//! and for a demonstration that never checks in.
//!
//! **It fails closed.**
//!
//! * **Time first.** [`tick`](KioskApp::tick) applies everything that has
//!   fallen due - the idle warning, the privacy reset, a submission's
//!   timeout, the stand-in's answer - at once, whatever room the ring to
//!   the UI has: the flow wipes the answers when they are due, and the
//!   reports wait in a small queue of their own until the UI takes them.
//!   Actions are taken only while that queue is empty, so the UI learns of
//!   a session's end before anything that follows it.
//! * **A fault it finds ends the session.** When the PD finds it cannot go
//!   on - the timer answers with an error or a malformed reply, or refuses
//!   a wake-up; a ring's indices are impossible - [`fail`](KioskApp::fail)
//!   ends any open session, wiping its answers, queues the session-ended
//!   record and the welcome screen for the UI, and puts the kiosk **out of
//!   service**: from then on it takes no action and starts no session, and
//!   the UI stays at its welcome screen. What it cannot find - a timer that
//!   goes silent or faults, a PD that faults - is left to the watchdog and
//!   the supervisor, which the reference kiosk does not
//!   have yet. The report queue cannot overflow in the PD, which takes
//!   actions only while it is empty ([`takes_actions`](KioskApp::takes_actions));
//!   should a caller ignore that, overflowing [`PENDING`] does the same.

use core::fmt;

use nowayos_bounded::BoundedVec;
use showcase_kiosk_contract::{
    APPROVAL_CODE_LEN, AppStart, CheckInTimes, CheckedIn, ContractError, EndReason, FromNetwork,
    MAX_RECORD_LEN, Replies, Report, SessionEnded, ShowWelcome, StartError, SubmissionOutcome,
    SubmissionResult, ToNetwork, decode_from_network, decode_to_network,
};
use showcase_kiosk_flow::{FailureReason, FlowConfig, Form, KioskFlow, Tick};

use crate::form::{CONFIG, FORM, VALUE_LEN};
use crate::kiosk::MAX_FIELDS;
use crate::stand_in::{Answer, CheckInStandIn, Plan, Policy, ServiceStandIn, approval};

/// One example kiosk's application: its form, its timeouts and how
/// its service's stand-in answers. [`REFERENCE`] is the reference kiosk's.
#[derive(Debug, Clone, Copy)]
pub struct Example {
    /// The form its flow runs; at most [`MAX_FIELDS`] fields, each text
    /// at most [`VALUE_LEN`] bytes.
    pub form: &'static Form<'static>,
    /// Its timeouts and tries.
    pub config: FlowConfig,
    /// How its stand-in for the service answers.
    pub policy: Policy,
}

/// The reference kiosk's application: the reference form and timeouts, and
/// a stand-in that leaves each session's first attempt unanswered.
pub const REFERENCE: Example = Example {
    form: &FORM,
    config: CONFIG,
    policy: Policy::FirstUnanswered,
};

/// The most reports waiting for room in the ring to the UI.
pub const PENDING: usize = 32;

/// The most notes waiting to be logged; later ones are dropped.
const NOTES: usize = 8;

/// The most timed events handled at once: more than can fall due together.
const MAX_EVENTS: usize = 8;

/// When a demonstration checks in: at once again after an answer that is
/// no approval, since its stand-in for the backend answers at once and
/// approves the second; a check-in unanswered in 10 s has failed. A
/// product's network path sets its own.
pub const CHECK_IN: CheckInTimes = CheckInTimes {
    retry: 0,
    timeout: 10_000,
};

/// Why the application went out of service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The timer answered a call with an error or a malformed reply.
    Timer,
    /// The timer refused a wake-up.
    Wake,
    /// A ring to or from the UI is broken.
    Ring,
    /// [`PENDING`] reports piled up untaken: only a caller that ignores
    /// [`KioskApp::takes_actions`] can cause it, which the PD never does.
    Backlog,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Timer => "the timer answered with an error",
            Self::Wake => "the timer refused a wake-up",
            Self::Ring => "a ring to or from the UI is broken",
            Self::Backlog => "the UI took no reports while they piled up",
        })
    }
}

/// A record between the App and the network path that was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFault {
    /// It could not be encoded or decoded.
    Record(ContractError),
    /// The App's start refused it ([`StartError`]).
    Start(StartError),
}

impl fmt::Display for NetworkFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record(error) => write!(f, "{error}"),
            Self::Start(error) => write!(f, "{error}"),
        }
    }
}

/// Something the PD logs. None holds an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// The App checked in, with this `seq`.
    CheckIn {
        /// The check-in's number.
        seq: u32,
    },
    /// The check-in's answer: approved with the epoch, not approved, or
    /// failed.
    CheckedIn(CheckedIn),
    /// A submission was answered *not approved*: no session opens again
    /// in this start, once the one open ends.
    Revoked,
    /// A record to or from the network path was refused.
    Network(NetworkFault),
    /// A submission began: what the service stand-in will do with it.
    StandIn {
        /// The attempt, from 1.
        attempt: u8,
        /// The flow's epoch, which the attempt's ticket carries.
        epoch: u32,
        /// What the stand-in will do.
        plan: Plan,
    },
    /// The stand-in accepted this attempt.
    Accepted {
        /// The attempt, from 1.
        attempt: u8,
    },
    /// The stand-in failed this attempt, the service unavailable: a
    /// demonstration's *service down*.
    Failed {
        /// The attempt, from 1.
        attempt: u8,
    },
    /// The person asked a demonstration to fail this session's
    /// submissions.
    Failing,
    /// A session ended, and why.
    Ended(SessionEnded),
    /// A record from the UI was refused, and any open session ended.
    Refused(ContractError),
    /// The kiosk went out of service.
    OutOfService(Fault),
}

/// An example kiosk's application - the reference kiosk's unless made
/// [`with`](Self::with) another [`Example`]; see the [module](self).
pub struct KioskApp {
    app: AppStart<'static, MAX_FIELDS, VALUE_LEN>,
    policy: Policy,
    stand_in: ServiceStandIn,
    backend: CheckInStandIn,
    /// Reports waiting for room in the ring to the UI, oldest first.
    pending: BoundedVec<Option<Report>, PENDING>,
    notes: BoundedVec<Option<Note>, NOTES>,
    fault: Option<Fault>,
}

impl KioskApp {
    /// The reference kiosk's application at its welcome screen, with the
    /// fixed flow epoch `epoch`.
    ///
    /// # Errors
    ///
    /// What refused the reference form: the flow or the contract - or an
    /// epoch of 0.
    pub fn new(epoch: u32) -> Result<Self, &'static str> {
        Self::with(&REFERENCE, epoch)
    }

    /// `example`'s application at its welcome screen, with the fixed flow
    /// epoch `epoch`: it never checks in.
    ///
    /// # Errors
    ///
    /// What refused the form: the flow or the contract - or an epoch of 0.
    pub fn with(example: &Example, epoch: u32) -> Result<Self, &'static str> {
        let app = AppStart::started(example.form, example.config, epoch).map_err(setup)?;
        Ok(Self::around(app, example.policy))
    }

    /// `example`'s application at its start: out of service,
    /// awaiting approval with its key's approval code `code`, until the
    /// backend's stand-in approves it with an epoch - which it checks in
    /// for at its first [`tick`](Self::tick). The out-of-service report
    /// waits for the UI already.
    ///
    /// # Errors
    ///
    /// What refused the form: the flow or the contract.
    pub fn checking_in(
        example: &Example,
        code: [u8; APPROVAL_CODE_LEN],
    ) -> Result<Self, &'static str> {
        let app = AppStart::new(example.form, example.config, code, CHECK_IN).map_err(setup)?;
        let mut kiosk = Self::around(app, example.policy);
        let first = kiosk.app.start_reports();
        kiosk.take(&first, 0);
        Ok(kiosk)
    }

    fn around(app: AppStart<'static, MAX_FIELDS, VALUE_LEN>, policy: Policy) -> Self {
        Self {
            app,
            policy,
            stand_in: ServiceStandIn::with(policy),
            backend: CheckInStandIn::new(),
            pending: BoundedVec::new(),
            notes: BoundedVec::new(),
            fault: None,
        }
    }

    /// The flow, to read: once the App has its epoch.
    #[must_use]
    pub fn flow(&self) -> Option<&KioskFlow<'static, MAX_FIELDS, VALUE_LEN>> {
        self.app.flow()
    }

    /// The App's start: its epoch, and why it is out of service, if it is.
    #[must_use]
    pub const fn start(&self) -> &AppStart<'static, MAX_FIELDS, VALUE_LEN> {
        &self.app
    }

    /// Why the kiosk is out of service, if it is.
    #[must_use]
    pub const fn fault(&self) -> Option<Fault> {
        self.fault
    }

    /// Whether to take the UI's next action: in service, with no report
    /// still waiting. The PD also needs room in the ring for the replies
    /// (`showcase_kiosk_contract::MAX_REPLIES`).
    #[must_use]
    pub fn takes_actions(&self) -> bool {
        self.fault.is_none() && self.pending.is_empty()
    }

    /// Takes one record from the UI, already copied out of its ring, at
    /// tick `now`. Out of service, it is dropped unread.
    pub fn receive(&mut self, record: &[u8], now: Tick) {
        if self.fault.is_some() {
            return;
        }
        match self.app.receive(record, now) {
            Ok(replies) => self.take(&replies, now),
            Err(faulted) => {
                self.note(Note::Refused(faulted.error));
                self.take(&faulted.replies, now);
            }
        }
    }

    /// Applies everything that has fallen due by `now`, at once: the
    /// check-in and its answer first, while the App has no epoch.
    pub fn tick(&mut self, now: Tick) {
        for _ in 0..MAX_EVENTS {
            if self.fault.is_some() {
                return;
            }
            if let Some(check_in) = self.app.check_in(now) {
                self.note(Note::CheckIn { seq: check_in.seq });
                match through_network(ToNetwork::CheckIn(check_in)) {
                    Ok(ToNetwork::CheckIn(check_in)) => self.backend.check_in(check_in),
                    Err(error) => self.note(Note::Network(NetworkFault::Record(error))),
                }
            } else if let Some(answer) = self.backend.take_answer() {
                self.take_network(FromNetwork::CheckedIn(answer), now);
            } else if self.app.next_tick().is_some_and(|due| due <= now) {
                let replies = self.app.tick(now);
                self.take(&replies, now);
            } else if let Some(answer) = self.stand_in.take_due(now) {
                let (ticket, outcome) = match answer {
                    Answer::Accepted(ticket, issued) => {
                        self.note(Note::Accepted {
                            attempt: ticket.attempt,
                        });
                        (ticket, SubmissionOutcome::Accepted { issued })
                    }
                    Answer::Failed(ticket, reason) => {
                        self.note(Note::Failed {
                            attempt: ticket.attempt,
                        });
                        let outcome = match reason {
                            FailureReason::Rejected => SubmissionOutcome::Rejected,
                            FailureReason::TimedOut | FailureReason::Unavailable => {
                                SubmissionOutcome::Unavailable
                            }
                        };
                        (ticket, outcome)
                    }
                };
                let result = SubmissionResult { ticket, outcome };
                self.take_network(FromNetwork::SubmissionResult(result), now);
            } else {
                return;
            }
        }
    }

    /// A record from the network path - here, a stand-in's - through its
    /// encoding and the App's decoder, then applied.
    fn take_network(&mut self, record: FromNetwork, now: Tick) {
        let mut slot = [0u8; MAX_RECORD_LEN];
        // A record that cannot be encoded - none of the stand-ins' - is
        // refused under its own kind.
        let decoded = record
            .encode(&mut slot)
            .ok()
            .and_then(|len| slot.get(..len))
            .map_or(
                Err(ContractError::UnexpectedKind {
                    kind: record.kind(),
                }),
                decode_from_network,
            );
        let applied = match decoded {
            Ok(FromNetwork::CheckedIn(answer)) => {
                let applied = self.app.checked_in(&answer, now);
                if applied.is_ok() {
                    self.note(Note::CheckedIn(answer));
                }
                applied
            }
            Ok(FromNetwork::SubmissionResult(result)) => {
                let revoked = self.app.is_revoked();
                let applied = self.app.submission_result(&result, now);
                if !revoked && self.app.is_revoked() {
                    self.note(Note::Revoked);
                }
                applied
            }
            Err(error) => {
                self.note(Note::Network(NetworkFault::Record(error)));
                return;
            }
        };
        match applied {
            Ok(replies) => self.take(&replies, now),
            Err(error) => self.note(Note::Network(NetworkFault::Start(error))),
        }
    }

    /// When to wake next: the App's next tick - its next check-in or a
    /// check-in's deadline, then the flow's next deadline, or a countdown's
    /// next second - or the stand-in's answer. None after a fault.
    #[must_use]
    pub fn next_wake(&self) -> Option<Tick> {
        if self.fault.is_some() {
            return None;
        }
        [self.app.next_tick(), self.stand_in.due()]
            .into_iter()
            .flatten()
            .min()
    }

    /// The PD cannot go on, for `fault`, at tick `now`: ends any open
    /// session, wiping its answers, queues its end and the welcome screen
    /// for the UI, and goes out of service. Only the first fault counts.
    pub fn fail(&mut self, fault: Fault, now: Tick) {
        if self.fault.is_some() {
            return;
        }
        self.fault = Some(fault);
        self.note(Note::OutOfService(fault));
        self.stand_in = ServiceStandIn::with(self.policy);
        let replies = self.app.end_session(now);
        self.take(&replies, now);
    }

    /// The reports waiting for the UI, oldest first.
    pub fn reports(&self) -> impl Iterator<Item = &Report> {
        self.pending.iter().flatten()
    }

    /// Forgets the oldest `count` reports, once they are in the ring.
    pub fn sent(&mut self, count: usize) {
        for _ in 0..count {
            if self.pending.remove(0).is_none() {
                break;
            }
        }
    }

    /// The oldest note not yet logged.
    pub fn take_note(&mut self) -> Option<Note> {
        self.notes.remove(0).flatten()
    }

    fn note(&mut self, note: Note) {
        // The log is best effort: a note with no room is dropped.
        let _ = self.notes.push(Some(note));
    }

    /// Acts on one call's replies: a submission goes to the stand-in, the
    /// session's end is noted, and the reports wait for the UI.
    fn take(&mut self, replies: &Replies, now: Tick) {
        if replies.simulate_failure()
            && let Some(flow) = self.app.flow()
        {
            self.stand_in.fail_session(flow.session());
            self.note(Note::Failing);
        }
        if let Some(ticket) = replies.submission() {
            // A card reader is asked for the options chosen alone.
            let asked = match (self.policy, self.app.flow()) {
                (Policy::Approves { fields }, Some(flow)) => {
                    approval(fields, |field| flow.value(field))
                }
                _ => None,
            };
            let plan = self.stand_in.submitted_asking(ticket, now, asked);
            self.note(Note::StandIn {
                attempt: ticket.attempt,
                epoch: ticket.epoch,
                plan,
            });
        }
        if let Some(ended) = replies.session_ended() {
            self.note(Note::Ended(ended));
        }
        for report in replies.reports() {
            if self.pending.push(Some(*report)).is_err() {
                self.backlog(now);
                return;
            }
        }
    }

    /// The UI took no reports while they piled up: the session ends, its
    /// answers wiped, and only its end and the welcome screen stay queued -
    /// what came before them no longer matters.
    fn backlog(&mut self, now: Tick) {
        let _ = self.app.end_session(now);
        self.stand_in = ServiceStandIn::with(self.policy);
        self.pending.clear();
        let ended = SessionEnded {
            session: self.app.flow().map_or(0, KioskFlow::session),
            reason: EndReason::Fault,
        };
        let _ = self.pending.push(Some(Report::SessionEnded(ended)));
        let _ = self.pending.push(Some(Report::ShowWelcome(ShowWelcome)));
        if self.fault.is_none() {
            self.fault = Some(Fault::Backlog);
            self.note(Note::OutOfService(Fault::Backlog));
        }
    }
}

impl fmt::Debug for KioskApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KioskApp")
            .field("phase", &self.app.flow().map(KioskFlow::phase))
            .field("pending", &self.pending.len())
            .field("fault", &self.fault)
            .finish_non_exhaustive()
    }
}

/// A record for the network path - here, the backend's stand-in - through
/// its encoding and the network path's decoder, as a network PD takes it.
fn through_network(record: ToNetwork) -> Result<ToNetwork, ContractError> {
    let mut slot = [0u8; MAX_RECORD_LEN];
    record
        .encode(&mut slot)
        .ok()
        .and_then(|len| slot.get(..len))
        .map_or(
            Err(ContractError::UnexpectedKind {
                kind: record.kind(),
            }),
            decode_to_network,
        )
}

/// Why the form cannot run, for the PD's log.
const fn setup(error: showcase_kiosk_contract::StartSetupError) -> &'static str {
    match error {
        showcase_kiosk_contract::StartSetupError::Flow(_) => "the flow refused the form",
        showcase_kiosk_contract::StartSetupError::Contract(_) => "the contract refused the form",
        showcase_kiosk_contract::StartSetupError::ZeroEpoch => "an epoch of 0",
    }
}
