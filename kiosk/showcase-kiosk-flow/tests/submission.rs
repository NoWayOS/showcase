// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sending the answers: submitting, success and confirmation, failure,
//! retries, timeouts and tickets.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

mod common;

use common::*;
use showcase_kiosk_flow::{FailureReason, FlowError, Phase, SubmissionTicket, TickEvent, Value};

#[test]
fn submitting_freezes_the_answers_and_hands_out_a_ticket() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    assert_eq!(
        ticket,
        SubmissionTicket {
            epoch: EPOCH,
            session: 1,
            attempt: 1
        }
    );
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 1 });
    assert_eq!(flow.attempts(), 1);
    assert_eq!(flow.attempts_left(), MAX_ATTEMPTS - 1);
    // The answers are there to send...
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
    assert_eq!(flow.values().count(), 6);
    // ...and cannot change, nor can the person leave, while they are sent.
    let busy = FlowError::NotAllowed(Phase::Submitting { attempt: 1 });
    assert_eq!(flow.set_text(NAME, "Someone Else", t + 2), Err(busy));
    assert_eq!(flow.back(t + 2), Err(busy));
    assert_eq!(flow.cancel(t + 2), Err(busy));
    assert_eq!(flow.submit(t + 2).err(), Some(busy));
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 1 });
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
}

#[test]
fn success_clears_the_answers_at_once_and_the_confirmation_times_out() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_succeeded(ticket, t + 500).unwrap();
    assert_eq!(flow.phase(), Phase::Submitted);
    assert_wiped(&flow);
    assert_eq!(flow.idle_deadline(), None);
    assert_eq!(flow.next_deadline(), Some(t + 500 + CONFIRMATION));
    assert_eq!(flow.tick(t + 500 + CONFIRMATION - 1), None);
    assert_eq!(
        flow.tick(t + 500 + CONFIRMATION),
        Some(TickEvent::ConfirmationEnded)
    );
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_wiped(&flow);
    assert_eq!(flow.attempts(), 0);
}

#[test]
fn finish_ends_the_confirmation_early() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_succeeded(ticket, t + 2).unwrap();
    // Activity does not extend the confirmation.
    flow.note_activity(t + 3).unwrap();
    assert_eq!(flow.next_deadline(), Some(t + 2 + CONFIRMATION));
    flow.finish(t + 4).unwrap();
    assert_eq!(flow.phase(), Phase::Welcome);
    // A second tap, or one after the confirmation ended, changes nothing.
    flow.finish(t + 5).unwrap();
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_eq!(flow.cancel(t + 6), Ok(()));
    // The next person starts a fresh session.
    flow.start(t + 7).unwrap();
    assert_eq!(flow.session(), 2);
    assert_wiped(&flow);
}

#[test]
fn start_is_refused_until_the_confirmation_has_ended() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_succeeded(ticket, t + 2).unwrap();
    assert_eq!(
        flow.start(t + 3),
        Err(FlowError::NotAllowed(Phase::Submitted))
    );
    assert_eq!(
        flow.cancel(t + 3),
        Err(FlowError::NotAllowed(Phase::Submitted))
    );
    assert_eq!(
        flow.tick(t + 2 + CONFIRMATION),
        Some(TickEvent::ConfirmationEnded)
    );
    flow.start(t + 3 + CONFIRMATION).unwrap();
}

#[test]
fn a_failure_keeps_the_answers_and_can_be_retried_with_a_new_ticket() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let first = flow.submit(t + 1).unwrap();
    flow.submission_failed(first, FailureReason::Unavailable, t + 2)
        .unwrap();
    assert_eq!(
        flow.phase(),
        Phase::Failed {
            reason: FailureReason::Unavailable,
            attempt: 1
        }
    );
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
    let second = flow.retry(t + 3).unwrap();
    assert_eq!(
        second,
        SubmissionTicket {
            epoch: EPOCH,
            session: 1,
            attempt: 2
        }
    );
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 2 });
    // The first attempt's ticket no longer counts.
    assert_eq!(
        flow.submission_succeeded(first, t + 4),
        Err(FlowError::StaleTicket)
    );
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 2 });
    flow.submission_succeeded(second, t + 5).unwrap();
    assert_eq!(flow.phase(), Phase::Submitted);
    assert_wiped(&flow);
}

#[test]
fn results_for_other_attempts_or_sessions_are_ignored() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    let wrong_attempt = SubmissionTicket {
        attempt: 2,
        ..ticket
    };
    let earlier_session = SubmissionTicket {
        session: 0,
        ..ticket
    };
    let before_a_restart = SubmissionTicket {
        epoch: EPOCH + 1,
        ..ticket
    };
    for stale in [wrong_attempt, earlier_session, before_a_restart] {
        assert_eq!(
            flow.submission_succeeded(stale, t + 2),
            Err(FlowError::StaleTicket)
        );
        assert_eq!(
            flow.submission_failed(stale, FailureReason::Rejected, t + 2),
            Err(FlowError::StaleTicket)
        );
    }
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 1 });

    // Once the result has arrived, a duplicate of it is stale too.
    flow.submission_succeeded(ticket, t + 3).unwrap();
    assert_eq!(
        flow.submission_failed(ticket, FailureReason::Unavailable, t + 4),
        Err(FlowError::StaleTicket)
    );
    assert_eq!(flow.phase(), Phase::Submitted);
}

#[test]
fn a_result_meant_for_the_previous_person_never_reaches_the_next_one() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let old = flow.submit(t + 1).unwrap();
    // No answer: the attempt times out, the person gives up and leaves.
    assert_eq!(flow.tick(t + 1 + SUBMIT), Some(TickEvent::SubmitTimedOut));
    flow.cancel(t + 2 + SUBMIT).unwrap();
    // The next person fills in the form and submits.
    let t = fill_all(&mut flow, t + 100_000);
    let new = flow.submit(t + 1).unwrap();
    assert_eq!(new.session, old.session + 1);
    assert_eq!(new.attempt, old.attempt);
    // The previous person's result finally turns up.
    assert_eq!(
        flow.submission_succeeded(old, t + 2),
        Err(FlowError::StaleTicket)
    );
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 1 });
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
}

#[test]
fn a_submission_with_no_result_times_out() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    assert_eq!(flow.next_deadline(), Some(t + 1 + SUBMIT));
    assert_eq!(flow.idle_deadline(), None);
    assert_eq!(flow.tick(t + SUBMIT), None);
    assert_eq!(flow.tick(t + 1 + SUBMIT), Some(TickEvent::SubmitTimedOut));
    assert_eq!(
        flow.phase(),
        Phase::Failed {
            reason: FailureReason::TimedOut,
            attempt: 1
        }
    );
    // A result that arrives after the flow noticed the timeout is stale.
    assert_eq!(
        flow.submission_succeeded(ticket, t + 2 + SUBMIT),
        Err(FlowError::StaleTicket)
    );
    // The answers are kept for a retry, and the idle timeout runs again
    // from the moment of failure.
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
    assert_eq!(flow.idle_deadline(), Some(t + 1 + SUBMIT + IDLE_RESET));
    let retry = flow.retry(t + 3 + SUBMIT).unwrap();
    assert_eq!(retry.attempt, 2);
}

#[test]
fn a_result_counts_until_tick_notices_the_deadline() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    // The deadline has passed, but tick has not been called yet.
    flow.submission_succeeded(ticket, t + 1 + SUBMIT + 5)
        .unwrap();
    assert_eq!(flow.phase(), Phase::Submitted);
}

#[test]
fn a_rejected_submission_cannot_be_retried_but_can_be_corrected() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_failed(ticket, FailureReason::Rejected, t + 2)
        .unwrap();
    assert_eq!(flow.retry(t + 3).err(), Some(FlowError::NotRetryable));
    // Back to the review, back to the first step, correct, and resubmit.
    flow.back(t + 4).unwrap();
    assert_eq!(flow.phase(), Phase::Review);
    for i in 0..4 {
        flow.back(t + 5 + i).unwrap();
    }
    assert_eq!(flow.phase(), Phase::Step { index: 0 });
    flow.set_text(NAME, "Sam Q Sample", t + 10).unwrap();
    for i in 0..4 {
        flow.next(t + 11 + i).unwrap();
    }
    let again = flow.submit(t + 20).unwrap();
    assert_eq!(again.attempt, 2);
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Q Sample")));
}

#[test]
fn attempts_are_limited_per_session() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let mut ticket = flow.submit(t + 1).unwrap();
    let mut t = t + 1;
    for attempt in 1..MAX_ATTEMPTS {
        assert_eq!(ticket.attempt, attempt);
        flow.submission_failed(ticket, FailureReason::Unavailable, t + 1)
            .unwrap();
        ticket = flow.retry(t + 2).unwrap();
        t += 2;
    }
    assert_eq!(ticket.attempt, MAX_ATTEMPTS);
    flow.submission_failed(ticket, FailureReason::TimedOut, t + 1)
        .unwrap();
    assert_eq!(flow.attempts_left(), 0);
    assert_eq!(flow.retry(t + 2).err(), Some(FlowError::NoAttemptsLeft));
    // Going back to the review does not reset the count.
    flow.back(t + 3).unwrap();
    assert_eq!(flow.submit(t + 4).err(), Some(FlowError::NoAttemptsLeft));
    assert_eq!(flow.phase(), Phase::Review);
    // Only ending the session does; cancel is allowed after a failure.
    flow.cancel(t + 5).unwrap();
    assert_wiped(&flow);
    let t = fill_all(&mut flow, t + 6);
    assert_eq!(flow.submit(t + 1).unwrap().attempt, 1);
}

#[test]
fn cancel_is_allowed_after_a_failure() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_failed(ticket, FailureReason::Unavailable, t + 2)
        .unwrap();
    flow.cancel(t + 3).unwrap();
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_wiped(&flow);
    assert_eq!(
        flow.submission_succeeded(ticket, t + 4),
        Err(FlowError::StaleTicket)
    );
}

#[test]
fn the_caller_may_report_its_own_timeout() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_failed(ticket, FailureReason::TimedOut, t + 2)
        .unwrap();
    assert!(FailureReason::TimedOut.is_retryable());
    assert!(FailureReason::Unavailable.is_retryable());
    assert!(!FailureReason::Rejected.is_retryable());
    assert_eq!(flow.retry(t + 3).unwrap().attempt, 2);
}

// ---- Regression tests: each failed before the fix it guards -----------------

#[test]
fn rejected_answers_cannot_be_sent_again_unchanged() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let first = flow.submit(t + 1).unwrap();
    flow.submission_failed(first, FailureReason::Rejected, t + 2)
        .unwrap();
    assert_eq!(flow.retry(t + 3).err(), Some(FlowError::NotRetryable));
    // Back to the review and straight to submit: the same answers again.
    flow.back(t + 4).unwrap();
    assert_eq!(flow.phase(), Phase::Review);
    assert_eq!(flow.submit(t + 5).err(), Some(FlowError::NotRetryable));
    // Entering the same value again is not a change either.
    for i in 0..4 {
        flow.back(t + 6 + i).unwrap();
    }
    flow.set_text(NAME, "Sam Sample", t + 10).unwrap();
    for i in 0..4 {
        flow.next(t + 11 + i).unwrap();
    }
    assert_eq!(flow.submit(t + 15).err(), Some(FlowError::NotRetryable));
    // A real change makes them worth sending.
    for i in 0..4 {
        flow.back(t + 16 + i).unwrap();
    }
    flow.set_text(NAME, "Sam Q Sample", t + 20).unwrap();
    for i in 0..4 {
        flow.next(t + 21 + i).unwrap();
    }
    assert_eq!(flow.submit(t + 25).unwrap().attempt, 2);
}

#[test]
fn a_result_from_before_a_restart_is_not_applied_after_it() {
    // The application restarts while a submission is in flight; the result
    // arrives once the restarted flow has a submission of its own in flight.
    let mut before = new_flow();
    let t = fill_all(&mut before, 0);
    let old = before.submit(t + 1).unwrap();
    let mut after = restarted_flow();
    let t = fill_all(&mut after, 500_000);
    let new = after.submit(t + 1).unwrap();
    assert_ne!(old, new);
    assert_eq!(
        after.submission_succeeded(old, t + 2),
        Err(FlowError::StaleTicket)
    );
    assert_eq!(after.phase(), Phase::Submitting { attempt: 1 });
    assert_eq!(after.value(NAME), Some(Value::Text("Sam Sample")));
}
