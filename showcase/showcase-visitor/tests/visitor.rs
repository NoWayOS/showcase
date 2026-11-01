// SPDX-License-Identifier: MIT OR Apache-2.0
//! The visitor sign-in end to end, on the host: its UI
//! (`VisitorUi`, the kiosk's form UI on the visitor's screens) joined to
//! its application (`KioskApp` on `EXAMPLE`) as the two PDs join them,
//! every record encoded on the way, with time passing as the application
//! PD's timer would let it.
//!
//! * The whole flow: welcome, about you, who you are visiting, why, the
//!   site rules, check your details, signing in, signed in.
//! * The pass shows a number and no name; every answer is wiped, in both,
//!   before it appears; it clears itself after 10 s, counting down.
//! * Walking away mid-form counts down, then wipes everything.
//! * *Demo: service down* says reception does not know you are here yet.
//! * *Signing out* is fixed text, and sends nothing.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed step fails the test"
)]

use nowayos_glyphs::REGULAR_24;
use nowayos_input::{InputEvent, KeyCode};
use nowayos_ui::{NodeKind, TextMetrics};
use showcase_kiosk_contract::{Action, EndReason, MAX_RECORD_LEN, Report, decode_action};
use showcase_kiosk_flow::Tick;
use showcase_kiosk_screens::app::KioskApp;
use showcase_visitor::generated::check::check_screen as check;
use showcase_visitor::generated::signed_in::signed_in_screen as signed_in;
use showcase_visitor::{EXAMPLE, VisitorUi};

/// The visitor sign-in's two PDs, joined.
struct Desk {
    ui: VisitorUi,
    app: KioskApp,
    now: Tick,
    reports: Vec<Report>,
    actions: Vec<Action>,
}

impl Desk {
    fn new() -> Self {
        Self {
            ui: VisitorUi::new(TextMetrics::from_atlas(&REGULAR_24)).unwrap(),
            app: KioskApp::with(&EXAMPLE, 5).unwrap(),
            now: 1_000,
            reports: Vec::new(),
            actions: Vec::new(),
        }
    }

    fn screen(&self) -> &'static str {
        self.ui.screen()
    }

    /// Sends the UI's actions to the application, and its reports back.
    fn pump(&mut self) {
        loop {
            let reports: Vec<Report> = self.app.reports().copied().collect();
            self.app.sent(reports.len());
            for report in reports {
                let mut slot = [0u8; MAX_RECORD_LEN];
                report.encode(&mut slot).unwrap();
                self.reports.push(report);
                self.ui.receive_report(&slot).unwrap();
            }
            while self.app.take_note().is_some() {}
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
                self.app.receive(&slot, self.now);
            }
        }
    }

    /// Lets `ms` pass, waking the application at each deadline.
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
            .map(|ch| KeyCode::letter(ch).unwrap())
            .collect();
        self.type_keys(&keys);
    }

    fn tabs(&mut self, count: usize) {
        self.type_keys(&vec![KeyCode::TAB; count]);
    }

    /// From the welcome screen to *Check your details*, every answer
    /// given: a name, an organisation, who they visit, the first reason,
    /// the rules agreed.
    fn fill_in_to_check(&mut self) {
        assert_eq!(self.key(KeyCode::ENTER), ["Sign in as a visitor"]);
        assert_eq!(self.screen(), "about-you");
        self.text("zqvjname");
        self.tabs(1);
        self.text("zqvjorg");
        self.tabs(2);
        assert_eq!(self.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(self.screen(), "visiting");
        self.text("zqvjhost");
        self.tabs(3);
        assert_eq!(self.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(self.screen(), "purpose");
        self.key(KeyCode::DOWN);
        self.tabs(3);
        assert_eq!(self.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(self.screen(), "site-rules");
        self.key(KeyCode::SPACE);
        self.tabs(3);
        assert_eq!(self.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(self.screen(), "check");
    }

    /// Every text the screen showing holds, in paint order: fixed texts,
    /// buttons, slots and values.
    fn shown(&self) -> Vec<String> {
        self.ui
            .tree()
            .paint_order()
            .filter_map(|(_, node)| match node.kind() {
                NodeKind::Text(text) | NodeKind::Button { label: text, .. } => {
                    Some((*text).to_string())
                }
                NodeKind::Slot { text, .. } => Some(text.as_str().to_string()),
                NodeKind::TextInput { value, .. } => Some(value.as_str().to_string()),
                _ => None,
            })
            .collect()
    }

    fn ended(&self) -> Vec<EndReason> {
        self.reports
            .iter()
            .filter_map(|report| match report {
                Report::SessionEnded(ended) => Some(ended.reason),
                _ => None,
            })
            .collect()
    }

    /// Neither end holds anything a person entered.
    fn assert_holds_nothing(&self, when: &str) {
        assert!(self.ui.holds_nothing(), "the UI, {when}");
        assert!(
            !self.app.flow().unwrap().has_entered_data(),
            "the application, {when}"
        );
    }
}

#[test]
fn the_whole_sign_in_ends_at_a_pass_with_no_name_and_nothing_kept() {
    let mut desk = Desk::new();
    assert_eq!(desk.screen(), "welcome");
    desk.fill_in_to_check();
    // The check screen lists the answers, from the UI's own copies.
    let tree = desk.ui.tree();
    assert_eq!(tree.slot_text(check::slots::NAME), Some("zqvjname"));
    assert_eq!(tree.slot_text(check::slots::ORGANISATION), Some("zqvjorg"));
    assert_eq!(tree.slot_text(check::slots::HOST), Some("zqvjhost"));
    assert_eq!(tree.slot_text(check::slots::PURPOSE), Some("A meeting"));
    assert!(desk.app.flow().unwrap().has_entered_data());
    desk.tabs(3);
    assert_eq!(desk.key(KeyCode::ENTER), ["Sign in"]);
    assert_eq!(desk.screen(), "signing-in");
    desk.wait(1_000);
    assert_eq!(desk.screen(), "signed-in");
    // A number, never the name: every answer is gone, in both, before the
    // pass appears - the session ended, then the pass was shown.
    assert_eq!(desk.ui.issued(), Some("4821"));
    assert_eq!(
        desk.ui.tree().slot_text(signed_in::slots::PASS),
        Some("4821")
    );
    let shown = desk.shown();
    assert!(shown.iter().all(|text| !text.contains("zqvj")), "{shown:?}");
    desk.assert_holds_nothing("once the pass shows");
    let ended_at = desk
        .reports
        .iter()
        .position(|report| matches!(report, Report::SessionEnded(_)))
        .unwrap();
    let pass_at = desk
        .reports
        .iter()
        .position(|report| matches!(report, Report::Issued(_)))
        .unwrap();
    assert!(ended_at < pass_at);
    assert_eq!(desk.ended(), [EndReason::Submitted]);
    // The next pass has the next number.
    assert_eq!(desk.key(KeyCode::ENTER), ["Done"]);
    assert_eq!(desk.screen(), "welcome");
    desk.fill_in_to_check();
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    desk.wait(1_000);
    assert_eq!(desk.ui.issued(), Some("4822"));
}

#[test]
fn the_pass_clears_itself_after_ten_seconds_counting_down() {
    let mut desk = Desk::new();
    desk.fill_in_to_check();
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    desk.wait(1_000);
    assert_eq!(desk.screen(), "signed-in");
    assert_eq!(desk.ui.seconds_left(), Some(10));
    for left in (1..10).rev() {
        desk.step();
        assert_eq!(desk.ui.seconds_left(), Some(left));
        assert_eq!(desk.ui.issued(), Some("4821"), "the pass stays");
    }
    desk.step();
    assert_eq!(desk.screen(), "welcome");
    assert_eq!(desk.ui.issued(), None);
    // One end of the session, when the sign-in was accepted.
    assert_eq!(desk.ended(), [EndReason::Submitted]);
    desk.assert_holds_nothing("after the pass");
}

#[test]
fn walking_away_mid_form_counts_down_then_wipes_everything() {
    let mut desk = Desk::new();
    desk.key(KeyCode::ENTER);
    desk.text("zqvjname");
    desk.wait(15_000);
    assert_eq!(desk.screen(), "idle-warning");
    assert_eq!(desk.ui.seconds_left(), Some(15));
    for left in (1..15).rev() {
        desk.step();
        assert_eq!(desk.ui.seconds_left(), Some(left));
    }
    desk.step();
    assert_eq!(desk.screen(), "welcome");
    assert_eq!(desk.ended(), [EndReason::Idle]);
    desk.assert_holds_nothing("after walking away");
    // The next visitor finds the step empty.
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "about-you");
    let shown = desk.shown();
    assert!(shown.iter().all(|text| !text.contains("zqvj")), "{shown:?}");
}

#[test]
fn the_service_down_sends_the_visitor_to_reception() {
    let mut desk = Desk::new();
    desk.fill_in_to_check();
    desk.tabs(2);
    assert_eq!(desk.key(KeyCode::ENTER), ["Demo: service down"]);
    assert!(
        desk.actions
            .iter()
            .any(|action| matches!(action, Action::SimulateFailure(_)))
    );
    assert_eq!(desk.screen(), "signing-in");
    desk.wait(1_000);
    assert_eq!(desk.screen(), "not-signed-in");
    let shown = desk.shown();
    assert!(shown.contains(&"Reception does not know you are here yet".to_string()));
    assert!(
        shown
            .iter()
            .any(|text| text.contains("go to the reception desk"))
    );
    // Trying again does not help while the service is down.
    assert_eq!(
        desk.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Try again"]
    );
    desk.wait(1_000);
    assert_eq!(desk.screen(), "not-signed-in");
    // Finish: the session ends, everything wiped.
    assert_eq!(desk.type_keys(&[KeyCode::TAB, KeyCode::ENTER]), ["Finish"]);
    assert_eq!(desk.screen(), "welcome");
    assert_eq!(desk.ended(), [EndReason::Cancelled]);
    desk.assert_holds_nothing("after Finish");
    // The service is down for that session only.
    desk.fill_in_to_check();
    desk.tabs(3);
    assert_eq!(desk.key(KeyCode::ENTER), ["Sign in"]);
    desk.wait(1_000);
    assert_eq!(desk.screen(), "signed-in");
}

#[test]
fn signing_out_is_fixed_text_and_sends_nothing() {
    let mut desk = Desk::new();
    desk.tabs(1);
    assert_eq!(desk.key(KeyCode::ENTER), ["Signing out"]);
    assert_eq!(desk.screen(), "signing-out");
    assert!(
        desk.shown()
            .iter()
            .any(|text| text.contains("keeps no list of visitors"))
    );
    assert_eq!(desk.key(KeyCode::ENTER), ["Back"]);
    assert_eq!(desk.screen(), "welcome");
    assert!(desk.actions.is_empty(), "{:?}", desk.actions);
    assert!(desk.reports.is_empty());
}

#[test]
fn each_step_names_what_it_still_needs() {
    // No name: Next is refused, and the step says why.
    let mut desk = Desk::new();
    desk.key(KeyCode::ENTER);
    desk.tabs(3);
    assert_eq!(desk.key(KeyCode::ENTER), ["Next"]);
    assert_eq!(desk.screen(), "about-you");
    assert!(desk.shown().contains(&"Enter your name".to_string()));
    // No one to visit.
    let mut desk = Desk::new();
    desk.key(KeyCode::ENTER);
    desk.text("zqvjname");
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "visiting");
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "visiting");
    assert!(
        desk.shown()
            .contains(&"Enter who you are visiting".to_string())
    );
    // No reason chosen, then the rules not agreed.
    let mut desk = Desk::new();
    desk.key(KeyCode::ENTER);
    desk.text("zqvjname");
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    desk.text("zqvjhost");
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "purpose");
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "purpose");
    assert!(
        desk.shown()
            .contains(&"Choose why you are visiting".to_string())
    );
    let mut desk = Desk::new();
    desk.fill_in_to_check();
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "site-rules", "Change goes back a step");
    desk.key(KeyCode::SPACE);
    desk.tabs(3);
    desk.key(KeyCode::ENTER);
    assert_eq!(desk.screen(), "site-rules");
    assert!(desk.shown().contains(&"Tick this to sign in".to_string()));
}
