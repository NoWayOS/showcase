// SPDX-License-Identifier: MIT OR Apache-2.0
//! The kitchen sink: all ten kiosk reference screens in one bounded
//! tree, toured with their own buttons - the React screens as native Rust,
//! to view and click through.
//!
//! **A demo driver, not the kiosk.** It is shaped like NoWayOS's clickable
//! demo, `nowayos-demo`: [`WIDTH`] and [`HEIGHT`], [`KitchenSink::new`]
//! taking the theme's metrics, [`KitchenSink::tree`],
//! [`KitchenSink::handle`] returning the action a button reported, and
//! [`action_name`] - so a copy of NoWayOS's `ui-demo` PD can run it in place
//! of `Demo`. One difference: [`KitchenSink::screen`] names the screen
//! showing, where `Demo::screen` returns an enum. The kiosk proper keeps its
//! rules in the application PD (`showcase-kiosk-flow`, behind the app-UI
//! contract); here the screens follow one fixed tour, with just enough of a
//! form's behaviour to show what the screens do:
//!
//! welcome → about you → your question → confirm → before you send →
//! check your answers → sending → not sent → thank you → are you still
//! there? → welcome
//!
//! * **Next** on a step with a required answer missing - a name, a topic,
//!   the agreement - shows that field's error line (a group holding a slot)
//!   instead of moving on.
//! * **Back** returns to the screen before, with the answers it showed set
//!   back; **Cancel** (and *Clear it now*) starts again from the welcome
//!   screen.
//! * **Check your answers** shows the answers given, each in a slot.
//! * **Sending** has no control: a tap, a click or Enter moves on, as a
//!   service's answer would. The tour shows **not sent** - a timeout, two
//!   tries left, *Try again* shown: *Check answers* returns to the review
//!   with the answers still there, and *Try again* is accepted.
//!
//! Sending, not sent and thank you each take the place of the screen before
//! them rather than going on the back stack, so no tour, however long or
//! often repeated, fills it: normal navigation never fails.
//!
//! The answers it carries between screens are kept for a retry until the
//! tour reaches *thank you*, and wiped in place there
//! (`nowayos_bounded::Wipe`), and whenever the tour starts again; the tree's
//! own values are wiped whenever the screen changes ([`Navigator`]). Nothing
//! is logged, and nothing is ever sent blank: the review is reached only
//! through the steps that check each required answer.

use nowayos_bounded::{BoundedStr, Wipe};
use nowayos_display::Rect;
use nowayos_input::{KeyCode, TouchPhase};
use nowayos_ui::{
    ActionId, FieldId, GroupId, InputEvent, Navigator, NodeId, ScreenId, SlotId, TextMetrics, Tree,
    UiError, UiEvent,
};

use crate::generated::{
    CAPACITY, TEXT, build_failed, build_idle_warning, build_review, build_step_checkbox,
    build_step_choice, build_step_notice, build_step_text, build_submitted, build_submitting,
    build_welcome, failed::failed_screen as failed, idle_warning::idle_warning_screen as idle,
    review::review_screen as review, step_checkbox::confirm_step as confirm,
    step_choice::question_step as question, step_notice::notice_step as notice,
    step_text::about_you_step as about_you, submitted::submitted_screen as submitted,
    welcome::welcome_screen as welcome,
};

/// Screen width, in pixels: the kiosk's layout size, which the tour is laid
/// out in until the display announces its own ([`KitchenSink::resize`]):
/// on the Pi 5, 1920 x 1080 at device scale 2, drawn at the monitor's own
/// pixels.
pub const WIDTH: u32 = 800;
/// Screen height, in pixels.
pub const HEIGHT: u32 = 480;

/// The welcome screen's Start.
pub const START: ActionId = ActionId(1);
/// A step's Next (the notice's Continue).
pub const NEXT: ActionId = ActionId(2);
/// A step's Back, or the review's Change.
pub const BACK: ActionId = ActionId(3);
/// Cancel, on any screen that has it.
pub const CANCEL: ActionId = ActionId(4);
/// The review's Send.
pub const SEND: ActionId = ActionId(5);
/// The not-sent screen's Check answers.
pub const CHECK_ANSWERS: ActionId = ActionId(6);
/// The not-sent screen's Try again.
pub const TRY_AGAIN: ActionId = ActionId(7);
/// The thank-you screen's Done.
pub const DONE: ActionId = ActionId(8);
/// The idle warning's Yes, I am here.
pub const STILL_HERE: ActionId = ActionId(9);
/// The idle warning's Clear it now.
pub const CLEAR: ActionId = ActionId(10);

/// The name of an action, for the session's log.
#[must_use]
pub const fn action_name(action: ActionId) -> &'static str {
    match action {
        START => "Start",
        NEXT => "Next",
        BACK => "Back",
        CANCEL => "Cancel",
        SEND => "Send",
        CHECK_ANSWERS => "Check answers",
        TRY_AGAIN => "Try again",
        DONE => "Done",
        STILL_HERE => "Yes, I am here",
        CLEAR => "Clear it now",
        _ => "unknown",
    }
}

/// The tour's screens, in order.
pub const TOUR: [&str; 10] = [
    "welcome",
    "step-text",
    "step-choice",
    "step-checkbox",
    "step-notice",
    "review",
    "submitting",
    "failed",
    "submitted",
    "idle-warning",
];

const WELCOME: ScreenId = ScreenId(0);
const ABOUT_YOU: ScreenId = ScreenId(1);
const QUESTION: ScreenId = ScreenId(2);
const CONFIRM: ScreenId = ScreenId(3);
const NOTICE: ScreenId = ScreenId(4);
const REVIEW: ScreenId = ScreenId(5);
const SUBMITTING: ScreenId = ScreenId(6);
const FAILED: ScreenId = ScreenId(7);
const SUBMITTED: ScreenId = ScreenId(8);
const IDLE: ScreenId = ScreenId(9);

/// The most screens the back stack holds: more than the five steps before
/// the review, the most it ever holds.
const DEPTH: usize = TOUR.len();

/// The topics `step-choice` offers, as its radio group shows them.
const TOPICS: [&str; 3] = ["Opening hours", "An order", "Something else"];

/// Builds the tour's screen `screen` into the empty `tree`.
fn build(screen: ScreenId, tree: &mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError> {
    match screen {
        WELCOME => build_welcome(tree),
        ABOUT_YOU => build_step_text(tree),
        QUESTION => build_step_choice(tree),
        CONFIRM => build_step_checkbox(tree),
        NOTICE => build_step_notice(tree),
        REVIEW => build_review(tree),
        SUBMITTING => build_submitting(tree),
        FAILED => build_failed(tree),
        SUBMITTED => build_submitted(tree),
        _ => build_idle_warning(tree),
    }
}

/// The tour's action for the button a screen reported, if it has one.
fn tour_action(screen: ScreenId, action: ActionId) -> Option<ActionId> {
    Some(match (screen, action) {
        (WELCOME, welcome::actions::START) => START,
        (ABOUT_YOU, about_you::actions::NEXT)
        | (QUESTION, question::actions::NEXT)
        | (CONFIRM, confirm::actions::NEXT)
        | (NOTICE, notice::actions::NEXT) => NEXT,
        (QUESTION, question::actions::BACK)
        | (CONFIRM, confirm::actions::BACK)
        | (NOTICE, notice::actions::BACK)
        | (REVIEW, review::actions::BACK) => BACK,
        (ABOUT_YOU, about_you::actions::CANCEL)
        | (QUESTION, question::actions::CANCEL)
        | (CONFIRM, confirm::actions::CANCEL)
        | (NOTICE, notice::actions::CANCEL)
        | (REVIEW, review::actions::CANCEL)
        | (FAILED, failed::actions::CANCEL) => CANCEL,
        (REVIEW, review::actions::SEND) => SEND,
        (FAILED, failed::actions::BACK) => CHECK_ANSWERS,
        (FAILED, failed::actions::RETRY) => TRY_AGAIN,
        (SUBMITTED, submitted::actions::DONE) => DONE,
        (IDLE, idle::actions::STILL_HERE) => STILL_HERE,
        (IDLE, idle::actions::CANCEL) => CLEAR,
        _ => return None,
    })
}

/// The answers carried from one screen to the next.
#[derive(Default)]
struct Answers {
    name: BoundedStr<64>,
    reference: BoundedStr<64>,
    topic: Option<u8>,
    details: BoundedStr<64>,
    agree: bool,
    receipt: bool,
}

impl Wipe for Answers {
    fn wipe(&mut self) {
        self.name.wipe();
        self.reference.wipe();
        self.topic = None;
        self.details.wipe();
        self.agree = false;
        self.receipt = false;
    }
}

/// The ten screens on a tour; see the [module](self).
pub struct KitchenSink {
    tree: Tree<CAPACITY, TEXT>,
    nav: Navigator<DEPTH>,
    metrics: TextMetrics,
    answers: Answers,
}

impl KitchenSink {
    /// The tour at its welcome screen, laid out for a [`WIDTH`] × [`HEIGHT`]
    /// screen with `metrics` (the theme's: `Theme::metrics()` in
    /// `nowayos-render-eg`).
    ///
    /// # Errors
    ///
    /// The tree's error, if the welcome screen cannot be built or laid out.
    pub fn new(metrics: TextMetrics) -> Result<Self, UiError> {
        let mut sink = Self {
            tree: Tree::new(),
            nav: Navigator::new(WELCOME, Rect::new(0, 0, WIDTH, HEIGHT), metrics),
            metrics,
            answers: Answers::default(),
        };
        sink.nav.show_start(&mut sink.tree, build)?;
        Ok(sink)
    }

    /// The tree to draw.
    #[must_use]
    pub const fn tree(&self) -> &Tree<CAPACITY, TEXT> {
        &self.tree
    }

    /// The tree, to draw only what changed and then mark it drawn
    /// (`Tree::clear_dirty`, through `nowayos-ui-loop`'s `draw_dirty`).
    pub const fn tree_mut(&mut self) -> &mut Tree<CAPACITY, TEXT> {
        &mut self.tree
    }

    /// Lays the tour out in the display's `viewport` with `metrics` - the
    /// theme's at the display's device scale - from now on, the screen
    /// showing at once, keeping what it holds: what the UI PD
    /// calls when the display announces its layout.
    ///
    /// # Errors
    ///
    /// The tree's error, if the screen cannot be laid out there.
    pub fn resize(&mut self, viewport: Rect, metrics: TextMetrics) -> Result<(), UiError> {
        self.metrics = metrics;
        self.nav.resize(&mut self.tree, viewport, metrics)
    }

    /// The screen showing: its file's name, as in [`TOUR`].
    #[must_use]
    pub fn screen(&self) -> &'static str {
        TOUR.get(usize::from(self.nav.current().0))
            .copied()
            .unwrap_or("welcome")
    }

    /// Whether it carries any answer between screens: false once the tour
    /// starts again and once it reaches *thank you*.
    #[must_use]
    pub fn holds_answers(&self) -> bool {
        let a = &self.answers;
        !(a.name.is_zeroed()
            && a.reference.is_zeroed()
            && a.details.is_zeroed()
            && a.topic.is_none()
            && !a.agree
            && !a.receipt)
    }

    /// Applies one canonical input event; returns the action a button
    /// reported, if any ([`action_name`]), after acting on it.
    ///
    /// # Errors
    ///
    /// The tree's error, if a screen cannot be built, laid out or given its
    /// values - never for the tour's own screens, which fit their tree.
    pub fn handle(&mut self, event: InputEvent) -> Result<Option<ActionId>, UiError> {
        let reported = self.tree.handle(event);
        if self.nav.current() == SUBMITTING && moves_on(event) {
            // The service's answer: not sent.
            self.replace(FAILED)?;
            return Ok(None);
        }
        let Some(UiEvent::Action(action)) = reported else {
            return Ok(None);
        };
        let Some(action) = tour_action(self.nav.current(), action) else {
            return Ok(None);
        };
        self.act(action)?;
        Ok(Some(action))
    }

    /// What the tour's `action` does on the screen showing.
    fn act(&mut self, action: ActionId) -> Result<(), UiError> {
        match (self.nav.current(), action) {
            (WELCOME, START) => self.go(ABOUT_YOU),
            (ABOUT_YOU, NEXT) => {
                let name = self.tree.value(about_you::fields::NAME).unwrap_or_default();
                if name.trim().is_empty() {
                    return self.show_error(
                        about_you::groups::NAME_ERROR,
                        about_you::slots::NAME_ERROR,
                        "Enter your name",
                    );
                }
                self.save();
                self.go(QUESTION)
            }
            (QUESTION, NEXT) => {
                if self.tree.selected(question::fields::TOPIC).is_none() {
                    return self.show_error(
                        question::groups::TOPIC_ERROR,
                        question::slots::TOPIC_ERROR,
                        "Choose what it is about",
                    );
                }
                self.save();
                self.go(CONFIRM)
            }
            (CONFIRM, NEXT) => {
                if self.tree.checked(confirm::fields::AGREE) != Some(true) {
                    return self.show_error(
                        confirm::groups::AGREE_ERROR,
                        confirm::slots::AGREE_ERROR,
                        "Tick this to send your answers",
                    );
                }
                self.save();
                self.go(NOTICE)
            }
            (NOTICE, NEXT) => self.go(REVIEW),
            // Sending takes the review's place: the answers stay for a
            // retry, and the back stack does not grow.
            (REVIEW, SEND) => self.replace(SUBMITTING),
            (FAILED, CHECK_ANSWERS) => self.replace(REVIEW),
            (FAILED, TRY_AGAIN) => {
                // Accepted: the answers are wiped as soon as they are sent.
                self.answers.wipe();
                self.replace(SUBMITTED)
            }
            (SUBMITTED, DONE) => self.replace(IDLE),
            (_, BACK) => {
                // What the step shows now, edits and all, is what it shows
                // when it comes back.
                self.save();
                let shown = self.nav.back(&mut self.tree, build).map(|_| ());
                self.settle(shown)
            }
            (_, CANCEL | CLEAR | STILL_HERE) => self.start_again(),
            _ => Ok(()),
        }
    }

    /// Keeps the answers the step showing holds now, as the tree has them:
    /// on Next, once they are checked, and on Back, edits and all.
    fn save(&mut self) {
        let tree = &self.tree;
        let a = &mut self.answers;
        match self.nav.current() {
            ABOUT_YOU => {
                a.name = copy(tree.value(about_you::fields::NAME).unwrap_or_default());
                a.reference = copy(tree.value(about_you::fields::REFERENCE).unwrap_or_default());
            }
            QUESTION => {
                a.topic = tree.selected(question::fields::TOPIC);
                a.details = copy(tree.value(question::fields::DETAILS).unwrap_or_default());
            }
            CONFIRM => {
                a.agree = tree.checked(confirm::fields::AGREE) == Some(true);
                a.receipt = tree.checked(confirm::fields::RECEIPT) == Some(true);
            }
            _ => {}
        }
    }

    /// Shows `screen`, remembering this one for Back, then what it shows of
    /// the answers and the tour.
    fn go(&mut self, screen: ScreenId) -> Result<(), UiError> {
        let shown = self.nav.go(screen, &mut self.tree, build);
        self.settle(shown)
    }

    /// Shows `screen` in this one's place, then what it shows.
    fn replace(&mut self, screen: ScreenId) -> Result<(), UiError> {
        let shown = self.nav.replace(screen, &mut self.tree, build);
        self.settle(shown)
    }

    /// After a switch: fills the screen now showing - or, if the switch or
    /// the filling failed, wipes the answers and starts the tour again, so a
    /// failure never leaves one person's answers for the next. (The
    /// Navigator has then shown the welcome screen already; the tour's own
    /// screens fit their tree, so this is a guard no test can reach.)
    fn settle(&mut self, shown: Result<(), UiError>) -> Result<(), UiError> {
        match shown.and_then(|()| self.fill()) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.answers.wipe();
                // The error to report is the first; starting again only
                // repeats the welcome screen the Navigator showed.
                let _restarted = self.nav.reset(&mut self.tree, build);
                Err(error)
            }
        }
    }

    /// Wipes the answers and shows the welcome screen.
    fn start_again(&mut self) -> Result<(), UiError> {
        self.answers.wipe();
        self.nav.reset(&mut self.tree, build)
    }

    /// Shows `message` in an error line - `group` holding `slot` - and lays
    /// the screen out again, so the line takes its place.
    fn show_error(&mut self, group: GroupId, slot: SlotId, message: &str) -> Result<(), UiError> {
        self.tree.set_slot(slot, message)?;
        self.tree.set_visible(group, true)?;
        self.relayout()
    }

    /// Sets the screen showing to what it shows of the answers and the
    /// tour, and lays it out again.
    fn fill(&mut self) -> Result<(), UiError> {
        let a = &self.answers;
        match self.nav.current() {
            ABOUT_YOU => {
                set_value(&mut self.tree, about_you::fields::NAME, &a.name)?;
                set_value(&mut self.tree, about_you::fields::REFERENCE, &a.reference)?;
            }
            QUESTION => {
                self.tree.set_selected(question::fields::TOPIC, a.topic)?;
                set_value(&mut self.tree, question::fields::DETAILS, &a.details)?;
            }
            CONFIRM => {
                self.tree.set_checked(confirm::fields::AGREE, a.agree)?;
                self.tree.set_checked(confirm::fields::RECEIPT, a.receipt)?;
            }
            REVIEW => {
                let topic = a
                    .topic
                    .and_then(|topic| TOPICS.get(usize::from(topic)))
                    .copied()
                    .unwrap_or("");
                let details = if a.details.is_empty() {
                    "None given"
                } else {
                    a.details.as_str()
                };
                let receipt = if a.receipt { "Yes" } else { "No" };
                self.tree.set_slot(review::slots::NAME, a.name.as_str())?;
                self.tree
                    .set_slot(review::slots::REFERENCE, a.reference.as_str())?;
                self.tree.set_slot(review::slots::TOPIC, topic)?;
                self.tree.set_slot(review::slots::DETAILS, details)?;
                self.tree.set_slot(review::slots::RECEIPT, receipt)?;
            }
            FAILED => {
                // Not sent: the service did not answer in time, two tries
                // are left, and a try may help.
                self.tree.set_slot(failed::slots::ATTEMPTS_LEFT, "2")?;
                self.tree.set_visible(failed::groups::REJECTED, false)?;
                self.tree.set_visible(failed::groups::CAN_RETRY, true)?;
            }
            IDLE => self.tree.set_slot(idle::slots::SECONDS_LEFT, "30")?,
            SUBMITTED => self.tree.set_slot(submitted::slots::SECONDS_LEFT, "10")?,
            _ => {}
        }
        self.relayout()
    }

    /// Lays the screen out again, after a group is shown or hidden.
    fn relayout(&mut self) -> Result<(), UiError> {
        self.tree.layout(self.nav.viewport(), &self.metrics)
    }
}

/// A press released, or Enter: what moves the sending screen on.
fn moves_on(event: InputEvent) -> bool {
    matches!(
        event,
        InputEvent::PointerButton {
            button: 0,
            down: false
        } | InputEvent::Touch {
            id: 0,
            phase: TouchPhase::End,
            ..
        } | InputEvent::KeyDown(KeyCode::ENTER)
    )
}

/// `text`, which a field of at most 64 bytes held, as an answer.
fn copy(text: &str) -> BoundedStr<64> {
    BoundedStr::try_from_str(text).unwrap_or_default()
}

/// Sets `field` back to `value`, unless it is empty.
fn set_value(
    tree: &mut Tree<CAPACITY, TEXT>,
    field: FieldId,
    value: &BoundedStr<64>,
) -> Result<(), UiError> {
    if value.is_empty() {
        Ok(())
    } else {
        tree.set_value(field, value.as_str())
    }
}
