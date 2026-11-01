// SPDX-License-Identifier: MIT OR Apache-2.0
//! The privacy guarantees of a shared public kiosk: when a person walks
//! away, their answers go, and the next person never sees them.

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
use showcase_kiosk_flow::{
    FailureReason, FlowConfig, FlowError, Phase, SetupError, TickEvent, Timeouts,
};

#[test]
fn an_idle_session_is_wiped_and_every_answer_cleared() {
    let mut flow = new_flow();
    let last = fill_all(&mut flow, 1_000);
    assert!(flow.has_entered_data());
    assert_eq!(flow.idle_deadline(), Some(last + IDLE_RESET));

    assert_eq!(
        flow.tick(last + IDLE_RESET - 1),
        Some(TickEvent::IdleWarning {
            reset_at: last + IDLE_RESET
        })
    );
    assert_eq!(flow.phase(), Phase::Review);
    assert!(flow.has_entered_data());

    assert_eq!(flow.tick(last + IDLE_RESET), Some(TickEvent::IdleReset));
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_wiped(&flow);
    assert_eq!(flow.attempts(), 0);
    assert_eq!(flow.next_deadline(), None);
    assert_eq!(flow.tick(last + 2 * IDLE_RESET), None);

    // The next person starts with an empty form, in a new session.
    flow.start(last + 2 * IDLE_RESET + 5).unwrap();
    assert_eq!(flow.session(), 2);
    assert_wiped(&flow);
}

#[test]
fn the_idle_timeout_applies_on_each_step_the_review_and_after_a_failure() {
    // Wherever the person stops, the session is wiped a timeout later.
    for stop_after in 0..=5 {
        let mut flow = new_flow();
        let mut t = fill_all(&mut flow, 0);
        match stop_after {
            // Somewhere in the steps, going back from the review.
            0..=3 => {
                for _ in 0..=stop_after {
                    t += 1;
                    flow.back(t).unwrap();
                }
                assert!(matches!(flow.phase(), Phase::Step { .. }));
            }
            // At the review.
            4 => {}
            // After a failed submission.
            _ => {
                let ticket = flow.submit(t + 1).unwrap();
                t += 2;
                flow.submission_failed(ticket, FailureReason::Unavailable, t)
                    .unwrap();
            }
        }
        assert!(flow.has_entered_data());
        assert_eq!(flow.tick(t + IDLE_RESET), Some(TickEvent::IdleReset));
        assert_eq!(flow.phase(), Phase::Welcome);
        assert_wiped(&flow);
    }
}

#[test]
fn a_session_with_no_answers_yet_is_ended_too() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    assert_eq!(flow.tick(IDLE_RESET), Some(TickEvent::IdleReset));
    assert_eq!(flow.phase(), Phase::Welcome);
}

#[test]
fn every_action_restarts_the_idle_timer() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    let mut t = 0;
    // Keep acting just inside the timeout, many times over: never wiped.
    for i in 0..10 {
        t += IDLE_RESET - 1;
        let _ = flow.tick(t);
        assert_eq!(flow.phase(), Phase::Step { index: 0 });
        match i % 3 {
            0 => flow.set_text(NAME, "Sam Sample", t).unwrap(),
            1 => flow.note_activity(t).unwrap(),
            _ => {
                // A refused action still shows the person is there.
                assert_eq!(flow.back(t), Err(FlowError::AtFirstStep));
            }
        }
        assert_eq!(flow.idle_deadline(), Some(t + IDLE_RESET));
    }
    assert!(flow.has_entered_data());
}

#[test]
fn the_warning_comes_once_and_activity_withdraws_it() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 10).unwrap();
    let warn_at = 10 + IDLE_RESET - IDLE_WARNING;
    assert_eq!(flow.next_deadline(), Some(warn_at));
    assert_eq!(flow.idle_warning(), None);
    assert_eq!(flow.tick(warn_at - 1), None);
    assert_eq!(
        flow.tick(warn_at),
        Some(TickEvent::IdleWarning {
            reset_at: 10 + IDLE_RESET
        })
    );
    assert_eq!(flow.idle_warning(), Some(10 + IDLE_RESET));
    assert_eq!(flow.next_deadline(), Some(10 + IDLE_RESET));
    // Only once per idle period.
    assert_eq!(flow.tick(warn_at + 1), None);
    // "I'm still here."
    flow.note_activity(warn_at + 2).unwrap();
    assert_eq!(flow.idle_warning(), None);
    assert_eq!(flow.idle_deadline(), Some(warn_at + 2 + IDLE_RESET));
    assert_eq!(flow.tick(10 + IDLE_RESET), None);
    assert!(flow.has_entered_data());
    // The next idle period warns again.
    assert_eq!(
        flow.tick(warn_at + 2 + IDLE_RESET - IDLE_WARNING),
        Some(TickEvent::IdleWarning {
            reset_at: warn_at + 2 + IDLE_RESET
        })
    );
}

#[test]
fn a_jump_past_the_whole_timeout_skips_the_warning() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();
    assert_eq!(flow.tick(1 + 10 * IDLE_RESET), Some(TickEvent::IdleReset));
    assert_wiped(&flow);
}

#[test]
fn no_warning_when_it_is_switched_off() {
    let config = FlowConfig {
        timeouts: Timeouts {
            idle_warning: 0,
            ..CONFIG.timeouts
        },
        ..CONFIG
    };
    let mut flow = Flow::new(&FORM, config, EPOCH).unwrap();
    flow.start(0).unwrap();
    assert_eq!(flow.next_deadline(), Some(IDLE_RESET));
    assert_eq!(flow.tick(IDLE_RESET - 1), None);
    assert_eq!(flow.tick(IDLE_RESET), Some(TickEvent::IdleReset));
}

#[test]
fn an_action_on_an_expired_session_is_refused_and_wipes_it() {
    // tick() has not been called since the person left (a late timer, say).
    // Whatever the next person presses must not reach the old session.
    type Action = fn(&mut Flow, u64) -> Result<(), FlowError>;
    let actions: [(&str, Action); 9] = [
        ("set_text", |f, t| f.set_text(NAME, "Someone Else", t)),
        ("select", |f, t| f.select(TOPIC, 0, t)),
        ("set_checked", |f, t| f.set_checked(CONSENT, true, t)),
        ("clear_field", |f, t| f.clear_field(NAME, t)),
        ("next", |f, t| f.next(t)),
        ("back", |f, t| f.back(t)),
        ("submit", |f, t| f.submit(t).map(|_| ())),
        ("note_activity", |f, t| f.note_activity(t)),
        ("finish", |f, t| f.finish(t)),
    ];
    for (name, action) in actions {
        let mut flow = new_flow();
        let last = fill_all(&mut flow, 0);
        assert_eq!(
            action(&mut flow, last + IDLE_RESET),
            Err(FlowError::Expired),
            "{name}"
        );
        assert_eq!(flow.phase(), Phase::Welcome, "{name}");
        assert_wiped(&flow);
    }
    // Retry, from a failed submission.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    flow.submission_failed(ticket, FailureReason::Unavailable, t + 2)
        .unwrap();
    assert_eq!(
        flow.retry(t + 2 + IDLE_RESET).err(),
        Some(FlowError::Expired)
    );
    assert_wiped(&flow);
    // Cancel reaches the same end, so it simply succeeds.
    let mut flow = new_flow();
    let last = fill_all(&mut flow, 0);
    assert_eq!(flow.cancel(last + IDLE_RESET), Ok(()));
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_wiped(&flow);
}

#[test]
fn a_submission_in_flight_is_not_cut_short_by_the_idle_timeout() {
    // A long submit timeout, to show the idle timeout is not what applies.
    let config = FlowConfig {
        timeouts: Timeouts {
            submit: 3 * IDLE_RESET,
            ..CONFIG.timeouts
        },
        ..CONFIG
    };
    let mut flow = Flow::new(&FORM, config, EPOCH).unwrap();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    assert_eq!(flow.idle_deadline(), None);
    assert_eq!(flow.tick(t + 1 + 2 * IDLE_RESET), None);
    assert_eq!(flow.phase(), Phase::Submitting { attempt: 1 });
    assert!(flow.has_entered_data());
    // The result still lands, and clears the answers.
    flow.submission_succeeded(ticket, t + 2 + 2 * IDLE_RESET)
        .unwrap();
    assert_wiped(&flow);
}

#[test]
fn unattended_answers_after_a_timed_out_submission_go_within_both_timeouts() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let _ticket = flow.submit(t + 1).unwrap();
    // The person walks away as soon as they press submit.
    let timed_out = t + 1 + SUBMIT;
    assert_eq!(flow.tick(timed_out), Some(TickEvent::SubmitTimedOut));
    assert!(flow.has_entered_data());
    assert_eq!(
        flow.tick(timed_out + IDLE_RESET - IDLE_WARNING),
        Some(TickEvent::IdleWarning {
            reset_at: timed_out + IDLE_RESET
        })
    );
    assert_eq!(
        flow.tick(timed_out + IDLE_RESET),
        Some(TickEvent::IdleReset)
    );
    assert_wiped(&flow);
    // Last action at t + 1; wiped exactly one submit timeout plus one idle
    // timeout later.
    assert_eq!(timed_out + IDLE_RESET, t + 1 + SUBMIT + IDLE_RESET);
}

#[test]
fn repeated_readings_are_not_a_clock_fault() {
    // Several calls in the same tick are normal; only an earlier reading is
    // a fault (see a_clock_that_goes_backwards_wipes_the_session).
    let mut flow = new_flow();
    flow.start(100_000).unwrap();
    flow.set_text(NAME, "Sam Sample", 100_001).unwrap();
    assert_eq!(flow.tick(100_001), None);
    flow.note_activity(100_001).unwrap();
    flow.set_text(MEMBER_NUMBER, "123456", 100_001).unwrap();
    assert_eq!(flow.idle_deadline(), Some(100_001 + IDLE_RESET));
    assert!(flow.has_entered_data());
    assert_eq!(flow.tick(100_001 + IDLE_RESET), Some(TickEvent::IdleReset));
}

#[test]
fn extreme_ticks_saturate_instead_of_overflowing() {
    let mut flow = new_flow();
    let start = u64::MAX - 20;
    flow.start(start).unwrap();
    flow.set_text(NAME, "Sam Sample", start + 1).unwrap();
    // Every deadline saturates at the largest tick, the warning's included,
    // so the warning is skipped and the reset still comes.
    assert_eq!(flow.idle_deadline(), Some(u64::MAX));
    assert_eq!(flow.next_deadline(), Some(u64::MAX));
    assert_eq!(flow.tick(u64::MAX - 1), None);
    assert_eq!(flow.tick(u64::MAX), Some(TickEvent::IdleReset));
    assert_wiped(&flow);
    // A session started at the largest tick has every deadline there too,
    // and the next reading, necessarily earlier, is a clock fault: either
    // way it is wiped at the first chance.
    flow.start(u64::MAX).unwrap();
    assert_eq!(flow.tick(0), Some(TickEvent::IdleReset));
    assert_eq!(flow.phase(), Phase::Welcome);
}

#[test]
fn debug_output_and_errors_never_show_what_was_typed() {
    let secret_ish = "Sam Sample";
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let shown = format!("{flow:?}");
    assert!(!shown.contains(secret_ish), "{shown}");
    assert!(!shown.contains("12345678"), "{shown}");
    assert!(shown.contains("fields_filled: 6"), "{shown}");
    for (_, value) in flow.values() {
        let shown = format!("{value:?}");
        assert!(
            !shown.contains(secret_ish) && !shown.contains("Replacement"),
            "{shown}"
        );
    }
    // A refused value is not echoed back either.
    flow.back(t + 1).unwrap();
    flow.back(t + 2).unwrap();
    flow.back(t + 3).unwrap();
    flow.back(t + 4).unwrap();
    let err = flow.set_text(NAME, "Sam\nSample", t + 5).unwrap_err();
    assert!(!format!("{err:?}").contains("Sam"));
    assert!(!format!("{err}").contains("Sam"));
}

// ---- Regression tests: each failed before the fix it guards -----------------

#[test]
fn a_late_tick_while_submitting_wipes_the_session_instead_of_reopening_it() {
    // The person submits and walks away; the caller's timer fires ten minutes
    // late. On time, the submission would have timed out at its deadline and
    // the session been wiped one idle timeout after that.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    flow.submit(t + 1).unwrap();
    let later = t + 1 + SUBMIT + IDLE_RESET + 600_000;
    assert_eq!(flow.tick(later), Some(TickEvent::IdleReset));
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_wiped(&flow);
    // The next person cannot reach the old answers, nor send them again.
    assert_eq!(
        flow.back(later + 1),
        Err(FlowError::NotAllowed(Phase::Welcome))
    );
    assert_eq!(
        flow.retry(later + 2).err(),
        Some(FlowError::NotAllowed(Phase::Welcome))
    );
}

#[test]
fn a_late_action_while_submitting_finds_the_session_expired() {
    // As above, but the next person touches the screen before the late timer.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    flow.submit(t + 1).unwrap();
    let later = t + 1 + SUBMIT + IDLE_RESET + 600_000;
    assert_eq!(flow.note_activity(later), Err(FlowError::Expired));
    assert_wiped(&flow);
    assert_eq!(flow.tick(later), None);
}

#[test]
fn an_overdue_submission_times_out_as_of_its_deadline() {
    // tick() has not noticed the submit deadline when the person acts within
    // the following idle timeout: the action lands where an on-time tick
    // would have left the flow, on the failure screen.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    flow.submit(t + 1).unwrap();
    let deadline = t + 1 + SUBMIT;
    flow.note_activity(deadline + 5_000).unwrap();
    assert_eq!(
        flow.phase(),
        Phase::Failed {
            reason: FailureReason::TimedOut,
            attempt: 1
        }
    );
    assert_eq!(flow.idle_deadline(), Some(deadline + 5_000 + IDLE_RESET));
    // Without that action, a late tick also dates the failure from the
    // deadline, so the idle timeout is not restarted by the late tick.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    flow.submit(t + 1).unwrap();
    assert_eq!(flow.tick(deadline + 5_000), Some(TickEvent::SubmitTimedOut));
    assert_eq!(flow.idle_deadline(), Some(deadline + IDLE_RESET));
}

#[test]
fn a_late_failure_result_does_not_restart_the_idle_timeout() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    let deadline = t + 1 + SUBMIT;
    // Past the submit deadline (tick has not noticed yet): the result still
    // counts, but the failure dates from the deadline.
    flow.submission_failed(ticket, FailureReason::Unavailable, deadline + 5_000)
        .unwrap();
    assert_eq!(flow.idle_deadline(), Some(deadline + IDLE_RESET));
    // Past the deadline and a whole idle timeout: the session is gone.
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let ticket = flow.submit(t + 1).unwrap();
    assert_eq!(
        flow.submission_failed(
            ticket,
            FailureReason::Unavailable,
            deadline + IDLE_RESET + 600_000
        ),
        Err(FlowError::Expired)
    );
    assert_wiped(&flow);
}

#[test]
fn one_tick_in_the_wrong_unit_does_not_freeze_time_for_later_sessions() {
    let mut flow = new_flow();
    flow.start(1_000).unwrap();
    // One reading in microseconds instead of milliseconds.
    assert_eq!(flow.note_activity(2_000_000), Err(FlowError::Expired));
    // The next person, on the real clock.
    let last = fill_all(&mut flow, 3_000);
    let mut reset_at = None;
    for t in (last..=last + 30 * 60_000).step_by(1_000) {
        if flow.tick(t) == Some(TickEvent::IdleReset) {
            reset_at = Some(t);
            break;
        }
    }
    assert_eq!(reset_at, Some(last + IDLE_RESET));
    assert_wiped(&flow);
}

#[test]
fn a_bogus_reading_at_welcome_does_not_switch_the_idle_reset_off() {
    let mut flow = new_flow();
    assert_eq!(flow.tick(1_000_000_000_000), None);
    let last = fill_all(&mut flow, 2_000);
    assert_eq!(flow.idle_deadline(), Some(last + IDLE_RESET));
    assert_eq!(flow.tick(last + IDLE_RESET), Some(TickEvent::IdleReset));
    assert_wiped(&flow);
}

#[test]
fn a_clock_that_goes_backwards_wipes_the_session() {
    // A time source that restarts from zero after a month of uptime.
    let day: u64 = 24 * 60 * 60 * 1000;
    let mut flow = new_flow();
    fill_all(&mut flow, 30 * day);
    assert_eq!(flow.tick(0), Some(TickEvent::IdleReset));
    assert_wiped(&flow);
    // A 32-bit millisecond counter that wraps during a session.
    let mut flow = new_flow();
    fill_all(&mut flow, u64::from(u32::MAX) - 5_000);
    assert_eq!(flow.note_activity(1_000), Err(FlowError::Expired));
    assert_wiped(&flow);
    // Either way the flow carries on from the new reading.
    let last = fill_all(&mut flow, 2_000);
    assert_eq!(flow.idle_deadline(), Some(last + IDLE_RESET));
    assert!(flow.has_entered_data());
}

#[test]
fn after_the_largest_tick_a_new_time_base_works_normally() {
    let mut flow = new_flow();
    flow.start(u64::MAX).unwrap();
    assert_eq!(flow.tick(u64::MAX), Some(TickEvent::IdleReset));
    // The clock restarts: the next session is not wiped at once.
    flow.start(1).unwrap();
    flow.set_text(NAME, "Sam Sample", 2).unwrap();
    assert_eq!(flow.tick(3), None);
    assert_eq!(flow.idle_deadline(), Some(2 + IDLE_RESET));
    assert!(flow.has_entered_data());
}

#[test]
fn stuck_input_cannot_keep_a_session_alive_for_ever() {
    // Something resting on the screen: a refused action every 30 seconds,
    // for a day. Each one counts as activity, but the session has a
    // maximum length.
    let mut flow = new_flow();
    let start = fill_all(&mut flow, 0) - 10;
    let mut now = start;
    let mut expired_at = None;
    for _ in 0..(24 * 60 * 2) {
        now += 30_000;
        if flow.next(now) == Err(FlowError::Expired) {
            expired_at = Some(now);
            break;
        }
    }
    let expired_at = expired_at.expect("answers survived a day of stuck input");
    assert!(expired_at >= start + MAX_SESSION);
    assert!(expired_at < start + MAX_SESSION + 30_000);
    assert_wiped(&flow);
}

#[test]
fn timeouts_cannot_switch_the_privacy_reset_off() {
    let config = FlowConfig {
        timeouts: Timeouts {
            idle_reset: u64::MAX,
            idle_warning: 0,
            submit: u64::MAX,
            confirmation: 1,
            max_session: u64::MAX,
        },
        max_attempts: 1,
    };
    assert_eq!(config.check(), Err(SetupError::BadTimeouts));
    // Each must be shorter than the maximum session length.
    for timeouts in [
        Timeouts {
            idle_reset: MAX_SESSION,
            idle_warning: 0,
            ..CONFIG.timeouts
        },
        Timeouts {
            submit: MAX_SESSION,
            ..CONFIG.timeouts
        },
    ] {
        let config = FlowConfig { timeouts, ..CONFIG };
        assert_eq!(config.check(), Err(SetupError::BadTimeouts));
    }
}

#[test]
fn ending_the_session_for_a_fault_wipes_it_in_every_phase() {
    // From each phase that holds answers, and from the confirmation.
    for phase in 0..=5 {
        let mut flow = new_flow();
        let mut t = fill_all(&mut flow, 0);
        let mut ticket = None;
        match phase {
            // A step, going back from the review.
            0 => flow.back(t + 1).unwrap(),
            // The review.
            1 => {}
            // A submission in flight, which cancel refuses.
            2 => ticket = Some(flow.submit(t + 1).unwrap()),
            // A failed submission.
            3 => {
                let sent = flow.submit(t + 1).unwrap();
                flow.submission_failed(sent, FailureReason::Unavailable, t + 2)
                    .unwrap();
            }
            // The confirmation, whose answers are already gone.
            4 => {
                let sent = flow.submit(t + 1).unwrap();
                flow.submission_succeeded(sent, t + 2).unwrap();
            }
            // A clock that has gone backwards wipes it as well.
            _ => t = 0,
        }
        assert_eq!(
            flow.phase().holds_answers(),
            phase != 4,
            "{:?}",
            flow.phase()
        );
        flow.end_session(t + 3);
        assert_eq!(flow.phase(), Phase::Welcome);
        assert!(!flow.phase().holds_answers());
        assert_wiped(&flow);
        if let Some(sent) = ticket {
            assert_eq!(
                flow.submission_succeeded(sent, t + 4),
                Err(FlowError::StaleTicket)
            );
        }
        // Nothing to end at the welcome screen; the next session starts
        // empty.
        flow.end_session(t + 5);
        flow.start(t + 6).unwrap();
        assert_wiped(&flow);
    }
}
