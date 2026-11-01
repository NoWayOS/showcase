// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-timer`: the timer PD - NoWayOS's (`nowayos-timer`), run from a
//! program of Showcase's own. It alone holds the timer's interrupt, and wakes
//! the UART input PD for its polls. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_timer::TimerPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> TimerPd {
    nowayos_timer::init()
}
