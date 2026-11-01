// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-uart-input`: a development route only - NoWayOS's UART input PD
//! (`nowayos-uart-input`), run from a program of Showcase's own: keys
//! typed on a terminal attached to the board's debug UART, as canonical key
//! events for the input broker. Its name keeps the `-uart-input` ending, so
//! the grants check's development-only rule binds it: no product system may
//! run it. Nothing of its own.

#![no_std]
#![no_main]

use nowayos_uart_input::UartInputPd;
use sel4_microkit::protection_domain;

#[protection_domain]
fn init() -> UartInputPd {
    nowayos_uart_input::init()
}
