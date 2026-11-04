// SPDX-License-Identifier: MIT OR Apache-2.0
//! The launcher on three monitors - 1920 x 1080, 2560 x 1440 and
//! 3840 x 2160 - each at the device scale NoWayOS's display gives it
//! (`nowayos_display::device_scale`: 2, 2.5 and 4): laid out in layout
//! units, drawn at the monitor's own pixels with its text from the glyph
//! outlines at the size the scale asks, every edge on a whole pixel, and
//! matching its approved image (`tests/snapshots/launcher-WxH.png`;
//! `NOWAYOS_UPDATE_SNAPSHOTS=1` redraws them, for a person to approve).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a screen that cannot be drawn fails the test"
)]

use std::path::PathBuf;

use nowayos_display::{Rect, Size, device_scale};
use nowayos_render_eg::{Rgb888, Theme, render};
use nowayos_testkit::{Framebuffer, check_snapshot};
use showcase_app::Showcase;
use showcase_kiosk_screens::kiosk::KioskScreens;

#[test]
fn the_launcher_is_drawn_crisp_at_1080p_1440p_and_4k() {
    for (width, height, quarters, units) in [
        (1920, 1080, 8, Size::new(960, 540)),
        (2560, 1440, 10, Size::new(1024, 576)),
        (3840, 2160, 16, Size::new(960, 540)),
    ] {
        let pixels = Size::new(width, height);
        let scale = device_scale(pixels);
        assert_eq!(scale.quarters(), quarters, "{width}x{height}");
        assert_eq!(scale.layout_of(pixels), units, "{width}x{height}");
        let light = Theme::<Rgb888>::light();
        let mut showcase: Showcase = Showcase::new(light.metrics()).unwrap();
        showcase
            .resize(Rect::new(0, 0, width, height), light.metrics_at(scale))
            .unwrap();
        assert_eq!(showcase.screen(), "launcher");
        let (_, theme) = showcase.theme();
        let metrics = theme.metrics_at(scale);
        for (_, node) in showcase.tree().paint_order() {
            assert_eq!(
                node.cut_text(&metrics.for_style(node.style().text)),
                None,
                "{width}x{height}: {node:?}"
            );
        }
        let mut frame = Framebuffer::new(width, height).unwrap();
        nowayos_testkit::glyph_store!(glyphs);
        render(showcase.tree(), &mut frame, &theme, &mut glyphs).unwrap();
        let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(format!("launcher-{width}x{height}.png"));
        check_snapshot(&frame, &golden).unwrap();
    }
}
