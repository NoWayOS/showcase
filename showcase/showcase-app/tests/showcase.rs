// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase end to end, on the host: the showcase's UI
//! (`Showcase`, the launcher and the Service desk example) joined to the
//! reference kiosk's application (`KioskApp`) as the two PDs join them,
//! every record encoded on the way, with time passing as the application
//! PD's timer would let it.
//!
//! * The launcher fills the screen with one tile per example; the Service
//!   desk is the first.
//! * A tile chosen with the keyboard or the mouse opens its example at its
//!   welcome screen.
//! * Home, or the end of a session however it ends, wipes the example -
//!   both ends - and shows the launcher again.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed step fails the test"
)]

use nowayos_display::Rect;
use nowayos_glyphs::{REGULAR_24, REGULAR_48};
use nowayos_input::{InputEvent, KeyCode, TouchPhase};
use nowayos_ui::{ActionId, NodeKind, TextMetrics};
use showcase_app::generated::launcher::launcher_screen as launcher;
use showcase_app::{EXTRA_APP, HOME, PUMP_APP, SERVICE_DESK_APP, Showcase, VISITOR_APP, brands};
use showcase_kiosk_contract::{Action, EndReason, MAX_RECORD_LEN, Report};
use showcase_kiosk_flow::Tick;
use showcase_kiosk_screens::app::KioskApp;
use showcase_kiosk_screens::generated::step_text::about_you_step as about_you;
use showcase_kiosk_screens::kiosk::KioskScreens;

/// The reference the QEMU script types, as its mirror here does: a marker
/// every expect file forbids in the log.
const REFERENCE: &str = "9081726354";

/// The showcase's UI and its examples' applications - the Service desk's
/// and the visitor sign-in's - joined, each over its own rings.
struct Stand {
    ui: Showcase,
    app: KioskApp,
    visitor: KioskApp,
    pump_app: KioskApp,
    now: Tick,
    /// Every report the Service desk's application sent.
    reports: Vec<Report>,
    /// Every report the visitor sign-in's application sent.
    visitor_reports: Vec<Report>,
    /// Every report the pay at the pump's application sent.
    pump_reports: Vec<Report>,
    actions: Vec<Action>,
    visitor_actions: Vec<Action>,
    pump_actions: Vec<Action>,
}

impl Stand {
    fn new() -> Self {
        Self {
            ui: Showcase::new(TextMetrics::from_atlas(&REGULAR_24)).unwrap(),
            app: KioskApp::new(3).unwrap(),
            visitor: KioskApp::with(&showcase_visitor::EXAMPLE, 3).unwrap(),
            pump_app: KioskApp::with(&showcase_pump::EXAMPLE, 3).unwrap(),
            now: 1_000,
            reports: Vec::new(),
            visitor_reports: Vec::new(),
            pump_reports: Vec::new(),
            actions: Vec::new(),
            visitor_actions: Vec::new(),
            pump_actions: Vec::new(),
        }
    }

    fn screen(&self) -> &'static str {
        self.ui.screen()
    }

    /// Sends the UI's actions to each application, and its reports back,
    /// as the PDs do: each over its own rings.
    fn pump(&mut self) {
        loop {
            let mut moved = false;
            for index in [SERVICE_DESK_APP, VISITOR_APP, PUMP_APP] {
                let (app, reports_seen, actions_seen) = match index {
                    VISITOR_APP => (
                        &mut self.visitor,
                        &mut self.visitor_reports,
                        &mut self.visitor_actions,
                    ),
                    PUMP_APP => (
                        &mut self.pump_app,
                        &mut self.pump_reports,
                        &mut self.pump_actions,
                    ),
                    _ => (&mut self.app, &mut self.reports, &mut self.actions),
                };
                let reports: Vec<Report> = app.reports().copied().collect();
                app.sent(reports.len());
                for report in reports {
                    let mut slot = [0u8; MAX_RECORD_LEN];
                    report.encode(&mut slot).unwrap();
                    reports_seen.push(report);
                    self.ui.receive_report(index, &slot).unwrap();
                }
                while app.take_note().is_some() {}
                let actions: Vec<Action> = self.ui.actions(index).cloned().collect();
                self.ui.sent(index, actions.len());
                for action in actions {
                    let mut slot = [0u8; MAX_RECORD_LEN];
                    action.encode(&mut slot).unwrap();
                    actions_seen.push(action);
                    app.receive(&slot, self.now);
                    moved = true;
                }
            }
            if !moved {
                return;
            }
        }
    }

    fn wait(&mut self, ms: Tick) {
        let until = self.now + ms;
        loop {
            let next = [
                self.app.next_wake(),
                self.visitor.next_wake(),
                self.pump_app.next_wake(),
            ]
            .into_iter()
            .flatten()
            .min()
            .filter(|&at| at <= until);
            let Some(at) = next else { break };
            self.now = self.now.max(at);
            self.app.tick(self.now);
            self.visitor.tick(self.now);
            self.pump_app.tick(self.now);
            self.pump();
        }
        self.now = until;
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

    fn shift_tab(&mut self) {
        for event in [
            InputEvent::KeyDown(KeyCode::LEFT_SHIFT),
            InputEvent::KeyDown(KeyCode::TAB),
            InputEvent::KeyUp(KeyCode::TAB),
            InputEvent::KeyUp(KeyCode::LEFT_SHIFT),
        ] {
            self.ui.handle(event);
        }
        self.ui.end_input();
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
            .unwrap_or_else(|| panic!("no button {action:?} on {}", self.ui.screen()));
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

    fn ended(&self) -> Vec<EndReason> {
        self.reports
            .iter()
            .filter_map(|report| match report {
                Report::SessionEnded(ended) => Some(ended.reason),
                _ => None,
            })
            .collect()
    }

    /// Opens the Service desk from the launcher, starts a session and
    /// types a name: an example in use.
    fn in_use(&mut self) {
        assert_eq!(self.screen(), "launcher");
        assert_eq!(self.key(KeyCode::ENTER), ["Service desk"]);
        assert_eq!(self.screen(), "welcome");
        assert_eq!(self.key(KeyCode::ENTER), ["Start"]);
        assert_eq!(self.screen(), "step-text");
        self.text("zqvj");
        assert!(self.app.flow().unwrap().has_entered_data());
        assert!(!self.ui.holds_nothing());
    }

    /// From the name typed, through the steps to the review, and Send.
    fn send(&mut self) {
        self.tabs(3);
        self.key(KeyCode::ENTER);
        self.key(KeyCode::DOWN);
        self.tabs(4);
        self.key(KeyCode::ENTER);
        self.key(KeyCode::SPACE);
        self.tabs(4);
        self.key(KeyCode::ENTER);
        self.tabs(2);
        self.key(KeyCode::ENTER);
        self.tabs(2);
        assert_eq!(self.key(KeyCode::ENTER), ["Send"]);
        assert_eq!(self.screen(), "submitting");
    }

    /// A click with no move first: a button down and up wherever the
    /// pointer is, as a USB mouse reports a double-click.
    fn click_again(&mut self) -> Option<&'static str> {
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

    /// The launcher shows, and neither end holds anything entered.
    fn assert_back_at_the_launcher(&self, when: &str) {
        assert_eq!(self.screen(), "launcher", "{when}");
        assert!(self.ui.at_launcher(), "{when}");
        // In the launcher's own theme again.
        assert_eq!(self.ui.theme().0, "launcher", "{when}");
        assert!(self.ui.holds_nothing(), "the UI, {when}");
        assert!(
            !self.app.flow().unwrap().has_entered_data(),
            "the application, {when}"
        );
        assert!(
            !self.visitor.flow().unwrap().has_entered_data(),
            "the visitor's application, {when}"
        );
        assert!(
            !self.pump_app.flow().unwrap().has_entered_data(),
            "the pump's application, {when}"
        );
    }

    /// How each visitor sign-in session ended.
    fn visitor_ended(&self) -> Vec<EndReason> {
        self.visitor_reports
            .iter()
            .filter_map(|report| match report {
                Report::SessionEnded(ended) => Some(ended.reason),
                _ => None,
            })
            .collect()
    }

    /// The visitor sign-in from the launcher to its check screen, as the
    /// QEMU script types it: a name, who they visit, the first reason, the
    /// rules agreed.
    fn visitor_to_check(&mut self, name: &str, host: &str) {
        use KeyCode as K;
        assert_eq!(self.type_keys(&[K::TAB, K::ENTER]), ["Visitor sign-in"]);
        assert_eq!(self.screen(), "welcome");
        assert_eq!(self.key(K::ENTER), ["Sign in as a visitor"]);
        assert_eq!(self.screen(), "about-you");
        let then = [K::TAB, K::TAB, K::TAB, K::ENTER];
        assert_eq!(burst(self, name, &then), ["Next"]);
        assert_eq!(self.screen(), "visiting");
        assert_eq!(burst(self, host, &then), ["Next"]);
        assert_eq!(self.screen(), "purpose");
        assert_eq!(
            self.type_keys(&[K::DOWN, K::TAB, K::TAB, K::TAB, K::ENTER]),
            ["Next"]
        );
        assert_eq!(self.screen(), "site-rules");
        assert_eq!(
            self.type_keys(&[K::SPACE, K::TAB, K::TAB, K::TAB, K::ENTER]),
            ["Next"]
        );
        assert_eq!(self.screen(), "check");
        assert!(self.visitor.flow().unwrap().has_entered_data());
    }
}

/// The texts the screen showing holds, in paint order.
fn texts(ui: &Showcase) -> Vec<&'static str> {
    ui.tree()
        .paint_order()
        .filter_map(|(_, node)| match node.kind() {
            NodeKind::Text(text) | NodeKind::Button { label: text, .. } => Some(*text),
            _ => None,
        })
        .collect()
}

#[test]
fn the_pi_boots_to_a_launcher_of_tiles_the_service_desk_first() {
    let stand = Stand::new();
    assert_eq!(stand.screen(), "launcher");
    let shown = texts(&stand.ui);
    let tiles = [
        "Service desk",
        "Visitor sign-in",
        "Pay at the pump",
        "React gallery",
        "Why is this secure?",
    ];
    let at: Vec<usize> = tiles
        .iter()
        .map(|tile| shown.iter().position(|text| text == tile).unwrap())
        .collect();
    assert!(
        at.windows(2).all(|pair| pair[0] < pair[1]),
        "in order: {at:?}"
    );
    assert_eq!(shown.iter().filter(|text| **text == "Ready").count(), 5);
    // A program's tile: the showcase's own program adds none, so
    // it is hidden.
    assert!(!shown.contains(&"Try it"), "{shown:?}");
    assert!(shown.contains(&"Demo: nothing kept"));
    // Every tile is a control, so the keys reach each one; the Service
    // desk, the first ready one, has the focus.
    assert_eq!(buttons(&stand.ui), tiles);
    assert_eq!(focused_label(&stand.ui), Some("Service desk"));
    assert!(stand.ui.holds_nothing());
}

/// The launcher's buttons, in the tree's order.
fn buttons(ui: &Showcase) -> Vec<&'static str> {
    ui.tree()
        .paint_order()
        .filter_map(|(_, node)| match node.kind() {
            NodeKind::Button { label, .. } => Some(*label),
            _ => None,
        })
        .collect()
}

/// The label of the button with the focus, if a button has it.
fn focused_label(ui: &Showcase) -> Option<&'static str> {
    let node = ui.tree().get(ui.tree().focused()?)?;
    match node.kind() {
        NodeKind::Button { label, .. } => Some(*label),
        _ => None,
    }
}

#[test]
fn the_arrow_keys_move_across_the_grid_of_tiles() {
    let mut stand = Stand::new();
    // Right along the top row, stopping before the program's tile's
    // place, which is empty: the showcase's own program adds none.
    for (key, tile) in [
        (KeyCode::RIGHT, "Visitor sign-in"),
        (KeyCode::RIGHT, "Visitor sign-in"),
        (KeyCode::DOWN, "React gallery"),
        (KeyCode::DOWN, "React gallery"),
        (KeyCode::RIGHT, "Why is this secure?"),
        (KeyCode::UP, "Why is this secure?"),
        (KeyCode::RIGHT, "Why is this secure?"),
        (KeyCode::LEFT, "React gallery"),
        (KeyCode::LEFT, "Pay at the pump"),
        (KeyCode::LEFT, "Pay at the pump"),
        (KeyCode::UP, "Service desk"),
        (KeyCode::UP, "Service desk"),
        (KeyCode::DOWN, "Pay at the pump"),
    ] {
        assert!(stand.key(key).is_empty(), "{key:?} chooses nothing");
        assert_eq!(focused_label(&stand.ui), Some(tile), "after {key:?}");
    }
    // Tab still goes through the tiles in order, and Enter on a ready one
    // reached by the arrows opens it.
    stand.tabs(1);
    assert_eq!(focused_label(&stand.ui), Some("React gallery"));
    let mut stand = Stand::new();
    stand.key(KeyCode::DOWN);
    stand.key(KeyCode::RIGHT);
    stand.key(KeyCode::UP);
    assert_eq!(stand.key(KeyCode::ENTER), ["Visitor sign-in"]);
    assert_eq!(stand.screen(), "welcome");
}

/// Opens the gallery from the launcher by keyboard: down to the pump,
/// then right.
fn open_gallery(stand: &mut Stand) {
    assert_eq!(
        stand.type_keys(&[KeyCode::DOWN, KeyCode::RIGHT, KeyCode::ENTER]),
        ["React gallery"]
    );
    assert_eq!(stand.screen(), "gallery-layout");
}

/// The error line's message on the screen showing, if it shows one.
fn field_error(ui: &Showcase) -> Option<&'static str> {
    ui.tree()
        .paint_order()
        .find_map(|(_, node)| match node.kind() {
            NodeKind::FieldError { message, .. } => *message,
            _ => None,
        })
}

#[test]
fn the_gallery_goes_round_every_screen_and_home_closes_it() {
    use showcase_app::GALLERY;
    use showcase_app::generated::gallery_layout::gallery_layout_screen as gallery;
    let mut stand = Stand::new();
    open_gallery(&mut stand);
    assert_eq!(stand.ui.gallery_screen(), Some("gallery-layout"));
    assert_eq!(stand.ui.theme().0, brands::LAUNCHER.name);
    // Focus on Next: Enter goes round them all, each saying where it is.
    assert_eq!(focused_label(&stand.ui), Some("Next"));
    for (at, (name, _)) in GALLERY.iter().enumerate().skip(1) {
        assert_eq!(stand.key(KeyCode::ENTER), ["Next"]);
        assert_eq!(stand.screen(), *name);
        let place = format!("{} of {}", at + 1, GALLERY.len());
        assert!(texts(&stand.ui).iter().any(|text| *text == place), "{name}");
        assert_eq!(focused_label(&stand.ui), Some("Next"), "{name}");
    }
    assert_eq!(stand.key(KeyCode::ENTER), ["Next"]);
    assert_eq!(stand.screen(), "gallery-layout");
    // Back, by mouse, goes round the other way.
    assert_eq!(stand.click(gallery::actions::BACK), Some("Back"));
    assert_eq!(stand.screen(), "gallery-css");
    assert_eq!(stand.click(gallery::actions::HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after the gallery");
    // By mouse from the launcher too; nothing reaches an application.
    assert_eq!(
        stand.click(launcher::actions::GALLERY),
        Some("React gallery")
    );
    assert_eq!(stand.screen(), "gallery-layout");
    assert!(
        stand.actions.is_empty()
            && stand.visitor_actions.is_empty()
            && stand.pump_actions.is_empty()
    );
}

#[test]
fn the_gallery_wipes_what_is_typed_and_chosen() {
    use showcase_app::GALLERY_IDLE;
    let mut stand = Stand::new();
    open_gallery(&mut stand);
    // The text fields: Next five times, then Tab to the first.
    for _ in 0..5 {
        stand.key(KeyCode::ENTER);
    }
    assert_eq!(stand.screen(), "gallery-fields");
    stand.tabs(1);
    stand.text("zqvj");
    assert!(!stand.ui.holds_nothing(), "the field holds what was typed");
    // Next wipes it as the screen closes.
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Next"]);
    assert_eq!(stand.screen(), "gallery-choices");
    assert!(stand.ui.holds_nothing());
    // A box ticked, then Home.
    stand.tabs(1);
    stand.key(KeyCode::SPACE);
    assert!(!stand.ui.holds_nothing(), "the box is ticked");
    stand.shift_tab();
    stand.shift_tab();
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after the choices");
    // Typed, then left alone: it closes, wiped.
    open_gallery(&mut stand);
    for _ in 0..5 {
        stand.key(KeyCode::ENTER);
    }
    stand.tabs(1);
    stand.text("zqvj");
    assert_eq!(stand.ui.idle_after(), Some(GALLERY_IDLE));
    assert!(stand.ui.idle());
    stand.assert_back_at_the_launcher("after the gallery was left alone");
    assert!(stand.actions.is_empty());
}

#[test]
fn the_gallery_keyboard_checks_its_code_as_an_application_would() {
    let mut stand = Stand::new();
    open_gallery(&mut stand);
    for _ in 0..7 {
        stand.key(KeyCode::ENTER);
    }
    assert_eq!(stand.screen(), "gallery-keyboard");
    assert_eq!(field_error(&stand.ui), None);
    // Two digits, then Check: the error line says what it needs.
    stand.tabs(1);
    stand.text("12");
    stand.tabs(1);
    assert_eq!(stand.key(KeyCode::ENTER), ["Check"]);
    assert_eq!(field_error(&stand.ui), Some("Enter four digits"));
    // Four, and it clears.
    stand.shift_tab();
    stand.text("34");
    stand.tabs(1);
    assert_eq!(stand.key(KeyCode::ENTER), ["Check"]);
    assert_eq!(field_error(&stand.ui), None);
    assert!(!stand.ui.holds_nothing());
    assert_eq!(
        stand.click(
            showcase_app::generated::gallery_keyboard::gallery_keyboard_screen::actions::HOME
        ),
        Some("Home")
    );
    stand.assert_back_at_the_launcher("after the keyboard");
}

#[test]
fn a_tile_opens_its_example_at_its_welcome_screen_by_keyboard_or_mouse() {
    let mut stand = Stand::new();
    assert_eq!(stand.key(KeyCode::ENTER), ["Service desk"]);
    assert_eq!(stand.screen(), "welcome");
    // Focus is on the example's own first control, not Home.
    assert_eq!(stand.key(KeyCode::ENTER), ["Start"]);
    assert_eq!(stand.screen(), "step-text");

    let mut stand = Stand::new();
    assert_eq!(
        stand.click(launcher::actions::SERVICE_DESK),
        Some("Service desk")
    );
    assert_eq!(stand.screen(), "welcome");
    assert!(
        stand.actions.is_empty(),
        "opening asks the application nothing"
    );
}

/// The launcher shows in its own brand's theme, each example opened
/// from it in its brand's, and Home brings the launcher's back.
#[test]
fn each_example_shows_in_its_brand_and_home_restores_the_launchers() {
    let mut stand = Stand::new();
    assert_eq!(stand.ui.theme().0, "launcher");
    for (tile, brand) in [
        (launcher::actions::SERVICE_DESK, brands::SERVICE_DESK),
        (launcher::actions::VISITOR, brands::ORRINVALE),
        (launcher::actions::PUMP, brands::TARNWICK),
    ] {
        assert!(stand.click(tile).is_some());
        assert_eq!(stand.screen(), "welcome");
        let (name, theme) = stand.ui.theme();
        assert_eq!(name, brand.name);
        assert_eq!(
            (theme.button, theme.surfaces.header.fill, theme.focus),
            (brand.accent, brand.accent, brand.accent),
            "{name}"
        );
        assert_eq!(stand.click(HOME), Some("Home"));
        stand.assert_back_at_the_launcher(brand.name);
        assert_eq!(stand.ui.theme().1.button, brands::LAUNCHER.accent);
    }
}

/// Laid out in the Pi 5's window at device scale 2 - as the UI PD
/// does once the display announces it - the launcher and the Service desk
/// both fill the 1920 x 1080 window at scale 2, before and after Home, and
/// everything they show fits it.
#[test]
fn the_launcher_and_the_service_desk_follow_the_displays_scale() {
    let window = Rect::new(0, 0, 1920, 1080);
    let mut stand = Stand::new();
    let large = TextMetrics::from_atlas(&REGULAR_48)
        .with_scale(nowayos_display::DeviceScale::TWO)
        .with_min_target(48);
    stand.ui.resize(window, large).unwrap();
    let fits = |stand: &Stand, what: &str| {
        let tree = stand.ui.tree();
        assert_eq!(
            (tree.viewport(), tree.scale()),
            (window, nowayos_display::DeviceScale::TWO),
            "{what}"
        );
        for (_, node) in tree.paint_order() {
            assert!(
                window.contains_rect(&node.rect()),
                "{what}: {:?}",
                node.rect()
            );
        }
    };
    fits(&stand, "the launcher");
    stand.in_use();
    fits(&stand, "a step of the Service desk");
    let desk = stand.ui.service_desk().tree();
    assert_eq!(
        (desk.viewport(), desk.scale()),
        (window, nowayos_display::DeviceScale::TWO)
    );
    assert_eq!(stand.click(HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after Home at 2x");
    fits(&stand, "the launcher after Home");
    let desk = stand.ui.service_desk().tree();
    assert_eq!(
        (desk.viewport(), desk.scale()),
        (window, nowayos_display::DeviceScale::TWO),
        "the wiped example"
    );
}

#[test]
fn home_wipes_the_example_and_shows_the_launcher() {
    // A click on Home, from a step with a name typed.
    let mut stand = Stand::new();
    stand.in_use();
    assert_eq!(stand.click(HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after Home");
    assert_eq!(
        stand.ended(),
        [EndReason::Abandoned],
        "the session abandoned"
    );
    // Home from the keyboard: Shift-Tab from the step's first control.
    stand.in_use();
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after Home from the keyboard");
    // While the answers are on their way, too.
    stand.in_use();
    stand.send();
    assert_eq!(stand.click(HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after Home while sending");
    // The next visitor finds the example empty.
    stand.key(KeyCode::ENTER);
    stand.key(KeyCode::ENTER);
    assert_eq!(stand.ui.tree().value(about_you::fields::NAME), Some(""));
}

#[test]
fn a_second_click_lands_where_the_pointer_is_not_where_it_was() {
    // A double-click on Home: the second lands on the launcher where Home
    // was, not on the tile the launcher's pointer was last over.
    let mut stand = Stand::new();
    assert_eq!(
        stand.click(launcher::actions::SERVICE_DESK),
        Some("Service desk")
    );
    assert_eq!(stand.key(KeyCode::ENTER), ["Start"]);
    stand.text("zqvj");
    assert_eq!(stand.click(HOME), Some("Home"));
    assert_eq!(stand.click_again(), None);
    stand.assert_back_at_the_launcher("after a double-click on Home");
    // A double-click on the tile: the second lands on the Service desk
    // where the tile was, not on Home.
    assert_eq!(
        stand.click(launcher::actions::SERVICE_DESK),
        Some("Service desk")
    );
    assert_ne!(stand.click_again(), Some("Home"));
    assert!(!stand.ui.at_launcher(), "after a double-click on the tile");
    assert_eq!(stand.ended(), [EndReason::Abandoned]);
}

/// The focus ring only after keys, on whichever tree shows - the
/// launcher's or an example's - and the pointer where the mouse last was.
#[test]
fn keys_show_the_ring_on_every_tree_and_a_click_hides_it() {
    let mut stand = Stand::new();
    assert!(!stand.ui.tree().focus_visible(), "no key yet");
    // The arrow keys move across the launcher's grid: the ring shows.
    stand.key(KeyCode::RIGHT);
    assert!(stand.ui.tree().focus_visible());
    stand.key(KeyCode::LEFT);
    // Opened by keys: the example's first control shows its ring.
    assert_eq!(stand.key(KeyCode::ENTER), ["Service desk"]);
    assert!(stand.ui.tree().focus_visible());
    // Home by a click: the launcher shows no ring, and the pointer is
    // where the click was.
    assert_eq!(stand.click(HOME), Some("Home"));
    assert!(stand.ui.at_launcher());
    assert!(!stand.ui.tree().focus_visible());
    assert!(stand.ui.tree().pointer_shown().is_some());
    // A key again: the ring.
    stand.key(KeyCode::TAB);
    assert!(stand.ui.tree().focus_visible());
}

#[test]
fn a_screen_with_no_control_of_its_own_gives_home_no_focus() {
    let mut stand = Stand::new();
    stand.in_use();
    stand.send();
    assert_eq!(stand.ui.tree().focused(), None);
    let sent = stand.actions.len();
    // An impatient Enter or Space while the answers are on their way.
    assert!(stand.key(KeyCode::ENTER).is_empty());
    assert!(stand.key(KeyCode::SPACE).is_empty());
    assert_eq!(stand.screen(), "submitting");
    assert_eq!(
        stand.actions.len(),
        sent,
        "nothing asked of the application"
    );
    assert!(stand.ended().is_empty(), "the session goes on");
    assert!(stand.app.flow().unwrap().has_entered_data());
    // Home is still there for a click.
    assert_eq!(stand.click(HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after Home from the sending screen");
}

#[test]
fn every_ending_of_a_session_shows_the_launcher_again() {
    // Cancel (from the name: the reference, Cancel).
    let mut stand = Stand::new();
    stand.in_use();
    stand.tabs(2);
    assert_eq!(stand.key(KeyCode::ENTER), ["Cancel"]);
    stand.assert_back_at_the_launcher("after Cancel");
    // The idle reset.
    stand.in_use();
    stand.wait(15_000);
    assert_eq!(stand.screen(), "idle-warning");
    stand.wait(15_000);
    stand.assert_back_at_the_launcher("after the idle reset");
    // Clear it now.
    stand.in_use();
    stand.wait(15_000);
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Clear it now"]);
    stand.assert_back_at_the_launcher("after Clear it now");
    // Sent: the confirmation shows, then Done.
    stand.in_use();
    stand.send();
    stand.wait(5_000);
    assert_eq!(stand.screen(), "failed");
    stand.tabs(2);
    assert_eq!(stand.key(KeyCode::ENTER), ["Try again"]);
    stand.wait(1_000);
    assert_eq!(stand.screen(), "submitted");
    assert!(stand.ui.holds_nothing(), "wiped once sent");
    assert_eq!(stand.key(KeyCode::ENTER), ["Done"]);
    stand.assert_back_at_the_launcher("after Done");
    // The confirmation ending by itself.
    stand.in_use();
    stand.send();
    stand.wait(5_000);
    stand.tabs(2);
    stand.key(KeyCode::ENTER);
    stand.wait(1_000);
    assert_eq!(stand.screen(), "submitted");
    stand.wait(10_000);
    stand.assert_back_at_the_launcher("after the confirmation");
    assert_eq!(
        stand.ended(),
        [
            EndReason::Cancelled,
            EndReason::Idle,
            EndReason::Cancelled,
            EndReason::Submitted,
            EndReason::Submitted
        ]
    );
}

#[test]
fn a_report_the_ui_cannot_trust_shows_the_launcher_with_both_ends_wiped() {
    let mut stand = Stand::new();
    stand.in_use();
    assert!(
        stand
            .ui
            .receive_report(SERVICE_DESK_APP, &[0xFF; MAX_RECORD_LEN])
            .is_err()
    );
    assert_eq!(stand.screen(), "launcher");
    stand.pump();
    stand.assert_back_at_the_launcher("after a report it could not decode");
    assert_eq!(stand.ended(), [EndReason::Abandoned]);
}

#[test]
fn the_launcher_holds_no_field_and_ignores_input_but_its_tile() {
    let mut stand = Stand::new();
    stand.text("zqvj");
    stand.tabs(3);
    assert_eq!(stand.screen(), "launcher");
    assert!(stand.actions.is_empty());
    assert!(stand.ui.holds_nothing());
}

/// One burst of keys, as the UART input PD makes them from a `serial`
/// step: letters and digits, then the named keys.
fn burst(stand: &mut Stand, text: &str, then: &[KeyCode]) -> Vec<&'static str> {
    let mut keys: Vec<KeyCode> = text
        .chars()
        .map(|ch| match ch {
            '0'..='9' => KeyCode::digit(u8::try_from(ch.to_digit(10).unwrap()).unwrap()),
            _ => KeyCode::letter(ch),
        })
        .map(Option::unwrap)
        .collect();
    keys.extend_from_slice(then);
    stand.type_keys(&keys)
}

#[test]
fn the_qemu_script_s_keys_on_the_host() {
    // systems/showcase.qmp, burst by burst. First the visitor sign-in:
    // signed in, with a pass, and Done once it has counted down a second.
    let mut stand = Stand::new();
    stand.visitor_to_check("zqvje", "zqvjf");
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Sign in"]
    );
    assert_eq!(stand.screen(), "signing-in");
    stand.wait(1_000);
    assert_eq!(stand.screen(), "signed-in");
    assert_eq!(stand.ui.visitor().issued(), Some("4821"));
    assert_eq!(stand.ui.seconds_left(), Some(10));
    let at = stand.visitor.next_wake().unwrap();
    stand.wait(at - stand.now);
    assert_eq!(stand.ui.seconds_left(), Some(9));
    assert_eq!(stand.key(KeyCode::ENTER), ["Done"]);
    stand.assert_back_at_the_launcher("after the visitor's Done");
    // Again, with the building service down: sent to reception; Finish.
    stand.visitor_to_check("zqvjg", "zqvjh");
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Demo: service down"]
    );
    stand.wait(1_000);
    assert_eq!(stand.screen(), "not-signed-in");
    assert_eq!(stand.type_keys(&[KeyCode::TAB, KeyCode::ENTER]), ["Finish"]);
    stand.assert_back_at_the_launcher("after the visitor's Finish");
    assert_eq!(
        stand.visitor_ended(),
        [EndReason::Submitted, EndReason::Cancelled]
    );
    // Then the Service desk.
    assert_eq!(stand.key(KeyCode::ENTER), ["Service desk"]);
    assert_eq!(stand.key(KeyCode::ENTER), ["Start"]);
    // "zqvj\e[Z\r": a name, Shift-Tab to Home, Enter.
    stand.text("zqvj");
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after Home");
    // "zqvjb\t\t\r": a name, the reference, Cancel.
    stand.key(KeyCode::ENTER);
    stand.key(KeyCode::ENTER);
    assert_eq!(
        burst(
            &mut stand,
            "zqvjb",
            &[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]
        ),
        ["Cancel"]
    );
    stand.assert_back_at_the_launcher("after Cancel");
    // Through to sending, failed, Try again, Done.
    stand.key(KeyCode::ENTER);
    stand.key(KeyCode::ENTER);
    assert_eq!(
        burst(&mut stand, "zqvjc", &[KeyCode::TAB]),
        Vec::<&str>::new()
    );
    assert_eq!(
        burst(
            &mut stand,
            REFERENCE,
            &[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]
        ),
        ["Next"]
    );
    let down: Vec<KeyCode> = [KeyCode::DOWN]
        .into_iter()
        .chain([KeyCode::TAB; 4])
        .chain([KeyCode::ENTER])
        .collect();
    assert_eq!(stand.type_keys(&down), ["Next"]);
    let tick: Vec<KeyCode> = [KeyCode::SPACE]
        .into_iter()
        .chain([KeyCode::TAB; 4])
        .chain([KeyCode::ENTER])
        .collect();
    assert_eq!(stand.type_keys(&tick), ["Next"]);
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Continue"]
    );
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Send"]
    );
    stand.wait(5_000);
    assert_eq!(stand.screen(), "failed");
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::TAB, KeyCode::ENTER]),
        ["Try again"]
    );
    stand.wait(1_000);
    // Done, once the confirmation has counted down a second.
    assert_eq!(stand.ui.seconds_left(), Some(10));
    let at = stand.app.next_wake().unwrap();
    stand.wait(at - stand.now);
    assert_eq!(stand.ui.seconds_left(), Some(9));
    assert_eq!(stand.key(KeyCode::ENTER), ["Done"]);
    stand.assert_back_at_the_launcher("after Done");
    // A name, then the idle path.
    stand.key(KeyCode::ENTER);
    stand.key(KeyCode::ENTER);
    stand.text("zqvjd");
    stand.wait(15_000);
    assert_eq!(stand.screen(), "idle-warning");
    stand.wait(15_000);
    stand.assert_back_at_the_launcher("after the idle reset");
    assert_eq!(
        stand.ended(),
        [
            EndReason::Abandoned,
            EndReason::Cancelled,
            EndReason::Submitted,
            EndReason::Idle
        ]
    );
    // Last, the pay at the pump: pump 3, unleaded, £40, paid on the card
    // reader, followed to all done and cleared for the next customer.
    use KeyCode as K;
    let pick = |downs: usize, tabs: usize| {
        let mut keys = vec![K::DOWN; downs];
        keys.extend(vec![K::TAB; tabs]);
        keys.push(K::ENTER);
        keys
    };
    assert_eq!(stand.type_keys(&[K::DOWN, K::ENTER]), ["Pay at the pump"]);
    assert_eq!(stand.key(K::ENTER), ["Start"]);
    assert_eq!(stand.type_keys(&pick(3, 2)), ["Next"]);
    assert_eq!(stand.type_keys(&pick(1, 3)), ["Next"]);
    assert_eq!(stand.type_keys(&pick(3, 3)), ["Next"]);
    assert_eq!(stand.screen(), "check");
    assert_eq!(
        stand.type_keys(&[K::TAB, K::TAB, K::TAB, K::ENTER]),
        ["Pay on the card reader"]
    );
    stand.wait(1_000);
    assert_eq!(stand.screen(), "fuelling");
    stand.wait(24_000);
    stand.assert_back_at_the_launcher("after the pump");
    // Again, declined: payment not accepted; Finish.
    assert_eq!(stand.type_keys(&[K::DOWN, K::ENTER]), ["Pay at the pump"]);
    assert_eq!(stand.key(K::ENTER), ["Start"]);
    assert_eq!(stand.type_keys(&pick(1, 2)), ["Next"]);
    assert_eq!(stand.type_keys(&pick(1, 3)), ["Next"]);
    assert_eq!(stand.type_keys(&pick(1, 3)), ["Next"]);
    assert_eq!(
        stand.type_keys(&[K::TAB, K::TAB, K::ENTER]),
        ["Demo: card declined"]
    );
    stand.wait(1_000);
    assert_eq!(stand.screen(), "not-accepted");
    assert_eq!(stand.type_keys(&[K::TAB, K::ENTER]), ["Finish"]);
    stand.assert_back_at_the_launcher("after the pump's Finish");
    assert_eq!(
        stand.pump_ended(),
        [EndReason::Submitted, EndReason::Cancelled]
    );
    // "Why is this secure?": Down, Right, Right, Enter; Home.
    assert_eq!(
        stand.type_keys(&[K::DOWN, K::RIGHT, K::RIGHT, K::ENTER]),
        ["Why is this secure?"]
    );
    assert_eq!(stand.key(K::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after the panel");
    // The Screen Compiler gallery: Down, Right, Enter; Next round all
    // eleven screens and back to the first, with Check on the keyboard
    // screen's empty code; Shift-Tab twice to Home.
    assert_eq!(
        stand.type_keys(&[K::DOWN, K::RIGHT, K::ENTER]),
        ["React gallery"]
    );
    for (name, _) in showcase_app::GALLERY.iter().skip(1) {
        assert_eq!(stand.key(K::ENTER), ["Next"]);
        assert_eq!(stand.screen(), *name);
        if *name == "gallery-keyboard" {
            assert_eq!(stand.type_keys(&[K::TAB, K::TAB, K::ENTER]), ["Check"]);
            stand.shift_tab();
            stand.shift_tab();
        }
    }
    assert_eq!(stand.key(K::ENTER), ["Next"]);
    assert_eq!(stand.screen(), "gallery-layout");
    stand.shift_tab();
    stand.shift_tab();
    assert_eq!(stand.key(K::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after the gallery");
}

#[test]
fn each_example_hears_only_its_own_application() {
    // The visitor sign-in in use: a report on the Service desk's rings
    // does not reach it, and its actions go only to its own application.
    let mut stand = Stand::new();
    stand.visitor_to_check("zqvje", "zqvjf");
    assert!(
        !stand
            .actions
            .iter()
            .any(|action| matches!(action, Action::Start(_)))
    );
    assert!(
        stand
            .visitor_actions
            .iter()
            .any(|action| matches!(action, Action::Start(_)))
    );
    let mut slot = [0u8; MAX_RECORD_LEN];
    Report::ShowWelcome(showcase_kiosk_contract::ShowWelcome)
        .encode(&mut slot)
        .unwrap();
    stand.ui.receive_report(SERVICE_DESK_APP, &slot).unwrap();
    assert_eq!(stand.screen(), "check", "the visitor's screen stays");
    // A report from an application the showcase lacks - the program's
    // example's, as the showcase's own program adds none: every
    // example wiped and abandoned, the launcher shown.
    assert!(stand.ui.receive_report(EXTRA_APP, &slot).is_err());
    assert_eq!(stand.screen(), "launcher");
    stand.pump();
    stand.assert_back_at_the_launcher("after a report from no application");
    assert_eq!(stand.visitor_ended(), [EndReason::Abandoned]);
}

#[test]
fn home_wipes_the_visitor_sign_in_too() {
    let mut stand = Stand::new();
    assert_eq!(
        stand.type_keys(&[KeyCode::TAB, KeyCode::ENTER]),
        ["Visitor sign-in"]
    );
    assert_eq!(stand.key(KeyCode::ENTER), ["Sign in as a visitor"]);
    stand.text("zqvj");
    assert!(stand.visitor.flow().unwrap().phase().holds_answers());
    // Home, from the keyboard: Shift-Tab from the name.
    stand.shift_tab();
    assert_eq!(stand.key(KeyCode::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after Home from the visitor sign-in");
    assert_eq!(stand.visitor_ended(), [EndReason::Abandoned]);
    assert!(stand.ended().is_empty(), "the Service desk untouched");
}

impl Stand {
    /// How each pay at the pump session ended.
    fn pump_ended(&self) -> Vec<EndReason> {
        self.pump_reports
            .iter()
            .filter_map(|report| match report {
                Report::SessionEnded(ended) => Some(ended.reason),
                _ => None,
            })
            .collect()
    }

    /// The pay at the pump from the launcher to its check screen, by
    /// keyboard alone: pump 3, unleaded, £40.
    fn pump_to_check(&mut self) {
        use KeyCode as K;
        assert_eq!(self.type_keys(&[K::DOWN, K::ENTER]), ["Pay at the pump"]);
        assert_eq!(self.screen(), "welcome");
        assert_eq!(self.key(K::ENTER), ["Start"]);
        for (step, downs, tabs) in [("pump", 3, 2), ("fuel", 1, 3), ("amount", 3, 3)] {
            assert_eq!(self.screen(), step);
            let mut keys = vec![K::DOWN; downs];
            keys.extend(vec![K::TAB; tabs]);
            keys.push(K::ENTER);
            assert_eq!(self.type_keys(&keys), ["Next"], "on {step}");
        }
        assert_eq!(self.screen(), "check");
        assert!(self.pump_app.flow().unwrap().has_entered_data());
    }
}

/// The texts the screen showing holds, in paint order, the application's
/// slots included.
fn shown(ui: &Showcase) -> Vec<String> {
    ui.tree()
        .paint_order()
        .filter_map(|(_, node)| match node.kind() {
            NodeKind::Text(text) | NodeKind::Button { label: text, .. } => Some((*text).to_owned()),
            NodeKind::Slot { text, .. } => Some(text.as_str().to_owned()),
            _ => None,
        })
        .collect()
}

/// The pay at the pump from the launcher to the next customer -
/// choose, check, pay on the card reader, then the pump ready, fuelling
/// and all done as the confirmation counts down - with the screen learning
/// only what was approved, every choice wiped first.
#[test]
fn the_pump_runs_from_the_launcher_to_the_next_customer() {
    use showcase_pump::generated::check::check_screen as check;
    let mut stand = Stand::new();
    stand.pump_to_check();
    let on_screen = shown(&stand.ui);
    for text in ["3", "Unleaded E10", "£40.00"] {
        assert!(
            on_screen.iter().any(|t| t == text),
            "the check shows {text}: {on_screen:?}"
        );
    }
    assert_eq!(
        stand.click(check::actions::PAY),
        Some("Pay on the card reader")
    );
    assert_eq!(stand.screen(), "paying");
    assert!(
        shown(&stand.ui)
            .iter()
            .any(|t| t == "Follow the instructions on the card reader")
    );

    // The card reader approves a second later: the screen learns the pump,
    // fuel and amount approved, as a code, and nothing more.
    stand.wait(1_000);
    assert_eq!(stand.screen(), "fuelling");
    let issued: Vec<&str> = stand
        .pump_reports
        .iter()
        .filter_map(|report| match report {
            Report::Issued(issued) => Some(issued.code()),
            _ => None,
        })
        .collect();
    assert_eq!(issued, ["202"]);
    assert!(
        !stand.pump_app.flow().unwrap().has_entered_data(),
        "wiped first"
    );
    let on_screen = shown(&stand.ui);
    for text in ["£40.00", "Pump 3 ready: lift the nozzle", "£0.00", "0.00"] {
        assert!(
            on_screen.iter().any(|t| t == text),
            "ready shows {text}: {on_screen:?}"
        );
    }

    // Fuelling: the pounds and litres count up, labelled as simulated.
    stand.wait(6_000);
    let on_screen = shown(&stand.ui);
    assert!(
        on_screen.iter().any(|t| t == "Fuelling at pump 3"),
        "{on_screen:?}"
    );
    assert!(
        on_screen
            .iter()
            .any(|t| t == "Simulated and sped up. The pump's own display is the official reading.")
    );
    let cost = |stand: &Stand| {
        shown(&stand.ui)
            .into_iter()
            .rfind(|text| text.starts_with('£'))
            .map(|text| text.trim_start_matches('£').parse::<f64>().unwrap())
    };
    let early = cost(&stand).unwrap();
    stand.wait(3_000);
    let later = cost(&stand).unwrap();
    assert!(later > early, "{early} then {later}");

    // All done: exactly the amount approved, and the card never here.
    stand.wait(7_000);
    let on_screen = shown(&stand.ui);
    for text in ["All done. Card details: never here.", "£40.00", "27.99"] {
        assert!(
            on_screen.iter().any(|t| t == text),
            "all done shows {text}: {on_screen:?}"
        );
    }

    // The confirmation ends: cleared for the next customer.
    stand.wait(10_000);
    stand.assert_back_at_the_launcher("after the pump");
    assert_eq!(stand.pump_ended(), [EndReason::Submitted]);
}

/// A card declined - asked for by the demonstration - says the
/// payment was not accepted and starts no pump; Finish clears it.
#[test]
fn a_card_declined_says_payment_not_accepted_and_starts_no_pump() {
    use showcase_pump::generated::check::check_screen as check;
    use showcase_pump::generated::not_accepted::not_accepted_screen as failed;
    let mut stand = Stand::new();
    stand.pump_to_check();
    assert_eq!(
        stand.click(check::actions::DECLINE),
        Some("Demo: card declined")
    );
    stand.wait(1_000);
    assert_eq!(stand.screen(), "not-accepted");
    assert!(shown(&stand.ui).iter().any(|t| t == "Payment not accepted"));
    assert!(
        !stand
            .pump_reports
            .iter()
            .any(|report| matches!(report, Report::Issued(_))),
        "nothing approved"
    );
    assert_eq!(stand.click(failed::actions::CANCEL), Some("Finish"));
    stand.assert_back_at_the_launcher("after a card declined");
    assert_eq!(stand.pump_ended(), [EndReason::Cancelled]);
}

/// No pay at the pump screen has a field to type into - nowhere to
/// enter a card number or a PIN.
#[test]
fn no_pump_screen_asks_for_a_card_number_or_a_pin() {
    for (name, build) in showcase_pump::generated::SCREENS {
        let mut tree = nowayos_ui::Tree::<{ showcase_pump::generated::CAPACITY }>::new();
        let root = build(&mut tree).unwrap();
        tree.set_root(root).unwrap();
        assert!(
            !tree
                .paint_order()
                .any(|(_, node)| matches!(node.kind(), NodeKind::TextInput { .. })),
            "{name} has a text field"
        );
        for (_, node) in tree.paint_order() {
            if let NodeKind::Text(text) | NodeKind::Button { label: text, .. } = node.kind() {
                assert!(!text.contains("PIN"), "{name}: {text}");
            }
        }
    }
}

/// The sixth tile opens "Why is this secure?" in the launcher's
/// place - by keyboard or mouse - six short claims each marked Built,
/// Designed or Simulated, in words the maintainers have approved;
/// Home goes back, and nothing is asked of any application.
#[test]
fn why_is_this_secure_marks_every_claim_and_goes_home() {
    use KeyCode as K;
    use showcase_app::generated::why_secure::why_secure_screen as why_secure;
    let mut stand = Stand::new();
    // Down to the pump, then right twice.
    assert_eq!(
        stand.type_keys(&[K::DOWN, K::RIGHT, K::RIGHT, K::ENTER]),
        ["Why is this secure?"]
    );
    assert_eq!(stand.screen(), "why-secure");
    let on_screen = texts(&stand.ui);
    let marks = ["Built", "Designed", "Simulated"];
    let marked = on_screen.iter().filter(|text| marks.contains(text)).count();
    assert_eq!(marked, 6, "{on_screen:?}");
    for mark in marks {
        assert!(on_screen.contains(&mark), "some claim is {mark}");
    }
    for text in &on_screen {
        let lower = text.to_lowercase();
        for banned in [
            "unhackable",
            "bulletproof",
            "formally verified",
            "proven secure",
            "tamper",
            "pci",
            "emv",
            "never stay",
            "ready to deploy",
        ] {
            assert!(!lower.contains(banned), "{text}");
        }
        if lower.contains("certified") {
            assert_eq!(
                *text,
                "In development: a platform, not a certified product."
            );
        }
    }
    // Home has the focus: Enter goes back.
    assert_eq!(stand.key(K::ENTER), ["Home"]);
    stand.assert_back_at_the_launcher("after the panel");
    // By mouse too.
    assert_eq!(
        stand.click(launcher::actions::WHY_SECURE),
        Some("Why is this secure?")
    );
    assert_eq!(stand.screen(), "why-secure");
    assert_eq!(stand.click(why_secure::actions::HOME), Some("Home"));
    stand.assert_back_at_the_launcher("after the panel by mouse");
    assert!(stand.actions.is_empty() && stand.pump_actions.is_empty());
}

/// After a while with no input the launcher gives way to slides,
/// in turn; the first key, click or touch only wakes it and presses
/// nothing. Neither an example nor the panel ever goes idle.
#[test]
fn the_idle_launcher_shows_slides_and_the_first_input_only_wakes_it() {
    use showcase_app::{ATTRACT_AFTER, SLIDE_FOR, SLIDES};
    let mut stand = Stand::new();
    assert_eq!(stand.ui.idle_after(), Some(ATTRACT_AFTER));
    assert!(stand.ui.idle());
    assert_eq!(stand.screen(), "attract-1");
    assert!(texts(&stand.ui).contains(&"Press any key or touch the screen to start"));
    assert_eq!(stand.ui.idle_after(), Some(SLIDE_FOR));
    // Each slide in its example's brand, the last in the launcher's.
    assert_eq!(stand.ui.theme().0, brands::ORRINVALE.name);
    for (name, brand) in [
        ("attract-2", brands::TARNWICK),
        ("attract-3", brands::LAUNCHER),
        ("attract-1", brands::ORRINVALE),
    ] {
        assert!(stand.ui.idle());
        assert_eq!(stand.screen(), name);
        assert_eq!(stand.ui.theme().0, brand.name);
    }
    let shown = shown(&stand.ui);
    assert!(shown.iter().any(|text| text == SLIDES[0].0), "{shown:?}");
    // A key wakes the launcher, focus on its first tile, and presses
    // nothing: Enter would have opened the Service desk.
    assert_eq!(stand.key(KeyCode::ENTER), Vec::<&str>::new());
    stand.assert_back_at_the_launcher("woken by a key");
    assert_eq!(stand.ui.idle_after(), Some(ATTRACT_AFTER));
    // A click over the Service desk's tile: woken, nothing pressed.
    let tile = stand
        .ui
        .tree()
        .paint_order()
        .find(|(_, node)| matches!(node.kind(), NodeKind::Button { action, .. } if *action == launcher::actions::SERVICE_DESK))
        .map(|(_, node)| node.rect().center())
        .unwrap();
    assert!(stand.ui.idle());
    stand.ui.handle(InputEvent::PointerMove {
        x: tile.x,
        y: tile.y,
    });
    assert_eq!(stand.screen(), "attract-1", "a move alone does not wake it");
    for down in [true, false] {
        assert_eq!(
            stand
                .ui
                .handle(InputEvent::PointerButton { button: 0, down }),
            None
        );
    }
    stand.ui.end_input();
    stand.pump();
    stand.assert_back_at_the_launcher("woken by a click");
    // A touch too.
    assert!(stand.ui.idle());
    for phase in [TouchPhase::Start, TouchPhase::End] {
        assert_eq!(
            stand.ui.handle(InputEvent::Touch {
                id: 0,
                x: tile.x,
                y: tile.y,
                phase,
            }),
            None
        );
    }
    stand.assert_back_at_the_launcher("woken by a touch");
    assert!(stand.actions.is_empty(), "nothing asked of an application");
    // An example, and the panel, never go idle.
    assert_eq!(stand.key(KeyCode::ENTER), ["Service desk"]);
    assert_eq!(stand.ui.idle_after(), None);
    assert!(!stand.ui.idle());
    assert_eq!(stand.click(HOME), Some("Home"));
    assert_eq!(
        stand.click(launcher::actions::WHY_SECURE),
        Some("Why is this secure?")
    );
    assert_eq!(stand.ui.idle_after(), None);
    assert!(!stand.ui.idle());
}
