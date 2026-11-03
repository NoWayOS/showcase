// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-display-ramfb`: in QEMU only, the display PD - NoWayOS's
//! (`nowayos-display-ramfb`), run from a program of Showcase's own: it shows
//! each frame the UI presents on QEMU's ramfb, so a script's screendumps can
//! be matched with the approved images (`systems/showcase-gallery`). Quiet
//! (`init_quiet`), as the showcase's own display is: it prints no frame's
//! hash. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_display_ramfb::DisplayPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> DisplayPd {
    nowayos_display_ramfb::init_quiet()
}
