// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kitchen sink tours all ten screens with their own buttons, shows a
//! step's error line instead of moving on, carries the answers to the
//! review, back and through a failed send, wipes them once sent or when the
//! tour starts again, and never fails on normal navigation, however often
//! it goes round.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed step fails the test"
)]

use nowayos_display::Rect;
use nowayos_glyphs::{REGULAR_24, REGULAR_48};
use nowayos_input::{InputEvent, KeyCode};
use nowayos_ui::{ActionId, TextMetrics};
use showcase_kiosk_screens::generated::review::review_screen as review;
use showcase_kiosk_screens::generated::step_checkbox::confirm_step as confirm;
use showcase_kiosk_screens::generated::step_choice::question_step as question;
use showcase_kiosk_screens::generated::step_text::about_you_step as about_you;
use showcase_kiosk_screens::sink::{
    BACK, CHECK_ANSWERS, HEIGHT, KitchenSink, NEXT, SEND, START, TOUR, TRY_AGAIN, WIDTH,
    action_name,
};

/// The reference every test that mirrors a QEMU script types, as the
/// script does: a marker every expect file forbids in the log.
const REFERENCE: &str = "9081726354";

fn sink() -> KitchenSink {
    KitchenSink::new(TextMetrics::from_atlas(&REGULAR_24)).unwrap()
}

/// Presses and releases `key`; the action the press reported, if any.
fn key(sink: &mut KitchenSink, key: KeyCode) -> Option<ActionId> {
    let action = sink.handle(InputEvent::KeyDown(key)).unwrap();
    sink.handle(InputEvent::KeyUp(key)).unwrap();
    action
}

fn tabs(sink: &mut KitchenSink, count: usize) {
    for _ in 0..count {
        key(sink, KeyCode::TAB);
    }
}

fn type_text(sink: &mut KitchenSink, text: &str) {
    for ch in text.chars() {
        let code = match ch {
            '0'..='9' => KeyCode::digit(u8::try_from(ch.to_digit(10).unwrap()).unwrap()),
            _ => KeyCode::letter(ch),
        };
        key(sink, code.unwrap());
    }
}

/// From the welcome screen to the review, answering every step: a name
/// and a reference, the first topic, the agreement. Each screen starts with
/// focus on its first control (`Navigator`).
fn to_review(sink: &mut KitchenSink) {
    assert_eq!(key(sink, KeyCode::ENTER), Some(START));
    assert_eq!(sink.screen(), "step-text");
    type_text(sink, "zqvj");
    tabs(sink, 1);
    type_text(sink, REFERENCE);
    tabs(sink, 2);
    assert_eq!(key(sink, KeyCode::ENTER), Some(NEXT));
    assert_eq!(sink.screen(), "step-choice");
    key(sink, KeyCode::DOWN);
    tabs(sink, 4);
    key(sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
    key(sink, KeyCode::SPACE);
    tabs(sink, 4);
    key(sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-notice");
    tabs(sink, 2);
    key(sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "review");
}

/// The review shows the answers `to_review` gives.
fn assert_review_shows_the_answers(sink: &KitchenSink) {
    let tree = sink.tree();
    assert_eq!(tree.slot_text(review::slots::NAME), Some("zqvj"));
    assert_eq!(tree.slot_text(review::slots::REFERENCE), Some(REFERENCE));
    assert_eq!(tree.slot_text(review::slots::TOPIC), Some("Opening hours"));
    assert_eq!(tree.slot_text(review::slots::DETAILS), Some("None given"));
    assert_eq!(tree.slot_text(review::slots::RECEIPT), Some("No"));
}

/// Send from the review (Change, Cancel, Send), then the service's answer:
/// not sent.
fn send_and_fail(sink: &mut KitchenSink) {
    tabs(sink, 2);
    assert_eq!(key(sink, KeyCode::ENTER), Some(SEND));
    assert_eq!(sink.screen(), "submitting");
    key(sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "failed");
}

#[test]
fn it_is_shaped_like_the_nowayos_clickable_demo() {
    // `nowayos-demo`'s `Demo`: a screen laid out first at 800 x 480 (and in
    // the display's layout once it is announced, `resize`), `new(metrics)`, the
    // tree, `handle` returning the action, `action_name` for the log.
    assert_eq!((WIDTH, HEIGHT), (800, 480));
    let sink = sink();
    assert_eq!(sink.tree().viewport().size.width, WIDTH);
    assert_eq!(action_name(START), "Start");
    assert_eq!(action_name(CHECK_ANSWERS), "Check answers");
    assert_eq!(action_name(ActionId(99)), "unknown");
}

/// Once the display announces its layout, the tour is laid out
/// there at the display's scale - the Pi 5's window at 2x - keeping what
/// the screen holds, and every screen after it too.
#[test]
fn the_tour_follows_the_displays_layout_and_scale() {
    let mut sink = sink();
    assert_eq!(key(&mut sink, KeyCode::ENTER), Some(START));
    assert_eq!(sink.screen(), "step-text");
    type_text(&mut sink, "ada");
    let window = Rect::new(0, 0, 1920, 1080);
    let large = TextMetrics::from_atlas(&REGULAR_48)
        .with_scale(nowayos_display::DeviceScale::TWO)
        .with_min_target(48);
    sink.resize(window, large).unwrap();
    assert_eq!(
        (sink.tree().viewport(), sink.tree().scale()),
        (window, nowayos_display::DeviceScale::TWO)
    );
    assert_eq!(sink.tree().value(about_you::fields::NAME), Some("ada"));
    for (_, node) in sink.tree().paint_order() {
        assert!(window.contains_rect(&node.rect()), "{:?}", node.rect());
        if node.is_focusable() {
            assert!(node.rect().size.height >= 48, "{:?}", node.kind());
        }
    }
    tabs(&mut sink, 3);
    assert_eq!(key(&mut sink, KeyCode::ENTER), Some(NEXT));
    assert_eq!(sink.screen(), "step-choice");
    assert_eq!(sink.tree().viewport(), window, "the next screen too");
}

#[test]
fn the_tour_visits_all_ten_screens_with_their_own_buttons() {
    let mut sink = sink();
    let mut seen = vec![sink.screen()];
    to_review(&mut sink);
    assert_review_shows_the_answers(&sink);
    seen.extend([
        "step-text",
        "step-choice",
        "step-checkbox",
        "step-notice",
        "review",
    ]);
    send_and_fail(&mut sink);
    assert!(sink.holds_answers(), "kept for a retry");
    // Not sent: Check answers, Cancel, Try again.
    tabs(&mut sink, 2);
    assert_eq!(key(&mut sink, KeyCode::ENTER), Some(TRY_AGAIN));
    assert_eq!(sink.screen(), "submitted");
    assert!(!sink.holds_answers(), "wiped once sent");
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "idle-warning");
    tabs(&mut sink, 1);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "welcome");
    seen.extend(["submitting", "failed", "submitted", "idle-warning"]);
    assert_eq!(seen, TOUR);
    // The next person's first step holds nothing of the last.
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-text");
    assert!(sink.tree().is_wiped());
    assert_eq!(sink.tree().value(about_you::fields::NAME), Some(""));
}

#[test]
fn a_failed_send_keeps_the_answers_and_never_fills_the_back_stack() {
    let mut sink = sink();
    to_review(&mut sink);
    for round in 0..12 {
        send_and_fail(&mut sink);
        // Check answers: back to the review, every answer still there.
        assert_eq!(
            key(&mut sink, KeyCode::ENTER),
            Some(CHECK_ANSWERS),
            "round {round}"
        );
        assert_eq!(sink.screen(), "review");
        assert_review_shows_the_answers(&sink);
    }
    // Back from the review still goes to the step before it.
    assert_eq!(key(&mut sink, KeyCode::ENTER), Some(BACK));
    assert_eq!(sink.screen(), "step-notice");
    // Whole tours, round and round: handle() never fails.
    let mut sink = self::sink();
    for _ in 0..12 {
        to_review(&mut sink);
        send_and_fail(&mut sink);
        tabs(&mut sink, 2);
        key(&mut sink, KeyCode::ENTER);
        key(&mut sink, KeyCode::ENTER);
        tabs(&mut sink, 1);
        key(&mut sink, KeyCode::ENTER);
        assert_eq!(sink.screen(), "welcome");
    }
}

#[test]
fn next_with_an_answer_missing_shows_its_error_line_instead() {
    let mut sink = sink();
    key(&mut sink, KeyCode::ENTER);
    let before = sink.tree().paint_order().count();
    // Name left empty: Tab from it to Next (reference, Cancel, Next).
    tabs(&mut sink, 3);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-text", "the step stays");
    assert_eq!(
        sink.tree().slot_text(about_you::slots::NAME_ERROR),
        Some("Enter your name")
    );
    assert_eq!(
        sink.tree().visible(about_you::groups::NAME_ERROR),
        Some(true)
    );
    assert_eq!(sink.tree().paint_order().count(), before + 1);
    // Named: on to the question, where no topic is chosen. Focus is still
    // on Next: Tab round to the name (Next, then the name, wrapping).
    tabs(&mut sink, 1);
    type_text(&mut sink, "zqvj");
    tabs(&mut sink, 3);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-choice");
    // No topic: the radio group, details, Back, Cancel, Next.
    tabs(&mut sink, 4);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-choice", "the step stays");
    assert_eq!(
        sink.tree().slot_text(question::slots::TOPIC_ERROR),
        Some("Choose what it is about")
    );
    assert_eq!(
        sink.tree().visible(question::groups::TOPIC_ERROR),
        Some(true)
    );
    // A topic: on to the agreement, left unticked.
    tabs(&mut sink, 1);
    key(&mut sink, KeyCode::DOWN);
    tabs(&mut sink, 4);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
    tabs(&mut sink, 4);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox", "the step stays");
    assert_eq!(
        sink.tree().slot_text(confirm::slots::AGREE_ERROR),
        Some("Tick this to send your answers")
    );
    assert_eq!(
        sink.tree().visible(confirm::groups::AGREE_ERROR),
        Some(true)
    );
}

#[test]
fn back_keeps_what_a_step_shows_now_not_what_it_showed_at_next() {
    // Untick the agreement, go Back, then Next: it stays unticked.
    let mut sink = sink();
    to_review(&mut sink);
    key(&mut sink, KeyCode::ENTER);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
    assert_eq!(sink.tree().checked(confirm::fields::AGREE), Some(true));
    key(&mut sink, KeyCode::SPACE);
    assert_eq!(sink.tree().checked(confirm::fields::AGREE), Some(false));
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-choice");
    tabs(&mut sink, 4);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
    assert_eq!(sink.tree().checked(confirm::fields::AGREE), Some(false));
    // And Next refuses it until it is ticked again.
    tabs(&mut sink, 4);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
}

#[test]
fn back_brings_the_answers_back_and_cancel_wipes_them() {
    let mut sink = sink();
    to_review(&mut sink);
    // Change, then Back on each step: the notice, the agreement, the
    // question, the name - each Back after the step's own controls - with
    // each step's answers set back.
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-notice");
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-checkbox");
    assert_eq!(sink.tree().checked(confirm::fields::AGREE), Some(true));
    assert_eq!(sink.tree().checked(confirm::fields::RECEIPT), Some(false));
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-choice");
    assert_eq!(sink.tree().selected(question::fields::TOPIC), Some(0));
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-text");
    assert_eq!(sink.tree().value(about_you::fields::NAME), Some("zqvj"));
    assert_eq!(
        sink.tree().value(about_you::fields::REFERENCE),
        Some(REFERENCE)
    );
    assert!(sink.holds_answers());
    // An edit, then Next and Back: the step shows the edit, not the answer
    // saved before it.
    tabs(&mut sink, 1);
    key(&mut sink, KeyCode::BACKSPACE);
    type_text(&mut sink, "7");
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-choice");
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-text");
    assert_eq!(
        sink.tree().value(about_you::fields::REFERENCE),
        Some("9081726357")
    );
    assert!(!sink.tree().is_wiped());
    // Cancel: after the step's two fields. Then the next person's first
    // step, and the next, hold nothing.
    tabs(&mut sink, 2);
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "welcome");
    assert!(!sink.holds_answers());
    key(&mut sink, KeyCode::ENTER);
    assert_eq!(sink.screen(), "step-text");
    assert!(sink.tree().is_wiped());
    assert_eq!(sink.tree().value(about_you::fields::NAME), Some(""));
    assert_eq!(sink.tree().value(about_you::fields::REFERENCE), Some(""));
}
