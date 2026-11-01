// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kiosk reference screens: a synthetic, sector-neutral screen
//! for each phase of the kiosk's form flow (`showcase-kiosk-flow`'s `Phase`)
//! and each kind of field, written in TSX for Screen Compiler and importing only
//! from its kit (`tsx/` beside this crate's sources):
//!
//! | Screen | Phase or field kind |
//! | --- | --- |
//! | `welcome` | Welcome |
//! | `step-text` | a step with text fields: free text, and digits |
//! | `step-choice` | a step with a choice field, and optional text |
//! | `step-notice` | a step with no fields: a notice |
//! | `step-checkbox` | a step with checkboxes, one required |
//! | `review` | Review: every answer, from the UI's own copies |
//! | `submitting` | Submitting |
//! | `submitted` | Submitted: the confirmation |
//! | `failed` | Failed: why, the tries left, and retry when it may help |
//! | `idle-warning` | the idle warning before the privacy reset |
//!
//! All ten convert. [`generated`] holds the Rust Screen Compiler writes
//! for each, exactly as written, so the screens build here against NoWayOS's
//! UI core at the revision Showcase records. `gaps.txt` records what
//! Screen Compiler refuses, ranked by how many screens each gap blocks: nothing,
//! since subset v1's conditional rendering, text from props and form
//! controls. [`sink`] tours the ten screens with their own
//! buttons - the kitchen sink a UI protection domain can show. `make screens` runs Screen Compiler (from the screen-compiler clone beside
//! showcase) on every screen and fails if either differs; `make
//! screens-bless` rewrites them after a deliberate change.
//!
//! The reference kiosk puts the screens behind the kiosk flow:
//! [`form`] is the form they ask for, as the application PD's flow holds
//! it; [`app`] is what the application PD does - the flow behind the
//! app-UI contract, failing closed; [`kiosk`] is the UI that shows the
//! screen each report from the application names and sends it each action
//! the person takes, over the contract; and [`stand_in`] answers the
//! application's submissions in place of the service until the network
//! path exists.
//!
//! The screens hold no answers: the UI's code fills in values, errors and
//! application text by field, group and slot number - each screen's
//! `fields`, `groups` and `slots` modules - as the app-UI contract
//! (`showcase-kiosk-contract`) reports them. Every text is invented for
//! NoWayOS.

#![no_std]
#![forbid(unsafe_code)]

pub mod app;
pub mod form;
pub mod generated;
pub mod kiosk;
pub mod sink;
pub mod stand_in;
