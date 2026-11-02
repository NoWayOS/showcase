# Developing Showcase

What is in Showcase, how it uses NoWayOS, and how to build and test it. What Showcase is for: [README.md](README.md).

## Status

**Early, and not deployable** (the README's Status). The kiosk flow, its app-UI contract, the reference screens, the reference kiosk and the showcase - a launcher and its example kiosks - run in QEMU and on the Raspberry Pi 5. Each part:

| Path | What it is | State |
| --- | --- | --- |
| `kiosk/showcase-kiosk-flow` | The state machine behind a multi-step form kiosk. The application defines the steps and fields. The flow handles next, back and cancel, a review of every answer, answers that are bounded and checked as they are entered, and submission with a timeout and a limited number of retries. After a set time with no activity, or when a session reaches its maximum length, it wipes every answer, so the next person at a shared public kiosk never sees them; late timers and a clock that goes backwards cannot postpone that. `no_std`, no allocation, no `unsafe`, and no third-party code at run time | First cut, API provisional |
| `kiosk/showcase-kiosk-contract` | The kiosk's app-UI contract: the records the kiosk's UI and application protection domains exchange over `nowayos-ipc` rings. Actions from the UI (start, a value typed, chosen or ticked, next, back, submit, cancel and so on) and reports from the application (the screen to show, a refused value, the idle warning, a countdown's time left once a second, and a session-ended record), each a fixed-size record with its own kind and version, checked on receipt with typed errors. The application's end drives `showcase-kiosk-flow`, ends the session when the UI sends a record it cannot decode, and sends exactly one session-ended record, with the reason and no answer, however a session ends. Answers travel only from the UI to the application. `no_std`, no allocation, no `unsafe` | First cut, version 1 |
| `kiosk/showcase-kiosk-screens` | The kiosk reference screens: a synthetic, sector-neutral screen in TSX (`tsx/`) for each phase of the form flow (welcome, a step, review, submitting, submitted, failed, and the idle warning) and each kind of field (text, a choice, checkboxes, a notice with none), importing only from Screen Compiler's kit. All ten convert (with Screen Compiler's subset v1: conditional rendering, text from props, checkboxes, radio groups, input lengths and modes). The Rust Screen Compiler generates is checked in (`src/generated`) and built here against NoWayOS's UI core; each screen fits 800 x 480 (the kiosk's layout size), 1920 x 1080, and 960 x 540 at device scale 2 - the Pi 5's monitor drawn at its own pixels - in both of NoWayOS's themes, with each theme's own metrics - its type scale (body text 18 px and 24 px, at 2x from the 36 and 48 px atlases), its minimum target size and its buttons' height, as built and with every group shown and slot filled, with no fixed text cut. `gaps.txt` ranks what Screen Compiler refuses by how many screens each gap blocks: nothing now. `make screens` runs Screen Compiler on every screen and fails if either differs. `sink` is the kitchen sink: the ten screens toured with their own buttons, shaped like NoWayOS's clickable demo (`new(metrics)`, `tree`, `handle`, `action_name`; laid out at 800 x 480 until the display announces its layout and scale, `resize`) so a copy of its UI PD can show it - a demo driver, not the kiosk's flow. The reference kiosk puts the screens behind the flow: `form`, the form they ask for (four steps, six fields) and the kiosk's timeouts; `app`, what the application PD does - the flow behind the contract, the privacy reset applied when due whatever room the ring has, and out of service, the session ended, at a fault the PD finds (a timer error, a refused wake-up, a broken ring); `kiosk`, the UI that shows the screen each report from the application names and sends each action the person takes, keeping its own copies of the answers and wiping them in place - and any action not yet sent - when a session ends, and failing closed (everything wiped, the session abandoned) on a report it cannot trust; `stand_in`, a development stand-in for the service, which leaves each session's first attempt unanswered and accepts the next | First cut |
| `pds/kiosk` (`showcase-kiosk-stand-in`), `pds/kiosk-ui` (`showcase-kiosk-ui`) | The reference kiosk (`systems/kiosk.system` and its Pi 5 variant, `systems/rpi5b_2gb/`): the application PD runs `showcase-kiosk-flow` on the reference form behind the app-UI contract's application end, woken by the timer PD at each deadline - the idle warning after 15 s with no input, the privacy reset 15 s later, a submission's 5 s timeout, and each second of a countdown - and asks the service stand-in, never sending the answers anywhere; the UI PD shows the screen the application names, on NoWayOS's UI loop - and the seconds left of the idle warning and the confirmation, each second as the application reports them, changing only the digits that change - only they are drawn and copied to the display, their size logged - and sends it what the person does over two rings. Both fail closed on the faults they find: the application, at a timer error, a refused wake-up or a broken ring, ends the session and goes out of service; the UI, at a report it cannot decode, wipes everything, abandons the session and shows the welcome screen, and at a broken ring or display contract also goes out of service at once, then presents that screen when the display can take it - if it cannot, the screen may still show the last frame. A timer that goes silent, or a PD that faults, is left to a watchdog and a supervisor, which this demo system does not have. The same display, input broker, UART input, USB host and timer PDs as the kitchen sink. No device, interrupt or physical address in either (roles `app` and `ui`), no heap, no `unsafe`. A demo system: the UART input PD is a development route, and the service a stand-in built into the application PD's program, named `showcase-kiosk-stand-in` so that NoWayOS's development-only rule (`*-stand-in.elf`) refuses it in a product system | First cut |
| `pds/screens-ui` (`showcase-screens-ui`), `pds/display`, `pds/input-broker`, `pds/uart-input`, `pds/usb-host`, `pds/timer` | The kitchen sink on a screen (`systems/kitchen-sink.system` and its Pi 5 variant, `systems/rpi5b_2gb/`): a UI PD that shows the ten reference screens and tours them with their own buttons (`sink`), on NoWayOS's UI loop; and NoWayOS's display PD, input broker, UART input PD, USB host PD and timer PD, each run from a program of Showcase's own (`showcase-display` and so on). On the Pi 5 the display PD shows each frame on the HDMI monitor, and the keys come from a keyboard and mouse plugged into the Pi (a USB host PD for each of RP1's two controllers, the Pi variant only) and from a terminal on the Mac, through the Debug Probe and the debug UART - a development route only: `showcase-uart-input` is development-only by its name (NoWayOS's grant rules, `*-uart-input.elf`), refused in a product system. A UI app, not the kiosk: its screens follow the sink's fixed tour | First cut |
| `showcase/showcase-app` | The showcase: the launcher the Pi boots to - one tile per example kiosk, each with its status, fictional brand, title and one line, and a header with the demo line - in TSX (`tsx/launcher.tsx`), and the bar with Home above every Service desk screen (`tsx/service-desk-bar.tsx`), both converted by Screen Compiler (`src/generated`, `gaps.txt`). `Showcase` is the UI that shows the launcher or an example: the Service desk tile opens the reference kiosk (`showcase-kiosk-screens`' `KioskUi`, each screen inside the bar, its numbers moved past the screen's own); Home, or the end of a session however it ends, wipes the example - its copies, its outbox and the application's session - and shows the launcher. The visitor sign-in tile opens `showcase-visitor` under a bar of its own (`tsx/visitor-bar.tsx`), and the pay at the pump tile `showcase-pump` under its own (`tsx/pump-bar.tsx`). Each example talks to its own application PD over its own rings. The third tile is a program's own: `Extra` fills it - its brand, title and line in slots, and a button - and gives its example, a form UI of its screens under its own bar (`frame_with`), driven over the app-UI contract on application 3's rings, which the system description gives it, and wiped as the showcase's own are; the showcase's own program adds none (`NoExtra`), so the tile is hidden and its place on the grid empty. The other tiles say "Coming soon". Laid out at the Pi 5's monitor's own pixels once the display announces them - 960 x 540 at device scale 2 (`resize`). The launcher and each example are drawn in their own fictional brand's theme (`brands`): NoWayOS's light theme in the brand's colour, each with a high-contrast version, all put through NoWayOS's contrast check after `r5g6b5` in the tests; the UI PD switches theme as an example opens and back at the launcher. Each bar and header shows its example's icon, one of Lucide's (ISC) drawn from a build-time mask: an image links only the icons its screens name | First cut |
| `showcase/showcase-visitor` | The showcase's visitor sign-in: a visitor book that forgets you, at a fictional research centre (Orrinvale). Its eleven screens in TSX, converted by Screen Compiler (`tsx/`, `src/generated`, `gaps.txt`); its form - name, organisation, who you visit, why, the site rules - and timeouts; `Visitor`, the screens as `showcase-kiosk-screens`' form UI (`FormUi`) shows them; and `EXAMPLE`, what its application PD runs, with a stand-in for the building service that issues a pass number - or fails every attempt of a session after *Demo: service down*. The pass shows a number, never a name, every answer wiped first, and clears itself after 10 s, counting down. Not access control: no door, fire roll or badge | First cut |
| `pds/showcase-ui` (`showcase-ui`), `pds/visitor` (`showcase-visitor-stand-in`), `pds/pump` (`showcase-pump-stand-in`) | The showcase's UI PD (`systems/showcase.system` and its Pi 5 variant): `pds/kiosk-ui`'s library - now a library and a program - driving `Showcase`; the same rings, drawing and failing closed, with a pair of rings to each example's application PD. The library lays the screens out in the display's layout at its scale and draws only what changed while one tree shows. The visitor sign-in's and the pay at the pump's application PDs: `pds/kiosk`'s library (`showcase_kiosk_stand_in`) on `showcase_visitor::EXAMPLE` and `showcase_pump::EXAMPLE`, each service a stand-in, so development-only by its name. The showcase system is the reference kiosk's with this UI program and the two applications, each with exactly the grants of the Service desk's | First cut |
| `pds/run-watchdog` (`showcase-run-watchdog`), `pds/watchdog-stand-in` (`showcase-watchdog-stand-in`) | The showcase's tour on the Pi 5 (`systems/rpi5b_2gb/showcase-tour.system`): the showcase as a test system with a run of 300 s - NoWayOS's watchdog PD, watching no PD and keeping the PM watchdog armed, and its stand-in ticking it - so `make pi-run PI_SYSTEMS=showcase-tour` plays and checks the whole showcase script on the board. `showcase/showcase-app/tests/tour.rs` keeps the tour the showcase's own description, grants and script, plus the run. The card's demo image stays `showcase`, which runs until the power goes |
| `shared/` | Code shared by more than one Showcase application | Empty: README only |
| `customer-profiles/` | Per-customer configuration | Empty: README only. No customer data |

All form content in this repository, fixtures included, is invented and generic.

## Layout

```text
showcase/
├── kiosk/
│   ├── showcase-kiosk-flow/      multi-step form state machine (no_std)
│   ├── showcase-kiosk-contract/  the app-UI contract: records between the UI and application PDs (no_std)
│   └── showcase-kiosk-screens/   the kiosk reference screens: TSX, the Rust Screen Compiler makes of them, gaps.txt,
│                                the kitchen sink that tours them (src/sink.rs), and the reference kiosk's form,
│                                UI and service stand-in (src/form.rs, src/kiosk.rs, src/stand_in.rs)
├── pds/                      the protection domains: a Cargo workspace on NoWayOS's nightly and rust-sel4
│   ├── kiosk/                showcase-kiosk-stand-in, the reference kiosk's application PD: a library and a program
│   ├── visitor/              showcase-visitor-stand-in, the visitor sign-in's application PD, on kiosk's library
│   ├── pump/                 showcase-pump-stand-in, the pay at the pump's application PD, on kiosk's library
│   ├── kiosk-ui/             showcase-kiosk-ui, its UI PD: a library and a program
│   ├── showcase-ui/          showcase-ui, the showcase's UI PD, on kiosk-ui's library
│   ├── screens-ui/           showcase-screens-ui, the kitchen sink's UI PD
│   └── display/, input-broker/, uart-input/, usb-host/, timer/   NoWayOS's PDs, run from programs of Showcase's own
├── showcase/
│   ├── showcase-app/        the showcase: the launcher and each example's bar in TSX, converted, and the UI
│   │                            that opens each example and wipes it on Home or at a session's end
│   ├── showcase-visitor/         the visitor sign-in: its screens in TSX, converted, its form and its example
│   └── showcase-pump/            the pay at the pump: its screens in TSX, converted, its form and its example
├── systems/                  Microkit system descriptions, each with its .grants and .expect
├── shared/                   code common to Showcase applications (empty; see its README)
├── customer-profiles/        per-customer configuration (empty; see its README)
├── tools/check-nowayos-rev   checks ../nowayos is at the commit nowayos.rev records
├── tools/screens             the reference screens through Screen Compiler: checks or rewrites what is checked in
├── nowayos.rev the NoWayOS commit Showcase is tested against
├── Cargo.toml                workspace: kiosk/*, showcase/*
├── Cargo.lock                committed; `make ci` checks with --locked
├── deny.toml                 cargo-deny: licences, sources, bans, advisories
├── fuzz/                     libFuzzer targets for the contract's decoders: a workspace of its own (make fuzz)
├── rust-toolchain.toml       Rust 1.93.1 with rustfmt, clippy, aarch64-unknown-none
└── Makefile
```

## How Showcase uses NoWayOS

Showcase depends on NoWayOS crates **by path, from a sibling clone**. Keep the `nowayos` repo beside this one, so one folder holds `nowayos/`, `screen-compiler/` and `showcase/`. The paths are set once, in `[workspace.dependencies]` in `Cargo.toml`. Today `nowayos-bounded`, `nowayos-display`, `nowayos-input`, `nowayos-ipc` and `nowayos-ui` are used (and `nowayos-glyphs`, the glyph atlases, in tests). The kiosk reference screens also use the `screen-compiler` clone beside this one, as a build-time tool only (`make screens`), at whatever commit is checked out there, which it prints; no Screen Compiler revision is recorded.

**Showcase builds against the NoWayOS commit recorded in `nowayos.rev`**. A path dependency alone builds against whatever is checked out in `../nowayos`, and `Cargo.lock` cannot record which, so `tools/check-nowayos-rev` checks the clone:

* `make ci` starts by checking that `../nowayos` is at exactly that commit with no local changes (untracked files count). If it is, it reports the revision and builds; if not, it fails before building anything, naming both revisions.
* `make lint`, `test`, `doc`, `nostd` and `deny` fail the same way.
* **A deliberate local experiment** against another NoWayOS commit, or uncommitted changes in `../nowayos`, sets `NOWAYOS_EXPERIMENT=1` (for example `make test NOWAYOS_EXPERIMENT=1`). Those targets then print a loud warning instead of failing, and `make ci` refuses to run at all while it is set. Nothing built that way is to be reviewed or shipped.
* **Upgrading NoWayOS is a one-line change:** put the new commit's full hash in `nowayos.rev` and commit it, reviewed with whatever else the upgrade needs.

The dependency also requires NoWayOS's exact version (`=0.1.0`), so a NoWayOS version change fails here until it is reviewed.

## Showcase's protection domains

Showcase's PDs live in `pds/`, a Cargo workspace of its own, and its Microkit systems in `systems/`. They are built and checked with NoWayOS's make fragments (`../nowayos/tools/make/system.mk`), exactly as NoWayOS's own are: the same dev container, NoWayOS's pinned nightly (`pds/rust-toolchain.toml` must pin it), the Microkit SDK, NoWayOS's board facts and grant rules, the grants check of each system description and of the Microkit tool's capDL output, and NoWayOS's reviewed list of standard-library crates. `pds/.cargo/config.toml` takes the `aarch64-sel4` target spec from `../nowayos/pds/support/targets`.

rust-sel4 is the PD workspace's one git source, as in NoWayOS (a supply-chain change, flagged when it arrived): `pds/deny.toml` allows only its repository and only by commit, and `make pds-deny` then checks it is the commit NoWayOS's `pds/Cargo.toml` pins (`../nowayos/tools/check-git-pins`). `pds/Cargo.lock` started from NoWayOS's, so the PDs build with the crate versions NoWayOS's do. No Showcase crate has `unsafe` code: `make unsafe-audit` checks both workspaces with no approved crate.

## Prerequisites

* **The NoWayOS development container**, defined in the NoWayOS repo. Every target runs in it by default and starts it itself, so no host Rust is needed. It runs on Podman by default, or on Docker with `ENGINE=docker`. Build the image once with `make container` in `nowayos/`.
* **GNU Make** 3.81 or later.
* **The `nowayos` clone beside this one, at the commit `nowayos.rev` records.** It is needed for every build, not only for CI (see "How Showcase uses NoWayOS").
* **Optional, for the host loop (`IN=host` with `fmt`, `lint`, `test`, `doc` or `clean`): Rust 1.93.1 with rustfmt and clippy, through rustup.** `rust-toolchain.toml` pins it. The Makefile never installs it: it sets `RUSTUP_AUTO_INSTALL=0`, so a missing toolchain is reported rather than downloaded. If your stable Rust already is 1.93.1, use `IN=host CARGO="cargo +stable"`.

## Commands

```sh
make          # list the targets
make fmt      # format the code
make lint     # cargo fmt --check, then clippy on all targets with warnings denied
make test     # unit, integration, property and doc tests
make doc      # API docs, warnings as errors
make nostd    # build every crate for aarch64-unknown-none (dev container only)
make deny     # cargo-deny, then the local-crate check (dev container only)
make sbom     # a CycloneDX SBOM per workspace in build/sbom/: showcase, showcase-pds, showcase-fuzz
              # (dev container only)
make secrets  # the tree and every commit scanned for keys and credentials, offline, by
              # NoWayOS's tools/secrets-scan; reviewed false positives in secrets-allow.toml
make screens  # every kiosk reference screen through Screen Compiler (../screen-compiler): converts, or is
              # refused, exactly as checked in (src/generated, gaps.txt; dev container only)
make screens-bless  # rewrite them after a deliberate change, then review the diff
make ci       # check ../nowayos is the recorded NoWayOS, then secrets, lint, test, doc,
              # nostd, deny, sbom and screens (when ../screen-compiler is there), with --locked,
              # then fuzz, kani, the PDs' checks and every system booted in QEMU (dev container only)
make qemu-test SYSTEM=kiosk   # build a system's PDs and image, check its grants, boot it, check its output
make qemu-test-all            # every systems/*.system
make pi-show PI_DEVICE=/dev/cu.usbmodemXXXX SYSTEM=showcase PI_TYPE=1
                              # on the host, the Pi 5's Debug Probe plugged in and a monitor attached
                              # before power-on: the showcase's launcher on the HDMI monitor, driven by
                              # what you type in this terminal, or a USB keyboard on the Pi (Ctrl-] stops
                              # typing; NoWayOS's tools/pi-run); SYSTEM=kiosk the reference kiosk alone,
                              # SYSTEM=kitchen-sink the screens' tour
make pds-lint pds-deny        # rustfmt and clippy; cargo-deny and the rust-sel4 pin, for pds/
make quiet-check # every system prints nothing derived from what is typed:
                              # Showcase's programs only, the display quiet, no hash in any log, and
                              # everything a script types starting with a marker its expect files forbid
make unsafe-audit             # no unsafe code anywhere in Showcase
make fuzz # the contract's decoders fuzzed, FUZZ_SECONDS (10) s each
make std-crates-check         # the standard-library crates in the PDs (after qemu-test-all)
```

All of them run in the dev container; add `IN=host` to run `fmt`, `lint`, `test`, `doc` or `clean` on the host instead. In a container shell (`make shell`) they run directly; on a host that has the `aarch64-unknown-none` target, cargo-deny and Python 3.11, `make nostd-local`, `make deny-local` and `make ci-local IN=host` run them there (without `IN=host`, `ci-local`'s lint, test and doc start the container).

## Licence

The Showcase is licensed under **MIT OR Apache-2.0**, at your option (`LICENSE-MIT`, `LICENSE-APACHE`). Every crate declares it in its `license` field, and every source file but the generated screens carries an `SPDX-License-Identifier` line. The Rust the Screen Compiler writes into `src/generated` is the Showcase's, under the same licence, and is written only by `make screens-bless`.

Third-party code must be permissively licensed: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause or ISC. Anything else needs the project owner's written sign-off before it goes in. `deny.toml` enforces the list, dev-dependencies included, and bans by name the browser engines, WebViews, window systems and C GUI runtimes that every NoWayOS workspace bans (NoWayOS's baseline, `../nowayos/tools/baseline.toml`), as `pds/deny.toml` does. Third-party code comes from crates.io, never vendored as a local crate: `make deny` first runs the NoWayOS baseline check (`../nowayos/tools/check-baseline`), whose local-crate guard fails if any local crate is not Showcase's or NoWayOS's own. At run time the kiosk crates use only NoWayOS's crates (`nowayos-bounded`, `nowayos-ipc`, `nowayos-ui` and what it brings: `nowayos-display`, `nowayos-input`); the kitchen sink's PDs also NoWayOS's PD libraries and what they bring (`embedded-graphics`, `safe-mmio`, `zerocopy` and the like, all on the permissive list, and the glyph atlases, OFL-1.1), as NoWayOS's own PDs do. Screen Compiler (MIT) is a build-time tool, not a dependency, and the Rust it generates is ours. Its one third-party dependency is proptest (MIT OR Apache-2.0), used only in tests. `Cargo.lock` also lists `unicode-ident` (Unicode-3.0), which only a dependency that is never compiled for any target can reach. It has been flagged to the project owner and needs no exception today. The fuzz harness (`fuzz/`) links libFuzzer through `libfuzzer-sys` (MIT OR Apache-2.0; the libFuzzer sources it compiles, Apache-2.0 WITH LLVM-exception), signed off for development and CI fuzzing only: it stays in `fuzz/`'s own workspace and lock, whose `deny.toml` alone allows that licence; the other two ban the crate.

The PDs (`pds/`) also link rust-sel4 (BSD-2-Clause, from git at the commit NoWayOS pins) and what it brings, exactly as NoWayOS's PDs do; `make pds-deny` checks that tree, and `make std-crates-check` the standard-library crates built into it (`compiler_builtins` is signed off).

## Documentation

* The guides - getting started, a first system, screens from React, the
  Raspberry Pi 5 - are on [www.nowayos.com](https://www.nowayos.com/docs).
* NoWayOS's crates, PDs and make fragments, which the Showcase builds on:
  [NoWayOS](https://github.com/NoWayOS/nowayos) and its `DEVELOPING.md`.
* The Screen Compiler, which turns the TSX screens into Rust:
  [Screen Compiler](https://github.com/NoWayOS/screen-compiler).
