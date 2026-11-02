// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's screens fit: the launcher, and every screen of
//! each example with its bar above it - every kiosk reference screen with
//! the Service desk's, every visitor sign-in screen with the visitor's -
//! as built and with every group shown and slot filled; and the
//! launcher with a program's tile.
//!
//! The showcase draws in the light theme, its body text 18 px. On
//! the Pi 5 it is laid out at the monitor's own pixels: the 1920 x
//! 1080 window at device scale 2, a 960 x 540 screen, its text from the 36
//! px atlas - so each screen must fit there, and at the kiosk's 800 x 480
//! layout size, in that theme, and at 1920 x 1080 in both of NoWayOS's
//! themes, each with its own metrics: type scale, minimum target size and
//! buttons' height. Not checked at 800 x 480 in the large-text
//! theme: the showcase does not offer that theme yet. Fitting means
//! every node that shows lies inside the display, and every fixed text is
//! shown whole.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a screen that fails to build or fit fails the test"
)]

use nowayos_display::Rect;
use nowayos_render_eg::{Rgb888, Theme};
use nowayos_ui::{GroupId, NodeId, NodeKind, SlotId, TextMetrics, Tree, UiError};
use showcase_app::generated::{CAPACITY, SCREENS};
use showcase_app::{pump_frame, service_desk_frame, visitor_frame};
use showcase_kiosk_screens::generated::SCREENS as KIOSK_SCREENS;
use showcase_pump::generated::SCREENS as PUMP_SCREENS;
use showcase_visitor::generated::SCREENS as VISITOR_SCREENS;

/// Where each screen must fit, and in which theme's metrics: its type
/// scale, minimum target size and buttons' height.
fn cases() -> [(&'static str, Rect, TextMetrics); 4] {
    let light = Theme::<Rgb888>::light();
    let large = Theme::<Rgb888>::high_contrast();
    [
        (
            "960 x 540 at 2x, light",
            Rect::new(0, 0, 1920, 1080),
            light.metrics_at(2),
        ),
        (
            "800 x 480, light",
            Rect::new(0, 0, 800, 480),
            light.metrics(),
        ),
        (
            "1920 x 1080, light",
            Rect::new(0, 0, 1920, 1080),
            light.metrics(),
        ),
        (
            "1920 x 1080, large text",
            Rect::new(0, 0, 1920, 1080),
            large.metrics(),
        ),
    ]
}

/// More groups and slots than any screen numbers.
const MOST: u16 = 32;

/// Text the application shows in a slot.
const SHOWN: &str = "Please enter the reference number in digits only";

/// What it shows in a slot in the display role, which draws digits and a
/// few signs only: the widest of the pump's figures.
const SHOWN_DISPLAY: &str = "£888.88";

/// Builds a screen with `build`, shows every group and fills every slot
/// when `shown`, and lays it out.
fn laid_out(
    what: &str,
    build: impl FnOnce(&mut Tree<CAPACITY>) -> Result<NodeId, UiError>,
    shown: bool,
    viewport: Rect,
    metrics: TextMetrics,
) -> Tree<CAPACITY> {
    let mut tree = Tree::<CAPACITY>::new();
    let root = build(&mut tree).unwrap_or_else(|err| panic!("{what}: {err:?}"));
    tree.set_root(root).unwrap();
    if shown {
        for number in 1..=MOST {
            match tree.set_visible(GroupId::new(number), true) {
                Ok(()) | Err(UiError::UnknownGroup(_)) => {}
                Err(err) => panic!("{what}: group {number}: {err:?}"),
            }
            let slot = SlotId::new(number);
            match tree.set_slot(slot, SHOWN) {
                Ok(()) | Err(UiError::UnknownSlot(_)) => {}
                Err(UiError::NoGlyph) => tree.set_slot(slot, SHOWN_DISPLAY).unwrap(),
                Err(err) => panic!("{what}: slot {number}: {err:?}"),
            }
        }
    }
    tree.layout(viewport, &metrics).unwrap();
    tree
}

/// Whether `id` is inside a scroll view, which shows what lies past its
/// edges by scrolling, so it may.
fn in_scroll_view(tree: &Tree<CAPACITY>, id: NodeId) -> bool {
    let mut at = tree.get(id).and_then(|node| node.parent());
    while let Some(parent) = at {
        let node = tree.get(parent).unwrap();
        if matches!(node.kind(), NodeKind::Scroll { .. }) {
            return true;
        }
        at = node.parent();
    }
    false
}

/// Every node that shows lies inside `viewport` - or inside a scroll view,
/// which itself does - and every fixed text - every line of one that
/// wraps - is shown whole, each in its own role and weight
/// (`Node::cut_text`); a slot's text, the application's, only if it wraps.
fn assert_fits(tree: &Tree<CAPACITY>, viewport: Rect, metrics: &TextMetrics, what: &str) {
    for (id, node) in tree.paint_order() {
        if in_scroll_view(tree, id) {
            continue;
        }
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
fn the_launcher_the_bars_and_the_gallery_convert() {
    let names: Vec<&str> = SCREENS.iter().map(|(name, _)| *name).collect();
    let mut expected = vec![
        "attract",
        "launcher",
        "pump-bar",
        "service-desk-bar",
        "visitor-bar",
        "why-secure",
    ];
    expected.extend(showcase_app::GALLERY.iter().map(|(name, _)| *name));
    expected.sort_unstable();
    assert_eq!(names, expected);
}

/// Every screen of the Screen Compiler gallery fits, as built and with
/// its progress bar and error line shown.
#[test]
fn every_gallery_screen_fits() {
    for (name, build) in showcase_app::GALLERY {
        for shown in [false, true] {
            for (case, viewport, metrics) in cases() {
                let what = format!("{name} at {case}, shown {shown}");
                let tree = laid_out(
                    &what,
                    |tree| {
                        let root = build(tree)?;
                        if shown {
                            for number in 1..=MOST {
                                let field = nowayos_ui::FieldId::new(number);
                                match tree.set_field_error(field, "Enter four digits") {
                                    Ok(()) | Err(UiError::UnknownField(_)) => {}
                                    Err(err) => return Err(err),
                                }
                            }
                        }
                        Ok(root)
                    },
                    shown,
                    viewport,
                    metrics,
                );
                assert_fits(&tree, viewport, &metrics, &what);
            }
        }
    }
}

#[test]
fn the_launcher_fits() {
    // And the "Why is this secure?" panel shown in its place.
    for screen in ["launcher", "why-secure"] {
        let (_, build) = SCREENS
            .iter()
            .find(|(name, _)| *name == screen)
            .copied()
            .unwrap();
        for (case, viewport, metrics) in cases() {
            let what = format!("{screen} at {case}");
            let tree = laid_out(&what, build, false, viewport, metrics);
            assert_fits(&tree, viewport, &metrics, &what);
        }
    }
}

/// The launcher fits with a program's tile shown, its brand, title
/// and line as long as the longest of the showcase's own tiles'.
#[test]
fn the_launcher_fits_with_a_program_s_tile() {
    use showcase_app::generated::launcher::launcher_screen::{groups, slots};
    let (_, build) = SCREENS
        .iter()
        .find(|(name, _)| *name == "launcher")
        .copied()
        .unwrap();
    for (case, viewport, metrics) in cases() {
        let what = format!("launcher with a program's tile at {case}");
        let mut tree = Tree::<CAPACITY>::new();
        let root = build(&mut tree).unwrap();
        tree.set_root(root).unwrap();
        tree.set_slot(slots::EXTRA_BRAND, "Larchway Kitchen")
            .unwrap();
        tree.set_slot(slots::EXTRA_TITLE, "Why is this secure?")
            .unwrap();
        tree.set_slot(slots::EXTRA_LINE, "Wiped, then a number")
            .unwrap();
        tree.set_visible(groups::EXTRA, true).unwrap();
        tree.layout(viewport, &metrics).unwrap();
        assert_fits(&tree, viewport, &metrics, &what);
        // And every slot shows its text whole, as the tile's fixed text is.
        for (id, node) in tree.paint_order() {
            if let Some(text) = node.cut_text(&metrics) {
                panic!("{what}: {id:?}'s {text:?} is cut");
            }
        }
    }
}

#[test]
fn every_service_desk_screen_fits_with_its_bar() {
    for (name, build) in KIOSK_SCREENS {
        for shown in [false, true] {
            for (case, viewport, metrics) in cases() {
                let what = format!("{name} with the bar at {case}, shown {shown}");
                let tree = laid_out(
                    &what,
                    |tree| {
                        let body = build(tree)?;
                        service_desk_frame(tree, body, name)
                    },
                    shown,
                    viewport,
                    metrics,
                );
                assert_fits(&tree, viewport, &metrics, &what);
            }
        }
    }
}

#[test]
fn every_visitor_screen_fits_with_its_bar() {
    let names: Vec<&str> = VISITOR_SCREENS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), 11, "{names:?}");
    for (name, build) in VISITOR_SCREENS {
        for shown in [false, true] {
            for (case, viewport, metrics) in cases() {
                let what = format!("{name} with the bar at {case}, shown {shown}");
                let tree = laid_out(
                    &what,
                    |tree| {
                        let body = build(tree)?;
                        visitor_frame(tree, body, name)
                    },
                    shown,
                    viewport,
                    metrics,
                );
                assert_fits(&tree, viewport, &metrics, &what);
            }
        }
    }
}

#[test]
fn every_visitor_screen_says_what_is_simulated() {
    let metrics = Theme::<Rgb888>::light().metrics();
    let viewport = Rect::new(0, 0, 800, 480);
    for (name, build) in VISITOR_SCREENS {
        let tree = laid_out(
            name,
            |tree| {
                let body = build(tree)?;
                visitor_frame(tree, body, name)
            },
            false,
            viewport,
            metrics,
        );
        let says = tree.paint_order().any(|(_, node)| {
            matches!(node.kind(), NodeKind::Text(text) if text.starts_with("Demo:") && text.contains("simulated"))
        });
        assert!(says, "{name} does not say what is simulated");
    }
}

/// Every pay at the pump screen fits with its bar, every group shown
/// and every slot filled - the litres and pounds at their widest - and
/// says what is simulated.
#[test]
fn every_pump_screen_fits_with_its_bar_and_says_what_is_simulated() {
    let names: Vec<&str> = PUMP_SCREENS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), 9, "{names:?}");
    for (name, build) in PUMP_SCREENS {
        for shown in [false, true] {
            for (case, viewport, metrics) in cases() {
                let what = format!("{name} with the bar at {case}, shown {shown}");
                let tree = laid_out(
                    &what,
                    |tree| {
                        let body = build(tree)?;
                        pump_frame(tree, body, name)
                    },
                    shown,
                    viewport,
                    metrics,
                );
                assert_fits(&tree, viewport, &metrics, &what);
                let says = tree.paint_order().any(|(_, node)| {
                    matches!(node.kind(), NodeKind::Text(text) if text.starts_with("Demo:") && text.contains("simulated"))
                });
                assert!(says, "{what} does not say what is simulated");
            }
        }
    }
}

/// Every attract slide fits, its title and line filled in.
#[test]
fn every_attract_slide_fits() {
    use showcase_app::SLIDES;
    use showcase_app::generated::attract::attract_screen::slots;
    let (_, build) = SCREENS
        .iter()
        .find(|(name, _)| *name == "attract")
        .copied()
        .unwrap();
    for (title, line, _) in SLIDES {
        for (case, viewport, metrics) in cases() {
            let what = format!("slide {title:?} at {case}");
            let mut tree = Tree::<CAPACITY>::new();
            let root = build(&mut tree).unwrap();
            tree.set_root(root).unwrap();
            tree.set_slot(slots::TITLE, title).unwrap();
            tree.set_slot(slots::LINE, line).unwrap();
            tree.layout(viewport, &metrics).unwrap();
            assert_fits(&tree, viewport, &metrics, &what);
        }
    }
}
