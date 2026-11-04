// SPDX-License-Identifier: MIT OR Apache-2.0
//! The reference kiosk's UI: the reference screens driven by the
//! kiosk flow, over the app-UI contract (`showcase-kiosk-contract`).
//!
//! **The application decides; the UI shows and asks.** [`KioskUi`] shows
//! the screen each [`Report`] names, and turns what the person does into
//! the contract's [`Action`]s for the application PD, which runs the flow
//! on the reference form ([`crate::form`]). It never moves on by itself:
//! *Next* is a request, and the next step appears only when the
//! application says so - or the step stays, with the field that needs
//! attention named in its error line.
//!
//! * **Answers go one way, as typed.** Each text field's value goes to the
//!   application as a whole ([`SetText`]) after each batch of input that
//!   changed it, and before any button's action; a choice or a checkbox at
//!   once ([`Select`], [`SetChecked`]). Other input on a screen that waits
//!   for the person - moving focus, a press on nothing - tells the
//!   application the person is still there ([`StillHere`]), at most once a
//!   batch, so the idle timer runs from their last touch.
//! * **The UI keeps copies**, as the contract says it must: what each step
//!   shows when the person comes back to it, and what the review lists. A
//!   value the application refuses is dropped from both, if the refused
//!   action was the UI's last for that field.
//! * **A session's end wipes everything, in place.** On
//!   [`SessionEnded`](showcase_kiosk_contract::SessionEnded),
//!   the copies are zeroed (`nowayos_bounded::Wipe`), the tree is wiped
//!   (`Tree::wipe_session`) and actions not yet sent are wiped in place and
//!   dropped, before the screen that follows is shown. An action's text is
//!   wiped in place as soon as it is sent, too.
//! * **It fails closed.** A report it cannot decode
//!   ([`receive_report`](KioskUi::receive_report)), a step the form lacks
//!   or a screen it cannot show means it can no longer trust what it shows:
//!   it wipes everything, shows the welcome screen and asks the application
//!   to end any session, whatever its phase ([`Abandon`]), so the next
//!   person never joins the last one's. [`reset`](KioskUi::reset) does the same, for a UI PD that
//!   gives up.
//! * **The idle warning** takes the place of the screen it interrupts, with
//!   the seconds left when it came, and focus on *Yes, I am here*, so a
//!   stray Enter keeps the session rather than ending it; that button - or
//!   any other key, or a press away from the buttons - is [`StillHere`],
//!   and the application then names the screen to go back to, which the UI
//!   rebuilds from its copies. There, Tab, Shift-Tab, Up and Down move
//!   focus and Left and Right do nothing - neither counts as still here -
//!   and a press on a button acts on its release, so *Clear it now*
//!   ([`Cancel`]) can be reached from the keyboard or clicked.
//!
//! Nothing here logs, and no error or `Debug` output holds an answer.

use core::fmt;
use core::marker::PhantomData;

use nowayos_bounded::{BoundedStr, BoundedVec, Wipe};
use nowayos_display::{Point, Rect};
use nowayos_input::{KeyCode, TouchPhase};
use nowayos_render_eg::{Rgb888, Theme};
use nowayos_ui::{
    ActionId, FieldId as UiField, GroupId, InputEvent, Navigator, NodeId, NodeKind, ScreenId,
    SlotId, TextMetrics, Tree, UiError, UiEvent,
};
use showcase_kiosk_contract::{
    Abandon, Action, Back, Cancel, ContractError, Finish, Issued, Next, OutOfServiceReason, Report,
    Retry, Select, SetChecked, SetText, ShowFailed, SimulateFailure, Start, StillHere, Submit,
};
use showcase_kiosk_flow::{FailureReason, FieldId as FormField, Issue};

use crate::form::{AGREE, DETAILS, NAME, RECEIPT, REFERENCE, TOPIC, TOPICS, VALUE_LEN};
use crate::generated::{
    CAPACITY, TEXT, build_failed, build_idle_warning, build_review, build_step_checkbox,
    build_step_choice, build_step_notice, build_step_text, build_submitted, build_submitting,
    build_welcome, failed::failed_screen as failed, idle_warning::idle_warning_screen as idle,
    review::review_screen as review, step_checkbox::confirm_step as confirm,
    step_choice::question_step as question, step_notice::notice_step as notice,
    step_text::about_you_step as about_you, submitted::submitted_screen as submitted,
    welcome::welcome_screen as welcome,
};

/// Screen width, in pixels: the kiosk's layout size.
pub const WIDTH: u32 = 800;
/// Screen height, in pixels.
pub const HEIGHT: u32 = 480;

/// Slots in each ring between the UI and application PDs; each slot holds
/// the contract's longest record (`showcase_kiosk_contract::MAX_RECORD_LEN`).
pub const RING_SLOTS: usize = 32;

/// The most actions waiting to be sent: as many as one ring holds.
pub const OUTBOX: usize = RING_SLOTS;

/// The most fields a form may have for its UI ([`FormUi`]) to keep copies
/// of: field ids 1 to `MAX_FIELDS`.
pub const MAX_FIELDS: usize = 8;

/// The longest text value a form's field may hold for its UI to keep a
/// copy of, in bytes.
pub const COPY_LEN: usize = VALUE_LEN;

/// What [`FormUi::screen`] names while the kiosk is out of service.
pub const OUT_OF_SERVICE: &str = "out-of-service";

/// How a form's field is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// A text input.
    Text,
    /// A radio group: one of a few options.
    Choice,
    /// A checkbox.
    Checkbox,
}

/// Where a form's field is shown: its step's screen and the control there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// The form's field.
    pub field: FormField,
    /// The step screen that shows it.
    pub screen: ScreenId,
    /// Its control on that screen.
    pub control: UiField,
    /// What kind of control it is.
    pub kind: ControlKind,
}

/// A form field's error line: its screen, the group that shows it and the
/// slot for the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorLine {
    /// The form's field.
    pub field: FormField,
    /// The step screen that shows it.
    pub screen: ScreenId,
    /// The group shown with the error.
    pub group: GroupId,
    /// The slot the message goes in.
    pub slot: SlotId,
}

/// What a button asks the application to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Start a session.
    Start,
    /// Go on from a step.
    Next,
    /// Go back a screen.
    Back,
    /// End the session.
    Cancel,
    /// Send the answers.
    Submit,
    /// Send them again.
    Retry,
    /// End the confirmation.
    Finish,
    /// The person is still there.
    StillHere,
    /// A demonstration's *service down*: ask that this session's
    /// submissions fail, then send the answers.
    SubmitFailing,
    /// Show a screen of fixed text, outside any session - the welcome
    /// screen's *Signing out*, and back: nothing is sent.
    Show(ScreenId),
    /// Start a session with the example's fictional details already in
    /// each step - a presenter's default, so nobody types real details
    /// ([`Screens::example_value`]).
    StartExample,
}

/// A fictional answer a step shows already when a session starts with
/// example details ([`Request::StartExample`]): sent to the
/// application as if typed, chosen or ticked, and wiped as any answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleValue {
    /// A text field's.
    Text(&'static str),
    /// A choice field's option.
    Choice(u8),
    /// A checkbox's.
    Checked(bool),
}

/// A form kiosk's screens: what [`FormUi`] needs to show one
/// example's form - its screens by role, the step screens in the form's
/// order, where each field is shown, what each button asks for, and how
/// the screens that are not steps are filled. [`Reference`] is the
/// reference kiosk's. Every text is fixed: none holds an answer but the
/// review's, filled from the UI's own copies.
pub trait Screens {
    /// The welcome screen.
    const WELCOME: ScreenId;
    /// The review: every answer, from the copies.
    const REVIEW: ScreenId;
    /// While the answers are on their way.
    const SUBMITTING: ScreenId;
    /// The answers were not taken: why, the tries left, and retry when it
    /// may help.
    const FAILED: ScreenId;
    /// The confirmation, which clears itself.
    const SUBMITTED: ScreenId;
    /// The idle warning before the privacy reset.
    const IDLE: ScreenId;
    /// The step screens, by the form's step index.
    const STEPS: &'static [ScreenId];
    /// Where each of the form's fields is shown. Field ids run from 1 to
    /// at most [`MAX_FIELDS`]; a text field holds at most [`COPY_LEN`]
    /// bytes.
    const CONTROLS: &'static [Control];
    /// The fields' error lines.
    const ERROR_LINES: &'static [ErrorLine];
    /// The idle warning's *Yes, I am here*: it takes the focus there, so
    /// a stray Enter keeps the session.
    const STILL_HERE: ActionId;
    /// Whether [`follow`](Self::follow) changes the screens that count
    /// down: then a tick repaints more than the seconds.
    const FOLLOWS: bool = false;

    /// A screen's file name, for the log.
    fn name(screen: ScreenId) -> &'static str;

    /// Builds `screen` into the empty `tree`.
    ///
    /// # Errors
    ///
    /// The tree's error.
    fn build(screen: ScreenId, tree: &mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>;

    /// What a screen's button asks for, and its label for the log.
    fn button(screen: ScreenId, action: ActionId) -> Option<(Request, &'static str)>;

    /// What a field's error line says about `issue`: fixed text, never the
    /// value.
    fn error_message(field: FormField, issue: Issue) -> &'static str;

    /// The slot that counts down the seconds left on `screen`: on
    /// the idle warning and the confirmation.
    fn countdown_slot(screen: ScreenId) -> Option<SlotId>;

    /// The slot on `screen` that shows what the service issued - a pass or
    /// queue number ([`Issued`]) - if it shows one.
    fn issued_slot(_screen: ScreenId) -> Option<SlotId> {
        None
    }

    /// The fictional answer a field shows already in a session started
    /// with example details ([`Request::StartExample`]), if it has
    /// one. Never a real person's.
    fn example_value(_field: FormField) -> Option<ExampleValue> {
        None
    }

    /// Fills a screen that is neither a step nor counts down - the review,
    /// from the copies; the failed screen, from the failure - once built.
    ///
    /// # Errors
    ///
    /// The tree's error.
    fn fill(
        screen: ScreenId,
        tree: &mut Tree<CAPACITY, TEXT>,
        copies: &Copies,
        failed: Option<ShowFailed>,
    ) -> Result<(), UiError>;

    /// Fills a screen that counts down from what the service issued and
    /// the time left, in milliseconds, each time either changes:
    /// for a screen that follows its countdown - the pump's litres and
    /// pounds, counting up as the confirmation counts down. Through
    /// [`Follow`], which repaints only what changes. The default does
    /// nothing.
    ///
    /// # Errors
    ///
    /// The tree's error.
    fn follow(
        _screen: ScreenId,
        _follow: &mut Follow<'_>,
        _issued: Option<&Issued>,
        _remaining: Option<u32>,
    ) -> Result<(), UiError> {
        Ok(())
    }
}

/// What [`Screens::follow`] may change on the screen showing: its
/// slots, progress bars and groups. A slot's new text repaints only the
/// part of it that changed, unless it no longer fits; a group
/// shown or hidden lays the screen out again.
pub struct Follow<'t> {
    tree: &'t mut Tree<CAPACITY, TEXT>,
    metrics: &'t TextMetrics,
    relayout: bool,
}

impl Follow<'_> {
    /// Shows `text` in `slot`: nothing is repainted if it shows already.
    ///
    /// # Errors
    ///
    /// The tree's error: an unknown slot, or text it cannot show.
    pub fn set_slot(&mut self, slot: SlotId, text: &str) -> Result<(), UiError> {
        let node = self.tree.find_slot(slot).and_then(|id| self.tree.get(id));
        if let Some(node) = node
            && matches!(node.kind(), NodeKind::Slot { text: shown, .. } if shown.as_str() == text)
        {
            return Ok(());
        }
        let room = node.map(|node| (node.rect().size.width, node.style().text));
        self.tree.set_slot(slot, text)?;
        if let Some((width, style)) = room
            && self.metrics.for_style(style).text_width(text) > width
        {
            self.relayout = true;
        }
        Ok(())
    }

    /// Shows `done` of `whole` on the progress bar bound to `slot`:
    /// only the part of the bar that changes is repainted.
    ///
    /// # Errors
    ///
    /// [`UiError::UnknownSlot`] for a bar the screen does not have.
    pub fn set_progress(&mut self, slot: SlotId, done: u32, whole: u32) -> Result<(), UiError> {
        self.tree.set_progress(slot, done, whole)
    }

    /// Shows or hides `group`.
    ///
    /// # Errors
    ///
    /// [`UiError::UnknownGroup`] for a group the screen does not have.
    pub fn set_visible(&mut self, group: GroupId, visible: bool) -> Result<(), UiError> {
        if self.tree.visible(group) != Some(visible) {
            self.tree.set_visible(group, visible)?;
            self.relayout = true;
        }
        Ok(())
    }
}

/// Wraps each screen as it is built: given the tree, the screen's root and
/// its name, builds what goes around it - a bar with Home, say - and
/// returns the new root: a container whose first child is the frame's own
/// part, the screen after it. Its controls come first in the focus order;
/// the first control of the screen itself takes the focus when it shows -
/// on a screen with none, nothing does. Its action, slot, group and field
/// numbers must differ from every screen's.
pub type Frame = fn(&mut Tree<CAPACITY, TEXT>, NodeId, &'static str) -> Result<NodeId, UiError>;

/// Builds screen `screen` into the empty `tree`, inside `frame` if there is
/// one.
fn build_framed<S: Screens>(
    screen: ScreenId,
    tree: &mut Tree<CAPACITY, TEXT>,
    frame: Option<Frame>,
) -> Result<NodeId, UiError> {
    let body = S::build(screen, tree)?;
    match frame {
        Some(frame) => frame(tree, body, S::name(screen)),
        None => Ok(body),
    }
}

/// Whether `id` is `part` or lies below it.
fn within(tree: &Tree<CAPACITY, TEXT>, id: NodeId, part: Option<NodeId>) -> bool {
    let mut at = Some(id);
    // Each step goes one level up; the bound only guards against corrupt
    // links.
    for _ in 0..=nowayos_ui::MAX_DEPTH {
        match at {
            Some(node) if Some(node) == part => return true,
            Some(node) => at = tree.get(node).and_then(nowayos_ui::Node::parent),
            None => return false,
        }
    }
    false
}

/// The form's field a screen's control holds.
fn form_field<S: Screens>(screen: ScreenId, control: UiField) -> Option<FormField> {
    S::CONTROLS
        .iter()
        .find(|c| c.screen == screen && c.control == control)
        .map(|c| c.field)
}

/// Where a form's field is shown.
fn control<S: Screens>(field: FormField) -> Option<Control> {
    S::CONTROLS.iter().find(|c| c.field == field).copied()
}

/// A form's field's error line, if it has one.
fn error_line<S: Screens>(field: FormField) -> Option<ErrorLine> {
    S::ERROR_LINES.iter().find(|e| e.field == field).copied()
}

/// The form field's index into per-field arrays, from 0.
fn index(field: FormField) -> Option<usize> {
    usize::from(field.0)
        .checked_sub(1)
        .filter(|&i| i < MAX_FIELDS)
}

/// The UI's copy of one field's answer.
#[derive(Default)]
struct FieldCopy {
    text: BoundedStr<COPY_LEN>,
    choice: Option<u8>,
    checked: bool,
}

impl FieldCopy {
    fn is_wiped(&self) -> bool {
        self.text.is_zeroed() && self.choice.is_none() && !self.checked
    }
}

impl Wipe for FieldCopy {
    fn wipe(&mut self) {
        self.text.wipe();
        self.choice = None;
        self.checked = false;
    }
}

/// The UI's copies of the answers, by field: what a step shows again, and
/// what the review lists.
#[derive(Default)]
pub struct Copies {
    fields: [FieldCopy; MAX_FIELDS],
}

impl Copies {
    fn get(&self, field: FormField) -> Option<&FieldCopy> {
        index(field).and_then(|i| self.fields.get(i))
    }

    fn get_mut(&mut self, field: FormField) -> Option<&mut FieldCopy> {
        index(field).and_then(|i| self.fields.get_mut(i))
    }

    /// The copy of a text field: empty if none.
    #[must_use]
    pub fn text(&self, field: FormField) -> &str {
        self.get(field).map_or("", |copy| copy.text.as_str())
    }

    /// The copy of a choice field: the option chosen, if any.
    #[must_use]
    pub fn choice(&self, field: FormField) -> Option<u8> {
        self.get(field).and_then(|copy| copy.choice)
    }

    /// The copy of a checkbox field.
    #[must_use]
    pub fn checked(&self, field: FormField) -> bool {
        self.get(field).is_some_and(|copy| copy.checked)
    }

    /// Drops the copy of `field`, zeroing a text in place.
    fn clear(&mut self, field: FormField) {
        if let Some(copy) = self.get_mut(field) {
            copy.wipe();
        }
    }

    /// Whether every copy is empty, every text's storage zeroed.
    fn is_wiped(&self) -> bool {
        self.fields.iter().all(FieldCopy::is_wiped)
    }
}

impl Wipe for Copies {
    fn wipe(&mut self) {
        for copy in &mut self.fields {
            copy.wipe();
        }
    }
}

/// A button an input event pressed ([`KioskUi::handle_event`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// One of the kiosk's own, by its label; its action is queued.
    Button(&'static str),
    /// One the kiosk does not know - in a [`Frame`] - by its action, for the
    /// caller to act on.
    Other(ActionId),
}

/// What a kiosk UI PD drives: a form kiosk's UI ([`FormUi`]: the
/// reference kiosk's, [`KioskUi`]), or a UI that holds several among
/// others, as the showcase's does. The PD around it moves input events, and
/// reports and actions to and from each of [`APPS`](Self::APPS) application
/// PDs, and draws [`tree`](Self::tree).
pub trait KioskScreens {
    /// How many application PDs it talks to, each over its own rings,
    /// numbered from 0.
    const APPS: usize;
    /// Whether the UI PD times how long each countdown tick takes to draw,
    /// with the timer, and says so: only screens whose system
    /// grants the UI the timer - the showcase's - may. Off by default.
    const TIMES_TICKS: bool = false;
    /// The tree to draw.
    fn tree(&self) -> &Tree<CAPACITY, TEXT>;
    /// The tree to draw, to draw only what changed and then mark it drawn
    /// (`nowayos-ui-loop`'s `draw_dirty`).
    fn tree_mut(&mut self) -> &mut Tree<CAPACITY, TEXT>;
    /// Lays every screen out in the display's `viewport` with `metrics` -
    /// the theme's at the display's device scale - from now on, the one
    /// showing at once, keeping what it holds: what the UI PD calls
    /// when the display announces its layout.
    ///
    /// # Errors
    ///
    /// The tree's error, if a screen cannot be laid out there.
    fn resize(&mut self, viewport: Rect, metrics: TextMetrics) -> Result<(), UiError>;
    /// The screen showing, by name, for the log.
    fn screen(&self) -> &'static str;
    /// Applies one input event; the label of a button it pressed, for the
    /// log - never what was typed.
    fn handle(&mut self, event: InputEvent) -> Option<&'static str>;
    /// Ends a batch of input.
    fn end_input(&mut self);
    /// Decodes and applies one report from application `app`, failing
    /// closed.
    ///
    /// # Errors
    ///
    /// As [`FormUi::receive_report`], and [`KioskUiError::UnknownApp`].
    fn receive_report(&mut self, app: usize, record: &[u8]) -> Result<(), KioskUiError>;
    /// The actions waiting to be sent to application `app`, oldest first.
    fn actions(&self, app: usize) -> impl Iterator<Item = &Action>;
    /// Forgets the oldest `count` actions to application `app`, once they
    /// are in its ring.
    fn sent(&mut self, app: usize, count: usize);
    /// Actions dropped so far.
    fn dropped(&self) -> u32;
    /// Wipes everything, ends any session and shows the first screen.
    fn reset(&mut self);
    /// The seconds left of the screen counting down, if one shows.
    fn seconds_left(&self) -> Option<u32>;
    /// Where the countdown's digits are drawn - their slot, which takes the
    /// rest of its line - if a screen counting down shows: what a
    /// tick alone repaints.
    fn countdown_area(&self) -> Option<Rect>;
    /// Whether the screen counting down also follows its countdown
    /// ([`Screens::follow`]): a tick then repaints its live figures
    /// too, and the whole screen when a stage changes.
    fn follows_countdown(&self) -> bool {
        false
    }
    /// How long the screen showing may go with no input, in milliseconds,
    /// before [`idle`](Self::idle) is called, if it wants that: the
    /// showcase's launcher, for its attract loop. The UI PD asks the timer
    /// to wake it only for screens that do; a form kiosk's never does - its
    /// time comes from its application.
    fn idle_after(&self) -> Option<u32> {
        None
    }
    /// That long has passed with no input: whether the screen changed.
    fn idle(&mut self) -> bool {
        false
    }
    /// The theme the screen showing is drawn in, with its name for the log:
    /// NoWayOS's light theme, unless the screens say otherwise -
    /// the showcase draws each example in its brand's. The UI PD draws in
    /// it, and when the name changes, lays every screen out again in its
    /// metrics.
    fn theme(&self) -> (&'static str, Theme<Rgb888>) {
        ("light", Theme::light())
    }
}

/// One form kiosk talks to one application, number 0.
impl<S: Screens> KioskScreens for FormUi<S> {
    const APPS: usize = 1;

    fn tree(&self) -> &Tree<CAPACITY, TEXT> {
        Self::tree(self)
    }
    fn tree_mut(&mut self) -> &mut Tree<CAPACITY, TEXT> {
        Self::tree_mut(self)
    }
    fn resize(&mut self, viewport: Rect, metrics: TextMetrics) -> Result<(), UiError> {
        Self::resize(self, viewport, metrics)
    }
    fn screen(&self) -> &'static str {
        Self::screen(self)
    }
    fn handle(&mut self, event: InputEvent) -> Option<&'static str> {
        Self::handle(self, event)
    }
    fn end_input(&mut self) {
        Self::end_input(self);
    }
    fn receive_report(&mut self, app: usize, record: &[u8]) -> Result<(), KioskUiError> {
        if app != 0 {
            self.restart();
            return Err(KioskUiError::UnknownApp(app));
        }
        Self::receive_report(self, record)
    }
    fn actions(&self, app: usize) -> impl Iterator<Item = &Action> {
        Self::actions(self).filter(move |_| app == 0)
    }
    fn sent(&mut self, app: usize, count: usize) {
        if app == 0 {
            Self::sent(self, count);
        }
    }
    fn dropped(&self) -> u32 {
        Self::dropped(self)
    }
    fn reset(&mut self) {
        Self::reset(self);
    }
    fn seconds_left(&self) -> Option<u32> {
        Self::seconds_left(self)
    }
    fn countdown_area(&self) -> Option<Rect> {
        Self::countdown_area(self)
    }
    fn follows_countdown(&self) -> bool {
        S::FOLLOWS && self.countdown_slot().is_some()
    }
}

/// Why [`FormUi::report`] could not show what a report asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KioskUiError {
    /// The tree refused a screen: it could not be built, filled or laid
    /// out. The welcome screen is showing instead, every copy wiped.
    Ui(UiError),
    /// The application named a step the form does not have. The welcome
    /// screen is showing instead, every copy wiped.
    UnknownStep(u16),
    /// A report could not be decoded. The welcome screen is showing
    /// instead, every copy wiped.
    Report(ContractError),
    /// A report came from an application the UI does not talk to: a UI
    /// PD's fault. The welcome screen is showing instead, every copy
    /// wiped.
    UnknownApp(usize),
}

impl fmt::Display for KioskUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ui(error) => write!(f, "a screen could not be shown: {error}"),
            Self::UnknownStep(index) => write!(
                f,
                "the application named step {index}, which the form lacks"
            ),
            Self::Report(error) => write!(f, "a report from the application refused: {error}"),
            Self::UnknownApp(app) => write!(f, "a report from application {app}, which it lacks"),
        }
    }
}

impl core::error::Error for KioskUiError {}

impl From<UiError> for KioskUiError {
    fn from(error: UiError) -> Self {
        Self::Ui(error)
    }
}

/// A form kiosk's UI, showing the screens `S` (see the [module](self)).
pub struct FormUi<S> {
    tree: Tree<CAPACITY, TEXT>,
    nav: Navigator<1>,
    metrics: TextMetrics,
    copies: Copies,
    /// Each field's last action's `seq`, by [`index`].
    last: [Option<u32>; MAX_FIELDS],
    /// Text fields whose value changed since it was last queued.
    changed: [bool; MAX_FIELDS],
    /// The last action's `seq`.
    seq: u32,
    /// Actions not yet sent, oldest first (`None` never stays in it).
    outbox: BoundedVec<Option<Action>, OUTBOX>,
    /// Actions the outbox had no room for.
    dropped: u32,
    /// The person did something this batch.
    active: bool,
    /// An action was queued this batch.
    queued: bool,
    /// The failure the failed screen shows.
    failed: Option<ShowFailed>,
    /// The seconds left the screen counting down shows - the idle warning
    /// or the confirmation - from the application's last report of them;
    /// `None` until it has said.
    seconds_left: Option<u32>,
    /// The time left that report gave, in milliseconds.
    remaining: Option<u32>,
    /// What the service issued for the answers, which the confirmation
    /// shows.
    issued: Option<Issued>,
    /// The session started with example details.
    example: bool,
    /// The fields whose example value each step has shown already: once
    /// each, so a person who clears one is not given it again.
    examples_given: [bool; MAX_FIELDS],
    /// Where the pointer last was.
    pointer: Point,
    /// What goes around each screen, if anything.
    frame: Option<Frame>,
    /// Out of service, and why: the kiosk's own buttons do
    /// nothing until the application shows another screen.
    out_of_service: Option<OutOfServiceReason>,
    screens: PhantomData<fn() -> S>,
}

/// The reference kiosk's UI: [`FormUi`] on the reference screens.
pub type KioskUi = FormUi<Reference>;

impl<S: Screens> FormUi<S> {
    /// The UI at its welcome screen, laid out for a [`WIDTH`] × [`HEIGHT`]
    /// screen with `metrics` (the theme's).
    ///
    /// # Errors
    ///
    /// The tree's error, if the welcome screen cannot be built or laid out.
    pub fn new(metrics: TextMetrics) -> Result<Self, UiError> {
        Self::with_frame(metrics, None)
    }

    /// As [`new`](Self::new), each screen built inside `frame` (see
    /// [`Frame`]): what the showcase does to put a bar with Home above
    /// every screen.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn framed(metrics: TextMetrics, frame: Frame) -> Result<Self, UiError> {
        Self::with_frame(metrics, Some(frame))
    }

    fn with_frame(metrics: TextMetrics, frame: Option<Frame>) -> Result<Self, UiError> {
        let mut ui = Self {
            tree: Tree::new(),
            nav: Navigator::new(S::WELCOME, Rect::new(0, 0, WIDTH, HEIGHT), metrics),
            metrics,
            copies: Copies::default(),
            last: [None; MAX_FIELDS],
            changed: [false; MAX_FIELDS],
            seq: 0,
            outbox: BoundedVec::new(),
            dropped: 0,
            active: false,
            queued: false,
            failed: None,
            seconds_left: None,
            remaining: None,
            issued: None,
            example: false,
            examples_given: [false; MAX_FIELDS],
            pointer: Point::new(0, 0),
            frame,
            out_of_service: None,
            screens: PhantomData,
        };
        ui.nav
            .show_start(&mut ui.tree, |s, t| build_framed::<S>(s, t, frame))?;
        ui.focus_screen();
        Ok(ui)
    }

    /// The tree, to draw only what changed and then mark it drawn
    /// (`Tree::clear_dirty`, through `nowayos-ui-loop`'s `draw_dirty`).
    pub const fn tree_mut(&mut self) -> &mut Tree<CAPACITY, TEXT> {
        &mut self.tree
    }

    /// Lays the screens out in the display's `viewport` with `metrics` -
    /// the theme's at the display's device scale - from now on, the screen
    /// showing at once, keeping what it holds: what the UI PD calls
    /// when the display announces its layout, so the kiosk is drawn at the
    /// monitor's own pixels.
    ///
    /// # Errors
    ///
    /// The tree's error, if the screen cannot be laid out there.
    pub fn resize(&mut self, viewport: Rect, metrics: TextMetrics) -> Result<(), UiError> {
        self.metrics = metrics;
        self.nav.resize(&mut self.tree, viewport, metrics)
    }

    /// The tree to draw.
    #[must_use]
    pub const fn tree(&self) -> &Tree<CAPACITY, TEXT> {
        &self.tree
    }

    /// Records that the areas of the screen that changed have been drawn
    /// (`Tree::clear_dirty`): what a renderer that draws only what changed
    /// does after each frame.
    pub fn clear_dirty(&mut self) {
        self.tree.clear_dirty();
    }

    /// The screen showing: its file's name - or `out-of-service`, while
    /// out of service.
    #[must_use]
    pub fn screen(&self) -> &'static str {
        if self.out_of_service.is_some() {
            return OUT_OF_SERVICE;
        }
        S::name(self.nav.current())
    }

    /// Why the kiosk is out of service, if the application said so last.
    #[must_use]
    pub const fn out_of_service(&self) -> Option<OutOfServiceReason> {
        self.out_of_service
    }

    /// The actions waiting to be sent to the application, oldest first.
    pub fn actions(&self) -> impl Iterator<Item = &Action> {
        self.outbox.iter().flatten()
    }

    /// Forgets the oldest `count` actions, once they are in the ring,
    /// wiping each one's text in place first.
    pub fn sent(&mut self, count: usize) {
        for _ in 0..count {
            if let Some(oldest) = self.outbox.as_mut_slice().first_mut() {
                oldest.wipe();
            }
            if self.outbox.remove(0).is_none() {
                break;
            }
        }
    }

    /// Actions dropped so far because the outbox was full.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Whether the UI holds nothing a person entered: every copy zeroed,
    /// the tree wiped (`Tree::is_wiped`) - but for a countdown's digits,
    /// which are time, not typing - and no action waiting that carries
    /// text. For tests; it tells one bit.
    #[must_use]
    pub fn holds_nothing(&self) -> bool {
        self.copies.is_wiped()
            && self.tree_holds_nothing()
            && !self
                .actions()
                .any(|action| matches!(action, Action::SetText(set) if !set.text().is_empty()))
    }

    /// Whether the tree is wiped, a countdown's digits apart: on a screen
    /// that counts down, every node but its countdown slot as
    /// `Tree::is_wiped` asks. Those screens hide nothing in a group, so
    /// every node shows.
    fn tree_holds_nothing(&self) -> bool {
        if self.tree.is_wiped() {
            return true;
        }
        let screen = self.nav.current();
        let Some(countdown) = self
            .countdown_slot()
            .and_then(|slot| self.tree.find_slot(slot))
        else {
            return false;
        };
        // What the service issued is not typing either.
        let issued = S::issued_slot(screen).and_then(|slot| self.tree.find_slot(slot));
        self.tree.paint_order().all(|(id, node)| match node.kind() {
            NodeKind::Slot { text, .. } => {
                id == countdown || Some(id) == issued || text.is_zeroed()
            }
            NodeKind::TextInput { value, .. } => value.is_zeroed(),
            NodeKind::Checkbox { checked, .. } => !checked,
            NodeKind::RadioGroup { selected, .. } | NodeKind::ListPicker { selected, .. } => {
                selected.is_none()
            }
            NodeKind::Keyboard { shift, .. } => !shift,
            NodeKind::Group { .. } => false,
            _ => true,
        })
    }

    /// The slot that counts down on the screen showing, if it counts down.
    fn countdown_slot(&self) -> Option<SlotId> {
        S::countdown_slot(self.nav.current())
    }

    /// Applies one input event. Returns the label of the button it
    /// pressed, if any, for the log - never what was typed. Call
    /// [`end_input`](Self::end_input) after each batch.
    pub fn handle(&mut self, event: InputEvent) -> Option<&'static str> {
        match self.handle_event(event)? {
            Pressed::Button(label) => Some(label),
            Pressed::Other(_) => None,
        }
    }

    /// As [`handle`](Self::handle), also reporting a button the kiosk does
    /// not know - one in a [`Frame`] - for the caller to act on.
    pub fn handle_event(&mut self, event: InputEvent) -> Option<Pressed> {
        let screen = self.nav.current();
        if let InputEvent::PointerMove { x, y } | InputEvent::Touch { x, y, .. } = event {
            self.pointer = Point::new(x, y);
        }
        if self.shows_presence(event) {
            self.active = true;
        }
        match self.tree.handle(event)? {
            UiEvent::ValueChanged(field) => {
                if let Some(i) = form_field::<S>(screen, field).and_then(index)
                    && let Some(changed) = self.changed.get_mut(i)
                {
                    *changed = true;
                }
                None
            }
            UiEvent::Toggled { field, checked } => {
                if let Some(field) = form_field::<S>(screen, field) {
                    self.queue_changed();
                    self.queue_for(field, |seq| {
                        Action::SetChecked(SetChecked {
                            seq,
                            field,
                            checked,
                        })
                    });
                }
                None
            }
            UiEvent::Selected { field, option } => {
                if let Some(field) = form_field::<S>(screen, field) {
                    self.queue_changed();
                    self.queue_for(field, |seq| Action::Select(Select { seq, field, option }));
                }
                None
            }
            // A kiosk's screens hold no virtual list.
            UiEvent::ListMoved(_) => None,
            UiEvent::Action(action) => {
                let Some((request, label)) = S::button(screen, action) else {
                    return Some(Pressed::Other(action));
                };
                // Out of service, the kiosk takes no input: a frame's own
                // buttons - the showcase's Home - still work.
                if self.out_of_service.is_some() {
                    return None;
                }
                let make: fn(u32) -> Action = match request {
                    Request::Show(next) => {
                        // Fixed text, no answer, nothing sent: a screen
                        // that cannot be shown leaves the one showing.
                        let _shown = self.show(next);
                        return Some(Pressed::Button(label));
                    }
                    Request::Start | Request::StartExample => |seq| Action::Start(Start { seq }),
                    Request::Next => |seq| Action::Next(Next { seq }),
                    Request::Back => |seq| Action::Back(Back { seq }),
                    Request::Cancel => |seq| Action::Cancel(Cancel { seq }),
                    Request::Submit | Request::SubmitFailing => {
                        |seq| Action::Submit(Submit { seq })
                    }
                    Request::Retry => |seq| Action::Retry(Retry { seq }),
                    Request::Finish => |seq| Action::Finish(Finish { seq }),
                    Request::StillHere => |seq| Action::StillHere(StillHere { seq }),
                };
                self.queue_changed();
                if request == Request::StartExample {
                    self.example = true;
                }
                if request == Request::SubmitFailing {
                    self.queue(|seq| Action::SimulateFailure(SimulateFailure { seq }));
                }
                self.queue(make);
                Some(Pressed::Button(label))
            }
            UiEvent::FocusChanged(_) | UiEvent::InputFull(_) => None,
        }
    }

    /// Whether `event` shows the person is there, before the tree acts on
    /// it: a key, a press or a touch. On the idle warning, not a key that
    /// moves focus (Tab, Shift-Tab, Up, Down), does nothing there (Left,
    /// Right, a modifier) or presses the focused button (Enter, Space), nor
    /// a press or touch on a button, which acts on its release: so *Clear
    /// it now* can be reached and pressed from the keyboard, or clicked,
    /// without bringing the step back first.
    fn shows_presence(&self, event: InputEvent) -> bool {
        let idle = self.nav.current() == S::IDLE;
        match event {
            InputEvent::KeyDown(key) => {
                let moves_or_presses = key.is_modifier()
                    || [
                        KeyCode::TAB,
                        KeyCode::UP,
                        KeyCode::DOWN,
                        KeyCode::LEFT,
                        KeyCode::RIGHT,
                        KeyCode::ENTER,
                        KeyCode::SPACE,
                    ]
                    .contains(&key);
                !(idle && moves_or_presses)
            }
            InputEvent::PointerButton { down: true, .. } => {
                !(idle && self.tree.control_at(self.pointer).is_some())
            }
            InputEvent::Touch {
                phase: TouchPhase::Start,
                x,
                y,
                ..
            } => !(idle && self.tree.control_at(Point::new(x, y)).is_some()),
            _ => false,
        }
    }

    /// Ends a batch of input: queues each text field's value that changed,
    /// and, if the person did something on a screen that waits for them
    /// but nothing was queued, [`StillHere`].
    pub fn end_input(&mut self) {
        self.queue_changed();
        let screen = self.nav.current();
        let waits = S::STEPS.contains(&screen) || [S::REVIEW, S::FAILED, S::IDLE].contains(&screen);
        if self.active && !self.queued && waits {
            self.queue(|seq| Action::StillHere(StillHere { seq }));
        }
        self.active = false;
        self.queued = false;
    }

    /// Decodes one report from the application, already copied out of its
    /// ring, and applies it.
    ///
    /// # Errors
    ///
    /// [`KioskUiError::Report`] if it cannot be decoded: a faulty or
    /// hostile application. The UI fails closed - it wipes everything,
    /// shows the welcome screen and asks the application to end any session
    /// ([`Abandon`]). Otherwise as [`report`](Self::report).
    pub fn receive_report(&mut self, record: &[u8]) -> Result<(), KioskUiError> {
        match showcase_kiosk_contract::decode_report(record) {
            Ok(report) => self.report(&report),
            Err(error) => {
                self.restart();
                Err(KioskUiError::Report(error))
            }
        }
    }

    /// Applies one report from the application.
    ///
    /// # Errors
    ///
    /// [`KioskUiError`] if the screen it names cannot be shown: the UI has
    /// then wiped everything and shows the welcome screen.
    pub fn report(&mut self, report: &Report) -> Result<(), KioskUiError> {
        // Any other screen the application names puts the kiosk back in
        // service.
        if report.is_screen() {
            self.out_of_service = match report {
                Report::OutOfService(out) => Some(out.reason),
                _ => None,
            };
        }
        let shown = match report {
            Report::ShowWelcome(_) => self.show(S::WELCOME),
            Report::ShowStep(step) => match S::STEPS.get(usize::from(step.index)) {
                Some(&screen) => self.show(screen),
                None => {
                    self.restart();
                    return Err(KioskUiError::UnknownStep(step.index));
                }
            },
            Report::ShowReview(_) => self.show(S::REVIEW),
            Report::ShowSubmitting(_) => self.show(S::SUBMITTING),
            Report::ShowSubmitted(_) => {
                // What was issued and its seconds left follow, in reports
                // of their own.
                self.seconds_left = None;
                self.remaining = None;
                self.issued = None;
                self.show(S::SUBMITTED)
            }
            Report::ShowFailed(failure) => {
                self.failed = Some(*failure);
                self.show(S::FAILED)
            }
            Report::IdleWarning(warning) => {
                self.seconds_left = Some(warning.remaining.div_ceil(1_000));
                self.remaining = Some(warning.remaining);
                self.show(S::IDLE)
            }
            Report::Countdown(countdown) => self.count_down(countdown.remaining),
            Report::Issued(issued) => self.show_issued(*issued),
            Report::ValueRefused(refused) => {
                if let Some(i) = index(refused.field)
                    && self.last.get(i).copied().flatten() == Some(refused.seq)
                {
                    self.drop_value(refused.field);
                }
                self.show_error(refused.field, refused.issue)
            }
            Report::Incomplete(incomplete) => self.show_error(incomplete.field, incomplete.issue),
            // The application follows a refusal with the screen to show.
            Report::Refused(_) => Ok(()),
            Report::SessionEnded(_) => {
                self.wipe();
                Ok(())
            }
            // Out of service: no session opens, and the kiosk's
            // own buttons do nothing. A fixed out-of-service screen is
            // still to come; until then the welcome screen, every copy
            // wiped - and the application refuses a start.
            Report::OutOfService(_) => {
                self.wipe();
                self.issued = None;
                self.show(S::WELCOME)
            }
        };
        shown.map_err(|error| {
            self.restart();
            KioskUiError::Ui(error)
        })
    }

    /// Wipes every answer the UI holds, in place, and forgets every action
    /// not yet sent: the session has ended.
    fn wipe(&mut self) {
        self.copies.wipe();
        self.tree.wipe_session();
        self.outbox.wipe();
        self.last = [None; MAX_FIELDS];
        self.changed = [false; MAX_FIELDS];
        self.failed = None;
        self.example = false;
        self.examples_given = [false; MAX_FIELDS];
    }

    /// Wipes everything the UI holds, shows the welcome screen and asks the
    /// application to end any session: what a UI PD does before it gives
    /// up.
    pub fn reset(&mut self) {
        self.restart();
    }

    /// After a failure: wipes everything, asks the application to end any
    /// session, whatever its phase ([`Abandon`]) - so both ends close, and
    /// the next person never joins this one's - and shows the welcome
    /// screen.
    fn restart(&mut self) {
        self.wipe();
        self.issued = None;
        let _ = self.queue(|seq| Action::Abandon(Abandon { seq }));
        let frame = self.frame;
        let _shown = self
            .nav
            .reset(&mut self.tree, |s, t| build_framed::<S>(s, t, frame));
        self.focus_screen();
    }

    /// With a frame, moves the focus past the frame's controls - Home, and
    /// any other its bar has - to the screen's first. On a screen
    /// with no control of its own nothing takes it, as without a frame, so
    /// a stray Enter there does nothing.
    fn focus_screen(&mut self) {
        if self.frame.is_none() {
            return;
        }
        // The frame's own part: the new root's first child.
        let part = self
            .tree
            .root()
            .and_then(|root| self.tree.children(root).next());
        let first = self
            .tree
            .paint_order()
            .filter(|&(id, node)| node.is_focusable() && self.tree.is_shown(id))
            .map(|(id, _)| id)
            .find(|&id| !within(&self.tree, id, part));
        let _ = self.tree.set_focus(first);
    }

    /// Queues an action, numbering it.
    fn queue(&mut self, make: impl FnOnce(u32) -> Action) -> Option<u32> {
        self.seq = self.seq.wrapping_add(1);
        self.queued = true;
        if self.outbox.push(Some(make(self.seq))).is_err() {
            self.dropped = self.dropped.saturating_add(1);
            return None;
        }
        Some(self.seq)
    }

    /// Queues an action for `field`, remembering it as the field's last.
    fn queue_for(&mut self, field: FormField, make: impl FnOnce(u32) -> Action) {
        if let Some(seq) = self.queue(make)
            && let Some(last) = index(field).and_then(|i| self.last.get_mut(i))
        {
            *last = Some(seq);
        }
    }

    /// Queues the value of each text field that changed, as the screen
    /// showing holds it.
    fn queue_changed(&mut self) {
        for control in S::CONTROLS {
            if control.kind != ControlKind::Text {
                continue;
            }
            let field = control.field;
            let Some(changed) = index(field).and_then(|i| self.changed.get_mut(i)) else {
                continue;
            };
            if !core::mem::replace(changed, false) || control.screen != self.nav.current() {
                continue;
            }
            let text = self.tree.value(control.control).unwrap_or_default();
            // A screen's input holds no more than its field allows, and
            // every field allows less than a `SetText` carries.
            let Ok(record) = SetText::new(0, field, text) else {
                continue;
            };
            self.queue_for(field, |seq| {
                let mut record = record;
                record.seq = seq;
                Action::SetText(record)
            });
        }
    }

    /// Drops a refused value: the UI's copy, and the control's value if its
    /// screen is showing.
    fn drop_value(&mut self, field: FormField) {
        self.copies.clear(field);
        let Some(control) = control::<S>(field) else {
            return;
        };
        if control.screen != self.nav.current() {
            return;
        }
        let _cleared = match control.kind {
            ControlKind::Choice => self.tree.set_selected(control.control, None),
            ControlKind::Checkbox => self.tree.set_checked(control.control, false),
            ControlKind::Text => self.tree.set_value(control.control, ""),
        };
    }

    /// Shows `field`'s error line, saying what `issue` asks for, if its
    /// screen is showing and it has one.
    fn show_error(&mut self, field: FormField, issue: Issue) -> Result<(), UiError> {
        let Some(line) = error_line::<S>(field) else {
            return Ok(());
        };
        if line.screen != self.nav.current() {
            return Ok(());
        }
        self.tree
            .set_slot(line.slot, S::error_message(field, issue))?;
        self.tree.set_visible(line.group, true)?;
        self.relayout()
    }

    /// Shows `screen`, after keeping what the step showing holds, then
    /// fills it from the copies and the last report.
    fn show(&mut self, screen: ScreenId) -> Result<(), UiError> {
        self.save();
        let frame = self.frame;
        self.nav.replace(screen, &mut self.tree, |s, t| {
            build_framed::<S>(s, t, frame)
        })?;
        self.focus_screen();
        self.fill()
    }

    /// Keeps the answers the step showing holds, as its controls have them.
    fn save(&mut self) {
        let screen = self.nav.current();
        for control in S::CONTROLS.iter().filter(|c| c.screen == screen) {
            let tree = &self.tree;
            let Some(copy) = self.copies.get_mut(control.field) else {
                continue;
            };
            match control.kind {
                ControlKind::Text => {
                    copy.text = copy_text(tree.value(control.control).unwrap_or_default());
                }
                ControlKind::Choice => copy.choice = tree.selected(control.control),
                ControlKind::Checkbox => {
                    copy.checked = tree.checked(control.control) == Some(true);
                }
            }
        }
    }

    /// Fills the screen showing from the copies and the last report, and
    /// lays it out.
    fn fill(&mut self) -> Result<(), UiError> {
        let screen = self.nav.current();
        let mut number = [0u8; 10];
        if S::STEPS.contains(&screen) {
            if self.example {
                self.give_examples(screen);
            }
            for control in S::CONTROLS.iter().filter(|c| c.screen == screen) {
                let c = &self.copies;
                match control.kind {
                    ControlKind::Text => {
                        let text = c.text(control.field);
                        if !text.is_empty() {
                            self.tree.set_value(control.control, text)?;
                        }
                    }
                    ControlKind::Choice => {
                        self.tree
                            .set_selected(control.control, c.choice(control.field))?;
                    }
                    ControlKind::Checkbox => {
                        self.tree
                            .set_checked(control.control, c.checked(control.field))?;
                    }
                }
            }
        } else if let Some(slot) = S::countdown_slot(screen) {
            let seconds = self
                .seconds_left
                .map_or("", |left| digits(left, &mut number));
            self.tree.set_slot(slot, seconds)?;
            if let Some(slot) = S::issued_slot(screen) {
                let code = self.issued.as_ref().map_or("", Issued::code);
                self.tree.set_slot(slot, code)?;
            }
            // Laid out below, whatever it changes.
            let _ = self.follow()?;
            if screen == S::IDLE {
                // Focus on Yes, I am here: a stray Enter keeps the session.
                let still_here = self.tree.paint_order().find_map(|(id, node)| {
                    matches!(node.kind(), NodeKind::Button { action, .. } if *action == S::STILL_HERE)
                        .then_some(id)
                });
                if let Some(id) = still_here {
                    self.tree.set_focus(Some(id))?;
                }
            }
        } else {
            S::fill(screen, &mut self.tree, &self.copies, self.failed)?;
        }
        self.relayout()
    }

    /// Gives each field on `screen` its example value, once a session
    /// started with example details: into the copy, which the step
    /// then shows, and to the application as the person's answer would go -
    /// a text with the next batch, before its button, a choice or a tick at
    /// once.
    fn give_examples(&mut self, screen: ScreenId) {
        for control in S::CONTROLS.iter().filter(|c| c.screen == screen) {
            let field = control.field;
            let Some(i) = index(field) else { continue };
            let Some(value) = S::example_value(field) else {
                continue;
            };
            let Some(given) = self.examples_given.get_mut(i) else {
                continue;
            };
            if core::mem::replace(given, true) {
                continue;
            }
            let Some(copy) = self.copies.get_mut(field) else {
                continue;
            };
            match value {
                ExampleValue::Text(text) => {
                    copy.text = copy_text(text);
                    if let Some(changed) = self.changed.get_mut(i) {
                        *changed = true;
                    }
                }
                ExampleValue::Choice(option) => {
                    copy.choice = Some(option);
                    self.queue_for(field, |seq| Action::Select(Select { seq, field, option }));
                }
                ExampleValue::Checked(checked) => {
                    copy.checked = checked;
                    self.queue_for(field, |seq| {
                        Action::SetChecked(SetChecked {
                            seq,
                            field,
                            checked,
                        })
                    });
                }
            }
        }
    }

    /// Shows what the service issued for the answers - a pass or queue
    /// number - on the confirmation: only its slot changes.
    fn show_issued(&mut self, issued: Issued) -> Result<(), UiError> {
        self.issued = Some(issued);
        if let Some(slot) = S::issued_slot(self.nav.current()) {
            self.tree.set_slot(slot, issued.code())?;
        }
        if self.follow()? {
            self.relayout()?;
        }
        Ok(())
    }

    /// Lets the screen showing follow what was issued and the time left
    /// ([`Screens::follow`]): whether it must be laid out again.
    fn follow(&mut self) -> Result<bool, UiError> {
        let mut follow = Follow {
            tree: &mut self.tree,
            metrics: &self.metrics,
            relayout: false,
        };
        S::follow(
            self.nav.current(),
            &mut follow,
            self.issued.as_ref(),
            self.remaining,
        )?;
        Ok(follow.relayout)
    }

    /// Shows the seconds left of the screen counting down, `remaining`
    /// ticks rounded up: the time comes from the application, and
    /// the UI keeps no clock. Only the digits change, so only their slot
    /// is marked for repainting - unless they no longer fit it, when the
    /// screen is laid out again. Nothing else moves: not the focus, not
    /// what was typed. A screen that does not count down ignores it.
    fn count_down(&mut self, remaining: u32) -> Result<(), UiError> {
        let Some(slot) = self.countdown_slot() else {
            return Ok(());
        };
        let seconds = remaining.div_ceil(1_000);
        self.seconds_left = Some(seconds);
        self.remaining = Some(remaining);
        let mut number = [0u8; 10];
        let text = digits(seconds, &mut number);
        let room = self
            .tree
            .find_slot(slot)
            .and_then(|id| self.tree.get(id))
            .map_or(0, |node| node.rect().size.width);
        self.tree.set_slot(slot, text)?;
        let outgrown = self.metrics.text_width(text) > room;
        if self.follow()? || outgrown {
            self.relayout()?;
        }
        Ok(())
    }

    /// The seconds left the screen showing counts down from, if it counts
    /// down and the application has said: for the log.
    #[must_use]
    pub fn seconds_left(&self) -> Option<u32> {
        self.countdown_slot().and(self.seconds_left)
    }

    /// Where the countdown's digits are drawn - their slot - if the screen
    /// showing counts down.
    #[must_use]
    pub fn countdown_area(&self) -> Option<Rect> {
        let slot = self.countdown_slot()?;
        let id = self.tree.find_slot(slot)?;
        self.tree.get(id).map(|node| node.rect())
    }

    /// What the service issued, while the confirmation shows it.
    #[must_use]
    pub fn issued(&self) -> Option<&str> {
        S::issued_slot(self.nav.current())
            .and(self.issued.as_ref())
            .map(Issued::code)
    }

    /// Lays the screen out again, after a group is shown or hidden.
    fn relayout(&mut self) -> Result<(), UiError> {
        self.tree.layout(self.nav.viewport(), &self.metrics)
    }
}

/// `text`, which a control of at most [`COPY_LEN`] bytes held, as a copy.
fn copy_text(text: &str) -> BoundedStr<COPY_LEN> {
    BoundedStr::try_from_str(text).unwrap_or_default()
}

/// `n` in decimal digits, written into `buf`.
fn digits(mut n: u32, buf: &mut [u8; 10]) -> &str {
    let mut start = buf.len();
    loop {
        let Some(at) = start.checked_sub(1) else {
            break;
        };
        start = at;
        if let Some(slot) = buf.get_mut(at) {
            let digit = u8::try_from(n.checked_rem(10).unwrap_or(0)).unwrap_or(0);
            *slot = b'0'.saturating_add(digit);
        }
        n = n.checked_div(10).unwrap_or(0);
        if n == 0 {
            break;
        }
    }
    buf.get(start..)
        .and_then(|bytes| core::str::from_utf8(bytes).ok())
        .unwrap_or("0")
}

/// The reference kiosk's screens: [`KioskUi`] shows them,
/// on the reference form ([`crate::form`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference;

const ABOUT_YOU: ScreenId = ScreenId(1);
const QUESTION: ScreenId = ScreenId(2);
const CONFIRM: ScreenId = ScreenId(3);
const NOTICE: ScreenId = ScreenId(4);

/// Each reference screen's file name, by screen id: what the UI PD logs.
const NAMES: [&str; 10] = [
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

impl Screens for Reference {
    const WELCOME: ScreenId = ScreenId(0);
    const REVIEW: ScreenId = ScreenId(5);
    const SUBMITTING: ScreenId = ScreenId(6);
    const FAILED: ScreenId = ScreenId(7);
    const SUBMITTED: ScreenId = ScreenId(8);
    const IDLE: ScreenId = ScreenId(9);
    const STEPS: &'static [ScreenId] = &[ABOUT_YOU, QUESTION, CONFIRM, NOTICE];
    const CONTROLS: &'static [Control] = &[
        Control {
            field: NAME,
            screen: ABOUT_YOU,
            control: about_you::fields::NAME,
            kind: ControlKind::Text,
        },
        Control {
            field: REFERENCE,
            screen: ABOUT_YOU,
            control: about_you::fields::REFERENCE,
            kind: ControlKind::Text,
        },
        Control {
            field: TOPIC,
            screen: QUESTION,
            control: question::fields::TOPIC,
            kind: ControlKind::Choice,
        },
        Control {
            field: DETAILS,
            screen: QUESTION,
            control: question::fields::DETAILS,
            kind: ControlKind::Text,
        },
        Control {
            field: AGREE,
            screen: CONFIRM,
            control: confirm::fields::AGREE,
            kind: ControlKind::Checkbox,
        },
        Control {
            field: RECEIPT,
            screen: CONFIRM,
            control: confirm::fields::RECEIPT,
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
            field: REFERENCE,
            screen: ABOUT_YOU,
            group: about_you::groups::REFERENCE_ERROR,
            slot: about_you::slots::REFERENCE_ERROR,
        },
        ErrorLine {
            field: TOPIC,
            screen: QUESTION,
            group: question::groups::TOPIC_ERROR,
            slot: question::slots::TOPIC_ERROR,
        },
        ErrorLine {
            field: AGREE,
            screen: CONFIRM,
            group: confirm::groups::AGREE_ERROR,
            slot: confirm::slots::AGREE_ERROR,
        },
    ];
    const STILL_HERE: ActionId = idle::actions::STILL_HERE;

    fn name(screen: ScreenId) -> &'static str {
        NAMES
            .get(usize::from(screen.0))
            .copied()
            .unwrap_or("welcome")
    }

    fn build(screen: ScreenId, tree: &mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError> {
        match screen {
            Self::WELCOME => build_welcome(tree),
            ABOUT_YOU => build_step_text(tree),
            QUESTION => build_step_choice(tree),
            CONFIRM => build_step_checkbox(tree),
            NOTICE => build_step_notice(tree),
            Self::REVIEW => build_review(tree),
            Self::SUBMITTING => build_submitting(tree),
            Self::FAILED => build_failed(tree),
            Self::SUBMITTED => build_submitted(tree),
            _ => build_idle_warning(tree),
        }
    }

    fn button(screen: ScreenId, action: ActionId) -> Option<(Request, &'static str)> {
        Some(match (screen, action) {
            (Self::WELCOME, welcome::actions::START) => (Request::Start, "Start"),
            (ABOUT_YOU, about_you::actions::NEXT)
            | (QUESTION, question::actions::NEXT)
            | (CONFIRM, confirm::actions::NEXT) => (Request::Next, "Next"),
            (NOTICE, notice::actions::NEXT) => (Request::Next, "Continue"),
            (QUESTION, question::actions::BACK)
            | (CONFIRM, confirm::actions::BACK)
            | (NOTICE, notice::actions::BACK) => (Request::Back, "Back"),
            (Self::REVIEW, review::actions::BACK) => (Request::Back, "Change"),
            (Self::FAILED, failed::actions::BACK) => (Request::Back, "Check answers"),
            (ABOUT_YOU, about_you::actions::CANCEL)
            | (QUESTION, question::actions::CANCEL)
            | (CONFIRM, confirm::actions::CANCEL)
            | (NOTICE, notice::actions::CANCEL)
            | (Self::REVIEW, review::actions::CANCEL)
            | (Self::FAILED, failed::actions::CANCEL) => (Request::Cancel, "Cancel"),
            (Self::IDLE, idle::actions::CANCEL) => (Request::Cancel, "Clear it now"),
            (Self::REVIEW, review::actions::SEND) => (Request::Submit, "Send"),
            (Self::FAILED, failed::actions::RETRY) => (Request::Retry, "Try again"),
            (Self::SUBMITTED, submitted::actions::DONE) => (Request::Finish, "Done"),
            (Self::IDLE, idle::actions::STILL_HERE) => (Request::StillHere, "Yes, I am here"),
            _ => return None,
        })
    }

    fn error_message(field: FormField, issue: Issue) -> &'static str {
        match (field, issue) {
            (NAME, Issue::Missing) => "Enter your name",
            (NAME, Issue::TooLong) => "Use at most 32 characters",
            (NAME, _) => "Use letters, digits and punctuation",
            (REFERENCE, Issue::TooLong) => "Use at most 10 digits",
            (REFERENCE, _) => "Use digits only",
            (TOPIC, _) => "Choose what it is about",
            (AGREE, _) => "Tick this to send your answers",
            _ => "Check this answer",
        }
    }

    fn countdown_slot(screen: ScreenId) -> Option<SlotId> {
        match screen {
            Self::IDLE => Some(idle::slots::SECONDS_LEFT),
            Self::SUBMITTED => Some(submitted::slots::SECONDS_LEFT),
            _ => None,
        }
    }

    fn fill(
        screen: ScreenId,
        tree: &mut Tree<CAPACITY, TEXT>,
        copies: &Copies,
        failed: Option<ShowFailed>,
    ) -> Result<(), UiError> {
        let mut number = [0u8; 10];
        match screen {
            Self::REVIEW => {
                let topic = copies
                    .choice(TOPIC)
                    .and_then(|topic| TOPICS.get(usize::from(topic)))
                    .copied()
                    .unwrap_or("");
                let reference = match copies.text(REFERENCE) {
                    "" => "None given",
                    text => text,
                };
                let details = match copies.text(DETAILS) {
                    "" => "None given",
                    text => text,
                };
                let receipt = if copies.checked(RECEIPT) { "Yes" } else { "No" };
                tree.set_slot(review::slots::NAME, copies.text(NAME))?;
                tree.set_slot(review::slots::REFERENCE, reference)?;
                tree.set_slot(review::slots::TOPIC, topic)?;
                tree.set_slot(review::slots::DETAILS, details)?;
                tree.set_slot(review::slots::RECEIPT, receipt)?;
            }
            Self::FAILED => {
                let (rejected, left) = failed.map_or((false, 0), |failure| {
                    (
                        failure.reason == FailureReason::Rejected,
                        failure.attempts_left,
                    )
                });
                tree.set_slot(
                    failed::slots::ATTEMPTS_LEFT,
                    digits(u32::from(left), &mut number),
                )?;
                tree.set_visible(failed::groups::REJECTED, rejected)?;
                tree.set_visible(failed::groups::CAN_RETRY, !rejected && left > 0)?;
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
    fn numbers_are_written_in_digits() {
        let mut buf = [0u8; 10];
        assert_eq!(digits(0, &mut buf), "0");
        assert_eq!(digits(7, &mut buf), "7");
        assert_eq!(digits(15, &mut buf), "15");
        assert_eq!(digits(u32::MAX, &mut buf), "4294967295");
    }

    #[test]
    fn every_form_field_has_a_control_that_maps_back_to_it() {
        for field in crate::form::FORM.fields().map(|field| field.id) {
            let found = control::<Reference>(field).map(|c| (c.screen, c.control));
            let (screen, ui) = found.unwrap_or((Reference::WELCOME, UiField::new(0)));
            assert_eq!(form_field::<Reference>(screen, ui), Some(field));
            assert!(index(field).is_some());
        }
        assert_eq!(
            crate::form::FORM.fields().count(),
            Reference::CONTROLS.len()
        );
    }
}
