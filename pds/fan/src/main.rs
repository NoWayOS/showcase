// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-fan`: NoWayOS's fan PD (`nowayos-fan`), run from a
//! program of Showcase's own - the Pi 5's active cooler, on the device tree's
//! cooling curve. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_fan::FanPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> FanPd {
    nowayos_fan::init()
}
