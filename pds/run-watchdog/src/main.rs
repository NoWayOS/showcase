// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-run-watchdog`: NoWayOS's watchdog PD (`nowayos-watchdog`), run
//! from a program of Showcase's own in a Pi 5 test system. It
//! watches no PD and keeps BCM2712's PM watchdog armed while the ticker
//! ticks, so the run lasts the system's run length, then the board resets
//! into U-Boot's prompt. Never in a demo or product system: their
//! watchdog would watch every PD. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_watchdog::WatchdogPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> WatchdogPd {
    nowayos_watchdog::init_pm_ticks_only(0, 0)
}
