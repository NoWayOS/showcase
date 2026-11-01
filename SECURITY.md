# Security policy

The Showcase is NoWayOS's family of secure applications: a kit for form
kiosks, a reference kiosk and a showcase of example kiosks, on NoWayOS. A
kiosk takes what people type into it, so its flaws matter. If you find one,
please tell us privately, so that we can fix it before anyone else can use it.

## Reporting a vulnerability

**Please do not open a public issue, pull request or discussion.** Report it
privately, either:

- through GitHub's private vulnerability reporting: **Report a vulnerability**
  on this repository's **Security** tab; or
- by email to **security@nowayos.com**.

Please tell us what is affected (crate, protection domain, system, commit or
release), how to reproduce it, and what an attacker could do with it. Use
synthetic test cases only: never send personal data or anyone's real answers.

## Scope

In scope: the code in this repository - its crates, protection domains,
systems, screens and build tooling - on `main` and in each supported release,
built as its documentation says. That includes any way for one person's
answers to reach the next person at a kiosk, to outlive the session they
belong to, or to appear in a log, an error or a fault report.

Out of scope:

- **NoWayOS and the Screen Compiler**: report those to their own repositories.
- **Third-party code**, including the seL4 kernel, Microkit and our Rust
  dependencies. Please report those to their maintainers - and tell us too,
  if the Showcase's use of them is affected.
- **The stand-ins and development routes the documentation names**: the
  service stand-ins, the UART input and the test systems.
- **Physical attacks on a Raspberry Pi 5.** The Pi is a prototype target, not
  a hardened one: it is not expected to resist invasive hardware attacks,
  glitching, or anyone with debug access.

## What we aim to do

- To acknowledge your report within **5 working days**.
- To give you our assessment - whether we consider it a vulnerability, and how
  severe - within **15 working days**.
- To keep you informed at least every **30 days** until it is resolved.
- To agree a disclosure date with you: by default when the fix is released, and
  **no later than 90 days** after your report unless we both agree otherwise.
- To credit you in the advisory, by the name you choose, unless you would
  rather not be credited.

## Supported versions

Until the first release, only `main` is supported. Once releases begin, each
release states how long it receives security fixes.

## How fixes are announced

Each fixed vulnerability gets a security advisory where this repository is
hosted, and a line in the release notes' security changes. An advisory says
what is affected, how severe it is, the fixed version, any workaround, and who
reported it.
