// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's brands:
//! the launcher and each example kiosk drawn in its own fictional brand's
//! colour, each with a high-contrast version. Every brand is fictional.
//!
//! A brand is NoWayOS's light theme with its colour where the approved look
//! puts one - primary buttons, secondary and quiet buttons' outlines and
//! labels, the header bar and the focus ring - and its high-contrast
//! version NoWayOS's large-text, high-contrast theme with the brand's
//! header bar. Both go through NoWayOS's contrast check, after `r5g6b5`, in
//! this crate's tests (`tests/brands.rs`): a brand that would ship
//! unreadable fails the build.

use nowayos_render_eg::{Rgb888, SurfaceColours, Surfaces, Theme};

/// One brand: its name, as the UI PD logs the theme it draws in, and its
/// colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brand {
    /// The brand's theme, by name.
    pub name: &'static str,
    /// Its high-contrast version, by name.
    pub high_contrast_name: &'static str,
    /// The brand's colour.
    pub accent: Rgb888,
    /// A primary button's fill while pressed: the brand's colour, darker.
    pub pressed: Rgb888,
}

/// White: the text on a brand's header bar.
const WHITE: Rgb888 = Rgb888::new(0xFF, 0xFF, 0xFF);

impl Brand {
    /// Its header bar: filled with the brand's colour, its text white.
    const fn header(&self) -> SurfaceColours<Rgb888> {
        SurfaceColours {
            fill: self.accent,
            border: self.accent,
            text: WHITE,
        }
    }

    /// The brand's theme: NoWayOS's light theme in the brand's colour.
    #[must_use]
    pub fn theme(&self) -> Theme<Rgb888> {
        let light = Theme::light();
        Theme {
            button: self.accent,
            button_pressed: self.pressed,
            focus: self.accent,
            surfaces: Surfaces {
                header: self.header(),
                ..light.surfaces
            },
            ..light
        }
    }

    /// Its high-contrast version: NoWayOS's large-text, high-contrast theme,
    /// with the brand's header bar.
    #[must_use]
    pub fn high_contrast(&self) -> Theme<Rgb888> {
        let large = Theme::high_contrast();
        Theme {
            surfaces: Surfaces {
                header: self.header(),
                ..large.surfaces
            },
            ..large
        }
    }

    /// The theme to draw in, with its name: the high-contrast version if
    /// `larger`, else the brand's own.
    #[must_use]
    pub fn look(&self, larger: bool) -> (&'static str, Theme<Rgb888>) {
        if larger {
            (self.high_contrast_name, self.high_contrast())
        } else {
            (self.name, self.theme())
        }
    }
}

/// The launcher's: a dark navy.
pub const LAUNCHER: Brand = Brand {
    name: "launcher",
    high_contrast_name: "launcher, high contrast",
    accent: Rgb888::new(0x24, 0x32, 0x4A),
    pressed: Rgb888::new(0x1A, 0x24, 0x36),
};

/// The Service desk's, the reference kiosk: blue.
pub const SERVICE_DESK: Brand = Brand {
    name: "Service desk",
    high_contrast_name: "Service desk, high contrast",
    accent: Rgb888::new(0x1F, 0x5F, 0xBF),
    pressed: Rgb888::new(0x15, 0x42, 0x86),
};

/// Orrinvale's, the visitor sign-in: green.
pub const ORRINVALE: Brand = Brand {
    name: "Orrinvale",
    high_contrast_name: "Orrinvale, high contrast",
    accent: Rgb888::new(0x1E, 0x6B, 0x52),
    pressed: Rgb888::new(0x16, 0x4F, 0x3D),
};

/// Tarnwick's, pay at the pump: amber - `#A84D08`, a little darker than the mockup's
/// `#B45309`, whose quiet buttons' labels on a notice and pressed Home on
/// its header bar fell below 4.5:1.
pub const TARNWICK: Brand = Brand {
    name: "Tarnwick",
    high_contrast_name: "Tarnwick, high contrast",
    accent: Rgb888::new(0xA8, 0x4D, 0x08),
    pressed: Rgb888::new(0x7E, 0x3A, 0x06),
};

/// Larchway's, ordering food: berry.
pub const LARCHWAY: Brand = Brand {
    name: "Larchway",
    high_contrast_name: "Larchway, high contrast",
    accent: Rgb888::new(0x9D, 0x17, 0x4D),
    pressed: Rgb888::new(0x75, 0x11, 0x3A),
};

/// Every brand of the showcase's own, the launcher's first; a program's
/// example brings its own.
pub const ALL: [Brand; 5] = [LAUNCHER, SERVICE_DESK, ORRINVALE, TARNWICK, LARCHWAY];
