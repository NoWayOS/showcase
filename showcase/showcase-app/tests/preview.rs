// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's screens drawn as the Pi draws them - each in its own
//! brand's theme, at the monitor's own pixels: the 1920 x 1080
//! window at device scale 2 - to images a person can look at before
//! the board does: set `SHOWCASE_PREVIEW_DIR` to a folder, and each
//! screen is written there as a PNG, the launcher and each example's
//! welcome again in its brand's high-contrast version. Without it, the
//! screens are only drawn.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a screen that cannot be drawn fails the test"
)]

use nowayos_input::{InputEvent, KeyCode};
use nowayos_render_eg::{Rgb888, Theme, render};
use nowayos_testkit::Framebuffer;
use nowayos_ui::Tree;
use showcase_app::{Showcase, brands, pump_frame};
use showcase_kiosk_screens::kiosk::KioskScreens;
use showcase_pump::generated::{
    CAPACITY, SCREENS as PUMP_SCREENS, amount::amount_step as amount, check::check_screen as check,
    fuel::fuel_step as fuel, fuelling::fuelling_screen as fuelling,
    idle_warning::idle_warning_screen as idle, not_accepted::not_accepted_screen as failed,
    pump::pump_step as pump,
};
use showcase_pump::{Approval, Stage, figures};

/// The Pi 5's window.
const WINDOW: nowayos_display::Rect = nowayos_display::Rect::new(0, 0, 1920, 1080);

fn draw_tree<const N: usize>(tree: &Tree<N>, theme: &Theme<Rgb888>, name: &str) {
    let size = tree.viewport().size;
    let mut fb = Framebuffer::new(size.width, size.height).unwrap();
    render(tree, &mut fb, theme).unwrap();
    if let Ok(dir) = std::env::var("SHOWCASE_PREVIEW_DIR") {
        std::fs::write(format!("{dir}/{name}.png"), fb.to_png().unwrap()).unwrap();
    }
}

#[test]
fn the_launcher_and_each_example_draw() {
    // The "Why is this secure?" panel: Down, Right, Right, Enter.
    // A screen of the launcher's tree, in the launcher's theme.
    let mut showcase = fresh();
    for key in [
        KeyCode::DOWN,
        KeyCode::RIGHT,
        KeyCode::RIGHT,
        KeyCode::ENTER,
    ] {
        showcase.handle(InputEvent::KeyDown(key));
        showcase.handle(InputEvent::KeyUp(key));
    }
    showcase.end_input();
    assert_eq!(showcase.screen(), "why-secure");
    let (shown, theme) = showcase.theme();
    assert_eq!(shown, brands::LAUNCHER.name);
    draw_tree(showcase.tree(), &theme, "why-secure");
    // The attract loop's first slide, in the launcher's theme.
    let mut showcase = fresh();
    assert!(showcase.idle());
    assert_eq!(showcase.screen(), "attract-1");
    let (_, theme) = showcase.theme();
    draw_tree(showcase.tree(), &theme, "attract-1");
    for (tabs, name, brand) in [
        (None, "launcher", brands::LAUNCHER),
        (Some(0), "service-desk-welcome", brands::SERVICE_DESK),
        (Some(1), "visitor-welcome", brands::ORRINVALE),
        (Some(2), "pump-welcome", brands::TARNWICK),
    ] {
        let mut showcase = fresh();
        // Each example's welcome screen: Tab to its tile, Enter.
        if let Some(tabs) = tabs {
            let mut keys = vec![KeyCode::TAB; tabs];
            keys.push(KeyCode::ENTER);
            for key in keys {
                showcase.handle(InputEvent::KeyDown(key));
                showcase.handle(InputEvent::KeyUp(key));
            }
            showcase.end_input();
            assert_eq!(showcase.screen(), "welcome");
        }
        // In the theme the screen asks for: its brand's, as the UI PD
        // draws it.
        let (shown, theme) = showcase.theme();
        assert_eq!(shown, brand.name);
        draw_tree(showcase.tree(), &theme, name);
        // And in the brand's high-contrast version, laid out in its
        // metrics.
        let large = brand.high_contrast();
        showcase.resize(WINDOW, large.metrics_at(2)).unwrap();
        draw_tree(showcase.tree(), &large, &format!("{name}-high-contrast"));
    }
}

/// The showcase at its launcher, as the UI PD lays it out once the display
/// announces the Pi 5's window: the light theme's metrics at scale 2 - its
/// type scale from atlases twice the size, its buttons 56 units
/// high - which every brand's theme shares.
fn fresh() -> Showcase {
    let theme = Theme::<Rgb888>::light();
    let mut showcase = Showcase::new(theme.metrics()).unwrap();
    showcase.resize(WINDOW, theme.metrics_at(2)).unwrap();
    showcase
}

/// Every pay at the pump screen with its bar, as the Pi draws it -
/// in Tarnwick's theme -
/// a choice made on each step, the check filled in, and the fuelling screen
/// at each stage, with the figures it shows for pump 3, unleaded, £40.
#[test]
fn every_pump_screen_draws() {
    let theme = brands::TARNWICK.theme();
    let viewport = nowayos_display::Rect::new(0, 0, 1920, 1080);
    let approval = Approval {
        pump: 3,
        fuel: 0,
        pounds: 40,
    };
    for (name, build) in PUMP_SCREENS {
        let stages: &[Option<Stage>] = if *name == "fuelling" {
            &[
                Some(Stage::Ready),
                Some(Stage::Fuelling(500)),
                Some(Stage::Done),
            ]
        } else {
            &[None]
        };
        for (at, stage) in stages.iter().enumerate() {
            let mut tree = Tree::<CAPACITY>::new();
            let body = build(&mut tree).unwrap();
            let root = pump_frame(&mut tree, body, name).unwrap();
            tree.set_root(root).unwrap();
            match *name {
                "pump" => tree.set_selected(pump::fields::PUMP, Some(2)).unwrap(),
                "fuel" => tree.set_selected(fuel::fields::FUEL, Some(0)).unwrap(),
                "amount" => tree.set_selected(amount::fields::AMOUNT, Some(2)).unwrap(),
                "check" => {
                    tree.set_slot(check::slots::PUMP, "3").unwrap();
                    tree.set_slot(check::slots::FUEL, "Unleaded E10").unwrap();
                    tree.set_slot(check::slots::AMOUNT, "£40.00").unwrap();
                }
                "not-accepted" => tree.set_visible(failed::groups::CAN_RETRY, true).unwrap(),
                "idle-warning" => tree.set_slot(idle::slots::SECONDS_LEFT, "12").unwrap(),
                "fuelling" => {
                    let stage = stage.unwrap();
                    let (litres, cost) = figures(approval, stage);
                    tree.set_slot(fuelling::slots::APPROVED, "£40.00").unwrap();
                    let title = stage.title(Some(3));
                    tree.set_slot(fuelling::slots::STAGE, title.as_str())
                        .unwrap();
                    tree.set_slot(fuelling::slots::COST, cost.as_str()).unwrap();
                    tree.set_slot(fuelling::slots::LITRES, litres.as_str())
                        .unwrap();
                    tree.set_slot(fuelling::slots::SECONDS_LEFT, "18").unwrap();
                    // The bar, as far as the stage has gone.
                    let share = match stage {
                        Stage::Ready => 0,
                        Stage::Fuelling(share) => share,
                        Stage::Done => 1_000,
                    };
                    tree.set_progress(fuelling::slots::FUELLED, share, 1_000)
                        .unwrap();
                }
                _ => {}
            }
            tree.layout(viewport, &theme.metrics_at(2)).unwrap();
            let file = if stages.len() > 1 {
                format!("pump-{name}-{at}")
            } else {
                format!("pump-{name}")
            };
            draw_tree(&tree, &theme, &file);
        }
    }
}
