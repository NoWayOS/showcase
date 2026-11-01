// SPDX-License-Identifier: MIT OR Apache-2.0
//! Moving through the form: start, answers, next, back, the review and
//! cancel.

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
    Charset, FieldProblem, FlowError, Issue, Phase, StepId, TextRule, Value,
};

fn problem(field: showcase_kiosk_flow::FieldId, issue: Issue) -> FieldProblem {
    FieldProblem { field, issue }
}

#[test]
fn a_new_flow_waits_at_welcome_holding_nothing() {
    let flow = new_flow();
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_eq!(flow.session(), 0);
    assert_eq!(flow.current_step(), None);
    assert_eq!(flow.next_deadline(), None);
    assert_eq!(flow.idle_deadline(), None);
    assert_eq!(flow.problems().count(), 0);
    assert_wiped(&flow);
}

#[test]
fn start_opens_a_new_session_on_the_first_step() {
    let mut flow = new_flow();
    flow.start(100).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 0 });
    assert_eq!(flow.current_step().map(|step| step.id), Some(StepId(10)));
    assert_eq!(flow.session(), 1);
    assert_eq!(flow.attempts(), 0);
    assert_eq!(
        flow.start(200),
        Err(FlowError::NotAllowed(Phase::Step { index: 0 }))
    );
    assert_eq!(flow.session(), 1);
}

#[test]
fn next_refuses_an_incomplete_step_and_names_the_first_problem() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    assert_eq!(
        flow.next(1),
        Err(FlowError::Incomplete(problem(NAME, Issue::Missing)))
    );
    assert_eq!(flow.phase(), Phase::Step { index: 0 });
}

#[test]
fn problems_lists_every_field_that_needs_attention_on_the_step() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    // Optional, but if given it must be at least six digits.
    flow.set_text(MEMBER_NUMBER, "123", 1).unwrap();
    let problems: Vec<FieldProblem> = flow.problems().collect();
    assert_eq!(
        problems,
        [
            problem(NAME, Issue::Missing),
            problem(MEMBER_NUMBER, Issue::TooShort)
        ]
    );
    // Only the current step is reported.
    assert!(
        problems
            .iter()
            .all(|p| p.field == NAME || p.field == MEMBER_NUMBER)
    );

    flow.set_text(NAME, "S", 2).unwrap();
    assert_eq!(flow.problems().next(), Some(problem(NAME, Issue::TooShort)));
    flow.set_text(NAME, "Sam Sample", 3).unwrap();
    flow.set_text(MEMBER_NUMBER, "123456", 4).unwrap();
    assert_eq!(flow.problems().count(), 0);
    flow.next(5).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 1 });
}

#[test]
fn optional_fields_may_stay_empty() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();
    flow.next(2).unwrap(); // no membership number
    flow.select(TOPIC, 0, 3).unwrap();
    flow.next(4).unwrap(); // no details
    flow.next(5).unwrap();
    flow.set_checked(CONSENT, true, 6).unwrap();
    flow.next(7).unwrap(); // no receipt
    assert_eq!(flow.phase(), Phase::Review);
    assert_eq!(flow.value(MEMBER_NUMBER), Some(Value::Text("")));
    assert_eq!(flow.value(RECEIPT), Some(Value::Checkbox(false)));
}

#[test]
fn walking_forward_and_back_keeps_every_answer() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    flow.back(t + 1).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 3 });
    flow.back(t + 2).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 2 });
    flow.back(t + 3).unwrap();
    flow.back(t + 4).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 0 });
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));
    assert_eq!(flow.value(TOPIC), Some(Value::Choice(Some(2))));
    assert_eq!(flow.value(CONSENT), Some(Value::Checkbox(true)));
    for step in 0..4 {
        assert_eq!(flow.phase(), Phase::Step { index: step });
        flow.next(t + 10 + step as u64).unwrap();
    }
    assert_eq!(flow.phase(), Phase::Review);
}

#[test]
fn back_on_the_first_step_is_refused() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    assert_eq!(flow.back(1), Err(FlowError::AtFirstStep));
    assert_eq!(flow.phase(), Phase::Step { index: 0 });
}

#[test]
fn a_step_without_fields_is_always_complete() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();
    flow.next(2).unwrap();
    flow.select(TOPIC, 1, 3).unwrap();
    flow.next(4).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 2 });
    assert_eq!(flow.current_step().map(|step| step.fields.len()), Some(0));
    assert_eq!(flow.problems().count(), 0);
    flow.next(5).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 3 });
}

#[test]
fn review_follows_the_last_step_and_back_returns_to_it() {
    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    assert_eq!(flow.current_step(), None);
    assert_eq!(flow.problems().count(), 0);
    flow.back(t + 1).unwrap();
    assert_eq!(flow.phase(), Phase::Step { index: 3 });
    flow.next(t + 2).unwrap();
    assert_eq!(flow.phase(), Phase::Review);
}

#[test]
fn only_fields_on_the_current_step_can_be_edited() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    assert_eq!(
        flow.select(TOPIC, 1, 1),
        Err(FlowError::NotOnThisStep(TOPIC))
    );
    assert_eq!(
        flow.set_checked(CONSENT, true, 2),
        Err(FlowError::NotOnThisStep(CONSENT))
    );
    assert_eq!(
        flow.clear_field(DETAILS, 3),
        Err(FlowError::NotOnThisStep(DETAILS))
    );
    assert!(!flow.has_entered_data());
}

#[test]
fn unknown_fields_and_the_wrong_kind_of_value_are_refused() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    assert_eq!(
        flow.set_text(NO_SUCH_FIELD, "x", 1),
        Err(FlowError::UnknownField(NO_SUCH_FIELD))
    );
    assert_eq!(flow.value(NO_SUCH_FIELD), None);
    assert_eq!(flow.select(NAME, 0, 2), Err(FlowError::WrongKind(NAME)));
    assert_eq!(
        flow.set_checked(NAME, true, 3),
        Err(FlowError::WrongKind(NAME))
    );
    flow.set_text(NAME, "Sam Sample", 4).unwrap();
    flow.next(5).unwrap();
    assert_eq!(
        flow.set_text(TOPIC, "2", 6),
        Err(FlowError::WrongKind(TOPIC))
    );
}

#[test]
fn text_is_checked_as_it_is_entered_and_a_refusal_keeps_the_old_value() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();

    let too_long = "x".repeat(33);
    assert_eq!(
        flow.set_text(NAME, &too_long, 2),
        Err(FlowError::InvalidValue(problem(NAME, Issue::TooLong)))
    );
    for bad in ["Sam\nSample", "Sam\tSample", "Sam\u{1b}[2J", "Sam\u{7f}"] {
        assert_eq!(
            flow.set_text(NAME, bad, 3),
            Err(FlowError::InvalidValue(problem(NAME, Issue::BadCharacter)))
        );
    }
    assert_eq!(flow.value(NAME), Some(Value::Text("Sam Sample")));

    // The limit is in bytes: 16 two-byte letters fill the 32-byte field.
    let accented = "é".repeat(16);
    flow.set_text(NAME, &accented, 4).unwrap();
    assert_eq!(
        flow.set_text(NAME, &"é".repeat(17), 5),
        Err(FlowError::InvalidValue(problem(NAME, Issue::TooLong)))
    );
    assert_eq!(flow.value(NAME), Some(Value::Text(accented.as_str())));

    assert_eq!(
        flow.set_text(MEMBER_NUMBER, "12 34 56", 6),
        Err(FlowError::InvalidValue(problem(
            MEMBER_NUMBER,
            Issue::BadCharacter
        )))
    );
    flow.set_text(MEMBER_NUMBER, "0123456789", 7).unwrap();

    // Empty text clears the field.
    flow.set_text(MEMBER_NUMBER, "", 8).unwrap();
    assert_eq!(flow.value(MEMBER_NUMBER), Some(Value::Text("")));
}

#[test]
fn a_blank_required_field_counts_as_missing() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "     ", 1).unwrap();
    assert!(flow.has_entered_data());
    assert_eq!(
        flow.next(2),
        Err(FlowError::Incomplete(problem(NAME, Issue::Missing)))
    );
}

#[test]
fn choices_must_exist_and_checkboxes_can_be_unchecked() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();
    flow.next(2).unwrap();
    assert_eq!(
        flow.select(TOPIC, 4, 3),
        Err(FlowError::InvalidValue(problem(TOPIC, Issue::NoSuchOption)))
    );
    assert_eq!(flow.value(TOPIC), Some(Value::Choice(None)));
    flow.select(TOPIC, 3, 4).unwrap();
    flow.select(TOPIC, 0, 5).unwrap();
    assert_eq!(flow.value(TOPIC), Some(Value::Choice(Some(0))));
    flow.next(6).unwrap();
    flow.next(7).unwrap();

    flow.set_checked(CONSENT, true, 8).unwrap();
    flow.set_checked(CONSENT, false, 9).unwrap();
    assert_eq!(
        flow.next(10),
        Err(FlowError::Incomplete(problem(CONSENT, Issue::Missing)))
    );
}

#[test]
fn clear_field_empties_one_field_of_any_kind() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    flow.set_text(NAME, "Sam Sample", 1).unwrap();
    flow.set_text(MEMBER_NUMBER, "123456", 2).unwrap();
    flow.clear_field(NAME, 3).unwrap();
    assert_eq!(flow.value(NAME), Some(Value::Text("")));
    assert_eq!(flow.value(MEMBER_NUMBER), Some(Value::Text("123456")));
    flow.set_text(NAME, "Sam Sample", 4).unwrap();
    flow.next(5).unwrap();
    flow.select(TOPIC, 1, 6).unwrap();
    flow.clear_field(TOPIC, 7).unwrap();
    assert_eq!(flow.value(TOPIC), Some(Value::Choice(None)));
}

#[test]
fn actions_are_refused_where_they_do_not_apply() {
    let mut flow = new_flow();
    let welcome = Err(FlowError::NotAllowed(Phase::Welcome));
    assert_eq!(flow.set_text(NAME, "x", 1), welcome);
    assert_eq!(flow.clear_field(NAME, 1), welcome);
    assert_eq!(flow.next(1), welcome);
    assert_eq!(flow.back(1), welcome);
    assert_eq!(
        flow.submit(1).err(),
        Some(FlowError::NotAllowed(Phase::Welcome))
    );
    assert_eq!(
        flow.retry(1).err(),
        Some(FlowError::NotAllowed(Phase::Welcome))
    );

    flow.start(2).unwrap();
    let step = FlowError::NotAllowed(Phase::Step { index: 0 });
    assert_eq!(flow.submit(3).err(), Some(step));
    assert_eq!(flow.retry(3).err(), Some(step));
    assert_eq!(flow.finish(3), Err(step));

    let mut flow = new_flow();
    let t = fill_all(&mut flow, 0);
    let review = FlowError::NotAllowed(Phase::Review);
    assert_eq!(flow.set_text(NAME, "x", t + 1), Err(review));
    assert_eq!(flow.next(t + 1), Err(review));
    assert_eq!(flow.start(t + 1), Err(review));
    assert_eq!(flow.retry(t + 1).err(), Some(review));
    assert_eq!(flow.finish(t + 1), Err(review));
    assert_eq!(flow.phase(), Phase::Review);
}

#[test]
fn cancel_wipes_the_session_from_any_step_or_the_review() {
    for back_steps in 0..5 {
        let mut flow = new_flow();
        let mut t = fill_all(&mut flow, 1_000);
        for _ in 0..back_steps {
            t += 1;
            flow.back(t).unwrap();
        }
        assert!(flow.has_entered_data());
        flow.cancel(t + 1).unwrap();
        assert_eq!(flow.phase(), Phase::Welcome);
        assert_wiped(&flow);
        assert_eq!(flow.next_deadline(), None);
    }
    // Cancelling at Welcome does nothing.
    let mut flow = new_flow();
    flow.cancel(0).unwrap();
    assert_eq!(flow.phase(), Phase::Welcome);
    assert_eq!(flow.session(), 0);
}

#[test]
fn each_session_gets_a_new_number_and_starts_empty() {
    let mut flow = new_flow();
    for session in 1..=3u32 {
        let t = u64::from(session) * 1_000;
        flow.start(t).unwrap();
        assert_eq!(flow.session(), session);
        assert_wiped(&flow);
        flow.set_text(NAME, "Sam Sample", t + 1).unwrap();
        flow.cancel(t + 2).unwrap();
    }
}

// ---- Regression tests: each failed before the fix it guards -----------------

#[test]
fn text_refuses_line_breaks_bidi_controls_and_invisible_characters() {
    let rule = TextRule::new(Charset::Text, 0, 48);
    for bad in [
        "Sam\u{2028}Sample",     // line separator
        "Sam\u{2029}Sample",     // paragraph separator
        "Sam\u{202E}elpmaS",     // right-to-left override
        "Sam \u{2066}x\u{2069}", // directional isolates
        "Sam\u{200B}Sample",     // zero-width space
        "Sam\u{200D}Sample",     // zero-width joiner
        "\u{FEFF}Sam",           // byte order mark
        "Sam\u{AD}Sample",       // soft hyphen
        "Sam\u{A0}Sample",       // no-break space: looks like a space, is not one
        "Sam\u{E000}",           // private use
        "\u{3164}",              // Hangul filler: a letter that draws nothing
        "Sam\u{FFFF}",           // noncharacter
    ] {
        assert_eq!(rule.entry_issue(bad), Some(Issue::BadCharacter), "{bad:?}");
    }
    for good in [
        "Sam Sample",
        "Zoë Façade-Ødegård",
        "O'Brien, J.",
        "£5 & 10% (approx.)",
    ] {
        assert_eq!(rule.entry_issue(good), None, "{good:?}");
    }
}

#[test]
fn a_required_field_cannot_be_completed_with_invisible_characters() {
    let mut flow = new_flow();
    flow.start(0).unwrap();
    for invisible in ["\u{200B}\u{200B}", "\u{3164}\u{3164}", "\u{A0}\u{A0}"] {
        assert_eq!(
            flow.set_text(NAME, invisible, 1),
            Err(FlowError::InvalidValue(problem(NAME, Issue::BadCharacter))),
            "{invisible:?}"
        );
    }
    assert_eq!(
        flow.next(2),
        Err(FlowError::Incomplete(problem(NAME, Issue::Missing)))
    );
}
