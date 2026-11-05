// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-kiosk-ui`: the reference kiosk's UI PD: the kiosk
//! reference screens - the React screens as native Rust - driven by the
//! kiosk flow in the application PD, over the app-UI contract.
//!
//! A library and a program: the PD is [`init`], generic over the
//! screens it drives (`showcase_kiosk_screens::kiosk::KioskScreens`). This
//! crate's program runs the reference kiosk's UI alone
//! (`showcase_kiosk_screens::kiosk::KioskUi`); `showcase-ui` runs the
//! showcase's launcher with the same PD.
//!
//! It is shaped as Showcase's kitchen sink UI PD is: `nowayos-ui-loop` does
//! the display contract, the framebuffer it shares with the display PD, the
//! renderer and the input ring from the broker; `showcase_kiosk_screens::
//! kiosk::KioskUi` decides what to show. This file adds the two rings to
//! the application PD: each batch of input becomes the contract's actions,
//! sent to the application; each report back is copied out of its ring,
//! decoded and shown. A report that cannot be decoded is a faulty or
//! hostile application: the UI wipes everything it holds, shows the
//! welcome screen and asks the application to end any session.
//!
//! It draws only when the display has shown the last frame, and tells the
//! broker it is ready for more input once it has. A countdown's tick, and
//! the live figures that follow it, repaint only what changed; with
//! screens that may use the timer (`KioskScreens::TIMES_TICKS`) it says
//! how long each tick took to draw, the time per update to record on the
//! Pi. It draws in the theme
//! the screen showing asks for (`KioskScreens::theme`) - the
//! showcase's launcher and each example in its own brand - and when that
//! changes, lays every screen out again in the theme's metrics and draws
//! the next frame whole. It prints the screen it
//! shows and each button pressed - never what was typed.
//!
//! **It fails closed.** If a ring or the display contract breaks, it wipes
//! everything it holds, asks the application to end any session if the
//! ring to it still works, and goes out of service: it takes no more input
//! or reports, and only presents the wiped welcome screen - at once, or as
//! soon as the display has shown the frame before - so no answer stays on
//! the monitor. It says so only once that frame is presented; if the
//! display cannot take it, it says the screen may still show the last
//! frame.
//!
//! No `unsafe` code, no device, no interrupt (its role is `ui`).
//!
//! **One application PD or more**: the screens say how many they
//! talk to (`KioskScreens::APPS`, at most [`MAX_APPS`]) - one for the
//! reference kiosk, one for each of the showcase's examples - and each has
//! its own two rings and channel; a report from one reaches only the
//! screens' part for it, and each part's actions go only to its own.
//!
//! **An idle clock, only for screens that want one**: the
//! showcase's launcher asks to be told when it has had no input for a while
//! (`KioskScreens::idle_after`), for its attract loop; the PD then asks the
//! timer to wake it then. A form kiosk's screens never ask - their time
//! comes from the application - so the reference kiosk's UI never calls the
//! timer, and its system grants it none. If the timer fails, the clock
//! stops and the screens stay as they are: nothing else depends on it.
//!
//! Channels: 0 the display PD, 1 the input broker, 2 + *n* application PD
//! *n*, 6 the timer PD (`pp`, only in a system whose screens want the idle
//! clock). Regions: those of `nowayos_ui_loop::UiChannels`, and for
//! application *n* the ring to it (`nowayos_rw_region_{4 + 2n}` messages
//! read-write, `nowayos_ro_region_{3 + 2n}` control read-only) and from it
//! (`nowayos_rw_region_{5 + 2n}` control read-write,
//! `nowayos_ro_region_{4 + 2n}` messages read-only).

#![no_std]

use nowayos_bounded::Wipe;
use nowayos_display::{DeviceScale, Rect, UiEvent};
use nowayos_ipc::{Consumer, Producer};
use nowayos_region::{RingRx, RingTx, read_only, read_write, ring_consumer, ring_producer};
use nowayos_render_eg::{DEFAULT_BYTES, DEFAULT_SLOTS, Glyphs};
use nowayos_timer_api::Client;
use nowayos_ui::{TextMetrics, Tree, UiError};
use nowayos_ui_loop::{LoopError, UiChannels, UiLoop};
use sel4_microkit::{Channel, ChannelSet, Handler, Infallible, NullHandler, debug_println};
use showcase_kiosk_contract::MAX_RECORD_LEN;
use showcase_kiosk_screens::generated::{CAPACITY, TEXT};
use showcase_kiosk_screens::kiosk::{KioskScreens, RING_SLOTS};

/// The channel to the display PD.
const DISPLAY: Channel = Channel::new(0);

/// The channel to the input broker.
const INPUT: Channel = Channel::new(1);

/// The timer PD, called on channel 6, after the most application PDs' -
/// only by screens that want the idle clock.
const TIMER: Client = Client::new(Channel::new(6));

/// The one wake-up the idle clock sets.
const WAKE: u64 = 0;

/// The most application PDs: one for each of the showcase's examples.
/// NoWayOS's sixteen regions of each kind, less the display's and
/// the input's, leave room for six.
pub const MAX_APPS: usize = 4;

/// The channel to application PD `app`.
const fn app_channel(app: usize) -> Channel {
    Channel::new(app.saturating_add(2))
}

/// The two rings to and from one application PD.
struct AppRings {
    to_app: RingTx<RING_SLOTS, MAX_RECORD_LEN>,
    from_app: RingRx<RING_SLOTS, MAX_RECORD_LEN>,
}

/// Binds the rings of application PD `app`.
fn bind_app(app: usize) -> Result<AppRings, LoopError> {
    let at = app.saturating_mul(2);
    let to_app = ring_producer(
        read_write(at.saturating_add(4))?,
        read_only(at.saturating_add(3))?,
    )?;
    let from_app = ring_consumer(
        read_write(at.saturating_add(5))?,
        read_only(at.saturating_add(4))?,
    )?;
    Ok(AppRings { to_app, from_app })
}

/// Starts the UI PD on the screens `make` makes, with the theme's metrics:
/// the reference kiosk's UI (`KioskUi::new`), or the showcase's - and runs
/// its event loop, never returning. The glyph store's storage lives in this
/// frame, as a PD has no heap: every system gives the UI PD a stack to hold
/// it (`stack_size="0x600000"`).
pub fn init<D: KioskScreens>(make: fn(TextMetrics) -> Result<D, UiError>) -> NullHandler {
    debug_println!("SHOWCASE: ui: starting");
    let mut pixels = [0_u8; DEFAULT_BYTES];
    let mut slots = [nowayos_render_eg::Slot::EMPTY; DEFAULT_SLOTS];
    let glyphs = Glyphs::new(&mut pixels, &mut slots);
    let state = match start(make, glyphs) {
        Ok(state) => Some(state),
        Err(error) => {
            debug_println!("SHOWCASE: ui: gave up: {error}; nothing was shown");
            None
        }
    };
    run_forever(&mut UiPd { state })
}

/// rust-sel4's event loop for `pd`, which never returns.
#[allow(
    unreachable_code,
    reason = "Handler::run never returns: its type, Result<Never, Infallible>, has no value"
)]
fn run_forever<D: KioskScreens>(pd: &mut UiPd<'_, D>) -> ! {
    match pd.run() {
        Ok(never) => match never {},
        Err(infallible) => match infallible {},
    }
}

fn start<D: KioskScreens>(
    make: fn(TextMetrics) -> Result<D, UiError>,
    glyphs: Glyphs<'_>,
) -> Result<State<'_, D>, LoopError> {
    let channels = UiChannels::bind()?;
    if D::APPS == 0 || D::APPS > MAX_APPS {
        return Err(LoopError::Ring);
    }
    // Only the application PDs the screens talk to: a system grants no
    // more regions than that.
    let mut apps: [Option<AppRings>; MAX_APPS] = [None, None, None, None];
    for (app, rings) in apps.iter_mut().enumerate().take(D::APPS) {
        *rings = Some(bind_app(app)?);
    }
    // A mouse's pointer drawn over the screens.
    let ui = UiLoop::new(channels.frame_len()).with_pointer();
    let kiosk = make(ui.theme().metrics()).map_err(|_| LoopError::Ring)?;
    Ok(State {
        channels,
        ui,
        glyphs,
        kiosk,
        apps,
        // No theme yet: the first frame draws in the one the first screen
        // asks for (`restyle`).
        theme: "",
        shown: "",
        counted: None,
        ticked: false,
        dirty: false,
        dropped: 0,
        closed: false,
        notice: None,
        drawn: None,
        idle: Idle::default(),
        frame_at: None,
    })
}

/// A record's bytes on the stack, zeroed by [`Wipe`] once sent.
struct Slot([u8; MAX_RECORD_LEN]);

impl Wipe for Slot {
    fn wipe(&mut self) {
        self.0.fill(0);
    }
}

/// Everything the UI PD holds once started.
struct State<'g, D> {
    channels: UiChannels,
    ui: UiLoop,
    /// Where text's glyphs come from: drawn from the outlines, kept.
    glyphs: Glyphs<'g>,
    kiosk: D,
    /// Each application PD's rings, by number; `None` past the screens'
    /// count.
    apps: [Option<AppRings>; MAX_APPS],
    /// The name of the theme the UI draws in.
    theme: &'static str,
    /// The screen last reported.
    shown: &'static str,
    /// The seconds left last reported, while a screen counts down.
    counted: Option<u32>,
    /// A countdown ticked on the screen showing since the last frame: the
    /// next frame says what it drew.
    ticked: bool,
    /// The screen changed since the last frame presented.
    dirty: bool,
    /// Actions dropped, as last reported.
    dropped: u32,
    /// Out of service: no input or reports taken any more.
    closed: bool,
    /// Why it went out of service, to say once the wiped welcome screen is
    /// presented.
    notice: Option<LoopError>,
    /// The tree the last frame drew, compared by address only: while the
    /// same tree shows, a frame draws only what changed in it.
    drawn: Option<*const Tree<CAPACITY, TEXT>>,
    /// The idle clock.
    idle: Idle,
    /// When the frame clock last ticked, in milliseconds (#10086).
    frame_at: Option<u64>,
}

/// The idle clock's state.
#[derive(Default)]
struct Idle {
    /// When the screens last had input, or changed screen, in milliseconds.
    since: u64,
    /// The screen showing then.
    screen: &'static str,
    /// The wake-up set, if any, in milliseconds.
    armed: Option<u64>,
    /// Input came since the clock last looked.
    input: bool,
    /// The timer failed: the clock has stopped.
    stopped: bool,
}

impl<D: KioskScreens> State<'_, D> {
    /// Takes every record from the display and acts on it.
    fn serve(&mut self) -> Result<(), LoopError> {
        while let Some(record) = self.channels.next_from_display()? {
            match self.ui.receive(&record)? {
                UiEvent::Configured(layout) => {
                    debug_println!(
                        "SHOWCASE: ui: the display is {}x{} {:?} at scale {}",
                        layout.width(),
                        layout.height(),
                        layout.format(),
                        self.ui.scale().unwrap_or(DeviceScale::ONE)
                    );
                    // The screens laid out in the display's own pixels, at
                    // its scale.
                    let viewport = self.ui.viewport().ok_or(LoopError::Ring)?;
                    self.kiosk
                        .resize(viewport, self.ui.metrics())
                        .map_err(|_| LoopError::Ring)?;
                    self.note_screen();
                    self.dirty = true;
                    // Transitions and animations, now the frame clock runs
                    // (#10086).
                    self.kiosk
                        .tree_mut()
                        .set_motion(nowayos_ui::motion::MotionSetting::Full);
                }
                UiEvent::PresentDone(_) => {
                    // The frame clock: while anything moves, the next frame.
                    self.next_frame()?;
                    // Ready for more input once nothing waits to be drawn.
                    if !self.dirty && !self.closed {
                        INPUT.notify();
                    }
                }
            }
        }
        self.present()
    }

    /// The frame clock (#10086): once the display has shown a frame, while
    /// anything on the screen moves, lets the time since the last pass and
    /// presents what moved - so motion runs at the display's rate. Without
    /// the timer, a frame counts as a sixtieth of a second.
    fn next_frame(&mut self) -> Result<(), LoopError> {
        if !self.kiosk.tree().is_moving() {
            self.frame_at = None;
            return Ok(());
        }
        let now = TIMER.now().ok().map(|now| now.as_nanos() / 1_000_000);
        let elapsed = match (now, self.frame_at) {
            (Some(now), Some(then)) => u32::try_from(now.saturating_sub(then)).unwrap_or(u32::MAX),
            _ => 16,
        };
        self.frame_at = now;
        if let Some(present) = self.ui.tick(
            self.kiosk.tree_mut(),
            self.channels.frame(),
            &mut self.glyphs,
            elapsed,
        )? {
            self.channels.send(&present)?;
            DISPLAY.notify();
        }
        Ok(())
    }

    /// Takes every input record waiting, then sends the actions it made.
    fn take_input(&mut self) -> Result<(), LoopError> {
        let mut any = false;
        while let Some(record) = self.channels.next_input()? {
            any = true;
            match record {
                Ok(event) => {
                    if let Some(label) = self.kiosk.handle(event) {
                        debug_println!("SHOWCASE: ui: action {label}");
                    }
                }
                Err(error) => debug_println!("SHOWCASE: ui: input record rejected: {error}"),
            }
        }
        if any {
            self.idle.input = true;
            self.kiosk.end_input();
            // Input alone may change the screen - the showcase's launcher
            // and Home - before any report: say so before the application
            // hears of it.
            self.note_screen();
            self.dirty = true;
            self.send_all()?;
        }
        self.present()
    }

    /// Keeps the idle clock, for screens that want one: on the
    /// timer's wake-up (`woke`) tells the screens they are idle, then sets
    /// the next wake-up from the last input or change of screen. Screens
    /// that want none never reach the timer. A timer that fails stops the
    /// clock, said once; the screens stay as they are.
    fn watch_idle(&mut self, woke: bool) -> Result<(), LoopError> {
        if self.idle.stopped || (self.kiosk.idle_after().is_none() && self.idle.armed.is_none()) {
            return Ok(());
        }
        if self.tick_idle(woke).is_err() {
            self.idle.stopped = true;
            debug_println!("SHOWCASE: ui: the timer failed: the idle clock has stopped");
        }
        self.present()
    }

    /// [`watch_idle`](Self::watch_idle)'s work; `Err` if the timer failed.
    fn tick_idle(&mut self, woke: bool) -> Result<(), ()> {
        let now = TIMER.now().map_err(|_| ())?.as_nanos() / 1_000_000;
        if woke {
            while let Some(id) = TIMER.expired().map_err(|_| ())? {
                if id == WAKE {
                    self.idle.armed = None;
                }
            }
            if self.idle.armed.is_none() && self.kiosk.idle() {
                self.note_screen();
                self.dirty = true;
                self.idle.since = now;
            }
        }
        let screen = self.kiosk.screen();
        if core::mem::take(&mut self.idle.input) || screen != self.idle.screen {
            self.idle.since = now;
            self.idle.screen = screen;
        }
        let due = self
            .kiosk
            .idle_after()
            .map(|after| self.idle.since.saturating_add(u64::from(after)));
        if due != self.idle.armed {
            if self.idle.armed.take().is_some() {
                TIMER.cancel(WAKE).map_err(|_| ())?;
            }
            if let Some(at) = due {
                let millis = at.saturating_sub(now).max(1);
                TIMER
                    .set_timeout(WAKE, millis.saturating_mul(1_000_000))
                    .map_err(|_| ())?;
                self.idle.armed = Some(at);
            }
        }
        Ok(())
    }

    /// Takes every report from application `app` and shows what it asks,
    /// then sends any actions held back.
    fn take_reports(&mut self, app: usize) -> Result<(), LoopError> {
        loop {
            let rings = self
                .apps
                .get(app)
                .and_then(Option::as_ref)
                .ok_or(LoopError::Ring)?;
            let mut rx = Consumer::new(&rings.from_app).map_err(|_| LoopError::Ring)?;
            let Some(record) = rx.try_recv_record().map_err(|_| LoopError::Ring)? else {
                break;
            };
            if let Err(error) = self.kiosk.receive_report(app, record.as_bytes()) {
                debug_println!(
                    "SHOWCASE: ui: {error}; every answer wiped from memory, the welcome screen to follow, and the application asked to end the session"
                );
            }
            self.dirty = true;
        }
        self.note_screen();
        self.send_all()?;
        self.present()
    }

    /// Sends every application its actions waiting.
    /// Every application is tried, even after one's ring has failed, and
    /// the first failure returned after: the order matters when the UI
    /// fails closed (`close`), where a ring broken by one application -
    /// which can write a bad index into it - must not keep another
    /// application from its `Abandon`, and so its answers from their wipe.
    fn send_all(&mut self) -> Result<(), LoopError> {
        let mut result = Ok(());
        for app in 0..D::APPS {
            let sent = self.send_actions(app);
            if result.is_ok() {
                result = sent;
            }
        }
        result
    }

    /// Sends the actions waiting for application `app`, as many as its ring
    /// takes, and tells it. Each record's copy is zeroed once sent.
    fn send_actions(&mut self, app: usize) -> Result<(), LoopError> {
        let rings = self
            .apps
            .get(app)
            .and_then(Option::as_ref)
            .ok_or(LoopError::Ring)?;
        let mut tx = Producer::new(&rings.to_app).map_err(|_| LoopError::Ring)?;
        let mut sent: usize = 0;
        let mut slot = Slot([0; MAX_RECORD_LEN]);
        for action in self.kiosk.actions(app) {
            let Ok(len) = action.encode(&mut slot.0) else {
                break;
            };
            let Some(bytes) = slot.0.get(..len) else {
                break;
            };
            let delivered = tx.try_send_bytes(bytes).is_ok();
            slot.wipe();
            if !delivered {
                break;
            }
            sent = sent.saturating_add(1);
        }
        self.kiosk.sent(app, sent);
        if sent > 0 {
            app_channel(app).notify();
        }
        let dropped = self.kiosk.dropped();
        if dropped != self.dropped {
            self.dropped = dropped;
            debug_println!("SHOWCASE: ui: actions dropped so far: {dropped}");
        }
        Ok(())
    }

    /// Prints the screen showing, if it changed, and the seconds left of
    /// a screen counting down, as they change: time, never typing.
    fn note_screen(&mut self) {
        let screen = self.kiosk.screen();
        if screen != self.shown {
            self.shown = screen;
            debug_println!("SHOWCASE: ui: showing {screen}");
        }
        let counted = self.kiosk.seconds_left();
        if counted != self.counted {
            // A tick, not a new screen counting down from the start.
            self.ticked = self.counted.is_some() && counted.is_some();
            self.counted = counted;
            if let Some(seconds) = counted {
                debug_println!("SHOWCASE: ui: seconds left: {seconds}");
            }
        }
    }

    /// Draws what changed and presents it, if anything did and the display
    /// is ready for a frame: only the tree's dirty rectangles while the
    /// same tree shows, the whole screen when another tree takes
    /// its place - the showcase's launcher and its example are two - so
    /// no pixel of the one before is left.
    fn present(&mut self) -> Result<(), LoopError> {
        self.restyle()?;
        if !self.dirty || !self.ui.can_draw() {
            return Ok(());
        }
        let tree: *const Tree<CAPACITY, TEXT> = self.kiosk.tree();
        let only_changes = self.drawn == Some(tree);
        // A tick's drawing timed, where the system grants the timer.
        let started = if D::TIMES_TICKS && self.ticked && only_changes {
            TIMER.now().ok()
        } else {
            None
        };
        let present = if only_changes {
            self.ui.draw_dirty(
                self.kiosk.tree_mut(),
                self.channels.frame(),
                &mut self.glyphs,
            )?
        } else {
            self.ui
                .draw_all(self.kiosk.tree(), self.channels.frame(), &mut self.glyphs)?
        };
        self.drawn = Some(tree);
        if core::mem::replace(&mut self.ticked, false) && only_changes {
            self.note_tick(present.damage());
            if let Some(start) = started
                && let Ok(end) = TIMER.now()
            {
                let micros = end.as_nanos().saturating_sub(start.as_nanos()) / 1_000;
                debug_println!("SHOWCASE: ui: tick drawn in {micros} us");
            }
        }
        self.channels.send(&present)?;
        self.dirty = false;
        DISPLAY.notify();
        if let Some(error) = self.notice.take() {
            debug_println!(
                "SHOWCASE: ui: out of service: {error}; every answer wiped and the welcome screen shown"
            );
        }
        Ok(())
    }

    /// Draws in the theme the screen showing asks for, if it changed:
    /// every screen laid out again in its metrics, once the
    /// display has announced its layout - keeping what each holds - and
    /// the next frame drawn whole, as every pixel may change.
    fn restyle(&mut self) -> Result<(), LoopError> {
        let (name, theme) = self.kiosk.theme();
        if name == self.theme {
            return Ok(());
        }
        self.ui.set_theme(theme);
        self.theme = name;
        if let Some(viewport) = self.ui.viewport() {
            self.kiosk
                .resize(viewport, self.ui.metrics())
                .map_err(|_| LoopError::Ring)?;
        }
        self.drawn = None;
        self.dirty = true;
        debug_println!("SHOWCASE: ui: theme {name}");
        Ok(())
    }

    /// Says what a countdown's tick drew: its slot alone - the
    /// rest of the line its digits are on - or more, which no tick should,
    /// but on a screen that follows its countdown. Areas and sizes
    /// only: a countdown screen holds nothing typed.
    fn note_tick(&self, damage: &[Rect]) {
        let screen = self.ui.viewport().unwrap_or(Rect::EMPTY).size;
        match (damage, self.kiosk.countdown_area()) {
            ([drawn], Some(slot)) if slot.contains_rect(drawn) => {
                debug_println!(
                    "SHOWCASE: ui: countdown drawn alone: {}x{} px of the {}x{} screen",
                    drawn.size.width,
                    drawn.size.height,
                    screen.width,
                    screen.height
                );
            }
            _ => {
                let pixels = damage.iter().fold(0_u64, |sum, rect| {
                    let area =
                        u64::from(rect.size.width).saturating_mul(u64::from(rect.size.height));
                    sum.saturating_add(area)
                });
                // A screen that follows its countdown - the pump's litres
                // and pounds - draws them too, by design.
                let what = if self.kiosk.follows_countdown() {
                    "countdown and live figures drawn"
                } else {
                    "countdown drawn with more"
                };
                debug_println!(
                    "SHOWCASE: ui: {what}: {} area(s), {pixels} px",
                    damage.len()
                );
            }
        }
    }

    /// Fails closed after `error`: wipes everything, asks the application
    /// to end any session, and presents the wiped welcome screen - now, or
    /// once the display has shown the frame before.
    fn close(&mut self, error: LoopError) {
        self.closed = true;
        self.kiosk.reset();
        let _ = self.send_all();
        self.note_screen();
        self.dirty = true;
        self.notice = Some(error);
        if let Err(present) = self.present() {
            self.notice = None;
            debug_println!(
                "SHOWCASE: ui: out of service: {error}; every answer wiped from memory, but the screen could not be repainted ({present}): it may still show the last frame"
            );
        } else if self.notice.is_some() {
            debug_println!(
                "SHOWCASE: ui: out of service: {error}; every answer wiped from memory; the welcome screen follows once the display is ready"
            );
        }
    }
}

/// The UI PD's state: running, out of service (`closed`), or `None` if it
/// could not start or the display could not take the wiped screen.
pub struct UiPd<'g, D> {
    state: Option<State<'g, D>>,
}

impl<D: KioskScreens> Handler for UiPd<'_, D> {
    type Error = Infallible;

    fn notified(&mut self, channels: ChannelSet) -> Result<(), Self::Error> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };
        if state.closed {
            // Out of service: only the display, so the wiped welcome screen
            // is presented once it can be.
            if channels.contains(DISPLAY)
                && let Err(present) = state.serve()
            {
                debug_println!(
                    "SHOWCASE: ui: the screen could not be repainted ({present}): it may still show the last frame"
                );
                self.state = None;
            }
            return Ok(());
        }
        let mut result = Ok(());
        if channels.contains(DISPLAY) {
            result = state.serve();
        }
        if result.is_ok() && channels.contains(INPUT) {
            result = state.take_input();
        }
        for app in 0..D::APPS {
            if result.is_ok() && channels.contains(app_channel(app)) {
                result = state.take_reports(app);
            }
        }
        if result.is_ok() {
            result = state.watch_idle(channels.contains(TIMER.channel()));
        }
        if let Err(error) = result {
            state.close(error);
        }
        Ok(())
    }
}
