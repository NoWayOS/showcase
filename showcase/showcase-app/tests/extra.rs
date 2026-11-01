// SPDX-License-Identifier: MIT OR Apache-2.0
//! A program's example: the launcher's third tile, which a
//! program may fill, and the example it opens, driven over the app-UI
//! contract on application 3's rings and wiped as the showcase's own are.
//! The sample here is the reference kiosk's screens under the Service
//! desk's bar, in Larchway's colours; a real one is a private product's.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed step fails the test"
)]

use nowayos_glyphs::REGULAR_24;
use nowayos_input::{InputEvent, KeyCode};
use nowayos_ui::{NodeId, NodeKind, TextMetrics, Tree, UiError};
use showcase_app::generated::CAPACITY;
use showcase_app::generated::launcher::launcher_screen as launcher;
use showcase_app::{EXTRA_APP, Extra, HOME, Showcase, Tile, brands, service_desk_frame};
use showcase_kiosk_contract::{EndReason, MAX_RECORD_LEN, Report};
use showcase_kiosk_screens::app::KioskApp;
use showcase_kiosk_screens::kiosk::{KioskScreens, Reference};

/// The sample program's example.
struct Sample;

impl Extra for Sample {
    type Screens = Reference;

    const TILE: Option<Tile> = Some(Tile {
        brand: "Larchway Kitchen",
        title: "Sample example",
        line: "A program's own",
        slide: "Added by a program",
        look: brands::LARCHWAY,
        larger_text: None,
    });

    fn frame(
        tree: &mut Tree<CAPACITY>,
        body: NodeId,
        screen: &'static str,
    ) -> Result<NodeId, UiError> {
        service_desk_frame(tree, body, screen)
    }
}

/// The showcase with the sample, and the sample's application.
struct Stand {
    ui: Showcase<Sample>,
    app: KioskApp,
    reports: Vec<Report>,
}

impl Stand {
    fn new() -> Self {
        Self {
            ui: Showcase::new(TextMetrics::from_atlas(&REGULAR_24)).unwrap(),
            app: KioskApp::new(3).unwrap(),
            reports: Vec::new(),
        }
    }

    /// Sends the UI's actions for application 3 to the sample's
    /// application, and its reports back.
    fn pump(&mut self) {
        loop {
            let reports: Vec<Report> = self.app.reports().copied().collect();
            self.app.sent(reports.len());
            for report in reports {
                let mut slot = [0u8; MAX_RECORD_LEN];
                report.encode(&mut slot).unwrap();
                self.reports.push(report);
                self.ui.receive_report(EXTRA_APP, &slot).unwrap();
            }
            while self.app.take_note().is_some() {}
            let actions: Vec<_> = self.ui.actions(EXTRA_APP).cloned().collect();
            self.ui.sent(EXTRA_APP, actions.len());
            if actions.is_empty() {
                return;
            }
            for action in actions {
                let mut slot = [0u8; MAX_RECORD_LEN];
                action.encode(&mut slot).unwrap();
                self.app.receive(&slot, 1_000);
            }
        }
    }

    fn key(&mut self, key: KeyCode) -> Vec<&'static str> {
        let mut chosen = Vec::new();
        for event in [InputEvent::KeyDown(key), InputEvent::KeyUp(key)] {
            chosen.extend(self.ui.handle(event));
        }
        self.ui.end_input();
        self.pump();
        chosen
    }
}

/// The texts and labels the screen showing holds, the slots' included, in
/// paint order.
fn shown(ui: &Showcase<Sample>) -> Vec<String> {
    ui.tree()
        .paint_order()
        .filter_map(|(_, node)| match node.kind() {
            NodeKind::Text(text) | NodeKind::Button { label: text, .. } => Some((*text).to_owned()),
            NodeKind::Slot { text, .. } => Some(text.as_str().to_owned()),
            _ => None,
        })
        .collect()
}

/// The label of the button with the focus.
fn focused(ui: &Showcase<Sample>) -> Option<&'static str> {
    match ui.tree().get(ui.tree().focused()?)?.kind() {
        NodeKind::Button { label, .. } => Some(*label),
        _ => None,
    }
}

#[test]
fn a_program_s_tile_shows_its_brand_title_and_line_in_the_third_place() {
    let stand = Stand::new();
    assert_eq!(<Showcase<Sample> as KioskScreens>::APPS, 4);
    assert_eq!(<Showcase as KioskScreens>::APPS, 3);
    let shown = shown(&stand.ui);
    let tile = [
        "Larchway Kitchen",
        "Sample example",
        "Try it",
        "A program's own",
    ];
    let at = shown.iter().position(|text| text == tile[0]).unwrap();
    assert_eq!(shown.get(at..at + tile.len()).unwrap(), tile);
    let visitor = shown.iter().position(|text| text == "Forgets you").unwrap();
    let pump = shown
        .iter()
        .position(|text| text == "Tarnwick Fuel")
        .unwrap();
    assert!(visitor < at && at < pump, "{shown:?}");
    assert!(stand.ui.holds_nothing(), "fixed text, nothing entered");
}

#[test]
fn the_arrow_keys_and_tab_reach_a_program_s_tile() {
    let mut stand = Stand::new();
    for (key, label) in [
        (KeyCode::RIGHT, "Visitor sign-in"),
        (KeyCode::RIGHT, "Try it"),
        (KeyCode::DOWN, "Why is this secure?"),
        (KeyCode::UP, "Try it"),
        (KeyCode::LEFT, "Visitor sign-in"),
    ] {
        assert!(stand.key(key).is_empty());
        assert_eq!(focused(&stand.ui), Some(label), "after {key:?}");
    }
    assert_eq!(stand.key(KeyCode::TAB), Vec::<&str>::new());
    assert_eq!(focused(&stand.ui), Some("Try it"));
}

#[test]
fn a_program_s_example_runs_over_its_own_rings_and_home_wipes_it() {
    let mut stand = Stand::new();
    // Tab twice to its tile: its title is what the log shows.
    assert_eq!(stand.key(KeyCode::TAB), Vec::<&str>::new());
    assert_eq!(stand.key(KeyCode::TAB), Vec::<&str>::new());
    assert_eq!(stand.key(KeyCode::ENTER), ["Sample example"]);
    assert_eq!(stand.ui.screen(), "welcome");
    assert_eq!(stand.ui.theme().0, brands::LARCHWAY.name);
    assert_eq!(stand.key(KeyCode::ENTER), ["Start"]);
    assert_eq!(stand.ui.screen(), "step-text");
    for ch in "zqvj".chars() {
        stand.key(KeyCode::letter(ch).unwrap());
    }
    assert!(!stand.ui.holds_nothing());
    assert!(stand.app.flow().unwrap().has_entered_data());
    // Its actions reach only application 3.
    assert_eq!(stand.ui.actions(0).count(), 0);
    // Home wipes both ends and shows the launcher, in its own brand.
    let home = stand
        .ui
        .tree()
        .paint_order()
        .find(|(_, node)| matches!(node.kind(), NodeKind::Button { action, .. } if *action == HOME))
        .map(|(_, node)| node.rect().center())
        .unwrap();
    stand.ui.handle(InputEvent::PointerMove {
        x: home.x,
        y: home.y,
    });
    for down in [true, false] {
        stand
            .ui
            .handle(InputEvent::PointerButton { button: 0, down });
    }
    stand.ui.end_input();
    stand.pump();
    assert!(stand.ui.at_launcher());
    assert_eq!(stand.ui.theme().0, "launcher");
    assert!(stand.ui.holds_nothing());
    assert!(!stand.app.flow().unwrap().has_entered_data());
    let ended: Vec<EndReason> = stand
        .reports
        .iter()
        .filter_map(|report| match report {
            Report::SessionEnded(ended) => Some(ended.reason),
            _ => None,
        })
        .collect();
    assert_eq!(ended, [EndReason::Abandoned]);
    // By mouse too, from the tile.
    let tile = stand
        .ui
        .tree()
        .paint_order()
        .find(|(_, node)| matches!(node.kind(), NodeKind::Button { action, .. } if *action == launcher::actions::EXTRA))
        .map(|(_, node)| node.rect().center())
        .unwrap();
    stand.ui.handle(InputEvent::PointerMove {
        x: tile.x,
        y: tile.y,
    });
    let mut chosen = Vec::new();
    for down in [true, false] {
        chosen.extend(
            stand
                .ui
                .handle(InputEvent::PointerButton { button: 0, down }),
        );
    }
    assert_eq!(chosen, ["Sample example"]);
}

#[test]
fn a_program_s_slide_shows_after_the_examples_in_its_brand() {
    let mut stand = Stand::new();
    for (name, brand) in [
        ("attract-1", brands::ORRINVALE),
        ("attract-2", brands::TARNWICK),
        ("attract-3", brands::LARCHWAY),
        ("attract-4", brands::LAUNCHER),
        ("attract-1", brands::ORRINVALE),
    ] {
        assert!(stand.ui.idle());
        assert_eq!(stand.ui.screen(), name);
        assert_eq!(stand.ui.theme().0, brand.name);
        if name == "attract-3" {
            let shown = shown(&stand.ui);
            assert!(
                shown.iter().any(|text| text == "Sample example"),
                "{shown:?}"
            );
            assert!(shown.iter().any(|text| text == "Added by a program"));
        }
    }
}
