// SPDX-License-Identifier: MIT OR Apache-2.0
//! `make dev`'s program: the showcase, or one of its examples by itself,
//! running on this machine in a browser window (NoWayOS's dev window), its
//! screens joined to its applications as their PDs join them
//! (`showcase-kiosk-dev`).
//!
//! ```text
//! showcase-dev APP [--size WxH] [--scale N] [--listen ADDRESS:PORT] [--session FILE]
//! ```
//!
//! `APP` is `showcase` (the launcher and every example, as the Pi boots
//! it), `kiosk` (the reference kiosk), `visitor` or `pump`. The screen is
//! the Pi's monitor, 1920 x 1080 at device scale 2, unless `--size` and
//! `--scale` say otherwise.
//!
//! A developer's tool, host only: no part of any system image.

use std::env;
use std::process::ExitCode;

use nowayos_dev_window::Options;
use nowayos_display::DeviceScale;
use showcase_app::{NoExtra, Showcase};
use showcase_kiosk_dev::{Shape, Stand, run};
use showcase_kiosk_screens::app::KioskApp;
use showcase_kiosk_screens::kiosk::KioskUi;
use showcase_pump::PumpUi;
use showcase_visitor::VisitorUi;

/// The epoch each application starts in: any but 0.
const EPOCH: u32 = 1;

const USAGE: &str = "usage: showcase-dev showcase|kiosk|visitor|pump [--size WxH] [--scale 1|2] [--listen ADDRESS:PORT] [--session FILE]";

fn main() -> ExitCode {
    match start(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("showcase-dev: {message}");
            ExitCode::FAILURE
        }
    }
}

fn start(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let app = args.next().ok_or(USAGE)?;
    let mut shape = Shape::PI;
    // The screen's shape is this program's; the rest are the window's.
    let mut rest = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                let text = args.next().ok_or(USAGE)?;
                let size = text
                    .split_once(['x', 'X'])
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .filter(|&(w, h): &(u32, u32)| w > 0 && h > 0);
                (shape.width, shape.height) =
                    size.ok_or_else(|| format!("--size takes WIDTHxHEIGHT, not {text:?}"))?;
            }
            "--scale" => {
                let text = args.next().ok_or(USAGE)?;
                shape.scale = match text.as_str() {
                    "1" => DeviceScale::ONE,
                    "2" => DeviceScale::TWO,
                    _ => return Err(format!("--scale takes 1 or 2, not {text:?}")),
                };
            }
            _ => rest.push(arg),
        }
    }
    let options = |title: &str| Options::from_args(title, rest.iter().cloned());
    match app.as_str() {
        "showcase" => run(&options("The showcase")?, || {
            let apps = vec![
                KioskApp::new(EPOCH)?,
                KioskApp::with(&showcase_visitor::EXAMPLE, EPOCH)?,
                KioskApp::with(&showcase_pump::EXAMPLE, EPOCH)?,
            ];
            Stand::new(Showcase::<NoExtra>::new, apps, shape)
        }),
        "kiosk" => run(&options("The reference kiosk")?, || {
            Stand::new(KioskUi::new, vec![KioskApp::new(EPOCH)?], shape)
        }),
        "visitor" => run(&options("Visitor sign-in")?, || {
            let apps = vec![KioskApp::with(&showcase_visitor::EXAMPLE, EPOCH)?];
            Stand::new(VisitorUi::new, apps, shape)
        }),
        "pump" => run(&options("Pay at the pump")?, || {
            let apps = vec![KioskApp::with(&showcase_pump::EXAMPLE, EPOCH)?];
            Stand::new(PumpUi::new, apps, shape)
        }),
        _ => Err(format!("no application called {app:?}\n{USAGE}")),
    }
}
