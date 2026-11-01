// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-display`: the display PD - NoWayOS's (`nowayos-display-stub`),
//! run from a program of Showcase's own: it checks each frame the UI
//! presents and shows it on the board's framebuffer when the system grants
//! one - the Pi 5's HDMI monitor. Quiet (`init_quiet`): it prints no
//! frame's hash, since the screens show what a person types, and one hash a
//! keystroke would let a log give the answer away. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_display_stub::StubPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> StubPd {
    nowayos_display_stub::init_quiet()
}
