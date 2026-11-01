// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-input-broker`: the input broker PD - NoWayOS's
//! (`nowayos-input-broker`), run from a program of Showcase's own:
//! every input source's events checked against the screen and held to the
//! source's rate before the UI sees them. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_input_broker::BrokerPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> BrokerPd {
    nowayos_input_broker::init()
}
