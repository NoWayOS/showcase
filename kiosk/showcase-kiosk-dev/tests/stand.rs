// SPDX-License-Identifier: MIT OR Apache-2.0
//! The stand, with the reference kiosk on it: its screens and its
//! application joined as the window drives them - input in batches, time by
//! the clock - with no window.
//!
//! * It starts at the welcome screen and draws the Pi's whole monitor.
//! * A key pressed reaches the screens, and the application's answer comes
//!   back: the form's first step.
//! * Time alone ends a session left idle, and the screen says so.
//! * Screens given the wrong number of applications are refused.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed step fails the test"
)]

use nowayos_dev_window::{Application, START};
use nowayos_input::{InputEvent, KeyCode};
use showcase_kiosk_dev::{Shape, Stand};
use showcase_kiosk_screens::app::KioskApp;
use showcase_kiosk_screens::kiosk::KioskUi;

fn stand() -> Stand<KioskUi> {
    Stand::new(KioskUi::new, vec![KioskApp::new(1).unwrap()], Shape::PI).unwrap()
}

fn press(stand: &mut Stand<KioskUi>, key: KeyCode, now: u64) {
    stand.advance(now);
    stand.input(&[InputEvent::KeyDown(key), InputEvent::KeyUp(key)], now);
}

#[test]
fn it_starts_at_the_welcome_screen_and_draws_the_whole_monitor() {
    let mut stand = stand();
    assert_eq!(stand.status(), "welcome");
    let frame = stand.draw().unwrap();
    assert_eq!((frame.width(), frame.height()), (1920, 1080));
    // Not one flat colour: the screen is drawn.
    let first = frame.pixel(0, 0).unwrap();
    assert!(frame.pixels().iter().any(|&pixel| pixel != first));
}

#[test]
fn a_key_reaches_the_screens_and_the_applications_answer_comes_back() {
    let mut stand = stand();
    press(&mut stand, KeyCode::ENTER, START + 100);
    assert_eq!(stand.status(), "step-text");
    let before = stand.draw().unwrap();
    press(&mut stand, KeyCode::A, START + 200);
    assert_eq!(stand.status(), "step-text");
    assert!(
        before.diff(&stand.draw().unwrap()).is_some(),
        "the letter typed is drawn"
    );
}

#[test]
fn time_alone_ends_a_session_left_idle() {
    let mut stand = stand();
    press(&mut stand, KeyCode::ENTER, START + 100);
    assert_eq!(stand.status(), "step-text");
    // Nothing for a moment changes nothing.
    assert!(!stand.advance(START + 200));
    // An hour later the session has ended and the kiosk is ready again.
    assert!(stand.advance(START + 3_600_000));
    assert_eq!(stand.status(), "welcome");
}

#[test]
fn screens_given_the_wrong_number_of_applications_are_refused() {
    let Err(why) = Stand::new(KioskUi::new, Vec::new(), Shape::PI) else {
        panic!("a kiosk with no application was accepted");
    };
    assert!(why.contains("1 application"), "{why}");
}
