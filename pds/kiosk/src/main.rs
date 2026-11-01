// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-kiosk-stand-in`: the reference kiosk's application PD,
//! the PD of [`showcase_kiosk_stand_in`] on the reference kiosk's example
//! (`showcase_kiosk_screens::app::REFERENCE`), with a development stand-in
//! for the service built in - so its name ends `-stand-in`, and NoWayOS's
//! grants check refuses it in a product system (`[development_only]`,
//! `*-stand-in.elf`).

#![no_std]
#![no_main]

use sel4_microkit::protection_domain;
use showcase_kiosk_screens::app::REFERENCE;
use showcase_kiosk_stand_in::AppPd;

#[protection_domain]
fn init() -> AppPd {
    showcase_kiosk_stand_in::init(&REFERENCE, "SHOWCASE: app:")
}
