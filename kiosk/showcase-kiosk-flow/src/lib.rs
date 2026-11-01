// SPDX-License-Identifier: MIT OR Apache-2.0
//! The state machine behind a multi-step form on a shared public kiosk: a
//! person works through a few screens of questions, checks their answers
//! and submits them, then walks away, and the next person must find no
//! trace of them.
//!
//! The application defines the form, its [`Step`]s and each step's
//! [`Field`]s, as static data. [`KioskFlow`] tracks where the person is,
//! holds their answers in bounded storage, and enforces the rules a shared
//! public device needs:
//!
//! * **Navigation.** [`start`](KioskFlow::start), [`next`](KioskFlow::next)
//!   (only from a complete step), [`back`](KioskFlow::back) and
//!   [`cancel`](KioskFlow::cancel), then a review of every answer before
//!   [`submit`](KioskFlow::submit).
//! * **Bounded, checked answers.** Text is checked against its field's
//!   [`TextRule`] as it is entered (one line of printable Latin-1 at most,
//!   for now: see [`Charset::Text`]) and stored in a fixed-size
//!   [`nowayos_bounded::BoundedStr`]; choices and checkboxes are checked
//!   against the form.
//! * **Privacy on a shared device.** When nobody has touched the kiosk for
//!   the idle timeout, the session is wiped: every answer is cleared and the
//!   flow returns to [`Phase::Welcome`], so the next person never sees the
//!   last person's answers. A warning can come first. A session is also
//!   wiped when it reaches its maximum length, however active the person
//!   is, so stuck input cannot keep it alive. Answers are also cleared on
//!   cancel and as soon as a submission succeeds.
//! * **Submission.** [`Phase::Submitting`], [`Phase::Submitted`] and
//!   [`Phase::Failed`], with a submit timeout and a bounded number of
//!   retries. Each attempt has a [`SubmissionTicket`], so a late result for
//!   an earlier attempt, for an earlier person's session, or from before
//!   the application restarted, is ignored rather than applied to the wrong
//!   one. Answers the service rejected are not sent again until one
//!   changes.
//!
//! The crate is `no_std`, allocates nothing, contains no `unsafe` and never
//! panics on its inputs. It has no clock: the caller passes in the current
//! [`Tick`], and [`KioskFlow::next_deadline`] says when to call
//! [`KioskFlow::tick`] next.
//!
//! The API is a **first cut and provisional**. Nothing here is specific to
//! any customer or sector: labels, wording and what happens to a submission
//! belong to the application built on top.
//!
//! # Where it runs
//!
//! On a NoWayOS device this state machine belongs in the application
//! protection domain, which keeps an application's state and rules apart
//! from its UI and from any device. The UI
//! draws the screen for [`KioskFlow::phase`] and reports what the person
//! did; the flow holds no credentials and talks to no device or network.
//!
//! # Example
//!
//! ```
//! use showcase_kiosk_flow::{
//!     Charset, Field, FieldId, FlowConfig, Form, KioskFlow, Phase, Step, StepId, TextRule,
//!     TickEvent, Timeouts,
//! };
//!
//! const NAME: FieldId = FieldId(1);
//! const AGREE: FieldId = FieldId(2);
//! static FORM: Form<'static> = Form::new(&[
//!     Step::new(StepId(1), &[Field::text(NAME, TextRule::new(Charset::Text, 1, 32)).required()]),
//!     Step::new(StepId(2), &[Field::checkbox(AGREE).required()]),
//! ]);
//! // Ticks are milliseconds here.
//! const CONFIG: FlowConfig = FlowConfig {
//!     timeouts: Timeouts {
//!         idle_reset: 60_000,
//!         idle_warning: 15_000,
//!         submit: 10_000,
//!         confirmation: 8_000,
//!         max_session: 20 * 60_000,
//!     },
//!     max_attempts: 3,
//! };
//! // Different every time the application starts (every boot, under
//! // NoWayOS's fault policy): a random number from the security PD, say.
//! // Submission tickets carry it.
//! let epoch = 1;
//!
//! // Room for up to 4 fields of up to 32 bytes each.
//! let mut flow: KioskFlow<'_, 4, 32> = KioskFlow::new(&FORM, CONFIG, epoch).unwrap();
//! flow.start(0).unwrap();
//! flow.set_text(NAME, "Sam Sample", 1_000).unwrap();
//! flow.next(2_000).unwrap();
//! assert_eq!(flow.phase(), Phase::Step { index: 1 });
//!
//! // The person walks away. Fifteen seconds before the reset the UI is told
//! // to ask whether anyone is still there...
//! assert_eq!(flow.next_deadline(), Some(47_000));
//! assert_eq!(flow.tick(47_000), Some(TickEvent::IdleWarning { reset_at: 62_000 }));
//! // ...and a minute after the last touch, everything they typed is gone.
//! assert_eq!(flow.tick(62_000), Some(TickEvent::IdleReset));
//! assert_eq!(flow.phase(), Phase::Welcome);
//! assert!(!flow.has_entered_data());
//! ```
//!
//! # What the privacy reset covers, and what it does not
//!
//! * It clears **every answer the flow holds**, on every step, whether or
//!   not the person reached it, **in place**: text storage is zeroed with
//!   [`nowayos_bounded::BoundedStr::clear`], not just forgotten. This
//!   crate's tests prove every byte of that storage is zero after a wipe
//!   ([`nowayos_bounded::BoundedStr::is_zeroed`]); its public API shows only
//!   that every value is empty.
//! * It happens on the idle timeout, when the session reaches its maximum
//!   length ([`Timeouts::max_session`]), when the clock goes backwards (a
//!   fault: the flow fails closed, see [`Tick`]), on [`KioskFlow::cancel`],
//!   as soon as a submission succeeds, when the application ends the
//!   session for a fault ([`KioskFlow::end_session`]), and again at the
//!   start of every session.
//! * Every call, whether an action, a service result or
//!   [`KioskFlow::tick`], first applies whatever fell due before it, as of
//!   the moment it fell due. So a call that arrives after the session has
//!   ended, however late `tick` is, finds it wiped: an action or a result
//!   is refused with [`FlowError::Expired`] instead of being applied to it.
//!   (A cancel simply succeeds: the session ends either way.)
//! * No `Debug` output or error of this crate contains typed text.
//! * It **cannot** clear copies held elsewhere: the UI's text inputs, a
//!   message on its way to the service, a log. Their owners must clear them
//!   whenever the flow ends a session: a [`TickEvent`] for which
//!   [`TickEvent::ends_session`] is true, [`FlowError::Expired`], a cancel,
//!   a successful submission.
//! * Nor can it wipe anything between calls, since it has no clock: answers
//!   stay in memory until the next call after the session ends. Call
//!   [`KioskFlow::tick`] at each [`KioskFlow::next_deadline`] to wipe them
//!   on time.
//! * Zeroing is hygiene, not a guarantee: the compiler may skip stores it can
//!   prove are never read, and moved values may leave copies. Kiosk forms are
//!   not for secrets. Passwords, PINs and card numbers do not belong in one;
//!   credentials belong in the security protection domain.
//!
//! # Time
//!
//! Timeouts are in ticks of the caller's monotonic clock, read at the moment
//! of each call (see [`Tick`]). The idle timeout runs in [`Phase::Step`],
//! [`Phase::Review`] and [`Phase::Failed`], measured from the person's last
//! action or the start of the phase, whichever is later. It does not run
//! while [`Phase::Submitting`], where the submit timeout applies instead, and
//! a failure dates from the submit deadline at the latest. The maximum
//! session length runs from [`KioskFlow::start`] in every phase that holds
//! answers.
//!
//! So, whatever the timing of the calls, no call made a submit timeout plus
//! an idle timeout or more after the person's last action, or
//! [`Timeouts::max_session`] or more after the session started, can reach
//! the answers: it finds them wiped.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// The flow takes input from the UI, which is untrusted: no indexing that can
// panic, no unchecked arithmetic, and no `as` casts that could silently
// truncate or change sign (use `From` / `TryFrom`).
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions
)]

mod error;
mod flow;
mod form;
mod value;

pub use error::{FieldProblem, FlowError, Issue, SetupError};
pub use flow::{
    FailureReason, FlowConfig, KioskFlow, Phase, SubmissionTicket, Tick, TickEvent, Timeouts,
};
pub use form::{Charset, Field, FieldId, FieldKind, Form, Step, StepId, TextRule};
pub use value::Value;
