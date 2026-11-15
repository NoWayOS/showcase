// SPDX-License-Identifier: MIT OR Apache-2.0
//! A kiosk's screens and its applications on the developer's machine, in a
//! browser window: for trying a flow while writing it, with nothing booted.
//!
//! A [`Stand`] holds what a kiosk's PDs hold - the UI PD's screens (any
//! [`KioskScreens`]) and each application PD's [`KioskApp`] - and joins them
//! as the PDs do: every action and report encoded into a record and decoded
//! at the other end, each application over its own pair, time reaching the
//! applications at each of their deadlines and the screens' idle clock kept
//! as the UI PD keeps it. It is the same Rust that goes on the device, built
//! for the host; only the rings, the timer and the display are stood in for.
//!
//! [`Stand`] is an [`Application`] for NoWayOS's dev window
//! (`nowayos-dev-window`), which serves it on the loopback address. A
//! program gives [`run`] how to make its stand:
//!
//! ```no_run
//! use nowayos_dev_window::Options;
//! use showcase_kiosk_dev::{Shape, Stand, run};
//! use showcase_kiosk_screens::app::KioskApp;
//! use showcase_kiosk_screens::kiosk::KioskUi;
//!
//! let options = Options::new("The reference kiosk");
//! run(&options, || Stand::new(KioskUi::new, vec![KioskApp::new(1)?], Shape::PI))?;
//! # Ok::<(), String>(())
//! ```
//!
//! A developer's tool, host only: no part of any system image. Provisional.

use nowayos_dev_window::{Application, Options, Server};
use nowayos_display::{DeviceScale, Rect};
use nowayos_input::InputEvent;
use nowayos_render_eg::{Glyphs, Rgb888, Theme, render};
use nowayos_testkit::Framebuffer;
use nowayos_ui::{TextMetrics, Tree, UiError};
use showcase_kiosk_contract::MAX_RECORD_LEN;
use showcase_kiosk_flow::Tick;
use showcase_kiosk_screens::app::KioskApp;
use showcase_kiosk_screens::generated::{CAPACITY, TEXT};
use showcase_kiosk_screens::kiosk::KioskScreens;

/// The most deadlines met in one step of time: far more than any second
/// holds, so a clock that does not move on cannot hold the window.
const MAX_WAKES: usize = 10_000;

/// The screen a stand draws on: its size in pixels and its device scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The width, in pixels.
    pub width: u32,
    /// The height, in pixels.
    pub height: u32,
    /// The device scale: pixels each way for each layout unit.
    pub scale: DeviceScale,
}

impl Shape {
    /// The Raspberry Pi 5's HDMI monitor as the showcase draws on it: 1920
    /// x 1080 at device scale 2.
    pub const PI: Self = Self {
        width: 1920,
        height: 1080,
        scale: DeviceScale::TWO,
    };
}

/// A kiosk's screens joined to its applications.
pub struct Stand<S: KioskScreens> {
    screens: S,
    /// Application PD *n*'s application, in order.
    apps: Vec<KioskApp>,
    shape: Shape,
    /// The name of the theme the screens are laid out in.
    theme: &'static str,
    /// When the idle clock last started again, and on which screen.
    idle_since: Tick,
    idle_screen: &'static str,
    /// The first thing that went wrong joining the two, if anything did.
    trouble: Option<String>,
    /// The glyphs drawn so far, kept from frame to frame as a UI PD keeps
    /// them. Their storage is never given back: a megabyte or so for each
    /// stand made, in a program that makes one each time it is started
    /// again.
    glyphs: Glyphs<'static>,
}

impl<S: KioskScreens> Stand<S> {
    /// The screens `make` gives, laid out for `shape`, joined to `apps`:
    /// one for each application PD the screens talk to, in order.
    ///
    /// # Errors
    ///
    /// The wrong number of applications, or what refused the screens or
    /// their layout.
    pub fn new(
        make: impl FnOnce(TextMetrics) -> Result<S, UiError>,
        apps: Vec<KioskApp>,
        shape: Shape,
    ) -> Result<Self, String> {
        if apps.len() != S::APPS {
            return Err(format!(
                "these screens talk to {} application(s), and {} were given",
                S::APPS,
                apps.len()
            ));
        }
        let screens = make(Theme::<Rgb888>::light().metrics()).map_err(|err| err.to_string())?;
        let (pixels, slots) = nowayos_testkit::glyph_storage();
        let glyphs = Glyphs::new(
            Box::leak(pixels.into_boxed_slice()),
            Box::leak(slots.into_boxed_slice()),
        );
        let mut stand = Self {
            glyphs,
            idle_screen: screens.screen(),
            screens,
            apps,
            shape,
            theme: "",
            idle_since: nowayos_dev_window::START,
            trouble: None,
        };
        stand.restyle()?;
        stand.pump(nowayos_dev_window::START);
        Ok(stand)
    }

    /// The screens, for a program that wants to look.
    pub const fn screens(&self) -> &S {
        &self.screens
    }

    /// Lays the screens out in the theme the screen showing asks for, if
    /// that changed, as the UI PD does.
    fn restyle(&mut self) -> Result<Theme<Rgb888>, String> {
        let (name, theme) = self.screens.theme();
        if name != self.theme {
            let viewport = Rect::new(0, 0, self.shape.width, self.shape.height);
            self.screens
                .resize(viewport, theme.metrics_at(self.shape.scale))
                .map_err(|err| err.to_string())?;
            self.theme = name;
        }
        Ok(theme)
    }

    /// Notes the first thing that goes wrong; the window's status shows it.
    fn note(&mut self, what: impl FnOnce() -> String) {
        if self.trouble.is_none() {
            self.trouble = Some(what());
        }
    }

    /// Sends the screens' actions to each application and each one's
    /// reports back, until neither has more: every record encoded and read
    /// again, as on the rings.
    fn pump(&mut self, now: Tick) {
        for _ in 0..MAX_WAKES {
            let mut moved = false;
            for index in 0..self.apps.len() {
                let Some(app) = self.apps.get_mut(index) else {
                    continue;
                };
                let reports: Vec<_> = app.reports().copied().collect();
                app.sent(reports.len());
                while app.take_note().is_some() {}
                for report in reports {
                    let mut slot = [0u8; MAX_RECORD_LEN];
                    if report.encode(&mut slot).is_err() {
                        self.note(|| format!("application {index}'s report did not encode"));
                    } else if let Err(err) = self.screens.receive_report(index, &slot) {
                        self.note(|| format!("application {index}'s report was refused: {err:?}"));
                    }
                    moved = true;
                }
                let actions: Vec<_> = self.screens.actions(index).cloned().collect();
                self.screens.sent(index, actions.len());
                for action in actions {
                    let mut slot = [0u8; MAX_RECORD_LEN];
                    if action.encode(&mut slot).is_err() {
                        self.note(|| format!("an action for application {index} did not encode"));
                    } else if let Some(app) = self.apps.get_mut(index) {
                        app.receive(&slot, now);
                    }
                    moved = true;
                }
            }
            if !moved {
                return;
            }
        }
    }
}

impl<S: KioskScreens> Application for Stand<S> {
    fn input(&mut self, events: &[InputEvent], now: u64) {
        for event in events {
            // What was pressed is the PD's to log; nothing is logged here.
            let _ = self.screens.handle(*event);
        }
        self.screens.end_input();
        self.pump(now);
        self.idle_since = now;
    }

    fn advance(&mut self, now: u64) -> bool {
        let mut changed = false;
        // Each application's deadlines up to now, the earliest first.
        for _ in 0..MAX_WAKES {
            let next = self.apps.iter().filter_map(KioskApp::next_wake).min();
            let Some(at) = next.filter(|&at| at <= now) else {
                break;
            };
            for app in &mut self.apps {
                app.tick(at);
            }
            self.pump(at);
            changed = true;
        }
        // The screens' idle clock: from the last input or change of screen.
        let screen = self.screens.screen();
        if screen != self.idle_screen {
            self.idle_screen = screen;
            self.idle_since = now;
        }
        if let Some(after) = self.screens.idle_after()
            && now >= self.idle_since.saturating_add(u64::from(after))
        {
            changed |= self.screens.idle();
            self.idle_since = now;
            self.idle_screen = self.screens.screen();
        }
        changed
    }

    fn draw(&mut self) -> Result<Framebuffer, String> {
        let theme = self.restyle()?;
        paint(self.screens.tree(), &theme, &mut self.glyphs, self.shape)
    }

    fn status(&self) -> String {
        match &self.trouble {
            Some(trouble) => format!("{} - {trouble}", self.screens.screen()),
            None => self.screens.screen().to_owned(),
        }
    }
}

/// Draws a tree. Apart from [`Stand`], and of no type of its own, so that
/// it is built once, in this crate - which `make dev`'s profile optimises -
/// and not again, unoptimised, in each program for its own screens.
fn paint(
    tree: &Tree<CAPACITY, TEXT>,
    theme: &Theme<Rgb888>,
    glyphs: &mut Glyphs<'static>,
    shape: Shape,
) -> Result<Framebuffer, String> {
    let mut frame = Framebuffer::new(shape.width, shape.height).map_err(|err| err.to_string())?;
    render(tree, &mut frame, theme, glyphs).map_err(|err| format!("{err:?}"))?;
    Ok(frame)
}

/// Shows the stand `start` makes in NoWayOS's dev window until the program
/// is stopped, saying where to open it; `start` is called again for the
/// page's *Start again*.
///
/// # Errors
///
/// Why the window could not listen, or `start`'s error.
pub fn run<S: KioskScreens>(
    options: &Options,
    start: impl FnMut() -> Result<Stand<S>, String>,
) -> Result<(), String> {
    let server =
        Server::bind(options.clone()).map_err(|err| format!("{}: {err}", options.listen))?;
    let port = server
        .local_addr()
        .map_or(options.listen.port(), |address| address.port());
    println!("{}: open http://localhost:{port}/", options.title);
    server.run(start).map_err(|err| err.to_string())
}
