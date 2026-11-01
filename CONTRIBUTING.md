# Contributing to the Showcase

Thank you for helping. The Showcase is NoWayOS's family of secure
applications - the kiosk kit, the reference kiosk and the showcase of example
kiosks - and every change to it is judged by the same gates as NoWayOS's,
written down in [Defence Agent](https://github.com/NoWayOS/defence-agent). Read
its `GATES.md` before your first pull request; this page is the short version.

## Before you start

- **Start from an issue** a maintainer has accepted (gate T1). Open one with
  the bug or feature template, or say on an existing one that you would like
  to take it.
- **A security problem is never an issue:** report it privately, as
  [SECURITY.md](SECURITY.md) says.
- Read [DEVELOPING.md](DEVELOPING.md): the layout, how the Showcase uses
  NoWayOS, and the commands.
- Something generic - a container, a widget, a driver, a message format - is
  built in [NoWayOS](https://github.com/NoWayOS/nowayos), not here.

## Build and check

The Showcase builds against NoWayOS at the commit `nowayos.rev` records, and
its screens with the Screen Compiler; clone all three side by side, as the
README's "Try it" shows. Then:

```sh
make -C ../nowayos container   # the dev image, once
make test lint                 # while you work
make screens                   # after changing a screen
make ci                        # before you open a pull request
```

`make ci` is the gates' build stage: a pull request must pass it.

## What a change keeps

- **Whatever a person types is protected.** Every field is treated as
  sensitive. A PD that holds entered values clears them in place on the idle
  timeout, at the session's maximum length, on cancel, once a submission
  succeeds and at the start of each session; nothing entered reaches a log,
  `Debug` output, an error or a fault report, or is stored on the device.
- No `unsafe`; no `unwrap`, `expect` or `panic!` outside tests; no unchecked
  indexing or arithmetic. The workspace lints enforce it.
- A decoder of untrusted input has Kani proofs and a fuzz target; messages
  between protection domains are fixed-size records, validated on receipt.
- A change that adds or widens a capability in a system description changes
  its `.grants` file, and the pull request says so: it is a security change.
- `src/generated` is written only by `make screens-bless`; change the TSX in
  `tsx/` instead.
- Forms, fixtures and screens are synthetic and sector-neutral: no real
  people, organisations or data.
- Dependencies are MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause or ISC, from
  crates.io. Anything else needs a maintainer's written agreement first.

## Pull requests

- Keep each one small and about one thing, with its tests.
- **Every commit carries a `Signed-off-by` line** (`git commit -s`): with it
  you certify the [Developer Certificate of Origin 1.1](https://developercertificate.org/),
  that you wrote the change or have the right to submit it under the
  project's licence. Gate P1 fails a commit without one.
- Fill in the pull-request template: what it changes, the grants, the sources,
  and any AI assistance.
- Your contribution is licensed as the project is, MIT OR Apache-2.0
  ([README](README.md#licence)).

## AI assistants

You may use one. Say so in the pull request, and which parts it wrote: you
answer for every line as if you had written it. [AGENTS.md](AGENTS.md) holds
the instructions for an assistant working in this repository.

## For maintainers

Until outside contributors join, the maintainers' own commits need no
`Signed-off-by` line; every contributor's commit does. An assistant never
signs off on anyone's behalf.
