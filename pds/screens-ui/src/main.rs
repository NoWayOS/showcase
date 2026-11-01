// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase-screens-ui`: the UI PD that shows Showcase's ten kiosk reference
//! screens - the React screens as native Rust - and tours them with their
//! own buttons: the kitchen sink (`showcase_kiosk_screens::sink`).
//!
//! It is shaped as NoWayOS's UI demo PD is: `nowayos-ui-loop` does the
//! display contract, the framebuffer it shares with the display PD, the
//! renderer and the rings; this file adds the screens and turns a
//! notification into a call to the loop. Input arrives from the input
//! broker as canonical `InputEvent` records - keys typed on the Mac, through
//! the debug UART, on the Pi 5 - and after each batch the UI draws and
//! presents a frame; once the display has shown it, the UI tells the broker
//! it is ready for more. A malformed record is rejected after it has been
//! copied out of the ring, and the rest of the batch is still handled.
//!
//! **A UI app, not the kiosk.** It follows one fixed tour with just enough
//! of a form's behaviour to show the screens; the kiosk's rules live in the
//! application PD (`showcase-kiosk-flow`). It prints the screen it shows and
//! each action it takes - never what was typed.
//!
//! No `unsafe` code, no device, no interrupt (its role is `ui`).

#![no_std]
#![no_main]

use nowayos_display::UiEvent;
use nowayos_ui_loop::{LoopError, UiChannels, UiLoop};
use sel4_microkit::{Channel, ChannelSet, Handler, Infallible, debug_println, protection_domain};
use showcase_kiosk_screens::sink::{KitchenSink, action_name};

/// The channel to the display PD.
const DISPLAY: Channel = Channel::new(0);

/// The channel to the input broker.
const INPUT: Channel = Channel::new(1);

#[protection_domain]
fn init() -> UiPd {
    debug_println!("SHOWCASE: ui: starting");
    let state = match start() {
        Ok(state) => Some(state),
        Err(error) => {
            debug_println!("SHOWCASE: ui: gave up: {error}");
            None
        }
    };
    UiPd { state }
}

fn start() -> Result<State, LoopError> {
    let channels = UiChannels::bind()?;
    // A mouse's pointer drawn over the screens.
    let ui = UiLoop::new(channels.frame_len()).with_pointer();
    let sink = KitchenSink::new(ui.theme().metrics()).map_err(|_| LoopError::Ring)?;
    Ok(State {
        channels,
        ui,
        sink,
        shown: "",
    })
}

/// Everything the UI PD holds once started.
struct State {
    channels: UiChannels,
    ui: UiLoop,
    sink: KitchenSink,
    /// The screen last reported.
    shown: &'static str,
}

impl State {
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
                        self.ui.scale().unwrap_or(1)
                    );
                    // The tour laid out in the display's own pixels, at its
                    // scale.
                    let viewport = self.ui.viewport().ok_or(LoopError::Ring)?;
                    self.sink
                        .resize(viewport, self.ui.metrics())
                        .map_err(|_| LoopError::Ring)?;
                    self.present()?;
                }
                UiEvent::PresentDone(_) => INPUT.notify(),
            }
        }
        Ok(())
    }

    /// Takes every input record waiting, then draws and presents the
    /// result.
    fn take_input(&mut self) -> Result<(), LoopError> {
        let mut any = false;
        while let Some(record) = self.channels.next_input()? {
            any = true;
            match record {
                Ok(event) => {
                    if let Some(action) = self.sink.handle(event).map_err(|_| LoopError::Ring)? {
                        debug_println!("SHOWCASE: ui: action {}", action_name(action));
                    }
                }
                Err(error) => debug_println!("SHOWCASE: ui: input record rejected: {error}"),
            }
        }
        if any {
            self.present()?;
        }
        Ok(())
    }

    /// Draws what changed - the whole screen when it changes - and presents
    /// it, saying which screen it is when that changed.
    fn present(&mut self) -> Result<(), LoopError> {
        let screen = self.sink.screen();
        if screen != self.shown {
            self.shown = screen;
            debug_println!("SHOWCASE: ui: showing {screen}");
        }
        let present = self
            .ui
            .draw_dirty(self.sink.tree_mut(), self.channels.frame())?;
        self.channels.send(&present)?;
        DISPLAY.notify();
        Ok(())
    }
}

/// The UI PD: `None` once it has given up.
struct UiPd {
    state: Option<State>,
}

impl Handler for UiPd {
    type Error = Infallible;

    fn notified(&mut self, channels: ChannelSet) -> Result<(), Self::Error> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };
        let mut result = Ok(());
        if channels.contains(DISPLAY) {
            result = state.serve();
        }
        if result.is_ok() && channels.contains(INPUT) {
            result = state.take_input();
        }
        if let Err(error) = result {
            debug_println!("SHOWCASE: ui: gave up: {error}");
            self.state = None;
        }
        Ok(())
    }
}
