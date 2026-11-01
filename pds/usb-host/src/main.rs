// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-usb-host`: the USB host PD - NoWayOS's (`nowayos-usb-host`),
//! run from a program of Showcase's own: one xHCI controller
//! driven - RP1's on the Pi 5 - and its keyboards, mice and absolute pointers
//! sent to the input broker as canonical events. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_usb_host::UsbHost;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> UsbHost {
    nowayos_usb_host::init()
}
