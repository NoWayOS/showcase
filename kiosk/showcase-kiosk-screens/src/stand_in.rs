// SPDX-License-Identifier: MIT OR Apache-2.0
//! A development stand-in for the service a kiosk sends its answers to,
//! until the network path exists: an example
//! kiosk's application PD asks it instead of a network PD.
//!
//! It answers by the ticket alone, as its [`Policy`] says, so a
//! demonstration shows each outcome:
//!
//! * [`Policy::FirstUnanswered`], the reference kiosk's: **the first
//!   attempt of every session is left unanswered**, so the flow's submit
//!   timeout fails it and the person is offered *Try again*; **every later
//!   attempt is accepted**, [`ANSWER_MS`] after it was sent.
//! * [`Policy::Issues`], the showcase's examples': **every attempt
//!   is accepted** [`ANSWER_MS`] after it was sent, and the service issues a
//!   number for it - a pass, a place in a queue - counted on from the last,
//!   never made from the answers; unless the person pressed a
//!   demonstration's *service down* in that session
//!   ([`fail_session`](ServiceStandIn::fail_session)), when **every attempt
//!   fails**, the service unavailable, as soon.
//! * [`Policy::Approves`], the pay at the pump's: the card reader
//!   and the pump controller, which **approve what was asked** - the pump,
//!   the fuel, the amount - and say what they approved, as the code issued
//!   ([`approval`]); or **decline every attempt** of a session that asked a
//!   demonstration to fail.
//!
//! It is given the submission's ticket and, under [`Policy::Approves`],
//! the options chosen in the fields the policy names, a digit each: never
//! text, and nothing else of the answers. Nothing leaves the application
//! PD.

use showcase_kiosk_contract::{CheckIn, CheckInAnswer, CheckedIn, ISSUED_LEN, Issued};
use showcase_kiosk_flow::{FailureReason, FieldId, SubmissionTicket, Tick, Value};

/// How long the stand-in takes to accept an attempt, in milliseconds.
pub const ANSWER_MS: Tick = 1_000;

/// How a stand-in answers; see the [module](self).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// The first attempt of each session unanswered; later ones accepted.
    FirstUnanswered,
    /// Every attempt accepted with a number issued - `prefix` then the
    /// number in `digits` digits, counting on from `first` - or failed
    /// while a session asked a demonstration to fail.
    Issues {
        /// Letters before the number: "" for a pass, "A" for a queue.
        prefix: &'static str,
        /// How many digits the number takes, with leading zeros.
        digits: u8,
        /// The first number issued.
        first: u32,
    },
    /// Every attempt approved, its code what was asked ([`approval`]), or
    /// declined while a session asked a demonstration to fail.
    Approves {
        /// The choice fields an attempt asks for, in order: at most
        /// [`ISSUED_LEN`].
        fields: &'static [FieldId],
    },
}

/// What an attempt asks a [`Policy::Approves`] stand-in to approve: the
/// option chosen in each of `fields`, in order, one digit each - `"201"`
/// for the third option of the first, the first of the second and the
/// second of the third. `None` if a field is not a choice, has no option
/// chosen or chose one past 9, or there are no fields or more than
/// [`ISSUED_LEN`].
pub fn approval<'a>(
    fields: &[FieldId],
    value: impl Fn(FieldId) -> Option<Value<'a>>,
) -> Option<Issued> {
    if fields.is_empty() || fields.len() > ISSUED_LEN {
        return None;
    }
    let mut code = [0u8; ISSUED_LEN];
    for (slot, &field) in code.iter_mut().zip(fields) {
        match value(field)? {
            Value::Choice(Some(option)) if option <= 9 => *slot = b'0'.saturating_add(option),
            _ => return None,
        }
    }
    let text = code
        .get(..fields.len())
        .and_then(|bytes| core::str::from_utf8(bytes).ok())?;
    Issued::new(text).ok()
}

/// What the stand-in does with an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Never answered: the flow's submit timeout fails it.
    Unanswered,
    /// Accepted at this tick.
    AcceptAt(Tick),
    /// Failed at this tick, the service unavailable: a demonstration's
    /// *service down*.
    FailAt(Tick),
}

/// The stand-in's answer, when it falls due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The attempt is accepted, with what was issued for it, if anything.
    Accepted(SubmissionTicket, Option<Issued>),
    /// The attempt failed, for this reason.
    Failed(SubmissionTicket, FailureReason),
}

/// The stand-in; see the [module](self).
#[derive(Debug)]
pub struct ServiceStandIn {
    policy: Policy,
    /// The attempt it will answer, when, and how.
    pending: Option<(SubmissionTicket, Tick, Plan)>,
    /// The session whose attempts fail, if one asked.
    failing: Option<u32>,
    /// How many numbers it has issued.
    issued: u32,
    /// What the attempt in hand asked to be approved, under
    /// [`Policy::Approves`].
    asked: Option<Issued>,
}

impl Default for ServiceStandIn {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceStandIn {
    /// The reference kiosk's stand-in, with nothing in hand.
    #[must_use]
    pub const fn new() -> Self {
        Self::with(Policy::FirstUnanswered)
    }

    /// A stand-in answering as `policy` says, with nothing in hand.
    #[must_use]
    pub const fn with(policy: Policy) -> Self {
        Self {
            policy,
            pending: None,
            failing: None,
            issued: 0,
            asked: None,
        }
    }

    /// The person asked a demonstration to fail session `session`'s
    /// submissions (a *service down* button): under [`Policy::Issues`] or
    /// [`Policy::Approves`], each fails from now on. The reference's policy
    /// ignores it.
    pub fn fail_session(&mut self, session: u32) {
        if matches!(self.policy, Policy::Issues { .. } | Policy::Approves { .. }) {
            self.failing = Some(session);
        }
    }

    /// A submission with `ticket` began at tick `now`: what the stand-in
    /// will do with it. It forgets any attempt still in hand, as the flow
    /// does.
    pub fn submitted(&mut self, ticket: SubmissionTicket, now: Tick) -> Plan {
        self.submitted_asking(ticket, now, None)
    }

    /// As [`submitted`](Self::submitted), the attempt asking for `asked`
    /// ([`approval`]): what a [`Policy::Approves`] stand-in approves. Asked
    /// for nothing, it declines - it never approves what it was not asked.
    pub fn submitted_asking(
        &mut self,
        ticket: SubmissionTicket,
        now: Tick,
        asked: Option<Issued>,
    ) -> Plan {
        let at = now.saturating_add(ANSWER_MS);
        let failing = self.failing == Some(ticket.session);
        let plan = match self.policy {
            Policy::FirstUnanswered if ticket.attempt <= 1 => Plan::Unanswered,
            Policy::FirstUnanswered => Plan::AcceptAt(at),
            Policy::Issues { .. } | Policy::Approves { .. } if failing => Plan::FailAt(at),
            Policy::Approves { .. } if asked.is_none() => Plan::FailAt(at),
            Policy::Issues { .. } | Policy::Approves { .. } => Plan::AcceptAt(at),
        };
        self.asked = asked;
        self.pending = match plan {
            Plan::Unanswered => None,
            Plan::AcceptAt(at) | Plan::FailAt(at) => Some((ticket, at, plan)),
        };
        plan
    }

    /// When the stand-in next answers, if it has an attempt in hand.
    #[must_use]
    pub fn due(&self) -> Option<Tick> {
        self.pending.map(|(_, at, _)| at)
    }

    /// The answer to give now, if its time has come at tick `now`. The flow
    /// ignores a ticket that is no longer in flight.
    pub fn take_due(&mut self, now: Tick) -> Option<Answer> {
        match self.pending {
            Some((ticket, at, plan)) if now >= at => {
                self.pending = None;
                Some(match plan {
                    Plan::FailAt(_) => Answer::Failed(ticket, FailureReason::Unavailable),
                    Plan::Unanswered | Plan::AcceptAt(_) => Answer::Accepted(ticket, self.issue()),
                })
            }
            _ => None,
        }
    }

    /// The next number, under [`Policy::Issues`]: never from the answers.
    /// Under [`Policy::Approves`], what was asked and approved.
    fn issue(&mut self) -> Option<Issued> {
        let Policy::Issues {
            prefix,
            digits,
            first,
        } = self.policy
        else {
            return self.asked.take();
        };
        let number = first.wrapping_add(self.issued);
        self.issued = self.issued.wrapping_add(1);
        let mut code = [0u8; ISSUED_LEN];
        let mut len = 0;
        for byte in prefix.bytes() {
            if let Some(slot) = code.get_mut(len) {
                *slot = byte;
                len = len.saturating_add(1);
            }
        }
        let width = usize::from(digits);
        let mut rest = number;
        for at in (0..width).rev() {
            if let Some(slot) = code.get_mut(len.saturating_add(at)) {
                let digit = u8::try_from(rest.checked_rem(10).unwrap_or(0)).unwrap_or(0);
                *slot = b'0'.saturating_add(digit);
            }
            rest = rest.checked_div(10).unwrap_or(0);
        }
        len = len.saturating_add(width).min(ISSUED_LEN);
        let text = code
            .get(..len)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())?;
        Issued::new(text).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ticket(attempt: u8) -> SubmissionTicket {
        SubmissionTicket {
            epoch: 1,
            session: 1,
            attempt,
        }
    }

    #[test]
    fn the_first_attempt_is_left_unanswered_and_a_later_one_accepted() {
        let mut stand_in = ServiceStandIn::new();
        assert_eq!(stand_in.submitted(ticket(1), 100), Plan::Unanswered);
        assert_eq!(stand_in.due(), None);
        assert_eq!(stand_in.take_due(u64::MAX), None);

        assert_eq!(stand_in.submitted(ticket(2), 100), Plan::AcceptAt(1_100));
        assert_eq!(stand_in.due(), Some(1_100));
        assert_eq!(stand_in.take_due(1_099), None);
        assert_eq!(
            stand_in.take_due(1_100),
            Some(Answer::Accepted(ticket(2), None))
        );
        assert_eq!(stand_in.due(), None);
        assert_eq!(stand_in.take_due(5_000), None);
    }

    #[test]
    fn a_new_first_attempt_forgets_the_one_in_hand() {
        let mut stand_in = ServiceStandIn::new();
        let _ = stand_in.submitted(ticket(2), 0);
        assert_eq!(stand_in.submitted(ticket(1), 10), Plan::Unanswered);
        assert_eq!(stand_in.take_due(u64::MAX), None);
    }

    #[test]
    fn the_answer_time_saturates() {
        let mut stand_in = ServiceStandIn::new();
        assert_eq!(
            stand_in.submitted(ticket(3), u64::MAX),
            Plan::AcceptAt(u64::MAX)
        );
        assert_eq!(
            stand_in.take_due(u64::MAX),
            Some(Answer::Accepted(ticket(3), None))
        );
    }

    #[test]
    fn an_issuing_stand_in_accepts_with_a_number_or_fails_when_asked() {
        let mut stand_in = ServiceStandIn::with(Policy::Issues {
            prefix: "A",
            digits: 3,
            first: 47,
        });
        assert_eq!(stand_in.submitted(ticket(1), 0), Plan::AcceptAt(1_000));
        let a047 = Issued::new("A047").ok();
        assert_eq!(
            stand_in.take_due(1_000),
            Some(Answer::Accepted(ticket(1), a047))
        );
        // The reference's switch does nothing; this one fails the session.
        stand_in.fail_session(1);
        assert_eq!(stand_in.submitted(ticket(2), 0), Plan::FailAt(1_000));
        assert_eq!(
            stand_in.take_due(1_000),
            Some(Answer::Failed(ticket(2), FailureReason::Unavailable))
        );
        // The next session is accepted again, with the next number.
        let next = SubmissionTicket {
            epoch: 1,
            session: 2,
            attempt: 1,
        };
        assert_eq!(stand_in.submitted(next, 5), Plan::AcceptAt(1_005));
        let a048 = Issued::new("A048").ok();
        assert_eq!(stand_in.take_due(1_005), Some(Answer::Accepted(next, a048)));
        let mut pass = ServiceStandIn::with(Policy::Issues {
            prefix: "",
            digits: 4,
            first: 4_821,
        });
        let _ = pass.submitted(ticket(1), 0);
        assert_eq!(
            pass.take_due(1_000),
            Some(Answer::Accepted(ticket(1), Issued::new("4821").ok()))
        );
        let mut reference = ServiceStandIn::new();
        reference.fail_session(1);
        assert_eq!(reference.submitted(ticket(2), 0), Plan::AcceptAt(1_000));
    }

    /// The card reader and pump controller approve what was asked,
    /// as the options chosen, or decline when a demonstration asks.
    #[test]
    fn an_approving_stand_in_approves_what_was_asked_or_declines() {
        const FIELDS: &[FieldId] = &[FieldId(1), FieldId(2), FieldId(3)];
        let chosen = |field: FieldId| match field.0 {
            1 => Some(Value::Choice(Some(2))),
            2 => Some(Value::Choice(Some(0))),
            3 => Some(Value::Choice(Some(1))),
            _ => None,
        };
        let asked = approval(FIELDS, chosen);
        assert_eq!(asked, Issued::new("201").ok());
        // Text, no choice, an option past 9 or no fields: nothing asked.
        assert_eq!(approval(FIELDS, |_| Some(Value::Text("4"))), None);
        assert_eq!(approval(FIELDS, |_| Some(Value::Choice(None))), None);
        assert_eq!(approval(FIELDS, |_| Some(Value::Choice(Some(10)))), None);
        assert_eq!(approval(&[], chosen), None);

        let mut stand_in = ServiceStandIn::with(Policy::Approves { fields: FIELDS });
        assert_eq!(
            stand_in.submitted_asking(ticket(1), 0, asked),
            Plan::AcceptAt(1_000)
        );
        assert_eq!(
            stand_in.take_due(1_000),
            Some(Answer::Accepted(ticket(1), asked))
        );
        // Asked for nothing: declined.
        assert_eq!(stand_in.submitted(ticket(2), 0), Plan::FailAt(1_000));
        // A demonstration's decline fails the session's every attempt.
        stand_in.fail_session(1);
        assert_eq!(
            stand_in.submitted_asking(ticket(3), 0, asked),
            Plan::FailAt(1_000)
        );
        assert_eq!(
            stand_in.take_due(1_000),
            Some(Answer::Failed(ticket(3), FailureReason::Unavailable))
        );
    }
}

/// A development stand-in for the backend's check-in, until the
/// network path exists: **the first check-in of each start is answered
/// *not approved*** - the kiosk's new key waits for a person's approval -
/// **and the next approved**, as if a person had approved it in between,
/// with the stand-in's count of check-ins as the epoch. It answers at once. Like the service's stand-in it lives
/// in the application PD and restarts with it, so no result from before a
/// restart can reach a later start.
#[derive(Debug, Default)]
pub struct CheckInStandIn {
    /// Check-ins so far.
    count: u32,
    /// The answer not yet taken.
    answer: Option<CheckedIn>,
}

impl CheckInStandIn {
    /// The stand-in, before any check-in.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            count: 0,
            answer: None,
        }
    }

    /// Takes a check-in: its answer is ready at once
    /// ([`take_answer`](Self::take_answer)).
    pub fn check_in(&mut self, check_in: CheckIn) {
        self.count = self.count.saturating_add(1);
        let answer = if self.count == 1 {
            CheckInAnswer::NotApproved
        } else {
            CheckInAnswer::Approved { epoch: self.count }
        };
        self.answer = Some(CheckedIn {
            seq: check_in.seq,
            answer,
        });
    }

    /// The answer to the last check-in, once.
    pub const fn take_answer(&mut self) -> Option<CheckedIn> {
        self.answer.take()
    }
}
