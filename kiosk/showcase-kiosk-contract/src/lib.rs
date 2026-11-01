// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kiosk's app-UI contract: the messages between the kiosk's UI
//! protection domain and its application protection domain, which runs the
//! form flow ([`showcase_kiosk_flow`]). Showcase owns this contract.
//!
//! # Two rings, two sets of records
//!
//! | Ring | Written by | Read by | Records |
//! | --- | --- | --- | --- |
//! | UI → app | UI PD | app PD | [`Action`]s: what the person did |
//! | app → UI | app PD | UI PD | [`Report`]s: what to show, and when to wipe |
//!
//! Every record is a fixed-size, plain-data `nowayos-ipc` record
//! ([`nowayos_ipc::Message`]): an 8-byte header naming its kind, layout
//! version ([`VERSION`]) and payload length, then a payload whose layout
//! each record's documentation gives. Each action and each report has a
//! kind of its own ([`ACTION_KINDS`], [`REPORT_KINDS`]); the two ranges do
//! not overlap, so a record on the wrong ring is refused. No record is
//! longer than [`MAX_RECORD_LEN`] bytes: ring slots on both rings must be at
//! least that long.
//!
//! # Each side treats the other as hostile
//!
//! * **Every record is checked on receipt**, after nowayos-ipc's ring has
//!   copied it into private memory: [`decode_action`] in the application,
//!   [`decode_report`] in the UI. A record of a kind the receiver never
//!   gets is [`ContractError::UnexpectedKind`]; a malformed one, a wrong
//!   version or length among them, is [`ContractError::Decode`]. Padding
//!   must be zero and every code known, so each record has one encoding.
//!   Decoding never panics, whatever the bytes.
//! * **The application fails closed.** A record it cannot decode means the
//!   UI is faulty or hostile, so [`AppEnd::receive`] ends the session there
//!   and then, [`EndReason::Fault`], rather than go on with one person's
//!   answers held by a UI it no longer trusts.
//! * **The application decides.** An action is only a request: the form
//!   flow applies its rules, and a refusal comes back as
//!   [`ValueRefused`], [`Incomplete`] or [`Refused`], quoting the action's
//!   `seq`.
//!
//! # Answers cross one way
//!
//! Answers (text typed, options chosen, boxes ticked) travel only from the
//! UI to the application, in [`SetText`], [`Select`] and [`SetChecked`].
//! **No report carries one.** The UI is where the person enters them, so it
//! keeps its own copies, for going back a step and for the review screen,
//! and the application holds the copy that counts. The two stay the same
//! because a value the form refuses is cleared on both sides: the
//! application clears the field, and the UI, on [`ValueRefused`], drops its
//! copy if that action was its last for the field. Sending answers back to
//! the UI would put them on a second path for no gain: the UI already has
//! them.
//!
//! # The end of a session
//!
//! Whenever the flow ends a session (the person cancels, nobody touches the
//! kiosk for the idle timeout, the session reaches its maximum length, the
//! service accepts the answers, or the application ends it for a fault)
//! the application sends **exactly one [`SessionEnded`]**: the session's
//! number and the [`EndReason`], and nothing about the answers. It comes
//! before the screen that follows. The UI must then clear, in place, every
//! value it holds for that session: its text inputs and its copies of the
//! answers (for the UI tree, NoWayOS's `Navigator::reset`). [`AppEnd`]
//! tracks whether a session is open, so it reports each end once, however
//! the session ended.
//!
//! # No entered text in anything formatted
//!
//! `Debug` output of a [`SetText`] or an [`Action`] shows the text's length,
//! never the text; errors name fields and codes, never contents; and the
//! reports refusing a value carry only an [`Issue`](showcase_kiosk_flow::Issue)
//! code. So logging a record, an error or a reply cannot leak what a person
//! typed. Dropping a [`SetText`] zeroes its copy of the text (hygiene, not a
//! guarantee: see [`nowayos_bounded::Wipe`]).
//!
//! # Records
//!
//! Actions (UI → app), each carrying the UI's `seq`:
//!
//! | Kind | Record | Payload |
//! | --- | --- | --- |
//! | `0x4B01` | [`Start`] | `seq` |
//! | `0x4B02` | [`SetText`] | `seq`, field, length, 64 bytes of text and zeros |
//! | `0x4B03` | [`Select`] | `seq`, field, option |
//! | `0x4B04` | [`SetChecked`] | `seq`, field, checked |
//! | `0x4B05` | [`ClearField`] | `seq`, field |
//! | `0x4B06` | [`Next`] | `seq` |
//! | `0x4B07` | [`Back`] | `seq` |
//! | `0x4B08` | [`Submit`] | `seq` |
//! | `0x4B09` | [`Retry`] | `seq` |
//! | `0x4B0A` | [`Cancel`] | `seq` |
//! | `0x4B0B` | [`Finish`] | `seq` |
//! | `0x4B0C` | [`StillHere`] | `seq` |
//! | `0x4B0D` | [`Abandon`] | `seq` (the UI fails closed) |
//! | `0x4B0E` | [`SimulateFailure`] | `seq` (a demonstration's *service down*) |
//!
//! Reports (app → UI):
//!
//! | Kind | Record | Payload |
//! | --- | --- | --- |
//! | `0x4B81` | [`ShowWelcome`] | none |
//! | `0x4B82` | [`ShowStep`] | step index |
//! | `0x4B83` | [`ShowReview`] | none |
//! | `0x4B84` | [`ShowSubmitting`] | attempt |
//! | `0x4B85` | [`ShowSubmitted`] | none |
//! | `0x4B86` | [`ShowFailed`] | reason, attempt, attempts left |
//! | `0x4B87` | [`IdleWarning`] | ticks remaining |
//! | `0x4B88` | [`ValueRefused`] | `seq`, field, issue |
//! | `0x4B89` | [`Incomplete`] | `seq`, field, issue |
//! | `0x4B8A` | [`Refused`] | `seq`, refusal |
//! | `0x4B8B` | [`SessionEnded`] | session, reason |
//! | `0x4B8C` | [`Countdown`] | ticks remaining (once a second, while a screen counts down) |
//! | `0x4B8D` | [`Issued`] | the code the service issued: a pass or queue number |
//! | `0x4B8E` | [`OutOfService`] | why: awaiting approval (with the approval code), no service, or updating |
//!
//! # The App's start and its network records
//!
//! [`AppStart`] wraps [`AppEnd`]: at each start the App checks in
//! with the backend, and opens no session until an answer gives it the
//! epoch it makes its flow with. Its
//! records with the network path ([`ToNetwork`], [`FromNetwork`]) are kinds
//! of their own:
//!
//! | Kind | Record | Payload |
//! | --- | --- | --- |
//! | `0x4B41` | [`CheckIn`] (app → network) | `seq` |
//! | `0x4BC1` | [`CheckedIn`] (network → app) | `seq`, approved with the epoch, not approved, or failed |
//! | `0x4BC2` | [`SubmissionResult`] (network → app) | the ticket; accepted (with what was issued), rejected, unavailable or not approved |
//!
//! # Status
//!
//! Version 1, a first cut, provisional: the reference kiosk's UI PD uses
//! it, which added [`Abandon`] - the UI failing closed - to the
//! actions; later versions added [`Countdown`], [`Issued`] and
//! [`OutOfService`] to the reports, each a kind of its own. Not yet covered: the
//! restart epoch of a peer that restarts, which
//! NoWayOS's fault policy does not need while the whole device restarts
//! together; and clearing the ring's own slots, which keep a [`SetText`]
//! record's bytes until a later record overwrites them.
//!
//! The crate is `no_std`, allocates nothing and contains no `unsafe`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// Records come from another protection domain, which is not trusted: no
// indexing that can panic, no unchecked arithmetic, and no `as` casts that
// could silently truncate or change sign (use `From` / `TryFrom`).
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod action;
mod app;
mod error;
mod network;
#[cfg(kani)]
mod proofs;
mod report;
mod start;

pub use action::{
    ABANDON_KIND, ACTION_KINDS, Abandon, Action, BACK_KIND, Back, CANCEL_KIND, CLEAR_FIELD_KIND,
    Cancel, ClearField, FINISH_KIND, Finish, MAX_TEXT_LEN, NEXT_KIND, Next, RETRY_KIND, Retry,
    SELECT_KIND, SET_CHECKED_KIND, SET_TEXT_KIND, SIMULATE_FAILURE_KIND, START_KIND,
    STILL_HERE_KIND, SUBMIT_KIND, Select, SetChecked, SetText, SimulateFailure, Start, StillHere,
    Submit, decode_action,
};
pub use app::{AppEnd, Faulted, MAX_REPLIES, Replies};
pub use error::{ContractError, SetupError};
pub use network::{
    CHECK_IN_KIND, CHECKED_IN_KIND, CheckIn, CheckInAnswer, CheckedIn, FROM_NETWORK_KINDS,
    FromNetwork, SUBMISSION_RESULT_KIND, SubmissionOutcome, SubmissionResult, TO_NETWORK_KINDS,
    ToNetwork, decode_from_network, decode_to_network,
};
pub use report::{
    APPROVAL_CODE_LEN, COUNTDOWN_KIND, Countdown, EndReason, IDLE_WARNING_KIND, INCOMPLETE_KIND,
    ISSUED_KIND, ISSUED_LEN, IdleWarning, Incomplete, Issued, OUT_OF_SERVICE_KIND, OutOfService,
    OutOfServiceReason, REFUSED_KIND, REPORT_KINDS, Refusal, Refused, Report, SESSION_ENDED_KIND,
    SHOW_FAILED_KIND, SHOW_REVIEW_KIND, SHOW_STEP_KIND, SHOW_SUBMITTED_KIND, SHOW_SUBMITTING_KIND,
    SHOW_WELCOME_KIND, SessionEnded, ShowFailed, ShowReview, ShowStep, ShowSubmitted,
    ShowSubmitting, ShowWelcome, VALUE_REFUSED_KIND, ValueRefused, decode_report,
};
pub use start::{AppStart, CheckInTimes, StartError, StartSetupError};

/// The layout version of every record in this contract. A record whose
/// layout changes gets a new version of its own; a receiver refuses any
/// version it does not read.
pub const VERSION: u16 = 1;

/// The longest record of the contract, a [`SetText`]: ring slots on both
/// rings must be at least this long.
pub const MAX_RECORD_LEN: usize = nowayos_ipc::encoded_len::<SetText>();
