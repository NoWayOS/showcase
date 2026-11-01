// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-kiosk-ui`: the reference kiosk's UI PD - this crate's
//! library, driving the reference kiosk's UI alone.

#![no_std]
#![no_main]

use sel4_microkit::protection_domain;
use showcase_kiosk_screens::kiosk::KioskUi;
use showcase_kiosk_ui::UiPd;

#[protection_domain]
fn init() -> UiPd<KioskUi> {
    showcase_kiosk_ui::init(KioskUi::new)
}
