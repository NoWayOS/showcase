// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's pay at the pump: the screen beside a fictional
//! fuel station's pumps (Tarnwick Fuel), which never sees a card.
//!
//! The most hostile place for a public kiosk: outdoors, unattended, with
//! money and fuel at stake. The driver chooses a pump, a fuel and a preset
//! amount here, then pays on the card reader beside the screen. The screen
//! asks for no card number and no PIN, and learns only what the card reader
//! and the pump controller approved - the pump, the fuel and the amount,
//! as the code they issue ([`Approval`]) - or that they did not. Every
//! choice is wiped, in both PDs, before the approval shows. The screen then
//! follows the pump - ready, fuelling, all done - while the confirmation
//! counts down, and clears itself for the next customer.
//!
//! | Screen | Phase or step |
//! | --- | --- |
//! | `welcome` | Welcome: *Start* |
//! | `pump` | step 0: [`PUMP`] (required, one of four) |
//! | `fuel` | step 1: [`FUEL`] (required, one of [`FUELS`]) |
//! | `amount` | step 2: [`AMOUNT`] (required, one of [`AMOUNTS`]) |
//! | `check` | Review: every choice; *Pay on the card reader*, or *Demo: card declined* |
//! | `paying` | Submitting: "Follow the instructions on the card reader" |
//! | `not-accepted` | Failed: "Payment not accepted" |
//! | `fuelling` | Submitted: the pump ready, fuelling, all done, and the seconds left |
//! | `idle-warning` | the idle warning before the privacy reset |
//!
//! **Simulated, and says so**: the card reader, the bank, the pump
//! controller and the receipt. Its stand-in, in the example's own
//! application PD (`showcase-pump-stand-in`), approves what was asked a
//! second after it is sent - unless the driver pressed *Demo: card
//! declined*, when every attempt of that session is declined. The litres
//! and pounds count up as the confirmation counts down, sped up and
//! labelled so: the pump's own display is the official reading.
//!
//! **Claims only that the screen cannot see the card or start a pump**:
//! never payment-ready, PCI, EMV or safe for PINs, and no card-scheme
//! logos (the story's wording rule).
//!
//! [`generated`] holds the Rust Screen Compiler writes for `tsx/`, exactly as
//! written (`make screens-bless`); [`Pump`] tells the kiosk's form UI
//! (`showcase_kiosk_screens::kiosk::FormUi`) how to show them; [`EXAMPLE`]
//! is what the application PD runs. Every brand, price and text is
//! invented.

#![no_std]
#![forbid(unsafe_code)]

pub mod generated;

use nowayos_bounded::BoundedStr;
use nowayos_ui::{ActionId, NodeId, ScreenId, SlotId, Tree, UiError};
use showcase_kiosk_contract::{Issued, ShowFailed};
use showcase_kiosk_flow::{FieldId, FlowConfig, Form, Issue, Step, StepId, Timeouts};
use showcase_kiosk_screens::app::Example;
use showcase_kiosk_screens::kiosk::{
    Control, ControlKind, Copies, ErrorLine, Follow, FormUi, Request, Screens,
};
use showcase_kiosk_screens::stand_in::Policy;

use generated::{
    CAPACITY, TEXT, amount::amount_step as amount, check::check_screen as check,
    fuel::fuel_step as fuel, fuelling::fuelling_screen as fuelling,
    idle_warning::idle_warning_screen as idle, not_accepted::not_accepted_screen as failed,
    pump::pump_step as pump, welcome::welcome_screen as welcome,
};

const _: () = assert!(CAPACITY == showcase_kiosk_screens::generated::CAPACITY);
const _: () = assert!(TEXT == showcase_kiosk_screens::generated::TEXT);

/// Which pump: required, one of four.
pub const PUMP: FieldId = FieldId(1);
/// Which fuel: required, one of [`FUELS`].
pub const FUEL: FieldId = FieldId(2);
/// How much: required, one of [`AMOUNTS`].
pub const AMOUNT: FieldId = FieldId(3);

/// How many pumps [`PUMP`] offers.
pub const PUMPS: u8 = 4;

/// The fuels [`FUEL`] offers: the name the review shows, and the price a
/// litre in tenths of a penny. Invented prices.
pub const FUELS: [(&str, u32); 3] = [
    ("Unleaded E10", 1_429),
    ("Super unleaded E5", 1_569),
    ("Diesel B7", 1_499),
];

/// The preset amounts [`AMOUNT`] offers, in pounds: the pump stops there.
pub const AMOUNTS: [u32; 4] = [10, 20, 40, 60];

/// The pay at the pump's form: three choices, one a step.
pub static FORM: Form<'static> = Form::new(&[
    Step::new(
        StepId(1),
        &[showcase_kiosk_flow::Field::choice(PUMP, PUMPS).required()],
    ),
    Step::new(
        StepId(2),
        &[showcase_kiosk_flow::Field::choice(FUEL, 3).required()],
    ),
    Step::new(
        StepId(3),
        &[showcase_kiosk_flow::Field::choice(AMOUNT, 4).required()],
    ),
]);

/// How long it waits, in milliseconds (shorter in QEMU's test images,
/// [`Timeouts::or_test`]): the idle warning after 15 s with no input, the
/// reset 15 s later, a payment's timeout after 5 s, the pump followed for
/// 24 s, a session at most 20 minutes, and three tries.
pub const CONFIG: FlowConfig = FlowConfig {
    timeouts: Timeouts {
        idle_reset: 30_000,
        idle_warning: 15_000,
        submit: 5_000,
        confirmation: 24_000,
        max_session: 20 * 60_000,
    }
    .or_test(),
    max_attempts: 3,
};

/// The fields the card reader and pump controller are asked to approve,
/// a digit each: the pump, the fuel and the amount.
const ASKED: &[FieldId] = &[PUMP, FUEL, AMOUNT];

/// What the pay at the pump's application PD runs: its form and timeouts,
/// and a stand-in for the card reader and pump controller that approves
/// the pump, fuel and amount asked.
pub const EXAMPLE: Example = Example {
    form: &FORM,
    config: CONFIG,
    policy: Policy::Approves { fields: ASKED },
};

/// What the card reader and pump controller approved, read from the code
/// they issued: one digit each for the pump, fuel and amount chosen
/// (`showcase_kiosk_screens::stand_in::approval`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Approval {
    /// The pump, from 1.
    pub pump: u8,
    /// The fuel's index in [`FUELS`].
    pub fuel: usize,
    /// The amount approved, in pounds.
    pub pounds: u32,
}

impl Approval {
    /// The approval `issued` says, or `None` for a code that is not three
    /// digits naming a pump, a fuel and an amount the pump offers.
    #[must_use]
    pub fn of(issued: &Issued) -> Option<Self> {
        let &[p, f, a] = issued.code().as_bytes() else {
            return None;
        };
        let digit = |byte: u8| byte.checked_sub(b'0').filter(|d| *d <= 9);
        let pump = digit(p)?;
        let fuel = usize::from(digit(f)?);
        let pounds = *AMOUNTS.get(usize::from(digit(a)?))?;
        if pump >= PUMPS || fuel >= FUELS.len() {
            return None;
        }
        Some(Self {
            pump: pump.saturating_add(1),
            fuel,
            pounds,
        })
    }

    /// The fuel's price a litre, in tenths of a penny.
    fn price(self) -> u32 {
        FUELS.get(self.fuel).map_or(1, |&(_, price)| price.max(1))
    }
}

/// Where the pump is, `elapsed` milliseconds into a confirmation of
/// `total`: the first eighth ready, the next half fuelling, the
/// rest all done - as long on the Pi as in QEMU's short test timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Ready: lift the nozzle.
    Ready,
    /// Fuelling, this many thousandths of the way to the amount approved.
    Fuelling(u32),
    /// All done: the amount approved taken.
    Done,
}

impl Stage {
    /// The stage `elapsed` milliseconds into a confirmation of `total`.
    #[must_use]
    pub fn at(elapsed: u32, total: u32) -> Self {
        let ready = total / 8;
        let fuelling = (total / 2).max(1);
        match elapsed.checked_sub(ready) {
            None => Self::Ready,
            Some(into) if into < fuelling => {
                let share = u64::from(into)
                    .saturating_mul(1_000)
                    .checked_div(u64::from(fuelling))
                    .unwrap_or(0);
                Self::Fuelling(u32::try_from(share).unwrap_or(1_000))
            }
            Some(_) => Self::Done,
        }
    }

    /// What the fuelling screen's title says at this stage, at the pump
    /// approved, if the approval has been read.
    #[must_use]
    pub fn title(self, pump: Option<u8>) -> BoundedStr<40> {
        let mut title = BoundedStr::new();
        let number = pump
            .and_then(|pump| PUMP_NUMBERS.get(usize::from(pump.saturating_sub(1))))
            .copied();
        let parts: [&str; 3] = match (self, number) {
            (Self::Ready, Some(number)) => ["Pump ", number, " ready: lift the nozzle"],
            (Self::Ready, None) => ["Pump ready: lift the nozzle", "", ""],
            (Self::Fuelling(_), Some(number)) => ["Fuelling at pump ", number, ""],
            (Self::Fuelling(_), None) => ["Fuelling", "", ""],
            (Self::Done, _) => ["All done. Card details: never here.", "", ""],
        };
        for part in parts {
            let _ = title.push_str(part);
        }
        title
    }

    /// Thousandths of the amount approved taken by now.
    const fn share(self) -> u32 {
        match self {
            Self::Ready => 0,
            Self::Fuelling(share) => share,
            Self::Done => 1_000,
        }
    }
}

/// The pump's figures for `approval` at `stage`: litres to two places, and
/// the cost in pounds and pence, as the display role shows them.
#[must_use]
pub fn figures(approval: Approval, stage: Stage) -> (BoundedStr<12>, BoundedStr<12>) {
    let pence = u64::from(approval.pounds)
        .saturating_mul(100)
        .saturating_mul(u64::from(stage.share()))
        / 1_000;
    // Hundredths of a litre: pence over the price a litre, in tenths of
    // a penny, times a thousand.
    let centilitres = pence
        .saturating_mul(1_000)
        .checked_div(u64::from(approval.price()))
        .unwrap_or(0);
    (decimal("", centilitres), decimal("£", pence))
}

/// `hundredths` as `prefix`, the whole part, a point and two digits.
fn decimal(prefix: &str, hundredths: u64) -> BoundedStr<12> {
    let mut text = BoundedStr::<12>::new();
    let _ = text.push_str(prefix);
    let whole = hundredths / 100;
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut rest = whole;
    while let Some(next) = at.checked_sub(1) {
        at = next;
        if let Some(slot) = digits.get_mut(at) {
            *slot = b'0'.saturating_add(u8::try_from(rest % 10).unwrap_or(0));
        }
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    let part = hundredths % 100;
    for &byte in digits.get(at..).unwrap_or_default() {
        let _ = text.push(char::from(byte));
    }
    let _ = text.push('.');
    let _ = text.push(char::from(
        b'0'.saturating_add(u8::try_from(part / 10).unwrap_or(0)),
    ));
    let _ = text.push(char::from(
        b'0'.saturating_add(u8::try_from(part % 10).unwrap_or(0)),
    ));
    text
}

/// The pay at the pump's screens; see the [crate documentation](crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pump;

/// The pay at the pump's UI.
pub type PumpUi = FormUi<Pump>;

const PUMP_STEP: ScreenId = ScreenId(1);
const FUEL_STEP: ScreenId = ScreenId(2);
const AMOUNT_STEP: ScreenId = ScreenId(3);

/// Each screen's file name, by screen id: what the UI PD logs.
const NAMES: [&str; 9] = [
    "welcome",
    "pump",
    "fuel",
    "amount",
    "check",
    "paying",
    "not-accepted",
    "fuelling",
    "idle-warning",
];

/// The pump numbers, as the review and the fuelling screen show them.
const PUMP_NUMBERS: [&str; 4] = ["1", "2", "3", "4"];
const _: () = assert!(PUMP_NUMBERS.len() == PUMPS as usize);

/// The amounts, as the review and the fuelling screen show them.
const AMOUNT_TEXTS: [&str; 4] = ["£10.00", "£20.00", "£40.00", "£60.00"];

impl Screens for Pump {
    const WELCOME: ScreenId = ScreenId(0);
    const REVIEW: ScreenId = ScreenId(4);
    const SUBMITTING: ScreenId = ScreenId(5);
    const FAILED: ScreenId = ScreenId(6);
    const SUBMITTED: ScreenId = ScreenId(7);
    const IDLE: ScreenId = ScreenId(8);
    const STEPS: &'static [ScreenId] = &[PUMP_STEP, FUEL_STEP, AMOUNT_STEP];
    const CONTROLS: &'static [Control] = &[
        Control {
            field: PUMP,
            screen: PUMP_STEP,
            control: pump::fields::PUMP,
            kind: ControlKind::Choice,
        },
        Control {
            field: FUEL,
            screen: FUEL_STEP,
            control: fuel::fields::FUEL,
            kind: ControlKind::Choice,
        },
        Control {
            field: AMOUNT,
            screen: AMOUNT_STEP,
            control: amount::fields::AMOUNT,
            kind: ControlKind::Choice,
        },
    ];
    const ERROR_LINES: &'static [ErrorLine] = &[
        ErrorLine {
            field: PUMP,
            screen: PUMP_STEP,
            group: pump::groups::PUMP_ERROR,
            slot: pump::slots::PUMP_ERROR,
        },
        ErrorLine {
            field: FUEL,
            screen: FUEL_STEP,
            group: fuel::groups::FUEL_ERROR,
            slot: fuel::slots::FUEL_ERROR,
        },
        ErrorLine {
            field: AMOUNT,
            screen: AMOUNT_STEP,
            group: amount::groups::AMOUNT_ERROR,
            slot: amount::slots::AMOUNT_ERROR,
        },
    ];
    const STILL_HERE: ActionId = idle::actions::STILL_HERE;
    const FOLLOWS: bool = true;

    fn name(screen: ScreenId) -> &'static str {
        NAMES
            .get(usize::from(screen.0))
            .copied()
            .unwrap_or("welcome")
    }

    fn build(screen: ScreenId, tree: &mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError> {
        match screen {
            Self::WELCOME => generated::build_welcome(tree),
            PUMP_STEP => generated::build_pump(tree),
            FUEL_STEP => generated::build_fuel(tree),
            AMOUNT_STEP => generated::build_amount(tree),
            Self::REVIEW => generated::build_check(tree),
            Self::SUBMITTING => generated::build_paying(tree),
            Self::FAILED => generated::build_not_accepted(tree),
            Self::SUBMITTED => generated::build_fuelling(tree),
            _ => generated::build_idle_warning(tree),
        }
    }

    fn button(screen: ScreenId, action: ActionId) -> Option<(Request, &'static str)> {
        Some(match (screen, action) {
            (Self::WELCOME, welcome::actions::START) => (Request::Start, "Start"),
            (PUMP_STEP, pump::actions::NEXT)
            | (FUEL_STEP, fuel::actions::NEXT)
            | (AMOUNT_STEP, amount::actions::NEXT) => (Request::Next, "Next"),
            (FUEL_STEP, fuel::actions::BACK) | (AMOUNT_STEP, amount::actions::BACK) => {
                (Request::Back, "Back")
            }
            (Self::REVIEW, check::actions::BACK) | (Self::FAILED, failed::actions::BACK) => {
                (Request::Back, "Change")
            }
            (PUMP_STEP, pump::actions::CANCEL)
            | (FUEL_STEP, fuel::actions::CANCEL)
            | (AMOUNT_STEP, amount::actions::CANCEL)
            | (Self::REVIEW, check::actions::CANCEL) => (Request::Cancel, "Cancel"),
            (Self::FAILED, failed::actions::CANCEL) => (Request::Cancel, "Finish"),
            (Self::IDLE, idle::actions::CANCEL) => (Request::Cancel, "Clear it now"),
            (Self::REVIEW, check::actions::PAY) => (Request::Submit, "Pay on the card reader"),
            (Self::REVIEW, check::actions::DECLINE) => {
                (Request::SubmitFailing, "Demo: card declined")
            }
            (Self::FAILED, failed::actions::RETRY) => (Request::Retry, "Try again"),
            (Self::SUBMITTED, fuelling::actions::DONE) => (Request::Finish, "Done"),
            (Self::IDLE, idle::actions::STILL_HERE) => (Request::StillHere, "Yes, I am here"),
            _ => return None,
        })
    }

    fn error_message(field: FieldId, _issue: Issue) -> &'static str {
        match field {
            PUMP => "Choose the pump you are at",
            FUEL => "Choose a fuel",
            AMOUNT => "Choose how much",
            _ => "Check this answer",
        }
    }

    fn countdown_slot(screen: ScreenId) -> Option<SlotId> {
        match screen {
            Self::IDLE => Some(idle::slots::SECONDS_LEFT),
            Self::SUBMITTED => Some(fuelling::slots::SECONDS_LEFT),
            _ => None,
        }
    }

    fn fill(
        screen: ScreenId,
        tree: &mut Tree<CAPACITY, TEXT>,
        copies: &Copies,
        failed: Option<ShowFailed>,
    ) -> Result<(), UiError> {
        match screen {
            Self::REVIEW => {
                let chosen = |field, texts: &[&'static str]| {
                    copies
                        .choice(field)
                        .and_then(|option| texts.get(usize::from(option)))
                        .copied()
                        .unwrap_or("")
                };
                let fuels = FUELS.map(|(name, _)| name);
                tree.set_slot(check::slots::PUMP, chosen(PUMP, &PUMP_NUMBERS))?;
                tree.set_slot(check::slots::FUEL, chosen(FUEL, &fuels))?;
                tree.set_slot(check::slots::AMOUNT, chosen(AMOUNT, &AMOUNT_TEXTS))?;
            }
            Self::FAILED => {
                let can_retry = failed.is_some_and(|failure| failure.attempts_left > 0);
                tree.set_visible(failed::groups::CAN_RETRY, can_retry)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// The fuelling screen follows the pump as the confirmation counts
    /// down: what was approved, then each stage and the litres and pounds
    /// so far. Before the approval is read, or for one it cannot read,
    /// nothing is shown as approved.
    fn follow(
        screen: ScreenId,
        follow: &mut Follow<'_>,
        issued: Option<&Issued>,
        remaining: Option<u32>,
    ) -> Result<(), UiError> {
        if screen != Self::SUBMITTED {
            return Ok(());
        }
        let total = u32::try_from(CONFIG.timeouts.confirmation).unwrap_or(u32::MAX);
        let elapsed = total.saturating_sub(remaining.unwrap_or(total));
        let stage = Stage::at(elapsed, total);
        let approval = issued.and_then(Approval::of);
        let approved = approval.map_or("", |approval| {
            AMOUNTS
                .iter()
                .position(|&pounds| pounds == approval.pounds)
                .and_then(|i| AMOUNT_TEXTS.get(i))
                .copied()
                .unwrap_or("")
        });
        follow.set_slot(fuelling::slots::APPROVED, approved)?;
        let title = stage.title(approval.map(|approval| approval.pump));
        follow.set_slot(fuelling::slots::STAGE, title.as_str())?;
        let (litres, cost) = approval.map_or_else(
            || (BoundedStr::new(), BoundedStr::new()),
            |approval| figures(approval, stage),
        );
        follow.set_slot(fuelling::slots::LITRES, litres.as_str())?;
        // The bar fills with the share of the approved amount taken.
        let share = approval.map_or(0, |_| stage.share());
        follow.set_progress(fuelling::slots::FUELLED, share, 1_000)?;
        follow.set_slot(fuelling::slots::COST, cost.as_str())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code: a failed check panics")]
mod tests {
    use super::*;

    #[test]
    fn every_form_field_is_shown_once_with_an_error_line() {
        for field in FORM.fields() {
            let shown = Pump::CONTROLS.iter().filter(|c| c.field == field.id);
            assert_eq!(shown.count(), 1);
            assert!(Pump::ERROR_LINES.iter().any(|e| e.field == field.id));
        }
        assert_eq!(FORM.fields().count(), Pump::CONTROLS.len());
    }

    #[test]
    fn an_approval_names_a_pump_a_fuel_and_an_amount_or_nothing() {
        let issued = |code| Issued::new(code).unwrap();
        assert_eq!(
            Approval::of(&issued("212")),
            Some(Approval {
                pump: 3,
                fuel: 1,
                pounds: 40
            })
        );
        for code in ["21", "2121", "412", "232", "214", "A12"] {
            assert_eq!(Approval::of(&issued(code)), None, "{code}");
        }
    }

    #[test]
    fn the_pump_is_ready_then_fuels_then_is_done() {
        assert_eq!(Stage::at(0, 24_000), Stage::Ready);
        assert_eq!(Stage::at(2_999, 24_000), Stage::Ready);
        assert_eq!(Stage::at(3_000, 24_000), Stage::Fuelling(0));
        assert_eq!(Stage::at(9_000, 24_000), Stage::Fuelling(500));
        assert_eq!(Stage::at(15_000, 24_000), Stage::Done);
        assert_eq!(Stage::at(u32::MAX, 24_000), Stage::Done);
        assert_eq!(Stage::at(0, 0), Stage::Fuelling(0));
    }

    #[test]
    fn the_figures_count_up_to_the_amount_approved() {
        let approval = Approval {
            pump: 3,
            fuel: 0,
            pounds: 40,
        };
        let shown = |stage| figures(approval, stage);
        assert_eq!(shown(Stage::Ready), (s("0.00"), s("£0.00")));
        // Half way: £20.00 of unleaded at 142.9p a litre is 13.99 litres.
        assert_eq!(shown(Stage::Fuelling(500)), (s("13.99"), s("£20.00")));
        // All done: exactly the amount approved.
        assert_eq!(shown(Stage::Done), (s("27.99"), s("£40.00")));
    }

    fn s(text: &str) -> BoundedStr<12> {
        BoundedStr::try_from_str(text).unwrap()
    }
}
