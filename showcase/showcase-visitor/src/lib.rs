// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's visitor sign-in: a visitor book that forgets
//! you, at a fictional research centre's reception (Orrinvale).
//!
//! A paper visitor book shows everyone who signed in before you; this one
//! keeps nothing. The visitor gives their name, their organisation if they
//! have one, who they are visiting - free text: the kiosk holds no list of
//! staff - and why, and agrees to the site's rules; the building service
//! takes the sign-in and issues a pass number. The pass screen shows the
//! number and never the name: every answer is wiped, in both PDs, before it
//! appears, and it clears itself after about 10 s, counting down. Walk away
//! mid-form and the countdown runs, then the next person finds nothing.
//!
//! | Screen | Phase or step |
//! | --- | --- |
//! | `welcome` | Welcome: *Sign in as a visitor*, or *Signing out* |
//! | `about-you` | step 0: [`NAME`] (required), [`ORGANISATION`] (optional) |
//! | `visiting` | step 1: [`HOST`] (required) |
//! | `purpose` | step 2: [`PURPOSE`] (required, one of [`PURPOSES`]) |
//! | `site-rules` | step 3: [`RULES`] (required, ticked) |
//! | `check` | Review: every answer; *Sign in*, or *Demo: service down* |
//! | `signing-in` | Submitting |
//! | `not-signed-in` | Failed: "Reception does not know you are here yet" |
//! | `signed-in` | Submitted: the pass number, and the seconds left |
//! | `idle-warning` | the idle warning before the privacy reset |
//! | `signing-out` | fixed text, outside any session: hand the pass back |
//!
//! **Simulated, and says so**: the building service, reception and pass
//! printing. Its stand-in, in the example's own application PD
//! (`showcase-visitor-stand-in`), accepts each sign-in a second after it is
//! sent and issues the next pass number - never one made from the answers -
//! unless the visitor pressed *Demo: service down*, when every attempt of
//! that session fails: the screen says reception does not know they are
//! here yet, and sends them to the desk.
//!
//! **Not access control.** No door opens, nothing is a fire roll, no staff
//! badge and no evacuation screen.
//!
//! [`generated`] holds the Rust Screen Compiler writes for `tsx/`, exactly as
//! written (`make screens-bless`); [`Visitor`] tells the kiosk's form UI
//! (`showcase_kiosk_screens::kiosk::FormUi`) how to show them; [`EXAMPLE`]
//! is what the application PD runs. Every brand and text is invented.

#![no_std]
#![forbid(unsafe_code)]

pub mod generated;

use nowayos_ui::{ActionId, NodeId, ScreenId, SlotId, Tree, UiError};
use showcase_kiosk_contract::ShowFailed;
use showcase_kiosk_flow::{
    Charset, Field, FieldId, FlowConfig, Form, Issue, Step, StepId, TextRule, Timeouts,
};
use showcase_kiosk_screens::app::Example;
use showcase_kiosk_screens::kiosk::{
    Control, ControlKind, Copies, ErrorLine, FormUi, Request, Screens,
};
use showcase_kiosk_screens::stand_in::Policy;

use generated::{
    CAPACITY, about_you::about_you_step as about_you, check::check_screen as check,
    idle_warning::idle_warning_screen as idle, not_signed_in::not_signed_in_screen as failed,
    purpose::purpose_step as purpose, signed_in::signed_in_screen as signed_in,
    signing_out::signing_out_screen as signing_out, site_rules::site_rules_step as rules,
    visiting::visiting_step as visiting, welcome::welcome_screen as welcome,
};

const _: () = assert!(CAPACITY == showcase_kiosk_screens::generated::CAPACITY);

/// The visitor's name: required, one line of text, at most 32 bytes.
pub const NAME: FieldId = FieldId(1);
/// Their organisation: optional, at most 32 bytes.
pub const ORGANISATION: FieldId = FieldId(2);
/// Who they are visiting: required, free text, at most 32 bytes.
pub const HOST: FieldId = FieldId(3);
/// Why they are visiting: required, one of [`PURPOSES`].
pub const PURPOSE: FieldId = FieldId(4);
/// They will follow the site rules: required, ticked.
pub const RULES: FieldId = FieldId(5);

/// The reasons [`PURPOSE`] offers, as `purpose` shows them.
pub const PURPOSES: [&str; 4] = ["A meeting", "An interview", "A delivery", "Work on site"];

/// The visitor sign-in's form.
pub static FORM: Form<'static> = Form::new(&[
    Step::new(
        StepId(1),
        &[
            Field::text(NAME, TextRule::new(Charset::Text, 1, 32)).required(),
            Field::text(ORGANISATION, TextRule::new(Charset::Text, 1, 32)),
        ],
    ),
    Step::new(
        StepId(2),
        &[Field::text(HOST, TextRule::new(Charset::Text, 1, 32)).required()],
    ),
    Step::new(StepId(3), &[Field::choice(PURPOSE, 4).required()]),
    Step::new(StepId(4), &[Field::checkbox(RULES).required()]),
]);

/// How long it waits, in milliseconds (shorter in QEMU's test images,
/// [`Timeouts::or_test`]): the reference kiosk's - the idle
/// warning after 15 s with no input, the reset 15 s later, a sign-in's
/// timeout after 5 s, the pass for 10 s, a session at most 20 minutes, and
/// three tries.
pub const CONFIG: FlowConfig = FlowConfig {
    timeouts: Timeouts {
        idle_reset: 30_000,
        idle_warning: 15_000,
        submit: 5_000,
        confirmation: 10_000,
        max_session: 20 * 60_000,
    }
    .or_test(),
    max_attempts: 3,
};

/// What the visitor sign-in's application PD runs: its form and timeouts,
/// and a stand-in for the building service that issues four-digit pass
/// numbers, from 4821.
pub const EXAMPLE: Example = Example {
    form: &FORM,
    config: CONFIG,
    policy: Policy::Issues {
        prefix: "",
        digits: 4,
        first: 4_821,
    },
};

/// The visitor sign-in's screens; see the [crate documentation](crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visitor;

/// The visitor sign-in's UI.
pub type VisitorUi = FormUi<Visitor>;

const ABOUT_YOU: ScreenId = ScreenId(1);
const VISITING: ScreenId = ScreenId(2);
const PURPOSE_STEP: ScreenId = ScreenId(3);
const RULES_STEP: ScreenId = ScreenId(4);
/// The fixed text of *Signing out*.
const SIGNING_OUT: ScreenId = ScreenId(10);

/// Each screen's file name, by screen id: what the UI PD logs.
const NAMES: [&str; 11] = [
    "welcome",
    "about-you",
    "visiting",
    "purpose",
    "site-rules",
    "check",
    "signing-in",
    "not-signed-in",
    "signed-in",
    "idle-warning",
    "signing-out",
];

impl Screens for Visitor {
    const WELCOME: ScreenId = ScreenId(0);
    const REVIEW: ScreenId = ScreenId(5);
    const SUBMITTING: ScreenId = ScreenId(6);
    const FAILED: ScreenId = ScreenId(7);
    const SUBMITTED: ScreenId = ScreenId(8);
    const IDLE: ScreenId = ScreenId(9);
    const STEPS: &'static [ScreenId] = &[ABOUT_YOU, VISITING, PURPOSE_STEP, RULES_STEP];
    const CONTROLS: &'static [Control] = &[
        Control {
            field: NAME,
            screen: ABOUT_YOU,
            control: about_you::fields::NAME,
            kind: ControlKind::Text,
        },
        Control {
            field: ORGANISATION,
            screen: ABOUT_YOU,
            control: about_you::fields::ORGANISATION,
            kind: ControlKind::Text,
        },
        Control {
            field: HOST,
            screen: VISITING,
            control: visiting::fields::HOST,
            kind: ControlKind::Text,
        },
        Control {
            field: PURPOSE,
            screen: PURPOSE_STEP,
            control: purpose::fields::PURPOSE,
            kind: ControlKind::Choice,
        },
        Control {
            field: RULES,
            screen: RULES_STEP,
            control: rules::fields::RULES,
            kind: ControlKind::Checkbox,
        },
    ];
    const ERROR_LINES: &'static [ErrorLine] = &[
        ErrorLine {
            field: NAME,
            screen: ABOUT_YOU,
            group: about_you::groups::NAME_ERROR,
            slot: about_you::slots::NAME_ERROR,
        },
        ErrorLine {
            field: ORGANISATION,
            screen: ABOUT_YOU,
            group: about_you::groups::ORGANISATION_ERROR,
            slot: about_you::slots::ORGANISATION_ERROR,
        },
        ErrorLine {
            field: HOST,
            screen: VISITING,
            group: visiting::groups::HOST_ERROR,
            slot: visiting::slots::HOST_ERROR,
        },
        ErrorLine {
            field: PURPOSE,
            screen: PURPOSE_STEP,
            group: purpose::groups::PURPOSE_ERROR,
            slot: purpose::slots::PURPOSE_ERROR,
        },
        ErrorLine {
            field: RULES,
            screen: RULES_STEP,
            group: rules::groups::RULES_ERROR,
            slot: rules::slots::RULES_ERROR,
        },
    ];
    const STILL_HERE: ActionId = idle::actions::STILL_HERE;

    fn name(screen: ScreenId) -> &'static str {
        NAMES
            .get(usize::from(screen.0))
            .copied()
            .unwrap_or("welcome")
    }

    fn build(screen: ScreenId, tree: &mut Tree<CAPACITY>) -> Result<NodeId, UiError> {
        match screen {
            Self::WELCOME => generated::build_welcome(tree),
            ABOUT_YOU => generated::build_about_you(tree),
            VISITING => generated::build_visiting(tree),
            PURPOSE_STEP => generated::build_purpose(tree),
            RULES_STEP => generated::build_site_rules(tree),
            Self::REVIEW => generated::build_check(tree),
            Self::SUBMITTING => generated::build_signing_in(tree),
            Self::FAILED => generated::build_not_signed_in(tree),
            Self::SUBMITTED => generated::build_signed_in(tree),
            SIGNING_OUT => generated::build_signing_out(tree),
            _ => generated::build_idle_warning(tree),
        }
    }

    fn button(screen: ScreenId, action: ActionId) -> Option<(Request, &'static str)> {
        Some(match (screen, action) {
            (Self::WELCOME, welcome::actions::START) => (Request::Start, "Sign in as a visitor"),
            (Self::WELCOME, welcome::actions::SIGNING_OUT) => {
                (Request::Show(SIGNING_OUT), "Signing out")
            }
            (SIGNING_OUT, signing_out::actions::BACK) => (Request::Show(Self::WELCOME), "Back"),
            (ABOUT_YOU, about_you::actions::NEXT)
            | (VISITING, visiting::actions::NEXT)
            | (PURPOSE_STEP, purpose::actions::NEXT)
            | (RULES_STEP, rules::actions::NEXT) => (Request::Next, "Next"),
            (VISITING, visiting::actions::BACK)
            | (PURPOSE_STEP, purpose::actions::BACK)
            | (RULES_STEP, rules::actions::BACK) => (Request::Back, "Back"),
            (Self::REVIEW, check::actions::BACK) => (Request::Back, "Change"),
            (Self::FAILED, failed::actions::BACK) => (Request::Back, "Check details"),
            (ABOUT_YOU, about_you::actions::CANCEL)
            | (VISITING, visiting::actions::CANCEL)
            | (PURPOSE_STEP, purpose::actions::CANCEL)
            | (RULES_STEP, rules::actions::CANCEL)
            | (Self::REVIEW, check::actions::CANCEL) => (Request::Cancel, "Cancel"),
            (Self::FAILED, failed::actions::CANCEL) => (Request::Cancel, "Finish"),
            (Self::IDLE, idle::actions::CANCEL) => (Request::Cancel, "Clear it now"),
            (Self::REVIEW, check::actions::SIGN_IN) => (Request::Submit, "Sign in"),
            (Self::REVIEW, check::actions::SERVICE_DOWN) => {
                (Request::SubmitFailing, "Demo: service down")
            }
            (Self::FAILED, failed::actions::RETRY) => (Request::Retry, "Try again"),
            (Self::SUBMITTED, signed_in::actions::DONE) => (Request::Finish, "Done"),
            (Self::IDLE, idle::actions::STILL_HERE) => (Request::StillHere, "Yes, I am here"),
            _ => return None,
        })
    }

    fn error_message(field: FieldId, issue: Issue) -> &'static str {
        match (field, issue) {
            (NAME, Issue::Missing) => "Enter your name",
            (HOST, Issue::Missing) => "Enter who you are visiting",
            (NAME | ORGANISATION | HOST, Issue::TooLong) => "Use at most 32 characters",
            (NAME | ORGANISATION | HOST, _) => "Use letters, digits and punctuation",
            (PURPOSE, _) => "Choose why you are visiting",
            (RULES, _) => "Tick this to sign in",
            _ => "Check this answer",
        }
    }

    fn countdown_slot(screen: ScreenId) -> Option<SlotId> {
        match screen {
            Self::IDLE => Some(idle::slots::SECONDS_LEFT),
            Self::SUBMITTED => Some(signed_in::slots::SECONDS_LEFT),
            _ => None,
        }
    }

    fn issued_slot(screen: ScreenId) -> Option<SlotId> {
        (screen == Self::SUBMITTED).then_some(signed_in::slots::PASS)
    }

    fn fill(
        screen: ScreenId,
        tree: &mut Tree<CAPACITY>,
        copies: &Copies,
        failed: Option<ShowFailed>,
    ) -> Result<(), UiError> {
        match screen {
            Self::REVIEW => {
                let purpose = copies
                    .choice(PURPOSE)
                    .and_then(|purpose| PURPOSES.get(usize::from(purpose)))
                    .copied()
                    .unwrap_or("");
                let organisation = match copies.text(ORGANISATION) {
                    "" => "None given",
                    text => text,
                };
                tree.set_slot(check::slots::NAME, copies.text(NAME))?;
                tree.set_slot(check::slots::ORGANISATION, organisation)?;
                tree.set_slot(check::slots::HOST, copies.text(HOST))?;
                tree.set_slot(check::slots::PURPOSE, purpose)?;
            }
            Self::FAILED => {
                let can_retry = failed.is_some_and(|failure| failure.attempts_left > 0);
                tree.set_visible(failed::groups::CAN_RETRY, can_retry)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_form_field_is_shown_once_with_an_error_line() {
        for field in FORM.fields() {
            let shown = Visitor::CONTROLS.iter().filter(|c| c.field == field.id);
            assert_eq!(shown.count(), 1);
            assert!(Visitor::ERROR_LINES.iter().any(|e| e.field == field.id));
        }
        assert_eq!(FORM.fields().count(), Visitor::CONTROLS.len());
    }
}
