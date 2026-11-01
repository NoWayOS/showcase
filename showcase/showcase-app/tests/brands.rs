// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's brands through NoWayOS's theme tests: every
//! brand, and its high-contrast version, meets WCAG 2's contrast minimums
//! for every pair of colours drawn together - 4.5:1 for text, 3:1 for a
//! control's boundary, a choice control's marks and the focus ring - as
//! specified and after `r5g6b5` (`nowayos_render_eg::contrast`); draws at
//! every device scale the display contract allows; and lays out exactly as
//! NoWayOS's own theme it is built on, so the fit tests of every screen in
//! NoWayOS's two themes cover each brand too. A brand that fails any of
//! these fails the build.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failed check panics"
)]

use nowayos_render_eg::{Rgb888, Theme, contrast};
use showcase_app::brands::{self, Brand};

/// Every theme of every brand, with its name.
fn themes() -> Vec<(&'static str, Theme<Rgb888>)> {
    brands::ALL
        .iter()
        .flat_map(|brand| [brand.look(false), brand.look(true)])
        .collect()
}

/// Every pair of `theme`'s below its minimum, one line each naming it.
fn failures(name: &str, theme: &Theme<Rgb888>) -> Vec<String> {
    contrast::failures(theme)
        .map(|failure| format!("{name}: {failure}"))
        .collect()
}

#[test]
fn every_brand_and_its_high_contrast_version_meet_the_contrast_minimums() {
    let themes = themes();
    assert_eq!(themes.len(), 10, "five brands, each in two versions");
    let found: Vec<String> = themes
        .iter()
        .flat_map(|(name, theme)| failures(name, theme))
        .collect();
    assert!(found.is_empty(), "\n{}", found.join("\n"));
}

/// The check fails a brand whose colour is too pale for white text on its
/// buttons and header bar, and its high-contrast version, naming each pair:
/// such a brand fails the build.
#[test]
fn a_brand_too_pale_fails_naming_each_pair() {
    let pale = Brand {
        name: "pale",
        high_contrast_name: "pale, high contrast",
        accent: Rgb888::new(0xF0, 0xC0, 0x40),
        pressed: Rgb888::new(0xE0, 0xB0, 0x30),
    };
    let light = failures(pale.name, &pale.theme());
    for expected in [
        "pale: a primary button's label: ",
        "pale: a primary button's label, after r5g6b5: ",
        "pale: body text, on Header: ",
        "pale: the focus ring: ",
    ] {
        assert!(
            light.iter().any(|f| f.starts_with(expected)),
            "{expected}\n{light:#?}"
        );
    }
    let large = failures(pale.high_contrast_name, &pale.high_contrast());
    assert!(
        large
            .iter()
            .any(|f| f.starts_with("pale, high contrast: body text, on Header: ")),
        "{large:#?}"
    );
}

/// Each brand's theme lays out as NoWayOS's light theme and its
/// high-contrast version as NoWayOS's high-contrast one, at every scale -
/// the same type scale, target size and button height - and draws at each.
#[test]
fn each_brand_lays_out_as_the_theme_it_is_built_on_at_every_scale() {
    let light = Theme::<Rgb888>::light();
    let large = Theme::<Rgb888>::high_contrast();
    for brand in brands::ALL {
        for scale in 1..=nowayos_display::MAX_SCALE {
            assert!(brand.theme().draws_at(scale), "{}", brand.name);
            assert!(brand.high_contrast().draws_at(scale), "{}", brand.name);
            assert_eq!(
                brand.theme().metrics_at(scale),
                light.metrics_at(scale),
                "{} at {scale}x",
                brand.name
            );
            assert_eq!(
                brand.high_contrast().metrics_at(scale),
                large.metrics_at(scale),
                "{} at {scale}x",
                brand.high_contrast_name
            );
        }
    }
}

/// Every brand has its own name and its own colour, and each version its
/// own name, which the UI PD tells one theme from another by.
#[test]
fn every_brand_has_its_own_name_and_colour() {
    let names: Vec<&str> = themes().iter().map(|(name, _)| *name).collect();
    let accents: Vec<Rgb888> = brands::ALL.iter().map(|brand| brand.accent).collect();
    for (at, name) in names.iter().enumerate() {
        assert!(!names[..at].contains(name), "{name} twice");
    }
    for (at, accent) in accents.iter().enumerate() {
        assert!(!accents[..at].contains(accent), "{accent:?} twice");
    }
    assert_eq!(brands::ALL[0], brands::LAUNCHER);
}
