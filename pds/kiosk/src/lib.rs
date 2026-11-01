// SPDX-License-Identifier: MIT OR Apache-2.0
//! `showcase_kiosk_stand_in`: an example kiosk's application PD, with a
//! development stand-in for the service built in - so each program's name
//! ends `-stand-in`, and NoWayOS's grants check refuses it in a product
//! system (`[development_only]`, `*-stand-in.elf`).
//!
//! A library and a program: the PD is [`init`], given the example
//! it runs (`showcase_kiosk_screens::app::Example`) and the tag its log
//! lines start with. This crate's program, `showcase-kiosk-stand-in`, runs
//! the reference kiosk's; `showcase-visitor-stand-in` runs the showcase's
//! visitor sign-in.
//!
//! What it does is `showcase_kiosk_screens::app::KioskApp`, tested on the
//! host joined to the UI; this file moves records and time:
//!
//! * **It checks in before its first session**. It starts out of service, awaiting approval with its
//!   key's approval code - scripted, [`APPROVAL_CODE`], until a security PD
//!   makes the key - and checks in, as it starts, with the backend's
//!   stand-in, which answers *not approved* and then, checked in again at
//!   once, approves the kiosk with an epoch. The flow is made with that
//!   epoch, and the UI is told the welcome screen follows - all before the
//!   UI PD starts, so its log comes first.
//! * **Time first.** On every wake-up it asks the timer for the time and
//!   lets the application apply whatever has fallen due - the idle warning,
//!   the privacy reset, a submission's timeout, the stand-in's answer - at
//!   once, whatever room the ring to the UI has; the reports wait in the
//!   application's queue until there is room.
//! * **The UI's actions** are taken from their ring on every wake-up, each
//!   copied out before it is decoded, while the application takes them and
//!   the ring to the UI has room for their replies; if any are left, the
//!   PD wakes again shortly to take them.
//! * **It asks the timer to wake it** at the application's next deadline,
//!   so a session is wiped on time however quiet the UI is.
//! * **It fails closed on the faults it finds.** If the timer answers a
//!   call with an error or a malformed reply, or refuses a wake-up, or a
//!   ring's indices are impossible, the application ends any open session -
//!   its answers wiped - and goes out of service: the session's end and the
//!   welcome screen are sent to the UI as soon as its ring takes them, and
//!   from then on no action is taken and no session starts. The PD stays to
//!   deliver them and to drain the UI's ring. It cannot find a timer that
//!   goes silent or faults, or its own fault: those are the watchdog's and
//!   the supervisor's, which the reference kiosk lacks.
//!
//! It logs a session's end and why, what the stand-in does, a refused
//! record and going out of service - never an answer. No device, no
//! interrupt, no physical address (its role is `app`), no heap and no
//! `unsafe` code.
//!
//! Channels: 0 to the UI PD, both ways; 1 to the timer PD (`pp`). Regions:
//! `nowayos_rw_region_0` and `nowayos_ro_region_0`, the ring from the UI
//! (control read-write, messages read-only); `nowayos_rw_region_1` and
//! `nowayos_ro_region_1`, the ring to the UI (messages read-write, control
//! read-only).

#![no_std]

use nowayos_ipc::{Consumer, Producer};
use nowayos_region::{RingRx, RingTx, read_only, read_write, ring_consumer, ring_producer};
use nowayos_timer_api::Client;
use sel4_microkit::{Channel, ChannelSet, Handler, Infallible, debug_println};
use showcase_kiosk_contract::{
    APPROVAL_CODE_LEN, CheckInAnswer, EndReason, MAX_RECORD_LEN, MAX_REPLIES,
};
use showcase_kiosk_flow::Tick;
use showcase_kiosk_screens::app::{Example, Fault, KioskApp, Note};
use showcase_kiosk_screens::kiosk::RING_SLOTS;
use showcase_kiosk_screens::stand_in::Plan;

/// The channel to the UI PD.
const UI: Channel = Channel::new(0);

/// The timer PD, called on channel 1.
const TIMER: Client = Client::new(Channel::new(1));

/// The one timeout this PD sets: its next wake-up.
const WAKE: u64 = 1;

/// How soon to wake again when reports or actions are left waiting for
/// room, in milliseconds.
const RETRY_MS: Tick = 10;

/// The kiosk's key's approval code, scripted until a
/// security PD makes the key: invented, and public like any approval code.
pub const APPROVAL_CODE: [u8; APPROVAL_CODE_LEN] = [0x3F, 0xA1, 0x07, 0x5C, 0xE2, 0x19, 0x88, 0x40];

/// Starts the application PD on `example`, every log line starting `tag`
/// (`"SHOWCASE: app:"` for the reference kiosk).
pub fn init(example: &'static Example, tag: &'static str) -> AppPd {
    debug_println!("{tag} starting");
    let mut state = match start(example, tag) {
        Ok(state) => state,
        Err(error) => {
            debug_println!("{tag} gave up: {error}; no session was started");
            return AppPd { state: None };
        }
    };
    let timeouts = example.config.timeouts;
    debug_println!(
        "{tag} ready: a form of {} steps and {} fields; the idle warning after {} s, the reset after {} s",
        example.form.steps.len(),
        example.form.fields().count(),
        timeouts.idle_reset.saturating_sub(timeouts.idle_warning) / 1_000,
        timeouts.idle_reset / 1_000,
    );
    // The check-in, at once: its answers, the UI's first reports and any
    // wake-up the start needs. Nothing is timed at the welcome screen: the
    // next wake-up is asked for once a session starts.
    state.wake(Woken::default());
    AppPd { state: Some(state) }
}

/// What woke the PD.
#[derive(Debug, Clone, Copy, Default)]
struct Woken {
    /// The timer: its wake-up may have expired.
    timer: bool,
    /// The UI: it sent actions, or waits to send more.
    ui: bool,
}

/// Binds the rings and starts the application.
fn start(example: &Example, tag: &'static str) -> Result<State, &'static str> {
    let from_ui = read_write(0)
        .and_then(|control| ring_consumer(control, read_only(0)?))
        .map_err(|_| "the ring from the UI")?;
    let to_ui = read_write(1)
        .and_then(|messages| ring_producer(messages, read_only(1)?))
        .map_err(|_| "the ring to the UI")?;
    Ok(State {
        tag,
        app: KioskApp::checking_in(example, APPROVAL_CODE)?,
        from_ui,
        to_ui,
        armed: None,
        now: 0,
    })
}

/// Everything the PD holds once started.
struct State {
    /// The start of every line this PD prints.
    tag: &'static str,
    app: KioskApp,
    from_ui: RingRx<RING_SLOTS, MAX_RECORD_LEN>,
    to_ui: RingTx<RING_SLOTS, MAX_RECORD_LEN>,
    /// The wake-up set, if any.
    armed: Option<Tick>,
    /// The time at the last wake-up, in milliseconds.
    now: Tick,
}

impl State {
    /// One wake-up; `Err` names the fault that put the kiosk out of
    /// service.
    fn serve(&mut self, woken: Woken) -> Result<bool, Fault> {
        self.now = TIMER
            .now()
            .map(|now| now.as_nanos() / 1_000_000)
            .map_err(|_| Fault::Timer)?;
        if woken.timer {
            while let Some(id) = TIMER.expired().map_err(|_| Fault::Timer)? {
                if id == WAKE {
                    self.armed = None;
                }
            }
        }
        // The privacy reset first, whatever room the UI's ring has.
        self.app.tick(self.now);
        let mut sent = self.flush()?;
        let mut left = false;
        loop {
            let room = self.room()?;
            let mut rx = Consumer::new(&self.from_ui).map_err(|_| Fault::Ring)?;
            if self.app.fault().is_none() && !(self.app.takes_actions() && room) {
                left = rx.pending().map_err(|_| Fault::Ring)? > 0;
                break;
            }
            let Some(record) = rx.try_recv_record().map_err(|_| Fault::Ring)? else {
                break;
            };
            // Out of service, the record is dropped unread.
            self.app.receive(record.as_bytes(), self.now);
            sent |= self.flush()?;
        }
        let waiting = left || self.app.reports().next().is_some();
        self.wake_at(waiting)?;
        Ok(sent)
    }

    /// Whether the ring to the UI has room for one call's replies.
    fn room(&self) -> Result<bool, Fault> {
        let tx = Producer::new(&self.to_ui).map_err(|_| Fault::Ring)?;
        let free = tx.free_slots().map_err(|_| Fault::Ring)?;
        Ok(usize::try_from(free).unwrap_or(0) >= MAX_REPLIES)
    }

    /// Sends the waiting reports, as many as the ring takes. Returns
    /// whether any was sent.
    fn flush(&mut self) -> Result<bool, Fault> {
        let mut tx = Producer::new(&self.to_ui).map_err(|_| Fault::Ring)?;
        let mut count: usize = 0;
        for report in self.app.reports() {
            let mut slot = [0u8; MAX_RECORD_LEN];
            let Ok(len) = report.encode(&mut slot) else {
                break;
            };
            let Some(bytes) = slot.get(..len) else { break };
            if tx.try_send_bytes(bytes).is_err() {
                break;
            }
            count = count.saturating_add(1);
        }
        self.app.sent(count);
        Ok(count > 0)
    }

    /// Asks the timer to wake the PD at the application's next deadline,
    /// or shortly, if something is `waiting` for room.
    fn wake_at(&mut self, waiting: bool) -> Result<(), Fault> {
        let next = [
            self.app.next_wake(),
            waiting.then(|| self.now.saturating_add(RETRY_MS)),
        ]
        .into_iter()
        .flatten()
        .min();
        if next == self.armed {
            return Ok(());
        }
        if self.armed.is_some() {
            TIMER.cancel(WAKE).map_err(|_| Fault::Wake)?;
            self.armed = None;
        }
        if let Some(at) = next {
            let millis = at.saturating_sub(self.now).max(1);
            TIMER
                .set_timeout(WAKE, millis.saturating_mul(1_000_000))
                .map_err(|_| Fault::Wake)?;
            self.armed = Some(at);
        }
        Ok(())
    }

    /// Logs what the application noted.
    fn log(&mut self) {
        let tag = self.tag;
        while let Some(note) = self.app.take_note() {
            match note {
                Note::CheckIn { seq } => debug_println!("{tag} checking in: {seq}"),
                Note::CheckedIn(checked) => {
                    let seq = checked.seq;
                    match checked.answer {
                        CheckInAnswer::Approved { epoch } => {
                            debug_println!(
                                "{tag} check-in {seq}: approved, epoch {epoch}; in service"
                            );
                        }
                        CheckInAnswer::NotApproved => {
                            let [a, b, c, d, e, f, g, h] = APPROVAL_CODE;
                            debug_println!(
                                "{tag} check-in {seq}: not approved; awaiting approval, code {a:02X}{b:02X} {c:02X}{d:02X} {e:02X}{f:02X} {g:02X}{h:02X}"
                            );
                        }
                        CheckInAnswer::Failed => {
                            debug_println!("{tag} check-in {seq}: failed; no service");
                        }
                    }
                }
                Note::Revoked => debug_println!(
                    "{tag} a submission answered not approved: no new session until a restart"
                ),
                Note::Network(fault) => {
                    debug_println!("{tag} a record to or from the network path refused: {fault}");
                }
                Note::StandIn {
                    attempt,
                    epoch,
                    plan,
                } => match plan {
                    Plan::Unanswered => {
                        debug_println!(
                            "{tag} the service stand-in leaves attempt {attempt} unanswered (epoch {epoch})"
                        );
                    }
                    Plan::AcceptAt(_) => {
                        debug_println!(
                            "{tag} the service stand-in will accept attempt {attempt} (epoch {epoch})"
                        );
                    }
                    Plan::FailAt(_) => {
                        debug_println!(
                            "{tag} the service stand-in will fail attempt {attempt} (epoch {epoch})"
                        );
                    }
                },
                Note::Accepted { attempt } => {
                    debug_println!("{tag} the service stand-in accepts attempt {attempt}");
                }
                Note::Failed { attempt } => {
                    debug_println!(
                        "{tag} the service stand-in fails attempt {attempt}: the service is down"
                    );
                }
                Note::Failing => {
                    debug_println!(
                        "{tag} demo: the service is down for this session, as the person asked"
                    );
                }
                Note::Ended(ended) => {
                    debug_println!(
                        "{tag} session {} ended: {}",
                        ended.session,
                        reason(ended.reason)
                    );
                }
                Note::Refused(error) => {
                    debug_println!(
                        "{tag} a record from the UI refused, and any open session ended: {error}"
                    );
                }
                Note::OutOfService(fault) => {
                    debug_println!(
                        "{tag} out of service: {fault}; any open session ended and its answers wiped"
                    );
                }
            }
        }
    }
}

/// The reason a session ended, for the log.
const fn reason(reason: EndReason) -> &'static str {
    match reason {
        EndReason::Cancelled => "cancelled",
        EndReason::Idle => "idle",
        EndReason::TimeLimit => "the session limit",
        EndReason::Submitted => "submitted",
        EndReason::Fault => "a fault",
        EndReason::Abandoned => "abandoned by the UI",
    }
}

/// The application PD: `None` only if it could not start.
pub struct AppPd {
    state: Option<State>,
}

impl AppPd {
    fn serve(&mut self, channels: ChannelSet) {
        if let Some(state) = self.state.as_mut() {
            state.wake(Woken {
                timer: channels.contains(TIMER.channel()),
                ui: channels.contains(UI),
            });
        }
    }
}

impl State {
    /// One wake-up - the first as the PD starts, woken by nothing.
    fn wake(&mut self, woken: Woken) {
        let sent = match self.serve(woken) {
            Ok(sent) => sent,
            Err(fault) => {
                // Fail closed: end the session, then tell the UI as far as
                // the ring allows - always, since a flush before the fault
                // may already have sent reports. The PD stays, out of
                // service, and wakes again only while reports wait for
                // room in a ring that works; a broken one is not retried.
                self.app.fail(fault, self.now);
                let ring_works = self.flush().is_ok();
                let waiting = ring_works && self.app.reports().next().is_some();
                let _ = self.wake_at(waiting);
                true
            }
        };
        self.log();
        // Tell the UI when there are reports, and after taking its actions,
        // so it sends any it held back.
        if sent || woken.ui {
            UI.notify();
        }
    }
}

impl Handler for AppPd {
    type Error = Infallible;

    fn notified(&mut self, channels: ChannelSet) -> Result<(), Self::Error> {
        self.serve(channels);
        Ok(())
    }
}
