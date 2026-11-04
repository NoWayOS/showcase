// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-ui`: the showcase's UI PD: the launcher of
//! example kiosks, and the Service desk example - the reference kiosk's UI,
//! a bar with Home above each screen - driven by the kiosk flow in the
//! application PD (`showcase_app::Showcase`). The PD itself is
//! `showcase-kiosk-ui`'s library: the same rings, drawing and failing closed.
//! Home, or a session's end, wipes the example and shows the launcher.

#![no_std]
#![no_main]

use sel4_microkit::{NullHandler, protection_domain};
use showcase_app::Showcase;

#[protection_domain]
fn init() -> NullHandler {
    showcase_kiosk_ui::init::<Showcase>(Showcase::new)
}
