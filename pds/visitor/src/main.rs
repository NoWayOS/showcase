// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-visitor-stand-in`: the showcase's visitor sign-in application
//! PD - the PD of `showcase_kiosk_stand_in`, the reference kiosk's
//! application PD library, on the visitor sign-in's example
//! (`showcase_visitor::EXAMPLE`): the kiosk flow on its form, and a
//! development stand-in for the building service built in, which issues a
//! pass number - so its name ends `-stand-in`, and NoWayOS's grants check
//! refuses it in a product system (`[development_only]`, `*-stand-in.elf`).
//! Its log lines start `SHOWCASE: visitor:`.

#![no_std]
#![no_main]

use sel4_microkit::protection_domain;
use showcase_kiosk_stand_in::AppPd;

#[protection_domain]
fn init() -> AppPd {
    showcase_kiosk_stand_in::init(&showcase_visitor::EXAMPLE, "SHOWCASE: visitor:")
}
