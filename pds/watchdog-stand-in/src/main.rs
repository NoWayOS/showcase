// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-watchdog-stand-in`: NoWayOS's watchdog stand-in
//! (`nowayos-watchdog-stand-in`), run as the ticker of a Pi 5 test system
//! from a program of Showcase's own: a tick to the watchdog every
//! 2 s for the run length it reads, then none, so the board resets.
//! A test instrument; as a stand-in it is development-only (NoWayOS's
//! grant-rules.toml, `*-stand-in.elf`). Nothing of its own.

#![no_std]
#![no_main]

use nowayos_watchdog_stand_in::StandInPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> StandInPd {
    nowayos_watchdog_stand_in::init()
}
