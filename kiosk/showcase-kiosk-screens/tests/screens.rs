// SPDX-License-Identifier: MIT OR Apache-2.0
//! Every kiosk reference screen converts (all ten), builds on a real
//! `nowayos-ui` tree, lays out, and fits the kiosk's display in both of
//! NoWayOS's themes - the light theme, its body text 18 px, and the
//! large-text theme, 24 px - with the metrics each gives layout:
//! its type scale, its minimum target size and its buttons' height.
//!
//! The kiosk's layout size is 800 x 480, where a screen must fit. Each
//! screen is also laid out at 1920 x 1080, and as the Pi 5's monitor shows
//! it, drawn at its own pixels: the 1920 x 1080 window at device
//! scale 2 - a 960 x 540 screen - with each font's atlas twice the size,
//! and must fit there too.
//! Each is checked as it is built - every group hidden and every slot empty -
//! and as the application may show it: every group visible, every slot
//! holding text. Fitting means every node that shows lies inside the
//! display, and every fixed text - a text, a label, an option, a
//! placeholder - is shown whole, not cut at its node's edge (a node shows
//! one line: text does not wrap yet). A slot's text is the application's,
//! cut at the slot's edge if it is longer.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a screen that fails to build or fit fails the test"
)]

use nowayos_display::Rect;
use nowayos_render_eg::{Rgb888, Theme};
use nowayos_ui::{GroupId, NodeKind, SlotId, TextMetrics, Tree, UiError};
use showcase_kiosk_screens::generated::{Build, CAPACITY, SCREENS, TEXT};

/// The kiosk's layout size, the Pi monitor's resolution, and the Pi 5's
/// window at device scale 2: each with its device scale.
const VIEWPORTS: [(&str, Rect, u32); 3] = [
    ("800 x 480", Rect::new(0, 0, 800, 480), 1),
    ("1920 x 1080", Rect::new(0, 0, 1920, 1080), 1),
    ("960 x 540 at 2x", Rect::new(0, 0, 1920, 1080), 2),
];

/// NoWayOS's two themes, as the UI PDs lay screens out with them.
fn themes() -> [(&'static str, Theme<Rgb888>); 2] {
    Theme::all()
}

/// More groups and slots than any screen numbers: they are numbered from 1.
const MOST: u16 = 32;

/// Text the application shows in a slot: an answer as long as a field
/// takes, or an error message.
const SHOWN: &str = "Please enter the reference number in digits only";

/// Builds `build` on a tree, shows every group and fills every slot when
/// `shown`, and lays it out.
fn laid_out(
    name: &str,
    build: Build,
    shown: bool,
    viewport: Rect,
    metrics: TextMetrics,
) -> Tree<CAPACITY, TEXT> {
    let mut tree = Tree::<CAPACITY, TEXT>::new();
    let root = build(&mut tree).unwrap_or_else(|err| panic!("{name}: {err:?}"));
    tree.set_root(root).unwrap();
    if shown {
        for number in 1..=MOST {
            match tree.set_visible(GroupId::new(number), true) {
                Ok(()) | Err(UiError::UnknownGroup(_)) => {}
                Err(err) => panic!("{name}: group {number}: {err:?}"),
            }
            match tree.set_slot(SlotId::new(number), SHOWN) {
                Ok(()) | Err(UiError::UnknownSlot(_)) => {}
                Err(err) => panic!("{name}: slot {number}: {err:?}"),
            }
        }
    }
    tree.layout(viewport, &metrics).unwrap();
    tree
}

/// Every node that shows lies inside `viewport`, and every fixed text -
/// every line of one that wraps - is shown whole, each in its own
/// role and weight (`Node::cut_text`); a slot's text, the application's,
/// only if it wraps.
fn assert_fits(tree: &Tree<CAPACITY, TEXT>, viewport: Rect, metrics: &TextMetrics, what: &str) {
    for (id, node) in tree.paint_order() {
        assert!(
            viewport.contains_rect(&node.rect()),
            "{what}: node {id:?} at {:?} does not fit",
            node.rect()
        );
        let one_line_slot =
            matches!(node.kind(), NodeKind::Slot { .. }) && !node.style().text.wraps();
        if let Some(text) = node.cut_text(metrics).filter(|_| !one_line_slot) {
            panic!("{what}: {text:?} is cut in {:?}", node.content());
        }
    }
}

#[test]
fn all_ten_reference_screens_convert() {
    let names: Vec<&str> = SCREENS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "failed",
            "idle-warning",
            "review",
            "step-checkbox",
            "step-choice",
            "step-notice",
            "step-text",
            "submitted",
            "submitting",
            "welcome"
        ]
    );
}

#[test]
fn every_screen_builds_and_fits_the_display_in_both_fonts() {
    for (name, build) in SCREENS {
        for shown in [false, true] {
            for (size, viewport, scale) in VIEWPORTS {
                for (font, theme) in themes() {
                    let metrics =
                        theme.metrics_at(nowayos_display::DeviceScale::whole(scale).unwrap());
                    let tree = laid_out(name, *build, shown, viewport, metrics);
                    let state = if shown {
                        "everything shown"
                    } else {
                        "as built"
                    };
                    assert_fits(
                        &tree,
                        viewport,
                        &metrics,
                        &format!("{name}, {state}, {font} theme, {size}"),
                    );
                }
            }
        }
    }
}

#[test]
fn an_error_line_takes_space_only_while_the_application_shows_it() {
    // The step with text fields: each field's error is a group holding a
    // slot, hidden until the application shows it; shown, it pushes the
    // buttons below it down, and hidden it leaves no gap.
    let (_, build) = SCREENS
        .iter()
        .find(|(name, _)| *name == "step-text")
        .unwrap();
    let (_, viewport, _) = VIEWPORTS[0];
    let metrics = Theme::<Rgb888>::light().metrics();
    let last_top = |tree: &Tree<CAPACITY, TEXT>| {
        let (_, last) = tree.paint_order().last().unwrap();
        (tree.paint_order().count(), last.rect().top())
    };
    let (hidden_nodes, hidden_top) =
        last_top(&laid_out("step-text", *build, false, viewport, metrics));
    let (shown_nodes, shown_top) =
        last_top(&laid_out("step-text", *build, true, viewport, metrics));
    // Two error lines, each a slot: two more nodes showing, and the Next
    // button two lines (24 px each, the 18 px atlas's) and two gaps (8 px)
    // lower.
    assert_eq!(shown_nodes, hidden_nodes + 2);
    assert_eq!(shown_top - hidden_top, 2 * (24 + 8));
}
