// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-pump-stand-in`: the showcase's pay at the pump application PD -
//! the PD of `showcase_kiosk_stand_in`, the reference kiosk's
//! application PD library, on the pay at the pump's example
//! (`showcase_pump::EXAMPLE`): the kiosk flow on its form, and a development
//! stand-in for the card reader and the pump controller built in, which
//! approves the pump, fuel and amount asked - so its name ends `-stand-in`,
//! and NoWayOS's grants check refuses it in a product system
//! (`[development_only]`, `*-stand-in.elf`). Its log lines start
//! `SHOWCASE: pump:`.

#![no_std]
#![no_main]

use sel4_microkit::protection_domain;
use showcase_kiosk_stand_in::AppPd;

#[protection_domain]
fn init() -> AppPd {
    showcase_kiosk_stand_in::init(&showcase_pump::EXAMPLE, "SHOWCASE: pump:")
}
