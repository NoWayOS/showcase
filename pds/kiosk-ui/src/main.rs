// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-kiosk-ui`: the reference kiosk's UI PD - this crate's
//! library, driving the reference kiosk's UI alone.

#![no_std]
#![no_main]

use sel4_microkit::{NullHandler, protection_domain};
use showcase_kiosk_screens::kiosk::KioskUi;

#[protection_domain]
fn init() -> NullHandler {
    showcase_kiosk_ui::init::<KioskUi>(KioskUi::new)
}
