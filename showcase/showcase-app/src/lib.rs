// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase: the Pi boots to a **launcher**, one
//! tile per example kiosk, and a tile opens its example at its welcome
//! screen. Every example is a kiosk written in TSX and converted to native
//! Rust by Screen Compiler; every brand is fictional.
//!
//! | Tile | Status | Opens | Application |
//! | --- | --- | --- | --- |
//! | Service desk (kitchen sink) | Ready | the reference kiosk: `showcase_kiosk_screens::kiosk` | 0, `showcase-kiosk-stand-in` |
//! | Visitor sign-in (Orrinvale) | Ready | `showcase_visitor`: a visitor book that forgets you | 1, `showcase-visitor-stand-in` |
//! | A program's own | Hidden unless a program fills it | its example ([`Extra`]) | 3, the program's |
//! | Pay at the pump (Tarnwick Fuel) | Ready | `showcase_pump`: pump, fuel and amount, then the card reader; the screen never sees the card | 2, `showcase-pump-stand-in` |
//! | React gallery (Screen Compiler) | Ready | the gallery: [`GALLERY`]'s screens, every component and style property the subset converts | |
//! | Why is this secure? (Under the glass) | Ready | a panel in the launcher's place: six short claims, each marked Built, Designed or Simulated | |
//!
//! [`Showcase`] is the UI a showcase UI PD drives: the launcher's tree, and
//! each ready example's UI, of which one shows at a time. Each example has
//! an application PD of its own, numbered as above: its reports reach only
//! its UI, and its UI's actions only it.
//!
//! * **A program may add one example**: [`Extra`] fills the
//!   launcher's third tile - its brand, title and line - and gives the
//!   example's screens and the bar above them; the showcase drives it as
//!   it drives its own, over the app-UI contract to its application PD on
//!   [`EXTRA_APP`]'s rings, which the system description gives it. The
//!   showcase's own program adds none ([`NoExtra`]): the tile stays hidden,
//!   and the UI talks to three applications.
//! * **An example is wiped when it closes.** Home - in the bar above every
//!   screen of an example - wipes everything the example's UI holds, its
//!   outbox included, asks its application to end any session whatever its
//!   phase (`Abandon`), and shows the launcher again. When a session ends
//!   by itself - finished, cancelled, the idle reset, the session limit, a
//!   fault - the example's UI is wiped on the session-ended record, and the
//!   launcher shows in place of the example's welcome screen.
//! * **The launcher holds nothing a person entered**: it has no field.
//! * **The Screen Compiler gallery** ([`GALLERY`]) shows every
//!   component, prop and style property Screen Compiler converts, each
//!   captioned with the TSX that made it; Back and Next go round its
//!   screens, Home closes it. It is not an example: no application, nothing
//!   sent. It shows in the launcher's tree, as the panel does, so its
//!   fields and choices are wiped whenever the launcher shows - Home, a
//!   reset, or [`GALLERY_IDLE`] with no input - and as it moves from one
//!   screen to the next.
//! * **An attract loop when idle**: after [`ATTRACT_AFTER`] with no
//!   input on the launcher, [`SLIDES`] take its place in turn; the first
//!   key, click or touch wakes the launcher and presses nothing. The UI PD
//!   keeps the time, asking the timer only for this.
//! * **Each in its own brand** ([`brands`]): the launcher and each
//!   example are drawn in their own fictional brand's theme
//!   ([`KioskScreens::theme`]), so the UI PD switches theme as a tile opens
//!   an example and back as the launcher shows again.
//! * **Every tile is a control**, on a 3 x 2 grid: Tab moves through them in
//!   order and the arrow keys across the grid, by each tile's place
//!   ([`TILES`]), so a hidden tile leaves its place empty; the focus starts
//!   on the Service desk, the first ready one.
//! * **The look will change.** The screens use today's widgets; the look
//!   work brings full-screen drawing, text roles, cards and a
//!   pointer, and every text and size is in the TSX (`tsx/`), so restyling
//!   them changes no code here.
//!
//! [`generated`] holds the Rust Screen Compiler writes for `tsx/launcher.tsx` and
//! the bars above each example's screens (`tsx/service-desk-bar.tsx`,
//! `tsx/visitor-bar.tsx`, `tsx/pump-bar.tsx`), exactly as written
//! (`make screens-bless`).

#![no_std]
#![forbid(unsafe_code)]

pub mod brands;
pub mod generated;

/// The pictures the screens show, which generated code names at the crate's
/// root: NoWayOS's samples, a landscape and a badge (`pictures.names`).
pub mod pictures {
    pub use nowayos_ui::pictures::samples::{BADGE, LANDSCAPE};
}

use nowayos_display::Rect;
use nowayos_input::{KeyCode, TouchPhase};
use nowayos_render_eg::{Rgb888, Theme};
use nowayos_ui::builder::{
    ButtonVariant, Cursor, Input, Layout, Look, Picker, Space, Surface, Template, TextStyle,
    UiBuilder,
};
use nowayos_ui::{
    ActionId, FieldId, GroupId, InputEvent, LayerId, Navigator, NodeId, NodeKind, ScreenId, SlotId,
    SwitchId, TextMetrics, Tree, UiError,
};
use showcase_kiosk_contract::Action;
use showcase_kiosk_screens::kiosk::{
    FormUi, HEIGHT, KioskScreens, KioskUi, KioskUiError, Pressed, Reference, Screens, WIDTH,
};
use showcase_pump::PumpUi;
use showcase_visitor::VisitorUi;

use brands::Brand;
use generated::attract::attract_screen as attract;
use generated::gallery_buttons::gallery_buttons_screen as gallery_buttons;
use generated::gallery_icons::gallery_icons_screen as gallery_icons;
use generated::gallery_keyboard::gallery_keyboard_screen as gallery_keyboard;
use generated::gallery_layout::gallery_layout_screen as gallery;
use generated::gallery_scroll::gallery_scroll_screen as scroll_screen;
use generated::launcher::launcher_screen as launcher;
use generated::pump_bar::pump_bar_screen as pump_bar;
use generated::service_desk_bar::service_desk_bar_screen as bar;
use generated::visitor_bar::visitor_bar_screen as visitor_bar;
use generated::why_secure::why_secure_screen as why_secure;

const _: () = assert!(generated::CAPACITY == showcase_kiosk_screens::generated::CAPACITY);

/// How far the bar's numbers are moved past an example screen's own: more
/// than any screen uses.
pub const SHIFT: u16 = 100;

/// Home, in the bar above an example's screens: the bar's own action,
/// moved by [`SHIFT`].
pub const HOME: ActionId = ActionId::new(bar::actions::HOME.get().saturating_add(SHIFT));

const _: () = assert!(bar::actions::HOME.get() == visitor_bar::actions::HOME.get());
const _: () = assert!(bar::actions::HOME.get() == pump_bar::actions::HOME.get());

/// The Service desk's application: application PD 0.
pub const SERVICE_DESK_APP: usize = 0;

/// The visitor sign-in's application: application PD 1.
pub const VISITOR_APP: usize = 1;

/// The pay at the pump's application: application PD 2.
pub const PUMP_APP: usize = 2;

/// The application of the example a program adds ([`Extra`]), if it adds
/// one: application PD 3, on the rings its system description gives it.
pub const EXTRA_APP: usize = 3;

/// An example a program adds to the showcase, beside the
/// showcase's own: it fills the launcher's third tile, and runs as they
/// do - a form UI of its screens ([`FormUi`]) under a bar of its own,
/// driven by its application PD over the app-UI contract on
/// [`EXTRA_APP`]'s rings - and is wiped as they are. The showcase names no
/// such example: the showcase's own program adds none ([`NoExtra`]).
pub trait Extra {
    /// Its screens.
    type Screens: Screens;

    /// Its tile and what it shows of itself outside its screens, or `None`
    /// for no example: the tile stays hidden and the UI talks to the
    /// showcase's own three applications only.
    const TILE: Option<Tile>;

    /// The bar above each of its screens, Home first: a
    /// `showcase_kiosk_screens::kiosk::Frame`, built with [`frame_with`].
    ///
    /// # Errors
    ///
    /// The tree's error, if it is full.
    fn frame(
        tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
        body: NodeId,
        screen: &'static str,
    ) -> Result<NodeId, UiError>;
}

/// What an example a program adds shows of itself outside its own screens:
/// its tile on the launcher, its attract slide and its brand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// Its fictional brand, as the tile names it.
    pub brand: &'static str,
    /// Its title: on the tile and its slide, and in the log as the action.
    pub title: &'static str,
    /// One line on the tile of what it shows.
    pub line: &'static str,
    /// Its attract slide's line, under its title.
    pub slide: &'static str,
    /// Its brand's colours: its screens and its slide are drawn in them.
    pub look: Brand,
    /// An action in its bar, moved by [`SHIFT`], that switches the example
    /// to its brand's large-text, high-contrast version and back, keeping
    /// every answer, if its bar has one; the launcher, and so the
    /// end of every session, brings the brand's own theme back.
    pub larger_text: Option<ActionId>,
}

/// No example added: the showcase's own program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoExtra;

impl Extra for NoExtra {
    /// Never built: with no tile, the showcase holds no example's UI.
    type Screens = Reference;

    const TILE: Option<Tile> = None;

    fn frame(
        tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
        body: NodeId,
        _screen: &'static str,
    ) -> Result<NodeId, UiError> {
        UiBuilder::column(tree, Layout::new(), &[body])
    }
}

/// The launcher's own screen.
const LAUNCHER: ScreenId = ScreenId(0);

/// The "Why is this secure?" panel, shown in the launcher's tree
/// in its place: fixed text and Home.
const WHY_SECURE: ScreenId = ScreenId(1);

/// The attract loop's slide, shown in the launcher's tree in its
/// place: one of [`SLIDES`] at a time.
const ATTRACT: ScreenId = ScreenId(2);

/// The Screen Compiler gallery's screens, in order: each its name, as the
/// log shows it, and the function that builds it. Every one has the same
/// bar - Home, Back, Next - so the same three actions.
pub const GALLERY: [(&str, generated::Build); 11] = [
    ("gallery-layout", generated::build_gallery_layout),
    ("gallery-type", generated::build_gallery_type),
    ("gallery-text", generated::build_gallery_text),
    ("gallery-buttons", generated::build_gallery_buttons),
    ("gallery-surfaces", generated::build_gallery_surfaces),
    ("gallery-fields", generated::build_gallery_fields),
    ("gallery-choices", generated::build_gallery_choices),
    ("gallery-keyboard", generated::build_gallery_keyboard),
    ("gallery-scroll", generated::build_gallery_scroll),
    ("gallery-icons", generated::build_gallery_icons),
    ("gallery-css", generated::build_gallery_css),
];

/// Every gallery screen numbers Home, Back and Next as the first does.
const _: () = {
    use generated::{
        gallery_buttons, gallery_choices, gallery_css, gallery_fields, gallery_scroll,
        gallery_surfaces, gallery_text, gallery_type,
    };
    macro_rules! same_bar {
        ($($screen:path),*) => {$(
            {
                use $screen as s;
                assert!(s::actions::HOME.get() == gallery::actions::HOME.get());
                assert!(s::actions::BACK.get() == gallery::actions::BACK.get());
                assert!(s::actions::NEXT.get() == gallery::actions::NEXT.get());
            }
        )*};
    }
    same_bar!(
        gallery_type::gallery_type_screen,
        gallery_text::gallery_text_screen,
        gallery_buttons::gallery_buttons_screen,
        gallery_surfaces::gallery_surfaces_screen,
        gallery_fields::gallery_fields_screen,
        gallery_choices::gallery_choices_screen,
        gallery_keyboard,
        gallery_scroll::gallery_scroll_screen,
        gallery_icons,
        gallery_css::gallery_css_screen
    );
};

/// The gallery's first screen in the launcher's tree: each shows as
/// `ScreenId(GALLERY_FROM + its place in GALLERY)`.
const GALLERY_FROM: u16 = 16;

/// The gallery's buttons screen, by its place in [`GALLERY`]: its More,
/// Ask and Info open a menu, a dialog and a popover over it.
const GALLERY_BUTTONS: usize = 3;

/// The gallery's keyboard screen, by its place in [`GALLERY`]: its
/// Check sets or clears the code's error line.
const GALLERY_KEYBOARD: usize = 7;

/// The gallery's icons screen, by its place in [`GALLERY`]: its
/// progress bar shows three quarters.
const GALLERY_ICONS: usize = 9;
/// The gallery's scroll screen, by its place in [`GALLERY`]: its long
/// text shows [`GALLERY_STORY`].
const GALLERY_SCROLL: usize = 8;

/// How many rows the scroll screen's virtual list holds.
const GALLERY_ROWS: u32 = 1_000;

/// The scroll screen's virtual list's rows' texts, by pool row.
const GALLERY_ROW_SLOTS: [SlotId; 8] = [
    scroll_screen::slots::ROWS_1_LABEL,
    scroll_screen::slots::ROWS_2_LABEL,
    scroll_screen::slots::ROWS_3_LABEL,
    scroll_screen::slots::ROWS_4_LABEL,
    scroll_screen::slots::ROWS_5_LABEL,
    scroll_screen::slots::ROWS_6_LABEL,
    scroll_screen::slots::ROWS_7_LABEL,
    scroll_screen::slots::ROWS_8_LABEL,
];

/// Binds each row of the scroll screen's virtual list to the item it
/// shows - "Row 1" to "Row 1000" - as an application binds its own,
/// skipping the rows already showing theirs.
fn bind_gallery_rows(
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
) -> Result<(), UiError> {
    let list = scroll_screen::lists::ROWS;
    let Some(window) = tree.list_window(list) else {
        return Ok(());
    };
    let last = window.first.saturating_add(window.pool).min(window.count);
    for item in window.first..last {
        let Some(slot) = tree
            .list_row(list, item)
            .and_then(|row| GALLERY_ROW_SLOTS.get(usize::from(row)))
        else {
            continue;
        };
        let mut text = [0_u8; 16];
        let label = row_label(item.saturating_add(1), &mut text);
        if tree.slot_text(*slot) != Some(label) {
            tree.set_slot(*slot, label)?;
        }
    }
    Ok(())
}

/// "Row N", written into `text`.
fn row_label(number: u32, text: &mut [u8; 16]) -> &str {
    let mut digits = [0_u8; 10];
    let mut rest = number;
    let mut count = 0_usize;
    loop {
        if let Some(digit) = digits.get_mut(count) {
            *digit = b'0'.saturating_add(u8::try_from(rest % 10).unwrap_or(0));
        }
        count = count.saturating_add(1);
        rest /= 10;
        if rest == 0 || count >= digits.len() {
            break;
        }
    }
    let prefix = b"Row ";
    for (slot, byte) in text
        .iter_mut()
        .zip(prefix.iter().chain(digits.iter().take(count).rev()))
    {
        *slot = *byte;
    }
    let len = prefix.len().saturating_add(count).min(text.len());
    text.get(..len)
        .and_then(|bytes| core::str::from_utf8(bytes).ok())
        .unwrap_or("Row")
}

/// What the scroll screen's long text shows: paragraphs the application
/// sets at run time (builder contract v16).
const GALLERY_STORY: &str = "A long text holds paragraphs the application sets while the screen shows - an article, a message, the terms of a service - up to the bytes its screen gives it, kept in the screen's tree, so nothing is allocated.\nIt wraps onto as many lines as it needs at the width it is given. A line break starts a new paragraph, and when the text changes only the lines that differ are drawn again.";

/// The code the keyboard screen's Check asks for: four digits.
const CODE_DIGITS: usize = 4;

/// How long the gallery waits with no input before it closes, wiped,
/// and the launcher shows, in milliseconds: twice [`ATTRACT_AFTER`], for
/// time to read.
pub const GALLERY_IDLE: u32 = ATTRACT_AFTER.saturating_mul(2);

/// The place in [`GALLERY`] of the launcher's tree's screen `screen`, if
/// it is one of the gallery's.
fn gallery_place(screen: ScreenId) -> Option<usize> {
    let place = usize::from(screen.0.checked_sub(GALLERY_FROM)?);
    (place < GALLERY.len()).then_some(place)
}

/// Builds the gallery's screen at `place` into the empty `tree`, its
/// progress bar, if it has one, set.
fn build_gallery(
    place: usize,
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
) -> Result<NodeId, UiError> {
    let (_, build) = GALLERY.get(place).copied().ok_or(UiError::NoRoot)?;
    let root = build(tree)?;
    if place == GALLERY_ICONS {
        tree.set_progress(gallery_icons::slots::DONE, 3, 4)?;
    }
    if place == GALLERY_SCROLL {
        tree.set_slot(scroll_screen::slots::STORY, GALLERY_STORY)?;
        tree.set_list_count(scroll_screen::lists::ROWS, GALLERY_ROWS)?;
        bind_gallery_rows(tree)?;
    }
    Ok(root)
}

/// The attract loop's own slides: each a title and a line, about an
/// example or an idea, in words the maintainers have approved, and
/// the brand it is drawn in. A program's example's slide ([`Tile`]) shows
/// after the examples', before the idea's.
pub const SLIDES: [(&str, &str, Brand); 3] = [
    (
        "Visitor sign-in",
        "A visitor book that forgets you",
        brands::ORRINVALE,
    ),
    (
        "Pay at the pump",
        "This screen never sees your card",
        brands::TARNWICK,
    ),
    (
        "No browser here",
        "React screens, run natively on seL4",
        brands::LAUNCHER,
    ),
];

/// Each slide's name, for the log, by its place: the slides rotate, and
/// each shows by its own name.
const SLIDE_NAMES: [&str; 4] = ["attract-1", "attract-2", "attract-3", "attract-4"];

/// The launcher's tiles by their places on its 3 x 2 grid, the top row
/// first: each its button's action. The third is a program's own.
pub const TILES: [ActionId; 6] = [
    launcher::actions::SERVICE_DESK,
    launcher::actions::VISITOR,
    launcher::actions::EXTRA,
    launcher::actions::PUMP,
    launcher::actions::GALLERY,
    launcher::actions::WHY_SECURE,
];

/// How long the launcher waits with no input before the attract loop
/// starts, and how long each slide shows, in milliseconds: 30 s
/// and 6 s, or, in one of Showcase's QEMU test images (`SHOWCASE_TEST_TIMINGS`,
/// as the kiosk flow's timeouts), 5 s and 2 s.
pub const ATTRACT_AFTER: u32 = if option_env!("SHOWCASE_TEST_TIMINGS").is_some() {
    5_000
} else {
    30_000
};

/// How long each slide shows, in milliseconds; see [`ATTRACT_AFTER`].
pub const SLIDE_FOR: u32 = if option_env!("SHOWCASE_TEST_TIMINGS").is_some() {
    2_000
} else {
    6_000
};

/// Where each arrow key moves from each tile on the launcher's 3 x 2 grid:
/// left, right, up and down, by the tiles' places ([`TILES`]). An arrow at
/// an edge, or towards a hidden tile, stays put.
const GRID: [[usize; 4]; 6] = [
    [0, 1, 0, 3],
    [0, 2, 1, 4],
    [1, 2, 2, 5],
    [3, 4, 0, 3],
    [3, 5, 1, 4],
    [4, 5, 2, 5],
];

/// Whether `event` is an arrow key going down or up.
const fn is_arrow(event: InputEvent) -> bool {
    matches!(
        event,
        InputEvent::KeyDown(KeyCode::LEFT | KeyCode::RIGHT | KeyCode::UP | KeyCode::DOWN)
            | InputEvent::KeyUp(KeyCode::LEFT | KeyCode::RIGHT | KeyCode::UP | KeyCode::DOWN)
    )
}

/// What shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    /// The launcher.
    Launcher,
    /// The Service desk example: the reference kiosk.
    ServiceDesk,
    /// The visitor sign-in example.
    Visitor,
    /// The pay at the pump example.
    Pump,
    /// The example a program adds.
    Extra,
    /// The Screen Compiler gallery.
    Gallery,
}

/// A builder that makes a converted screen's nodes in a tree with its
/// action, field, group, switch and slot numbers moved by `by`, so that
/// two screens built into one tree - the bar and an example's screen -
/// never share one. [`frame_with`] builds a bar with it.
pub struct Shifted<'t> {
    tree: &'t mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    by: u16,
}

impl Shifted<'_> {
    /// `number`, moved. A converted screen numbers from 1, in the tens at
    /// most, so the sum never reaches `u16::MAX`.
    const fn shift(&self, number: u16) -> u16 {
        number.saturating_add(self.by)
    }
}

impl UiBuilder for Shifted<'_> {
    type Node = NodeId;
    type Error = UiError;

    fn column(&mut self, layout: Layout, children: &[NodeId]) -> Result<NodeId, UiError> {
        UiBuilder::column(&mut *self.tree, layout, children)
    }
    fn row(&mut self, layout: Layout, children: &[NodeId]) -> Result<NodeId, UiError> {
        UiBuilder::row(&mut *self.tree, layout, children)
    }
    fn text(&mut self, text: &'static str) -> Result<NodeId, UiError> {
        UiBuilder::text(&mut *self.tree, text)
    }
    fn button(&mut self, label: &'static str, action: ActionId) -> Result<NodeId, UiError> {
        let action = ActionId::new(self.shift(action.get()));
        UiBuilder::button(&mut *self.tree, label, action)
    }
    fn text_input(&mut self, placeholder: Option<&'static str>) -> Result<NodeId, UiError> {
        UiBuilder::text_input(&mut *self.tree, placeholder)
    }
    fn spacer(&mut self, size: Option<u16>) -> Result<NodeId, UiError> {
        UiBuilder::spacer(&mut *self.tree, size)
    }
    fn spacer_space(&mut self, space: Space) -> Result<NodeId, UiError> {
        UiBuilder::spacer_space(&mut *self.tree, space)
    }
    fn button_look(
        &mut self,
        label: &'static str,
        action: ActionId,
        variant: ButtonVariant,
        look: Look,
    ) -> Result<NodeId, UiError> {
        let action = ActionId::new(self.shift(action.get()));
        UiBuilder::button_look(&mut *self.tree, label, action, variant, look)
    }
    fn scroll_view(&mut self, layout: Layout, child: NodeId) -> Result<NodeId, UiError> {
        UiBuilder::scroll_view(&mut *self.tree, layout, child)
    }
    fn scroll_view_axes(
        &mut self,
        layout: Layout,
        axes: nowayos_ui::ScrollAxes,
        child: NodeId,
    ) -> Result<NodeId, UiError> {
        UiBuilder::scroll_view_axes(&mut *self.tree, layout, axes, child)
    }
    fn virtual_list(
        &mut self,
        layout: Layout,
        list: nowayos_ui::ListId,
        row_height: u16,
        rows: &[NodeId],
    ) -> Result<NodeId, UiError> {
        let list = nowayos_ui::ListId::new(self.shift(list.get()));
        UiBuilder::virtual_list(&mut *self.tree, layout, list, row_height, rows)
    }
    fn slot(&mut self, slot: SlotId) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::slot(&mut *self.tree, slot)
    }
    fn text_styled(&mut self, text: &'static str, style: TextStyle) -> Result<NodeId, UiError> {
        UiBuilder::text_styled(&mut *self.tree, text, style)
    }
    fn slot_styled(&mut self, slot: SlotId, style: TextStyle) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::slot_styled(&mut *self.tree, slot, style)
    }
    fn slot_template(
        &mut self,
        slot: SlotId,
        template: &'static Template,
    ) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::slot_template(&mut *self.tree, slot, template)
    }
    fn slot_template_styled(
        &mut self,
        slot: SlotId,
        template: &'static Template,
        style: TextStyle,
    ) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::slot_template_styled(&mut *self.tree, slot, template, style)
    }
    fn icon(&mut self, icon: &'static nowayos_ui::icons::Icon) -> Result<NodeId, UiError> {
        UiBuilder::icon(&mut *self.tree, icon)
    }
    fn long_text(
        &mut self,
        slot: SlotId,
        capacity: u16,
        style: TextStyle,
    ) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::long_text(&mut *self.tree, slot, capacity, style)
    }
    fn image(
        &mut self,
        picture: &'static nowayos_ui::builder::Picture,
        fit: nowayos_ui::builder::Fit,
        alt: &'static str,
        layout: Layout,
    ) -> Result<NodeId, UiError> {
        UiBuilder::image(&mut *self.tree, picture, fit, alt, layout)
    }
    fn progress(&mut self, slot: SlotId) -> Result<NodeId, UiError> {
        let slot = SlotId::new(self.shift(slot.get()));
        UiBuilder::progress(&mut *self.tree, slot)
    }
    fn button_with(
        &mut self,
        label: &'static str,
        action: ActionId,
        variant: ButtonVariant,
    ) -> Result<NodeId, UiError> {
        let action = ActionId::new(self.shift(action.get()));
        UiBuilder::button_with(&mut *self.tree, label, action, variant)
    }
    fn column_on(
        &mut self,
        surface: Surface,
        layout: Layout,
        children: &[NodeId],
    ) -> Result<NodeId, UiError> {
        UiBuilder::column_on(&mut *self.tree, surface, layout, children)
    }
    fn row_on(
        &mut self,
        surface: Surface,
        layout: Layout,
        children: &[NodeId],
    ) -> Result<NodeId, UiError> {
        UiBuilder::row_on(&mut *self.tree, surface, layout, children)
    }
    fn group(&mut self, group: GroupId, child: NodeId) -> Result<NodeId, UiError> {
        let group = GroupId::new(self.shift(group.get()));
        UiBuilder::group(&mut *self.tree, group, child)
    }
    fn either(
        &mut self,
        group: GroupId,
        when_visible: NodeId,
        otherwise: NodeId,
    ) -> Result<NodeId, UiError> {
        let group = GroupId::new(self.shift(group.get()));
        UiBuilder::either(&mut *self.tree, group, when_visible, otherwise)
    }
    fn unless(&mut self, group: GroupId, child: NodeId) -> Result<NodeId, UiError> {
        let group = GroupId::new(self.shift(group.get()));
        UiBuilder::unless(&mut *self.tree, group, child)
    }
    fn case(&mut self, switch: SwitchId, when: u8, child: NodeId) -> Result<NodeId, UiError> {
        let switch = SwitchId::new(self.shift(switch.get()));
        UiBuilder::case(&mut *self.tree, switch, when, child)
    }
    fn case_or(
        &mut self,
        switch: SwitchId,
        when: u8,
        when_matched: NodeId,
        otherwise: NodeId,
    ) -> Result<NodeId, UiError> {
        let switch = SwitchId::new(self.shift(switch.get()));
        UiBuilder::case_or(&mut *self.tree, switch, when, when_matched, otherwise)
    }
    fn input(&mut self, field: FieldId, input: Input) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::input(&mut *self.tree, field, input)
    }
    fn text_area(
        &mut self,
        field: FieldId,
        rows: u8,
        max_len: u16,
        placeholder: &'static str,
    ) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::text_area(&mut *self.tree, field, rows, max_len, placeholder)
    }
    /// A rich text's runs are a constant: their links' actions are not
    /// moved, so a bar holds none.
    fn rich_text(
        &mut self,
        runs: &'static [nowayos_ui::rich::Run],
        style: TextStyle,
    ) -> Result<NodeId, UiError> {
        UiBuilder::rich_text(&mut *self.tree, runs, style)
    }
    fn dialog(&mut self, layer: LayerId, child: NodeId) -> Result<NodeId, UiError> {
        let layer = LayerId::new(self.shift(layer.get()));
        UiBuilder::dialog(&mut *self.tree, layer, child)
    }
    fn menu(&mut self, layer: LayerId, anchor: NodeId, child: NodeId) -> Result<NodeId, UiError> {
        let layer = LayerId::new(self.shift(layer.get()));
        UiBuilder::menu(&mut *self.tree, layer, anchor, child)
    }
    fn popover(
        &mut self,
        layer: LayerId,
        anchor: NodeId,
        child: NodeId,
    ) -> Result<NodeId, UiError> {
        let layer = LayerId::new(self.shift(layer.get()));
        UiBuilder::popover(&mut *self.tree, layer, anchor, child)
    }
    fn tooltip(&mut self, anchor: NodeId, text: &'static str) -> Result<NodeId, UiError> {
        UiBuilder::tooltip(&mut *self.tree, anchor, text)
    }
    fn context_action(&mut self, node: NodeId, action: ActionId) -> Result<(), UiError> {
        let action = ActionId::new(self.shift(action.get()));
        UiBuilder::context_action(&mut *self.tree, node, action)
    }
    fn cursor(&mut self, node: NodeId, cursor: Cursor) -> Result<(), UiError> {
        UiBuilder::cursor(&mut *self.tree, node, cursor)
    }
    fn checkbox(&mut self, field: FieldId, label: &'static str) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::checkbox(&mut *self.tree, field, label)
    }
    fn radio_group(
        &mut self,
        field: FieldId,
        options: &'static [&'static str],
    ) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::radio_group(&mut *self.tree, field, options)
    }
    fn list_picker(
        &mut self,
        field: FieldId,
        options: &'static [&'static str],
        picker: Picker,
    ) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::list_picker(&mut *self.tree, field, options, picker)
    }

    fn keyboard(&mut self) -> Result<NodeId, UiError> {
        UiBuilder::keyboard(&mut *self.tree)
    }

    fn field_error(&mut self, field: FieldId) -> Result<NodeId, UiError> {
        let field = FieldId::new(self.shift(field.get()));
        UiBuilder::field_error(&mut *self.tree, field)
    }
}

/// A bar above an example's screen: `bar`, built with its numbers moved by
/// [`SHIFT`] past the screen's own, over `body`. Each example's
/// `showcase_kiosk_screens::kiosk::Frame` is one, and so is a program's
/// ([`Extra::frame`]).
///
/// # Errors
///
/// The tree's error, if it is full.
pub fn frame_with(
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    body: NodeId,
    bar: impl FnOnce(&mut Shifted<'_>) -> Result<NodeId, UiError>,
) -> Result<NodeId, UiError> {
    let root = bar(&mut Shifted {
        tree: &mut *tree,
        by: SHIFT,
    })?;
    UiBuilder::column(tree, Layout::new(), &[root, body])
}

/// The bar above every Service desk screen: Home, the example's name and
/// the demo line, over the screen itself (`showcase_kiosk_screens::kiosk::Frame`).
///
/// # Errors
///
/// The tree's error, if it is full.
pub fn service_desk_frame(
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    body: NodeId,
    _screen: &'static str,
) -> Result<NodeId, UiError> {
    frame_with(tree, body, |b| bar::build(b).map(|nodes| nodes.root))
}

/// The bar above every visitor sign-in screen: Home, the example's name
/// and what is simulated, over the screen itself.
///
/// # Errors
///
/// The tree's error, if it is full.
pub fn visitor_frame(
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    body: NodeId,
    _screen: &'static str,
) -> Result<NodeId, UiError> {
    frame_with(tree, body, |b| {
        visitor_bar::build(b).map(|nodes| nodes.root)
    })
}

/// The bar above every pay at the pump screen: Home, the example's name
/// and what is simulated, over the screen itself.
///
/// # Errors
///
/// The tree's error, if it is full.
pub fn pump_frame(
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    body: NodeId,
    _screen: &'static str,
) -> Result<NodeId, UiError> {
    frame_with(tree, body, |b| pump_bar::build(b).map(|nodes| nodes.root))
}

/// Builds the launcher, or the panel shown in its place, into the empty
/// `tree`: the launcher with `E`'s tile filled and shown, if it has one.
fn build_launcher<E: Extra>(
    screen: ScreenId,
    tree: &mut Tree<{ generated::CAPACITY }, { generated::TEXT }>,
) -> Result<NodeId, UiError> {
    if let Some(place) = gallery_place(screen) {
        return build_gallery(place, tree);
    }
    match screen {
        WHY_SECURE => generated::build_why_secure(tree),
        ATTRACT => generated::build_attract(tree),
        _ => {
            let root = generated::build_launcher(tree)?;
            if let Some(tile) = E::TILE {
                tree.set_slot(launcher::slots::EXTRA_BRAND, tile.brand)?;
                tree.set_slot(launcher::slots::EXTRA_TITLE, tile.title)?;
                tree.set_slot(launcher::slots::EXTRA_LINE, tile.line)?;
                tree.set_visible(launcher::groups::EXTRA, true)?;
            }
            Ok(root)
        }
    }
}

/// The attract loop's slide at `place`, with `E`'s example's slide
/// after the showcase's examples', if it has one: its title, line and
/// brand.
fn slide<E: Extra>(place: usize) -> Option<(&'static str, &'static str, Brand)> {
    let examples = SLIDES.len().saturating_sub(1);
    match (E::TILE, place.checked_sub(examples)) {
        (Some(tile), Some(0)) => Some((tile.title, tile.slide, tile.look)),
        (Some(_), Some(_)) => SLIDES.get(place.saturating_sub(1)).copied(),
        _ => SLIDES.get(place).copied(),
    }
}

/// How many slides the attract loop shows in turn: the showcase's own, and
/// `E`'s example's, if it has one.
const fn slides<E: Extra>() -> usize {
    if E::TILE.is_some() {
        SLIDES.len().saturating_add(1)
    } else {
        SLIDES.len()
    }
}

/// The showcase's UI, with the example a program adds, if any (`E`); see
/// the [crate documentation](crate).
pub struct Showcase<E: Extra = NoExtra> {
    launcher: Tree<{ generated::CAPACITY }, { generated::TEXT }>,
    nav: Navigator<1>,
    service_desk: KioskUi,
    visitor: VisitorUi,
    pump: PumpUi,
    /// The example a program adds: `None` if it adds none.
    extra: Option<FormUi<E::Screens>>,
    showing: Showing,
    /// Where a mouse last moved the pointer: a button event carries no
    /// position, so the tree that shows next is told it. A touch carries
    /// its own, and hides the pointer.
    pointer: Option<(i32, i32)>,
    /// Whether the person last used keys rather than a click or a touch:
    /// the tree that shows next shows its focus ring if so.
    keyed: bool,
    /// The attract loop's slide showing, by its place, if it shows.
    slide: Option<usize>,
    /// Whether the example showing is in its brand's large-text,
    /// high-contrast version: Larger text, until the launcher shows.
    larger: bool,
}

impl<E: Extra> Showcase<E> {
    /// The showcase at its launcher, laid out for a
    /// `showcase_kiosk_screens::kiosk::WIDTH` × `HEIGHT` screen with `metrics`.
    ///
    /// # Errors
    ///
    /// The tree's error, if a screen cannot be built or laid out.
    pub fn new(metrics: TextMetrics) -> Result<Self, UiError> {
        let extra = match E::TILE {
            Some(_) => Some(FormUi::framed(metrics, E::frame)?),
            None => None,
        };
        let mut showcase = Self {
            launcher: Tree::new(),
            nav: Navigator::new(LAUNCHER, Rect::new(0, 0, WIDTH, HEIGHT), metrics),
            service_desk: KioskUi::framed(metrics, service_desk_frame)?,
            visitor: VisitorUi::framed(metrics, visitor_frame)?,
            pump: PumpUi::framed(metrics, pump_frame)?,
            extra,
            showing: Showing::Launcher,
            pointer: None,
            keyed: false,
            slide: None,
            larger: false,
        };
        showcase
            .nav
            .show_start(&mut showcase.launcher, build_launcher::<E>)?;
        Ok(showcase)
    }

    /// The Service desk example's UI, to read.
    #[must_use]
    pub const fn service_desk(&self) -> &KioskUi {
        &self.service_desk
    }

    /// The visitor sign-in example's UI, to read.
    #[must_use]
    pub const fn visitor(&self) -> &VisitorUi {
        &self.visitor
    }

    /// The pay at the pump example's UI, to read.
    #[must_use]
    pub const fn pump(&self) -> &PumpUi {
        &self.pump
    }

    /// The UI of the example a program adds, to read, if it adds one.
    #[must_use]
    pub const fn extra(&self) -> Option<&FormUi<E::Screens>> {
        self.extra.as_ref()
    }

    /// Whether the launcher shows.
    #[must_use]
    pub fn at_launcher(&self) -> bool {
        self.showing == Showing::Launcher
    }

    /// Whether nothing a person entered is held: no example holds any
    /// answer or action carrying text, and no field or choice in the
    /// launcher's tree holds anything - its own screens have none, and its
    /// slots hold only fixed text, a program's tile and the slides; the
    /// gallery's are wiped as it closes. For tests; it tells one bit.
    #[must_use]
    pub fn holds_nothing(&self) -> bool {
        self.service_desk.holds_nothing()
            && self.visitor.holds_nothing()
            && self.pump.holds_nothing()
            && self.extra.as_ref().is_none_or(FormUi::holds_nothing)
            && self.launcher.paint_order().all(|(_, node)| {
                node.kind().field().is_none_or(|field| {
                    self.launcher.value(field).is_none_or(str::is_empty)
                        && self.launcher.checked(field) != Some(true)
                        && self.launcher.selected(field).is_none()
                })
            })
    }

    /// Whether the Screen Compiler gallery shows, and which of its screens:
    /// its name in [`GALLERY`].
    #[must_use]
    pub fn gallery_screen(&self) -> Option<&'static str> {
        (self.showing == Showing::Gallery)
            .then(|| gallery_place(self.nav.current()))
            .flatten()
            .and_then(|place| GALLERY.get(place))
            .map(|(name, _)| *name)
    }

    /// Wipes every example's UI.
    fn reset_examples(&mut self) {
        self.service_desk.reset();
        self.visitor.reset();
        self.pump.reset();
        if let Some(extra) = self.extra.as_mut() {
            extra.reset();
        }
    }

    /// Wipes the example showing and shows the launcher again: Home.
    fn home(&mut self) {
        match self.showing {
            Showing::Launcher => {}
            Showing::ServiceDesk => self.service_desk.reset(),
            Showing::Visitor => self.visitor.reset(),
            Showing::Pump => self.pump.reset(),
            Showing::Extra => {
                if let Some(extra) = self.extra.as_mut() {
                    extra.reset();
                }
            }
            // Wiped as the launcher shows.
            Showing::Gallery => {}
        }
        self.show_launcher();
    }

    /// Shows the launcher, freshly built, focus on its first tile: its
    /// tree wiped first, the gallery's fields with it.
    fn show_launcher(&mut self) {
        self.showing = Showing::Launcher;
        self.slide = None;
        self.larger = false;
        let _shown = self.nav.reset(&mut self.launcher, build_launcher::<E>);
        self.point_shown();
    }

    /// Shows attract slide `place` in the launcher's place.
    fn show_slide(&mut self, place: usize) {
        let Some((title, line, _)) = slide::<E>(place) else {
            return;
        };
        if self.nav.current() != ATTRACT
            && self
                .nav
                .replace(ATTRACT, &mut self.launcher, build_launcher::<E>)
                .is_err()
        {
            return;
        }
        let filled = self
            .launcher
            .set_slot(attract::slots::TITLE, title)
            .and_then(|()| self.launcher.set_slot(attract::slots::LINE, line));
        if filled.is_ok() {
            self.slide = Some(place);
        }
    }

    /// Shows the "Why is this secure?" panel in the launcher's place, focus
    /// on its Home.
    fn show_why_secure(&mut self) {
        if self
            .nav
            .replace(WHY_SECURE, &mut self.launcher, build_launcher::<E>)
            .is_ok()
        {
            let _ = self.launcher.focus_next();
        }
        self.point_shown();
    }

    /// Opens the Screen Compiler gallery at its first screen, in the
    /// launcher's place, focus on Next.
    fn show_gallery(&mut self) {
        self.show_gallery_at(0);
    }

    /// Shows the gallery's screen at `place` in the launcher's tree,
    /// freshly built, the tree wiped first; focus on Next.
    fn show_gallery_at(&mut self, place: usize) {
        let Some(screen) = u16::try_from(place)
            .ok()
            .and_then(|place| GALLERY_FROM.checked_add(place))
        else {
            return;
        };
        self.launcher.wipe_session();
        if self
            .nav
            .replace(ScreenId(screen), &mut self.launcher, build_launcher::<E>)
            .is_ok()
        {
            self.showing = Showing::Gallery;
            self.focus_gallery_next();
        } else {
            self.show_launcher();
        }
        self.point_shown();
    }

    /// Moves the gallery one screen on, or back, round its ends.
    fn turn_gallery(&mut self, forward: bool) {
        let count = GALLERY.len();
        let at = gallery_place(self.nav.current()).unwrap_or(0);
        let to = if forward {
            at.saturating_add(1).checked_rem(count)
        } else {
            at.checked_add(count.saturating_sub(1))
                .and_then(|n| n.checked_rem(count))
        }
        .unwrap_or(0);
        self.show_gallery_at(to);
    }

    /// Focus on the gallery bar's Next, so Enter goes on.
    fn focus_gallery_next(&mut self) {
        let next = self
            .launcher
            .paint_order()
            .find(|(_, node)| {
                matches!(node.kind(), NodeKind::Button { action, .. } if *action == gallery::actions::NEXT)
            })
            .map(|(id, _)| id);
        if let Some(id) = next {
            let _ = self.launcher.set_focus(Some(id));
        }
    }

    /// The buttons screen's More, Ask and Info open their menu, dialog and
    /// popover; the dialog's Done, or Escape in it, closes it.
    fn gallery_layers(
        &mut self,
        action: Option<nowayos_ui::ActionId>,
        dismissed: Option<nowayos_ui::LayerId>,
    ) -> Option<&'static str> {
        use gallery_buttons::{actions, layers};
        let (opened, said) = match (action, dismissed) {
            (Some(actions::MORE), _) => (Some(layers::MENU_OPEN), "More"),
            (Some(actions::ASK), _) => (Some(layers::ASKING), "Ask"),
            (Some(actions::INFO), _) => (Some(layers::INFO_OPEN), "Info"),
            (Some(actions::DONE), _) | (None, Some(layers::ASKING)) => {
                let _ = self.launcher.close(layers::ASKING);
                return Some("Done");
            }
            _ => return None,
        };
        if let Some(layer) = opened {
            let _ = self.launcher.open(layer);
        }
        Some(said)
    }

    /// The keyboard screen's Check: the code's error line cleared if it is
    /// four digits, and set otherwise - as an application would.
    fn check_code(&mut self) {
        let field = gallery_keyboard::fields::CODE;
        let good = self.launcher.value(field).is_some_and(|code| {
            code.len() == CODE_DIGITS && code.bytes().all(|b| b.is_ascii_digit())
        });
        let set = if good {
            self.launcher.clear_field_error(field)
        } else {
            self.launcher.set_field_error(field, "Enter four digits")
        };
        if set.is_ok() {
            let _ = self
                .launcher
                .layout(self.nav.viewport(), &self.nav.metrics());
        }
    }

    /// Shows an example, at the screen it is on: the program's only if it
    /// adds one.
    fn show_example(&mut self, example: Showing) {
        if example == Showing::Extra && self.extra.is_none() {
            return;
        }
        self.showing = example;
        self.point_shown();
    }

    /// Tells the tree now showing where the pointer is, so a click with
    /// no move first lands where the pointer is, not where it was when
    /// that tree last showed: a second click on Home or a tile must not
    /// reach what was under it before. And whether the person last used
    /// keys, so its focus ring shows as it did on the tree before.
    fn point_shown(&mut self) {
        let keyed = self.keyed;
        self.tree_mut().set_focus_visible(keyed);
        let Some((x, y)) = self.pointer else {
            return;
        };
        let moved = InputEvent::PointerMove { x, y };
        match self.showing {
            Showing::Launcher => {
                let _ = self.launcher.handle(moved);
            }
            Showing::ServiceDesk => {
                let _ = self.service_desk.handle_event(moved);
            }
            Showing::Visitor => {
                let _ = self.visitor.handle_event(moved);
            }
            Showing::Pump => {
                let _ = self.pump.handle_event(moved);
            }
            Showing::Extra => {
                if let Some(extra) = self.extra.as_mut() {
                    let _ = extra.handle_event(moved);
                }
            }
            Showing::Gallery => {
                let _ = self.launcher.handle(moved);
            }
        }
    }

    /// Moves the launcher's focus to the tile `key` points at on the grid
    /// ([`GRID`]); from no tile, to the first. Towards a hidden tile, or
    /// any other key, it does nothing.
    fn move_on_grid(&mut self, key: KeyCode) {
        let way = match key {
            KeyCode::LEFT => 0,
            KeyCode::RIGHT => 1,
            KeyCode::UP => 2,
            KeyCode::DOWN => 3,
            _ => return,
        };
        let action_of = |id: NodeId| match self.launcher.get(id).map(|node| node.kind()) {
            Some(NodeKind::Button { action, .. }) => Some(*action),
            _ => None,
        };
        let to = self
            .launcher
            .focused()
            .and_then(action_of)
            .and_then(|action| TILES.iter().position(|tile| *tile == action))
            .and_then(|at| GRID.get(at))
            .and_then(|ways| ways.get(way))
            .copied()
            .unwrap_or(0);
        let Some(&action) = TILES.get(to) else {
            return;
        };
        let shown = self
            .launcher
            .paint_order()
            .find(|(_, node)| matches!(node.kind(), NodeKind::Button { action: a, .. } if *a == action))
            .map(|(id, _)| id);
        if let Some(id) = shown {
            let _ = self.launcher.set_focus(Some(id));
        }
    }

    /// What an example's button press means here: Home closes it, and a
    /// program's example's Larger text switches its theme.
    fn pressed(&mut self, pressed: Pressed) -> Option<&'static str> {
        let larger_text = E::TILE.and_then(|tile| tile.larger_text);
        match pressed {
            Pressed::Button(label) => Some(label),
            Pressed::Other(HOME) => {
                self.home();
                Some("Home")
            }
            Pressed::Other(action)
                if self.showing == Showing::Extra && Some(action) == larger_text =>
            {
                self.larger = !self.larger;
                Some("Larger text")
            }
            Pressed::Other(_) => None,
        }
    }

    /// An event for the example a program adds, if it adds one.
    fn handle_extra(&mut self, event: InputEvent) -> Option<&'static str> {
        let pressed = self.extra.as_mut()?.handle_event(event)?;
        self.pressed(pressed)
    }
}

impl<E: Extra> KioskScreens for Showcase<E> {
    // The showcase's system grants the UI the timer: its UI PD
    // times each tick's drawing.
    const TIMES_TICKS: bool = true;
    const APPS: usize = if E::TILE.is_some() {
        EXTRA_APP.saturating_add(1)
    } else {
        EXTRA_APP
    };

    fn tree(&self) -> &Tree<{ generated::CAPACITY }, { generated::TEXT }> {
        match (self.showing, self.extra.as_ref()) {
            (Showing::ServiceDesk, _) => self.service_desk.tree(),
            (Showing::Visitor, _) => self.visitor.tree(),
            (Showing::Pump, _) => self.pump.tree(),
            (Showing::Extra, Some(extra)) => extra.tree(),
            (Showing::Launcher | Showing::Gallery | Showing::Extra, _) => &self.launcher,
        }
    }

    fn tree_mut(&mut self) -> &mut Tree<{ generated::CAPACITY }, { generated::TEXT }> {
        match (self.showing, self.extra.as_mut()) {
            (Showing::ServiceDesk, _) => self.service_desk.tree_mut(),
            (Showing::Visitor, _) => self.visitor.tree_mut(),
            (Showing::Pump, _) => self.pump.tree_mut(),
            (Showing::Extra, Some(extra)) => extra.tree_mut(),
            (Showing::Launcher | Showing::Gallery | Showing::Extra, _) => &mut self.launcher,
        }
    }

    fn resize(&mut self, viewport: Rect, metrics: TextMetrics) -> Result<(), UiError> {
        self.nav.resize(&mut self.launcher, viewport, metrics)?;
        self.service_desk.resize(viewport, metrics)?;
        self.visitor.resize(viewport, metrics)?;
        self.pump.resize(viewport, metrics)?;
        match self.extra.as_mut() {
            Some(extra) => extra.resize(viewport, metrics),
            None => Ok(()),
        }
    }

    fn screen(&self) -> &'static str {
        match (self.showing, self.extra.as_ref()) {
            (Showing::Launcher, _) if self.nav.current() == WHY_SECURE => "why-secure",
            (Showing::Launcher, _) if self.nav.current() == ATTRACT => self
                .slide
                .and_then(|place| SLIDE_NAMES.get(place))
                .copied()
                .unwrap_or("attract-1"),
            (Showing::ServiceDesk, _) => self.service_desk.screen(),
            (Showing::Visitor, _) => self.visitor.screen(),
            (Showing::Pump, _) => self.pump.screen(),
            (Showing::Extra, Some(extra)) => extra.screen(),
            (Showing::Gallery, _) => self.gallery_screen().unwrap_or("gallery-layout"),
            (Showing::Launcher | Showing::Extra, _) => "launcher",
        }
    }

    fn handle(&mut self, event: InputEvent) -> Option<&'static str> {
        // As the trees take it: a mouse's move shows the pointer
        // and a touch hides it; a key shows the focus ring, and a press
        // hides it.
        match event {
            InputEvent::PointerMove { x, y } => self.pointer = Some((x, y)),
            InputEvent::Touch { id: 0, phase, .. } => {
                self.pointer = None;
                if phase == TouchPhase::Start {
                    self.keyed = false;
                }
            }
            InputEvent::PointerButton {
                button: 0,
                down: true,
            } => self.keyed = false,
            InputEvent::KeyDown(key) if !key.is_shift() => self.keyed = true,
            _ => {}
        }
        // The attract loop: the first key, click or touch only
        // wakes the launcher, and presses nothing.
        if self.slide.is_some() {
            if matches!(
                event,
                InputEvent::KeyDown(_)
                    | InputEvent::PointerButton { down: true, .. }
                    | InputEvent::Touch {
                        phase: TouchPhase::Start,
                        ..
                    }
            ) {
                self.show_launcher();
            }
            return None;
        }
        match self.showing {
            // The panel in the launcher's place: Home is its one control.
            Showing::Launcher if self.nav.current() == WHY_SECURE => {
                match self.launcher.handle(event)? {
                    nowayos_ui::UiEvent::Action(why_secure::actions::HOME) => {
                        self.show_launcher();
                        Some("Home")
                    }
                    _ => None,
                }
            }
            // The arrow keys move across the grid of tiles, not through
            // them in a line as the tree would.
            Showing::Launcher if is_arrow(event) => {
                if let InputEvent::KeyDown(key) = event {
                    self.move_on_grid(key);
                    self.launcher.set_focus_visible(true);
                }
                None
            }
            Showing::Launcher => match self.launcher.handle(event)? {
                nowayos_ui::UiEvent::Action(launcher::actions::SERVICE_DESK) => {
                    // The example's UI is at its welcome screen, wiped.
                    self.show_example(Showing::ServiceDesk);
                    Some("Service desk")
                }
                nowayos_ui::UiEvent::Action(launcher::actions::VISITOR) => {
                    self.show_example(Showing::Visitor);
                    Some("Visitor sign-in")
                }
                nowayos_ui::UiEvent::Action(launcher::actions::PUMP) => {
                    self.show_example(Showing::Pump);
                    Some("Pay at the pump")
                }
                nowayos_ui::UiEvent::Action(launcher::actions::EXTRA) => {
                    let tile = E::TILE?;
                    self.show_example(Showing::Extra);
                    Some(tile.title)
                }
                nowayos_ui::UiEvent::Action(launcher::actions::WHY_SECURE) => {
                    self.show_why_secure();
                    Some("Why is this secure?")
                }
                nowayos_ui::UiEvent::Action(launcher::actions::GALLERY) => {
                    self.show_gallery();
                    Some("React gallery")
                }
                _ => None,
            },
            Showing::ServiceDesk => {
                let pressed = self.service_desk.handle_event(event)?;
                self.pressed(pressed)
            }
            Showing::Visitor => {
                let pressed = self.visitor.handle_event(event)?;
                self.pressed(pressed)
            }
            Showing::Pump => {
                let pressed = self.pump.handle_event(event)?;
                self.pressed(pressed)
            }
            Showing::Extra => self.handle_extra(event),
            Showing::Gallery => match self.launcher.handle(event)? {
                nowayos_ui::UiEvent::Action(gallery::actions::HOME) => {
                    self.show_launcher();
                    Some("Home")
                }
                nowayos_ui::UiEvent::Action(gallery::actions::BACK) => {
                    self.turn_gallery(false);
                    Some("Back")
                }
                nowayos_ui::UiEvent::Action(gallery::actions::NEXT) => {
                    self.turn_gallery(true);
                    Some("Next")
                }
                // The buttons screen's layers, opened and closed as an
                // application would.
                nowayos_ui::UiEvent::Action(action)
                    if gallery_place(self.nav.current()) == Some(GALLERY_BUTTONS) =>
                {
                    self.gallery_layers(Some(action), None)
                }
                nowayos_ui::UiEvent::Dismissed(layer)
                    if gallery_place(self.nav.current()) == Some(GALLERY_BUTTONS) =>
                {
                    self.gallery_layers(None, Some(layer))
                }
                nowayos_ui::UiEvent::Action(gallery_keyboard::actions::CHECK)
                    if gallery_place(self.nav.current()) == Some(GALLERY_KEYBOARD) =>
                {
                    self.check_code();
                    Some("Check")
                }
                // The virtual list's window moved: its rows bound again.
                nowayos_ui::UiEvent::ListMoved(_)
                    if gallery_place(self.nav.current()) == Some(GALLERY_SCROLL) =>
                {
                    let _ = bind_gallery_rows(&mut self.launcher);
                    None
                }
                _ => None,
            },
        }
    }

    fn end_input(&mut self) {
        match self.showing {
            Showing::Launcher | Showing::Gallery => {}
            Showing::ServiceDesk => self.service_desk.end_input(),
            Showing::Visitor => self.visitor.end_input(),
            Showing::Pump => self.pump.end_input(),
            Showing::Extra => {
                if let Some(extra) = self.extra.as_mut() {
                    extra.end_input();
                }
            }
        }
    }

    fn receive_report(&mut self, app: usize, record: &[u8]) -> Result<(), KioskUiError> {
        let (example, before, applied, after) = match (app, self.extra.as_mut()) {
            (SERVICE_DESK_APP, _) => {
                let before = self.service_desk.screen();
                let applied = self.service_desk.receive_report(record);
                (
                    Showing::ServiceDesk,
                    before,
                    applied,
                    self.service_desk.screen(),
                )
            }
            (VISITOR_APP, _) => {
                let before = self.visitor.screen();
                let applied = self.visitor.receive_report(record);
                (Showing::Visitor, before, applied, self.visitor.screen())
            }
            (PUMP_APP, _) => {
                let before = self.pump.screen();
                let applied = self.pump.receive_report(record);
                (Showing::Pump, before, applied, self.pump.screen())
            }
            (EXTRA_APP, Some(extra)) => {
                let before = extra.screen();
                let applied = extra.receive_report(record);
                (Showing::Extra, before, applied, extra.screen())
            }
            _ => {
                // No such application: fail closed, every example wiped.
                self.reset_examples();
                self.show_launcher();
                return Err(KioskUiError::UnknownApp(app));
            }
        };
        // The example showing went back to its welcome screen - a session
        // ended, or it failed closed: the launcher shows instead.
        if self.showing == example && before != "welcome" && after == "welcome" {
            self.show_launcher();
        }
        applied
    }

    fn actions(&self, app: usize) -> impl Iterator<Item = &Action> {
        let service_desk = self
            .service_desk
            .actions()
            .filter(move |_| app == SERVICE_DESK_APP);
        let visitor = self.visitor.actions().filter(move |_| app == VISITOR_APP);
        let pump = self.pump.actions().filter(move |_| app == PUMP_APP);
        let extra = self
            .extra
            .iter()
            .flat_map(FormUi::actions)
            .filter(move |_| app == EXTRA_APP);
        service_desk.chain(visitor).chain(pump).chain(extra)
    }

    fn sent(&mut self, app: usize, count: usize) {
        match (app, self.extra.as_mut()) {
            (SERVICE_DESK_APP, _) => self.service_desk.sent(count),
            (VISITOR_APP, _) => self.visitor.sent(count),
            (PUMP_APP, _) => self.pump.sent(count),
            (EXTRA_APP, Some(extra)) => extra.sent(count),
            _ => {}
        }
    }

    fn dropped(&self) -> u32 {
        self.service_desk
            .dropped()
            .saturating_add(self.visitor.dropped())
            .saturating_add(self.pump.dropped())
            .saturating_add(self.extra.as_ref().map_or(0, FormUi::dropped))
    }

    fn reset(&mut self) {
        // Every example, not only the one showing: a UI PD gives up on all.
        self.reset_examples();
        self.show_launcher();
    }

    fn seconds_left(&self) -> Option<u32> {
        match (self.showing, self.extra.as_ref()) {
            (Showing::ServiceDesk, _) => self.service_desk.seconds_left(),
            (Showing::Visitor, _) => self.visitor.seconds_left(),
            (Showing::Pump, _) => self.pump.seconds_left(),
            (Showing::Extra, Some(extra)) => extra.seconds_left(),
            (Showing::Launcher | Showing::Extra | Showing::Gallery, _) => None,
        }
    }

    fn idle_after(&self) -> Option<u32> {
        match (self.showing, self.slide) {
            (Showing::Launcher, Some(_)) => Some(SLIDE_FOR),
            (Showing::Launcher, None) if self.nav.current() == LAUNCHER => Some(ATTRACT_AFTER),
            (Showing::Gallery, _) => Some(GALLERY_IDLE),
            _ => None,
        }
    }

    fn idle(&mut self) -> bool {
        // The gallery left alone closes, wiped.
        if self.showing == Showing::Gallery {
            self.show_launcher();
            return true;
        }
        let next = match (self.showing, self.slide) {
            (Showing::Launcher, Some(place)) => place
                .saturating_add(1)
                .checked_rem(slides::<E>())
                .unwrap_or(0),
            (Showing::Launcher, None) if self.nav.current() == LAUNCHER => 0,
            _ => return false,
        };
        self.show_slide(next);
        self.slide == Some(next)
    }

    fn follows_countdown(&self) -> bool {
        match (self.showing, self.extra.as_ref()) {
            (Showing::ServiceDesk, _) => self.service_desk.follows_countdown(),
            (Showing::Visitor, _) => self.visitor.follows_countdown(),
            (Showing::Pump, _) => self.pump.follows_countdown(),
            (Showing::Extra, Some(extra)) => extra.follows_countdown(),
            (Showing::Launcher | Showing::Extra | Showing::Gallery, _) => false,
        }
    }

    fn countdown_area(&self) -> Option<Rect> {
        match (self.showing, self.extra.as_ref()) {
            (Showing::ServiceDesk, _) => self.service_desk.countdown_area(),
            (Showing::Visitor, _) => self.visitor.countdown_area(),
            (Showing::Pump, _) => self.pump.countdown_area(),
            (Showing::Extra, Some(extra)) => extra.countdown_area(),
            (Showing::Launcher | Showing::Extra | Showing::Gallery, _) => None,
        }
    }

    fn theme(&self) -> (&'static str, Theme<Rgb888>) {
        let brand = match self.showing {
            // Each attract slide in its example's brand; the idea's
            // in the launcher's.
            Showing::Launcher => self
                .slide
                .and_then(slide::<E>)
                .map_or(brands::LAUNCHER, |(_, _, brand)| brand),
            Showing::ServiceDesk => brands::SERVICE_DESK,
            Showing::Visitor => brands::ORRINVALE,
            Showing::Pump => brands::TARNWICK,
            Showing::Extra => E::TILE.map_or(brands::LAUNCHER, |tile| tile.look),
            Showing::Gallery => brands::LAUNCHER,
        };
        brand.look(self.larger)
    }
}
