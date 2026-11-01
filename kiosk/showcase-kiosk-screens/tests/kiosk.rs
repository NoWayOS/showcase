// SPDX-License-Identifier: MIT OR Apache-2.0
//! The reference kiosk end to end, on the host: the UI
//! (`kiosk::KioskUi`) and the application (`app::KioskApp`: the flow on the
//! reference form behind the contract's application end, with the service
//! stand-in) joined as the two PDs join them, every record encoded and
//! decoded on the way, and time passing as the application PD's timer
//! would let it - the faults included. The keys are the ones the QEMU and
//! Pi tests type.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed step fails the test"
)]

use nowayos_display::Rect;
use nowayos_glyphs::REGULAR_24;
use nowayos_input::{InputEvent, KeyCode};
use nowayos_ui::{ActionId, NodeKind, SlotId, TextMetrics};
use showcase_kiosk_contract::{
    Action, Countdown, EndReason, IdleWarning, MAX_RECORD_LEN, Report, ValueRefused, decode_action,
};
use showcase_kiosk_flow::{Issue, Phase, Tick};
use showcase_kiosk_screens::app::{Fault, KioskApp, Note, PENDING};
use showcase_kiosk_screens::form::NAME;
use showcase_kiosk_screens::generated::failed::failed_screen as failed;
use showcase_kiosk_screens::generated::idle_warning::idle_warning_screen as idle;
use showcase_kiosk_screens::generated::review::review_screen as review;
use showcase_kiosk_screens::generated::step_text::about_you_step as about_you;
use showcase_kiosk_screens::generated::submitted::submitted_screen as submitted;
use showcase_kiosk_screens::kiosk::{KioskUi, KioskUiError};

/// The reference every test that mirrors a QEMU script types, as the
/// script does: a marker every expect file forbids in the log.
const REFERENCE: &str = "9081726354";

/// The two PDs, joined.
struct Kiosk {
    ui: KioskUi,
    app: KioskApp,
    now: Tick,
    /// Every report the UI was sent.
    reports: Vec<Report>,
    /// Every action the application was sent.
    actions: Vec<Action>,
    /// Every note the application made for its log.
    notes: Vec<Note>,
}

impl Kiosk {
    fn new() -> Self {
        Self {
            ui: KioskUi::new(TextMetrics::from_atlas(&REGULAR_24)).unwrap(),
            app: KioskApp::new(7).unwrap(),
            now: 1_000,
            reports: Vec::new(),
            actions: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn screen(&self) -> &'static str {
        self.ui.screen()
    }

    /// Sends the UI's actions to the application, and its reports back,
    /// as the two PDs do.
    fn pump(&mut self) {
        loop {
            self.deliver();
            let actions: Vec<Action> = self.ui.actions().cloned().collect();
            if actions.is_empty() {
                return;
            }
            self.ui.sent(actions.len());
            for action in actions {
                let mut slot = [0u8; MAX_RECORD_LEN];
                action.encode(&mut slot).unwrap();
                assert_eq!(decode_action(&slot).unwrap(), action);
                self.actions.push(action);
                assert!(self.app.takes_actions() || self.app.fault().is_some());
                self.app.receive(&slot, self.now);
                self.deliver();
            }
        }
    }

    /// Sends the application's waiting reports to the UI.
    fn deliver(&mut self) {
        let reports: Vec<Report> = self.app.reports().copied().collect();
        self.app.sent(reports.len());
        for report in reports {
            let mut slot = [0u8; MAX_RECORD_LEN];
            report.encode(&mut slot).unwrap();
            self.reports.push(report);
            self.ui.receive_report(&slot).unwrap();
        }
        while let Some(note) = self.app.take_note() {
            self.notes.push(note);
        }
    }

    /// Lets `ms` pass, waking the application at each deadline as its
    /// timer would.
    fn wait(&mut self, ms: Tick) {
        let until = self.now + ms;
        while let Some(at) = self.app.next_wake().filter(|&at| at <= until) {
            self.now = self.now.max(at);
            self.app.tick(self.now);
            self.pump();
        }
        self.now = until;
    }

    /// Lets time pass to the application's next wake, exactly.
    fn step(&mut self) {
        let at = self.app.next_wake().unwrap();
        self.wait(at - self.now);
    }

    /// The area the slot `slot` of the screen showing takes.
    fn slot_rect(&self, slot: SlotId) -> Rect {
        let tree = self.ui.tree();
        tree.get(tree.find_slot(slot).unwrap()).unwrap().rect()
    }

    /// Only the countdown's digits are repainted: one area, inside
    /// the slot, under a fifth of its line.
    fn assert_only_digits_repainted(&self, slot: SlotId) {
        let line = self.slot_rect(slot);
        let dirty = self.ui.tree().dirty_rects();
        assert_eq!(dirty.len(), 1, "{dirty:?}");
        assert!(line.contains_rect(&dirty[0]), "{dirty:?} outside {line:?}");
        assert!(
            dirty[0].size.width * 5 < line.size.width,
            "{dirty:?} of {line:?}"
        );
    }

    /// The PD cannot go on, for `fault`: what it does.
    fn fail(&mut self, fault: Fault) {
        self.app.fail(fault, self.now);
        self.pump();
    }

    /// Clicks the button that reports `action` on the screen showing.
    fn click(&mut self, action: ActionId) -> Option<&'static str> {
        let tree = self.ui.tree();
        let (_, button) = tree
            .paint_order()
            .find(|(_, node)| {
                matches!(node.kind(), NodeKind::Button { action: a, .. } if *a == action)
            })
            .unwrap();
        let centre = button.rect().center();
        self.ui.handle(InputEvent::PointerMove {
            x: centre.x,
            y: centre.y,
        });
        self.ui.handle(InputEvent::PointerButton {
            button: 0,
            down: true,
        });
        let label = self.ui.handle(InputEvent::PointerButton {
            button: 0,
            down: false,
        });
        self.ui.end_input();
        self.pump();
        label
    }

    /// Shift-Tab, as one batch.
    fn shift_tab(&mut self) {
        for event in [
            InputEvent::KeyDown(KeyCode::LEFT_SHIFT),
            InputEvent::KeyDown(KeyCode::TAB),
            InputEvent::KeyUp(KeyCode::TAB),
            InputEvent::KeyUp(KeyCode::LEFT_SHIFT),
        ] {
            assert_eq!(self.ui.handle(event), None);
        }
        self.ui.end_input();
        self.pump();
    }

    /// A click on the button that reports `action`, its press and its
    /// release in batches of their own, each sent before the next.
    fn slow_click(&mut self, action: ActionId) -> Option<&'static str> {
        let tree = self.ui.tree();
        let (_, button) = tree
            .paint_order()
            .find(|(_, node)| {
                matches!(node.kind(), NodeKind::Button { action: a, .. } if *a == action)
            })
            .unwrap();
        let centre = button.rect().center();
        self.ui.handle(InputEvent::PointerMove {
            x: centre.x,
            y: centre.y,
        });
        self.ui.handle(InputEvent::PointerButton {
            button: 0,
            down: true,
        });
        self.ui.end_input();
        self.pump();
        let label = self.ui.handle(InputEvent::PointerButton {
            button: 0,
            down: false,
        });
        self.ui.end_input();
        self.pump();
        label
    }

    /// Neither end holds anything a person entered.
    fn assert_holds_nothing(&self, when: &str) {
        assert!(self.ui.holds_nothing(), "the UI, {when}");
        assert!(
            !self.app.flow().unwrap().has_entered_data(),
            "the application, {when}"
        );
    }

    /// One batch of input: each key pressed and released, then the batch
    /// ended and its actions sent. Returns the labels of the buttons
    /// pressed.
    fn type_keys(&mut self, keys: &[KeyCode]) -> Vec<&'static str> {
        let mut pressed = Vec::new();
        for &key in keys {
            pressed.extend(self.ui.handle(InputEvent::KeyDown(key)));
            pressed.extend(self.ui.handle(InputEvent::KeyUp(key)));
        }
        self.ui.end_input();
        self.pump();
        self.now += 100;
        pressed
    }

    fn key(&mut self, key: KeyCode) -> Vec<&'static str> {
        self.type_keys(&[key])
    }

    fn text(&mut self, text: &str) {
        let keys: Vec<KeyCode> = text
            .chars()
            .map(|ch| match ch {
                '0'..='9' => KeyCode::digit(u8::try_from(ch.to_digit(10).unwrap()).unwrap()),
                _ => KeyCode::letter(ch),
            })
            .map(Option::unwrap)
            .collect();
        self.type_keys(&keys);
    }

    fn tabs(&mut self, count: usize) {
        self.type_keys(&vec![KeyCode::TAB; count]);
    }

    /// The session ends the reports name, in order.
    fn ended(&self) -> Vec<EndReason> {
        self.reports
            .iter()
            .filter_map(|report| match report {
                Report::SessionEnded(ended) => Some(ended.reason),
                _ => None,
            })
            .collect()
    }

    /// From the welcome screen to the review, as `systems/kiosk.qmp` types
    /// it: a name and a reference, the first topic, the agreement.
    fn go_to_review(&mut self) {
        assert_eq!(self.key(KeyCode::ENTER), ["Start"]);
        assert_eq!(self.screen(), "step-text");
        self.text("zqvj");
        self.tabs(1);
        self.text(REFERENCE);
        self.tabs(2);
        assert_eq!(self.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(self.screen(), "step-choice");
        self.key(KeyCode::DOWN);
        self.tabs(4);
        self.key(KeyCode::ENTER);
        assert_eq!(self.screen(), "step-checkbox");
        self.key(KeyCode::SPACE);
        self.tabs(4);
        self.key(KeyCode::ENTER);
        assert_eq!(self.screen(), "step-notice");
        self.tabs(2);
        assert_eq!(self.key(KeyCode::ENTER), ["Continue"]);
        assert_eq!(self.screen(), "review");
    }
}

#[test]
fn the_flow_drives_every_screen_and_a_retry_is_accepted() {
    let mut kiosk = Kiosk::new();
    assert_eq!(kiosk.screen(), "welcome");
    kiosk.go_to_review();
    let tree = kiosk.ui.tree();
    assert_eq!(tree.slot_text(review::slots::NAME), Some("zqvj"));
    assert_eq!(tree.slot_text(review::slots::REFERENCE), Some(REFERENCE));
    assert_eq!(tree.slot_text(review::slots::TOPIC), Some("Opening hours"));
    assert_eq!(tree.slot_text(review::slots::DETAILS), Some("None given"));
    assert_eq!(tree.slot_text(review::slots::RECEIPT), Some("No"));
    // The application holds the same answers.
    assert!(kiosk.app.flow().unwrap().has_entered_data());

    // Send (Change, Cancel, Send): the stand-in leaves the first attempt
    // unanswered, so the submit timeout fails it.
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Send"]);
    assert_eq!(kiosk.screen(), "submitting");
    kiosk.wait(4_000);
    assert_eq!(kiosk.screen(), "submitting");
    kiosk.wait(1_500);
    assert_eq!(kiosk.screen(), "failed");
    let tree = kiosk.ui.tree();
    assert_eq!(tree.visible(failed::groups::CAN_RETRY), Some(true));
    assert_eq!(tree.visible(failed::groups::REJECTED), Some(false));
    assert_eq!(tree.slot_text(failed::slots::ATTEMPTS_LEFT), Some("2"));

    // Try again (Check answers, Cancel, Try again): accepted a second later.
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Try again"]);
    assert_eq!(kiosk.screen(), "submitting");
    kiosk.wait(1_000);
    assert_eq!(kiosk.screen(), "submitted");
    assert_eq!(kiosk.ended(), [EndReason::Submitted]);
    assert!(kiosk.ui.holds_nothing(), "the UI wiped its copies");
    assert!(!kiosk.app.flow().unwrap().has_entered_data());

    // Done: the welcome screen, and the next person's first step is empty.
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Done"]);
    assert_eq!(kiosk.screen(), "welcome");
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some(""));
    assert!(kiosk.ui.holds_nothing());
}

#[test]
fn the_confirmation_ends_by_itself() {
    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    kiosk.tabs(2);
    kiosk.key(KeyCode::ENTER);
    kiosk.wait(6_000);
    kiosk.tabs(2);
    kiosk.key(KeyCode::ENTER);
    kiosk.wait(1_000);
    assert_eq!(kiosk.screen(), "submitted");
    kiosk.wait(10_000);
    assert_eq!(kiosk.screen(), "welcome");
}

#[test]
fn next_is_a_request_the_application_can_refuse() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    // No name: Tab to Next (reference, Cancel, Next).
    kiosk.tabs(3);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Next"]);
    assert_eq!(kiosk.screen(), "step-text", "the step stays");
    let tree = kiosk.ui.tree();
    assert_eq!(
        tree.slot_text(about_you::slots::NAME_ERROR),
        Some("Enter your name")
    );
    assert_eq!(tree.visible(about_you::groups::NAME_ERROR), Some(true));
    assert!(matches!(kiosk.reports.last(), Some(Report::Incomplete(_))));
    // Named (focus is still on Next: Tab round to the name), and on.
    kiosk.tabs(1);
    kiosk.text("bo");
    kiosk.tabs(3);
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "step-choice");
    // No topic: the radio group, details, Back, Cancel, Next.
    kiosk.tabs(4);
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "step-choice");
    assert!(matches!(kiosk.reports.last(), Some(Report::Incomplete(_))));
}

#[test]
fn back_shows_a_step_again_with_its_answers() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.tabs(3);
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "step-choice");
    // Back: details, Back.
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Back"]);
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some("zqvj"));
}

#[test]
fn the_idle_warning_comes_then_the_privacy_reset_wipes_everything() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(14_000);
    assert_eq!(kiosk.screen(), "step-text");
    kiosk.wait(1_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    assert_eq!(
        kiosk.ui.tree().slot_text(idle::slots::SECONDS_LEFT),
        Some("15")
    );
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.ended(), [EndReason::Idle]);
    assert!(kiosk.ui.holds_nothing());
    assert!(!kiosk.app.flow().unwrap().has_entered_data());
    assert_eq!(kiosk.app.flow().unwrap().phase(), Phase::Welcome);
}

#[test]
fn the_idle_warning_counts_down_each_second_redrawing_only_its_digits() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.step();
    assert_eq!(kiosk.screen(), "idle-warning");
    assert_eq!(kiosk.ui.seconds_left(), Some(15));
    let focus = kiosk.ui.tree().focused();
    let warned_at = kiosk.now;
    for left in (1..15_u64).rev() {
        kiosk.ui.clear_dirty();
        kiosk.step();
        // A second later, from the application's time: the UI has no clock.
        assert_eq!(kiosk.now, warned_at + (15 - left) * 1_000);
        let shown = left.to_string();
        assert_eq!(
            kiosk.ui.tree().slot_text(idle::slots::SECONDS_LEFT),
            Some(shown.as_str())
        );
        assert_eq!(kiosk.ui.seconds_left(), Some(u32::try_from(left).unwrap()));
        // Only the digits changed: only they are repainted, within
        // their slot, a small part of its line - the focus and the screen
        // as they were.
        kiosk.assert_only_digits_repainted(idle::slots::SECONDS_LEFT);
        assert_eq!(kiosk.ui.tree().focused(), focus);
        assert_eq!(kiosk.screen(), "idle-warning");
    }
    // At zero: exactly one session-ended record, and everything wiped.
    kiosk.step();
    assert_eq!(kiosk.now, warned_at + 15_000);
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.ui.seconds_left(), None);
    assert_eq!(kiosk.ended(), [EndReason::Idle]);
    assert!(kiosk.ui.holds_nothing());
    assert!(!kiosk.app.flow().unwrap().has_entered_data());
}

#[test]
fn the_confirmation_counts_down_each_second_then_clears_itself() {
    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Send"]);
    kiosk.wait(5_000);
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Try again"]);
    kiosk.wait(1_000);
    assert_eq!(kiosk.screen(), "submitted");
    assert_eq!(
        kiosk.ui.tree().slot_text(submitted::slots::SECONDS_LEFT),
        Some("10")
    );
    for left in (1..10_u32).rev() {
        kiosk.ui.clear_dirty();
        kiosk.step();
        let shown = left.to_string();
        assert_eq!(
            kiosk.ui.tree().slot_text(submitted::slots::SECONDS_LEFT),
            Some(shown.as_str())
        );
        kiosk.assert_only_digits_repainted(submitted::slots::SECONDS_LEFT);
    }
    kiosk.step();
    assert_eq!(kiosk.screen(), "welcome");
    // The session ended once, when the answers were accepted.
    assert_eq!(kiosk.ended(), [EndReason::Submitted]);
    assert!(kiosk.ui.holds_nothing());
}

#[test]
fn a_countdown_is_shown_only_where_one_counts_down_and_always_fits() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.ui.clear_dirty();
    // On a step: ignored, nothing repainted.
    kiosk
        .ui
        .report(&Report::Countdown(Countdown { remaining: 9_000 }))
        .unwrap();
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.seconds_left(), None);
    assert!(kiosk.ui.tree().dirty_rects().is_empty());
    // Wider digits than before: they fit, as a slot takes the rest of its
    // line, or the screen is laid out again.
    kiosk
        .ui
        .report(&Report::IdleWarning(IdleWarning { remaining: 1_000 }))
        .unwrap();
    let one = kiosk.slot_rect(idle::slots::SECONDS_LEFT);
    kiosk
        .ui
        .report(&Report::Countdown(Countdown { remaining: 120_000 }))
        .unwrap();
    assert_eq!(
        kiosk.ui.tree().slot_text(idle::slots::SECONDS_LEFT),
        Some("120")
    );
    let wide = kiosk.slot_rect(idle::slots::SECONDS_LEFT);
    assert!(
        wide.size.width >= TextMetrics::from_atlas(&REGULAR_24).text_width("120"),
        "{one:?} -> {wide:?}"
    );
}

#[test]
fn yes_i_am_here_goes_back_to_the_step_with_its_answers() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    // Focus is on Yes, I am here: a stray Enter keeps the session.
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Yes, I am here"]);
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some("zqvj"));
    // Any key but those that move focus is input too: the person is there,
    // and the step is back.
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    kiosk.tabs(1);
    assert_eq!(kiosk.screen(), "idle-warning", "Tab only moves focus");
    kiosk.text("x");
    assert_eq!(kiosk.screen(), "step-text");
    // A pointer move alone is not activity; a click on the button is.
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    let label = kiosk.ui.handle(InputEvent::PointerMove { x: 0, y: 0 });
    assert_eq!(label, None);
    kiosk.ui.end_input();
    kiosk.pump();
    assert_eq!(kiosk.screen(), "idle-warning");
    assert_eq!(
        kiosk.click(idle::actions::STILL_HERE),
        Some("Yes, I am here")
    );
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some("zqvj"));
    // And the reset comes 30 s after that.
    kiosk.wait(29_000);
    assert_ne!(kiosk.screen(), "welcome");
    kiosk.wait(1_000);
    assert_eq!(kiosk.screen(), "welcome");
    kiosk.assert_holds_nothing("after the reset");
}

#[test]
fn moving_focus_keeps_the_session_alive() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    for _ in 0..5 {
        kiosk.wait(10_000);
        kiosk.tabs(1);
    }
    assert_eq!(kiosk.screen(), "step-text");
    assert!(
        kiosk
            .actions
            .iter()
            .any(|action| matches!(action, Action::StillHere(_)))
    );
}

#[test]
fn clear_it_now_ends_the_session() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    assert_eq!(kiosk.click(idle::actions::CANCEL), Some("Clear it now"));
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.ended(), [EndReason::Cancelled]);
    kiosk.assert_holds_nothing("after Clear it now");
}

#[test]
fn clear_it_now_is_reached_from_the_keyboard_or_a_slow_click() {
    // Shift-Tab to it, and Enter, each a batch of its own: moving focus on
    // the warning is not still-here, so the warning stays until Enter.
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    kiosk.shift_tab();
    assert_eq!(kiosk.screen(), "idle-warning", "focus moved, nothing else");
    kiosk.key(KeyCode::UP);
    kiosk.key(KeyCode::DOWN);
    kiosk.shift_tab();
    assert_eq!(kiosk.screen(), "idle-warning");
    assert!(
        !kiosk
            .actions
            .iter()
            .any(|action| matches!(action, Action::StillHere(_)))
    );
    // Shift-Tab twice from Yes, I am here wraps back to Clear it now.
    kiosk.shift_tab();
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Clear it now"]);
    assert_eq!(kiosk.ended(), [EndReason::Cancelled]);
    kiosk.assert_holds_nothing("after Clear it now from the keyboard");

    // A click whose press and release arrive apart: the press on the
    // button is not still-here, so the release lands on it.
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(15_000);
    assert_eq!(
        kiosk.slow_click(idle::actions::CANCEL),
        Some("Clear it now")
    );
    assert_eq!(kiosk.ended(), [EndReason::Cancelled]);
    kiosk.assert_holds_nothing("after a slow click on Clear it now");

    // A press away from the buttons, or a letter, is still-here.
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    kiosk.wait(15_000);
    kiosk.ui.handle(InputEvent::PointerMove { x: 1, y: 1 });
    kiosk.ui.handle(InputEvent::PointerButton {
        button: 0,
        down: true,
    });
    kiosk.ui.end_input();
    kiosk.pump();
    assert_eq!(kiosk.screen(), "step-text");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    kiosk.text("q");
    assert_eq!(kiosk.screen(), "step-text");
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some("zqvj"));
}

#[test]
fn the_session_limit_ends_even_an_active_session() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    // Twenty minutes of activity: a Tab every 10 s.
    while kiosk.screen() != "welcome" && kiosk.now < 1_000 + 21 * 60_000 {
        kiosk.wait(10_000);
        kiosk.tabs(1);
    }
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.ended(), [EndReason::TimeLimit]);
    assert!(
        kiosk.now <= 1_000 + 20 * 60_000 + 10_100,
        "ended at {}",
        kiosk.now
    );
    kiosk.assert_holds_nothing("after the session limit");
}

#[test]
fn a_malformed_action_ends_the_session() {
    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    let mut garbage = [0u8; MAX_RECORD_LEN];
    garbage[..4].copy_from_slice(&[0x02, 0x4B, 9, 0]);
    kiosk.app.receive(&garbage, kiosk.now);
    kiosk.pump();
    assert!(
        kiosk
            .notes
            .iter()
            .any(|note| matches!(note, Note::Refused(_)))
    );
    assert_eq!(kiosk.ended(), [EndReason::Fault]);
    assert_eq!(kiosk.screen(), "welcome");
    kiosk.assert_holds_nothing("after a malformed action");
    // The kiosk stays in service: the next person can start.
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "step-text");
}

#[test]
fn a_malformed_report_wipes_the_ui_and_ends_the_session() {
    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    let garbage = [0xFFu8; MAX_RECORD_LEN];
    assert!(matches!(
        kiosk.ui.receive_report(&garbage),
        Err(KioskUiError::Report(_))
    ));
    assert_eq!(kiosk.screen(), "welcome");
    assert!(kiosk.ui.holds_nothing(), "the UI wipes at once");
    assert!(matches!(
        kiosk.ui.actions().last(),
        Some(Action::Abandon(_))
    ));
    kiosk.pump();
    assert_eq!(
        kiosk.ended(),
        [EndReason::Abandoned],
        "and the application's end closes too"
    );
    kiosk.assert_holds_nothing("after a malformed report");
}

/// Each phase a session can be in when a PD gives up, from the welcome
/// screen: a step with an answer, the review, sending, not sent.
fn into_phase(kiosk: &mut Kiosk, phase: usize) {
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    if phase == 0 {
        return;
    }
    kiosk.tabs(3);
    kiosk.key(KeyCode::ENTER);
    kiosk.key(KeyCode::DOWN);
    kiosk.tabs(4);
    kiosk.key(KeyCode::ENTER);
    kiosk.key(KeyCode::SPACE);
    kiosk.tabs(4);
    kiosk.key(KeyCode::ENTER);
    kiosk.tabs(2);
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "review");
    if phase == 1 {
        return;
    }
    kiosk.tabs(2);
    kiosk.key(KeyCode::ENTER);
    assert_eq!(kiosk.screen(), "submitting");
    if phase == 2 {
        return;
    }
    kiosk.wait(5_000);
    assert_eq!(kiosk.screen(), "failed");
}

#[test]
fn failing_ends_the_session_and_goes_out_of_service() {
    // What `KioskApp::fail` does, whatever the fault: the PD calls it for
    // a timer error, a refused wake-up or a broken ring, and that detection
    // is not tested here. It calls it again on every wake-up while a fault
    // lasts, so only the first counts.
    for (fault, later) in [
        (Fault::Timer, Fault::Ring),
        (Fault::Wake, Fault::Timer),
        (Fault::Ring, Fault::Wake),
    ] {
        for phase in 0..4 {
            let mut kiosk = Kiosk::new();
            into_phase(&mut kiosk, phase);
            assert!(kiosk.app.flow().unwrap().has_entered_data());
            kiosk.fail(fault);
            assert_eq!(kiosk.app.fault(), Some(fault));
            assert_eq!(kiosk.screen(), "welcome", "{fault:?} in phase {phase}");
            assert_eq!(
                kiosk.ended(),
                [EndReason::Fault],
                "{fault:?} in phase {phase}"
            );
            kiosk.assert_holds_nothing(&format!("after {fault:?} in phase {phase}"));
            assert!(kiosk.notes.contains(&Note::OutOfService(fault)));
            // Out of service: no session starts, and nothing is held.
            kiosk.key(KeyCode::ENTER);
            kiosk.text("bo");
            assert_eq!(kiosk.screen(), "welcome");
            assert_eq!(kiosk.app.next_wake(), None);
            kiosk.wait(60_000);
            kiosk.assert_holds_nothing(&format!("out of service after {fault:?}"));
            // Only the first fault counts: no second note, nothing sent.
            let notes = kiosk.notes.len();
            let reports = kiosk.reports.len();
            kiosk.fail(later);
            assert_eq!(kiosk.app.fault(), Some(fault));
            assert_eq!(kiosk.notes.len(), notes, "{later:?} after {fault:?}");
            assert_eq!(kiosk.reports.len(), reports);
        }
    }
}

#[test]
fn the_ui_giving_up_wipes_it_and_ends_the_session() {
    for phase in 0..4 {
        let mut kiosk = Kiosk::new();
        into_phase(&mut kiosk, phase);
        kiosk.ui.reset();
        assert_eq!(kiosk.screen(), "welcome");
        assert!(kiosk.ui.holds_nothing(), "phase {phase}");
        kiosk.pump();
        assert_eq!(kiosk.ended(), [EndReason::Abandoned], "phase {phase}");
        kiosk.assert_holds_nothing(&format!("after the UI gave up in phase {phase}"));
    }
}

#[test]
fn reports_left_waiting_too_long_end_the_session() {
    let mut kiosk = Kiosk::new();
    into_phase(&mut kiosk, 1);
    // Unreachable in the PD, which takes an action only while no report
    // waits (`takes_actions`): a guard for a caller that ignores it. Back
    // and Next, over and over, the reports never taken, until the queue is
    // full.
    let mut slot = [0u8; MAX_RECORD_LEN];
    for seq in 0..u32::try_from(2 * PENDING).unwrap() {
        let action = if seq % 2 == 0 {
            Action::Back(showcase_kiosk_contract::Back { seq })
        } else {
            Action::Next(showcase_kiosk_contract::Next { seq })
        };
        action.encode(&mut slot).unwrap();
        kiosk.app.receive(&slot, kiosk.now);
    }
    assert_eq!(kiosk.app.fault(), Some(Fault::Backlog));
    let waiting: Vec<Report> = kiosk.app.reports().copied().collect();
    assert!(matches!(
        waiting.as_slice(),
        [Report::SessionEnded(ended), Report::ShowWelcome(_)] if ended.reason == EndReason::Fault
    ));
    kiosk.pump();
    kiosk.assert_holds_nothing("after the backlog");
}

#[test]
fn the_privacy_reset_happens_on_time_even_while_reports_wait() {
    let mut kiosk = Kiosk::new();
    into_phase(&mut kiosk, 0);
    // The idle warning falls due, and its report is left untaken...
    kiosk.now += 15_000;
    kiosk.app.tick(kiosk.now);
    assert!(matches!(
        kiosk.app.reports().next(),
        Some(Report::IdleWarning(_))
    ));
    assert!(!kiosk.app.takes_actions(), "no action while a report waits");
    assert!(kiosk.app.flow().unwrap().has_entered_data());
    // ...and the privacy reset still comes on time, the warning still
    // waiting: the answers are wiped when they are due.
    kiosk.now += 15_000;
    kiosk.app.tick(kiosk.now);
    assert!(!kiosk.app.flow().unwrap().has_entered_data());
    assert_eq!(kiosk.app.flow().unwrap().phase(), Phase::Welcome);
    kiosk.pump();
    kiosk.assert_holds_nothing("once the reports are taken");
}

#[test]
fn an_action_waiting_in_the_outbox_is_wiped_when_the_session_ends() {
    // The privacy reset arrives while a typed name waits to be sent.
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    for key in "zqvj".chars().map(|ch| KeyCode::letter(ch).unwrap()) {
        kiosk.ui.handle(InputEvent::KeyDown(key));
        kiosk.ui.handle(InputEvent::KeyUp(key));
    }
    kiosk.ui.end_input();
    assert!(
        kiosk
            .ui
            .actions()
            .any(|action| matches!(action, Action::SetText(set) if set.text() == "zqvj"))
    );
    assert!(!kiosk.ui.holds_nothing());
    kiosk.now += 30_000;
    kiosk.app.tick(kiosk.now);
    kiosk.deliver();
    assert!(
        kiosk.ui.holds_nothing(),
        "the outbox wiped with the session"
    );
    assert_eq!(kiosk.ui.actions().count(), 0);

    // A report it cannot decode, the same: everything wiped, and only the
    // abandon left to send.
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    for key in "bo".chars().map(|ch| KeyCode::letter(ch).unwrap()) {
        kiosk.ui.handle(InputEvent::KeyDown(key));
        kiosk.ui.handle(InputEvent::KeyUp(key));
    }
    kiosk.ui.end_input();
    assert!(kiosk.ui.receive_report(&[0xFF; MAX_RECORD_LEN]).is_err());
    assert!(kiosk.ui.holds_nothing());
    let left: Vec<&Action> = kiosk.ui.actions().collect();
    assert!(matches!(left.as_slice(), [Action::Abandon(_)]), "{left:?}");
}

#[test]
fn the_qemu_script_s_keys_on_the_host() {
    // systems/kiosk.qmp's bursts, as the UART input PD makes them keys.
    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    // Change, then Continue.
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Change"]);
    assert_eq!(kiosk.screen(), "step-notice");
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Continue"]);
    assert_eq!(kiosk.screen(), "review");
    // Send, failed after 5 s; Try again, accepted; Done.
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Send"]);
    kiosk.wait(5_000);
    assert_eq!(kiosk.screen(), "failed");
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Try again"]);
    kiosk.wait(1_000);
    // Done, once the confirmation has counted down a second.
    assert_eq!(kiosk.ui.seconds_left(), Some(10));
    kiosk.step();
    assert_eq!(kiosk.ui.seconds_left(), Some(9));
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Done"]);
    // A name, the reference, then Cancel: "zqvjb\t\t\r" in one burst.
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Start"]);
    let mut keys: Vec<KeyCode> = "zqvjb"
        .chars()
        .map(|ch| KeyCode::letter(ch).unwrap())
        .collect();
    keys.extend([KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]);
    assert_eq!(kiosk.type_keys(&keys), ["Cancel"]);
    assert_eq!(kiosk.screen(), "welcome");
    // A name, then the idle warning and the privacy reset.
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvjc");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "idle-warning");
    kiosk.wait(15_000);
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(
        kiosk.ended(),
        [EndReason::Submitted, EndReason::Cancelled, EndReason::Idle]
    );
    kiosk.assert_holds_nothing("after the script");
}

#[test]
fn a_refused_value_is_dropped_from_the_ui_too() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    kiosk.text("zqvj");
    let seq = kiosk
        .actions
        .iter()
        .rev()
        .find_map(|action| match action {
            Action::SetText(set) if set.field == NAME => Some(set.seq),
            _ => None,
        })
        .unwrap();
    // As if the application had refused it.
    let refused = Report::ValueRefused(ValueRefused {
        seq,
        field: NAME,
        issue: Issue::BadCharacter,
    });
    kiosk.ui.report(&refused).unwrap();
    let tree = kiosk.ui.tree();
    assert_eq!(tree.value(about_you::fields::NAME), Some(""));
    assert_eq!(tree.visible(about_you::groups::NAME_ERROR), Some(true));
    // An older action's refusal leaves a newer value alone.
    kiosk.text("bo");
    let stale = Report::ValueRefused(ValueRefused {
        seq,
        field: NAME,
        issue: Issue::BadCharacter,
    });
    kiosk.ui.report(&stale).unwrap();
    assert_eq!(kiosk.ui.tree().value(about_you::fields::NAME), Some("bo"));
}

#[test]
fn a_text_field_is_sent_once_a_batch_and_before_the_button() {
    let mut kiosk = Kiosk::new();
    kiosk.key(KeyCode::ENTER);
    let before = kiosk.actions.len();
    // "zqvj", Tab, the reference, Tab, Tab, Enter: one burst, as the
    // terminal sends it.
    let mut keys: Vec<KeyCode> = "zqvj"
        .chars()
        .map(|ch| KeyCode::letter(ch).unwrap())
        .collect();
    keys.push(KeyCode::TAB);
    keys.extend(REFERENCE.bytes().map(|d| KeyCode::digit(d - b'0').unwrap()));
    keys.extend([KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]);
    assert_eq!(kiosk.type_keys(&keys), ["Next"]);
    let sent = &kiosk.actions[before..];
    assert_eq!(sent.len(), 3, "{sent:?}");
    assert!(matches!(&sent[0], Action::SetText(set) if set.field == NAME && set.text() == "zqvj"));
    assert!(matches!(&sent[1], Action::SetText(set) if set.text() == REFERENCE));
    assert!(matches!(&sent[2], Action::Next(_)));
    assert_eq!(kiosk.screen(), "step-choice");
}

#[test]
fn every_ending_leaves_nothing_in_either_however_often() {
    let mut kiosk = Kiosk::new();
    for round in 0..3 {
        // Cancel from the review (Change, Cancel).
        into_phase(&mut kiosk, 1);
        kiosk.tabs(1);
        assert_eq!(kiosk.key(KeyCode::ENTER), ["Cancel"], "round {round}");
        kiosk.assert_holds_nothing("after Cancel");
        // Submitted, after a failed first attempt.
        into_phase(&mut kiosk, 3);
        kiosk.tabs(2);
        kiosk.key(KeyCode::ENTER);
        kiosk.wait(1_000);
        assert_eq!(kiosk.screen(), "submitted");
        kiosk.assert_holds_nothing("once submitted");
        kiosk.key(KeyCode::ENTER);
        // The idle reset.
        into_phase(&mut kiosk, 1);
        kiosk.wait(30_000);
        kiosk.assert_holds_nothing("after the idle reset");
        // A malformed action.
        into_phase(&mut kiosk, 0);
        kiosk.app.receive(&[0xFF; MAX_RECORD_LEN], kiosk.now);
        kiosk.pump();
        kiosk.assert_holds_nothing("after a malformed action");
        // A malformed report.
        into_phase(&mut kiosk, 2);
        let _ = kiosk.ui.receive_report(&[0xFF; MAX_RECORD_LEN]);
        kiosk.pump();
        kiosk.assert_holds_nothing("after a malformed report");
        assert_eq!(kiosk.screen(), "welcome");
    }
    assert_eq!(kiosk.ended().len(), 15);
    assert_eq!(kiosk.ui.dropped(), 0);
}

#[test]
fn a_kiosk_that_checks_in_takes_no_input_until_approved_then_runs_on_that_epoch() {
    use showcase_kiosk_contract::{CheckInAnswer, CheckedIn};
    use showcase_kiosk_screens::app::REFERENCE;
    use showcase_kiosk_screens::stand_in::Plan;

    let mut kiosk = Kiosk::new();
    kiosk.app = KioskApp::checking_in(&REFERENCE, [0x3F, 0xA1, 0x07, 0x5C, 0xE2, 0x19, 0x88, 0x40])
        .unwrap();
    kiosk.deliver();
    assert_eq!(kiosk.screen(), "out-of-service");
    assert!(kiosk.app.flow().is_none());
    // The kiosk's own buttons do nothing: Enter on Start sends nothing.
    assert!(kiosk.key(KeyCode::ENTER).is_empty());
    assert!(kiosk.actions.is_empty());

    // Its first wake: a check-in, not approved; at once another, approved
    // with the stand-in's count of check-ins.
    kiosk.wait(0);
    assert_eq!(
        kiosk.notes,
        [
            Note::CheckIn { seq: 1 },
            Note::CheckedIn(CheckedIn {
                seq: 1,
                answer: CheckInAnswer::NotApproved
            }),
            Note::CheckIn { seq: 2 },
            Note::CheckedIn(CheckedIn {
                seq: 2,
                answer: CheckInAnswer::Approved { epoch: 2 }
            }),
        ]
    );
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.app.flow().unwrap().epoch(), 2);
    assert_eq!(kiosk.app.next_wake(), None, "no more check-ins");

    // A session on that epoch: its submission's ticket carries it.
    kiosk.go_to_review();
    kiosk.tabs(2);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Send"]);
    assert!(kiosk.notes.contains(&Note::StandIn {
        attempt: 1,
        epoch: 2,
        plan: Plan::Unanswered
    }));
}

#[test]
fn out_of_service_the_ui_wipes_everything_and_its_buttons_do_nothing() {
    use showcase_kiosk_contract::{OutOfService, OutOfServiceReason, ShowWelcome};

    let mut kiosk = Kiosk::new();
    kiosk.go_to_review();
    let out = Report::OutOfService(OutOfService {
        reason: OutOfServiceReason::Updating,
    });
    kiosk.ui.report(&out).unwrap();
    assert_eq!(kiosk.screen(), "out-of-service");
    assert_eq!(
        kiosk.ui.out_of_service(),
        Some(OutOfServiceReason::Updating)
    );
    assert!(kiosk.ui.holds_nothing());
    // Enter on Start: nothing is pressed, nothing sent.
    kiosk.actions.clear();
    assert!(kiosk.key(KeyCode::ENTER).is_empty());
    assert!(kiosk.actions.is_empty());
    // The welcome screen puts it back in service.
    kiosk.ui.report(&Report::ShowWelcome(ShowWelcome)).unwrap();
    assert_eq!(kiosk.screen(), "welcome");
    assert_eq!(kiosk.ui.out_of_service(), None);
    assert_eq!(kiosk.key(KeyCode::ENTER), ["Start"]);
}
