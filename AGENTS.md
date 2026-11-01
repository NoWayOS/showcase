# Instructions for AI agents

For any AI coding assistant working in this repository. Read
[CONTRIBUTING.md](CONTRIBUTING.md) too: these rules come from it and from the
gates in [Defence Agent](https://github.com/NoWayOS/defence-agent).

The Showcase is NoWayOS's family of secure applications: the kiosk kit, a
reference kiosk and a showcase of example kiosks. It consumes NoWayOS and
never defines it: anything generic is built in NoWayOS, and this repository
never edits `../nowayos`.

## Building

- **Install nothing on the host:** no toolchains, no `rustup`, no
  `cargo install`, no package managers. Every `make` target runs in NoWayOS's
  dev container.
- NoWayOS is used by path from `../nowayos`, at the commit `nowayos.rev`
  records; `make test` and `make ci` refuse another. `make screens` needs the
  Screen Compiler beside it too.
- Never run plain `cargo` on the host, and never in `pds/`.
- Before proposing a change: `make test`, `make lint`, `make screens`, then
  `make ci`.

## Rules the code keeps

- **Kiosk privacy.** A kiosk may collect sensitive data - passwords, PINs,
  card numbers, personal and health information - because taking it safely is
  its job: treat every field as sensitive. A shared device never shows one
  person's answers to the next. Every PD that holds entered values clears
  them in place on the idle timeout, at the session's maximum length, on
  cancel, as soon as a submission succeeds and at the start of each session;
  a late timer or a clock fault never postpones that. `Debug` output, errors
  and fault reports never contain entered text, and entered values are never
  logged or stored on the device.
- The trusted path is seL4 + Microkit + Rust, rendered by NoWayOS's UI runtime
  on `embedded-graphics`: no browser engine, WebView or C GUI runtime.
- Anything that ships in a PD is `#![no_std]`, uses bounded storage and
  allocates nothing. `unsafe` is forbidden.
- Never panic on input: no `unwrap`, `expect`, `panic!`, `todo!` or
  `unreachable!` outside tests; no unchecked indexing or arithmetic; every
  `#[allow]` gives a reason.
- Messages between PDs are fixed-size records on NoWayOS's IPC, validated on
  receipt and copied out of shared memory first. Every decoder of untrusted
  input has Kani proofs and a fuzz target.
- A system grants the narrowest rights; widening a grant is a security
  change, and the pull request must say so.
- `src/generated` is written only by `make screens-bless`, never by hand.
- Data is synthetic and sector-neutral. No secrets or keys, not even for
  development.

## Licences

- Our code is MIT OR Apache-2.0; every source file but `src/generated` starts
  with an `SPDX-License-Identifier` line, and no file carries a copyright line.
- Add a dependency only from crates.io and only if its whole tree is MIT,
  Apache-2.0, BSD-2-Clause, BSD-3-Clause or ISC (`make deny` checks); state its
  licence. For anything else, stop and ask a maintainer. Never copy, port or
  translate GPL code.

## Changes and pull requests

- Never weaken a gate to make a change pass: no deleted or skipped tests, no
  new allowlist entries or `deny.toml` exceptions without saying why, no edits
  to the files Defence Agent's `OWNERS.md` lists as gate-owned.
- Do not add a `Signed-off-by` line for a person: the person who submits the
  change signs it off (`git commit -s`).
- Say in the pull request that an assistant helped, and with what.
