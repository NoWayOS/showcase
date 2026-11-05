# SPDX-License-Identifier: MIT OR Apache-2.0
# Showcase: developer commands. `make` (or `make help`) lists them.
#
# Every target runs inside the NoWayOS dev container by default, through
# the shared helper in the sibling nowayos clone (build the image once with
# `make container` in nowayos/), so no host Rust is needed. Started
# inside the container itself (NOWAYOS_CONTAINER=1), they run directly.
#
# The everyday targets - fmt, lint, test, doc and clean - can run on the host
# instead with IN=host, using the toolchain pinned in rust-toolchain.toml, or
# the host's stable Rust where that already is 1.93.1:
#   make test IN=host CARGO="cargo +stable"
# Nothing here installs a toolchain on the host: RUSTUP_AUTO_INSTALL=0 below
# makes rustup report a missing toolchain or target instead of downloading
# it. nostd, deny, sbom, secrets and ci always run in the container.
#
# Showcase builds against NoWayOS by path, so every target needs the nowayos
# clone beside this one (../nowayos), at the commit nowayos.rev records:
# make ci fails before building on any other commit or on local changes, and
# so do lint, test, doc, nostd and deny unless NOWAYOS_EXPERIMENT=1 is set for
# a deliberate local experiment - they then warn loudly, and make ci refuses it.
#
# Written for GNU Make 3.81 (the version macOS ships) and later.

CARGO ?= cargo
# Never let a cargo, rustc or rustup call made from here install anything.
RUSTUP_AUTO_INSTALL ?= 0
export RUSTUP_AUTO_INSTALL
# `make ci` sets --locked so the committed Cargo.lock is exactly what is checked.
LOCKED ?=
NOSTD_TARGET ?= aarch64-unknown-none
# Every crate must build without std (`make nostd`) except these, which run on
# the host only: a new std crate fails until it is fixed or listed here (the
# NoWayOS baseline). None today.
HOST_ONLY_CRATES :=
PYTHON ?= python3
# make sbom writes a CycloneDX SBOM per workspace here.
SBOM_DIR := build/sbom
# The kiosk reference screens: synthetic TSX checked and converted by
# Screen Compiler, from the screen-compiler clone beside showcase, built in the container
# (make screens; in make ci when the clone is there).
SCREEN_COMPILER_DIR ?= ../screen-compiler
SCREEN_COMPILER_TARGET ?= $(if $(NOWAYOS_CONTAINER),/cache/target/screen-compiler,$(abspath $(SCREEN_COMPILER_DIR))/target)
# Each crate with screens in TSX: the kiosk reference screens, and the
# showcase's launcher and bar.
SCREENS_CRATES := kiosk/showcase-kiosk-screens showcase/showcase-app showcase/showcase-visitor showcase/showcase-pump
# Set to 1 for a local experiment against another NoWayOS commit or local
# changes in ../nowayos: the build targets warn instead of failing. make ci
# refuses it. Never set it for anything that is to be reviewed or shipped.
NOWAYOS_EXPERIMENT ?=
# Forwarded into the dev container with the target.
_nowayos_experiment_arg = $(if $(NOWAYOS_EXPERIMENT),NOWAYOS_EXPERIMENT=$(NOWAYOS_EXPERIMENT))
_check_nowayos = NOWAYOS_EXPERIMENT='$(NOWAYOS_EXPERIMENT)' sh tools/check-nowayos-rev

# Showcase's own Microkit systems (systems/) and protection domains (pds/),
# built and checked with NoWayOS's make fragments exactly as NoWayOS's are
# (nowayos/tools/make/system.mk, "Product repos"): NoWayOS's dev container,
# pinned nightly, Microkit SDK, board facts, grant rules and checks.
SYSTEM_REPO := $(CURDIR)
SYSTEM ?= kiosk
# pds/ has no library crate to test without seL4 yet.
PDS_LIB_CRATES :=
# No unsafe code anywhere in Showcase: no approved-unsafe list.
UNSAFE_ALLOWLIST :=
# Kani's proofs: the app-UI contract's decoders, and the mutant that
# shows they can fail (nowayos/tools/make/system.mk, make kani).
KANI_CRATES := showcase-kiosk-contract
KANI_MUTATIONS_FILE := tools/kani-mutations.json
# The fuzz targets: each in fuzz/fuzz_targets/, run for FUZZ_SECONDS
# by NoWayOS's make fuzz (nowayos/tools/make/system.mk), as NoWayOS's are.
# As in NoWayOS: make ci proves and fuzzes only what a change since CI_BASE
# (origin/main) reaches; CI_SCOPE=full runs them all, as make kani and make
# fuzz do.
CI_SCOPE ?= changed
SYSTEM_FORWARD = $(_nowayos_experiment_arg)

# Shared dev-container helper: ENGINE, IMAGE, WS, IN_CONTAINER, and the system
# targets. Optional, so a missing nowayos clone never breaks the host targets'
# help text.
-include ../nowayos/tools/make/container.mk
-include ../nowayos/tools/make/system.mk

# QEMU's test images wait out short idle, submission and confirmation
# timings, not the demonstration's 15 s and 30 s, so make ci runs in about
# a minute (showcase-kiosk-flow's Timeouts::or_test reads this when it is
# compiled; cargo rebuilds when it changes). The Pi's images and make qemu
# keep the demonstration's.
qemu-test qemu-test-all: export SHOWCASE_TEST_TIMINGS := 1

.DEFAULT_GOAL := help
.PHONY: help fmt lint test doc nostd deny sbom secrets ci nostd-local deny-local sbom-local secrets-local ci-local \
	screens screens-bless screens-local screens-bless-local screens-ci nowayos-rev nowayos-check clean

help:
	@echo "Showcase make targets"
	@echo "Everyday - in the dev container, or on the host with IN=host:"
	@echo "  make fmt     format the code (cargo fmt)"
	@echo "  make lint    cargo fmt --check, then clippy on all targets with warnings denied"
	@echo "  make test    unit, integration, property and doc tests"
	@echo "  make doc     build the API docs, warnings as errors"
	@echo "  make clean   remove build output (the container's, or the host's with IN=host)"
	@echo "Dev container only:"
	@echo "  make nostd   build every crate (but HOST_ONLY_CRATES) for $(NOSTD_TARGET)"
	@echo "  make deny    licence, source and advisory checks (cargo-deny), and the local-crate check"
	@echo "  make sbom    a CycloneDX SBOM per workspace: $(SBOM_DIR)/showcase.cdx.json, showcase-pds.cdx.json,"
	@echo "               showcase-fuzz.cdx.json"
	@echo "  make secrets scan the tree and every commit for keys and credentials, offline"
	@echo "               (NoWayOS's tools/secrets-scan; reviewed false positives in secrets-allow.toml)"
	@echo "  make screens the kiosk reference screens through Screen Compiler ($(SCREEN_COMPILER_DIR)): each converts,"
	@echo "               or is refused, exactly as checked in ($(SCREENS_CRATES): src/generated, gaps.txt)"
	@echo "  make screens-bless  rewrite them after a deliberate change; review the diff"
	@echo "  make quiet-check    every system prints nothing derived from what is typed (tools/check-quiet,"
	@echo "                      after its tests)"
	@echo "  make ci      check ../nowayos is the NoWayOS nowayos.rev records, then secrets, lint,"
	@echo "               test, doc, nostd, deny, sbom and screens (when ../screen-compiler is there), with"
	@echo "               --locked, then fuzz, kani, the PDs' checks and every system booted in QEMU (below)"
	@echo "Showcase's systems and PDs, with NoWayOS's make fragments (dev container only):"
	@echo "  make qemu-test [SYSTEM=$(SYSTEM)]   build systems/SYSTEM, check its grants, boot it, check its output"
	@echo "  make qemu-test-all             every systems/*.system"
	@echo "  make pi-show PI_DEVICE=... SYSTEM=showcase PI_TYPE=1   on the host: the showcase's launcher on the Pi 5's HDMI;"
	@echo "                                 SYSTEM=kiosk the reference kiosk alone"
	@echo "  make pi-show PI_DEVICE=... SYSTEM=kiosk PI_TYPE=1   on the host: the reference kiosk on the Pi 5's HDMI,"
	@echo "                                 driven by what you type here (NoWayOS's tools/pi-run; Ctrl-] stops typing);"
	@echo "                                 SYSTEM=kitchen-sink tours the screens"
	@echo "  make pi-run PI_DEVICE=... PI_SYSTEMS=showcase-tour   on the host: the whole showcase script in one"
	@echo " 300 s board run, checked line by line"
	@echo "  make image [SYSTEM=$(SYSTEM)]       only the image: build/BOARD/SYSTEM/loader.img"
	@echo "  make pds-lint / pds-deny       rustfmt and clippy / cargo-deny and the rust-sel4 pin, for pds/"
	@echo "  make unsafe-audit              no unsafe code anywhere in Showcase"
	@echo "  make kani                      Kani's proofs of the app-UI contract's decoders, then a mutant that must fail"
	@echo "  make fuzz                      the fuzz targets (fuzz/): the contract's decoders, each run for FUZZ_SECONDS"
	@echo "                                 ($(FUZZ_SECONDS)) s; FUZZ_TARGETS=NAME for one, FUZZ_INPUT=FILE one input"
	@echo "  make std-crates-check          the standard-library crates in the PDs (after qemu-test-all)"
	@echo "  make shell                     a shell in the dev container"
	@echo "every target but help, fmt and clean needs ../nowayos at the commit nowayos.rev records;"
	@echo "NOWAYOS_EXPERIMENT=1 lets a local experiment use another: they warn, make ci refuses."
	@echo "Current settings: IN=\"$(IN)\" CARGO=\"$(CARGO)\" ENGINE=\"$(ENGINE)\" IMAGE=\"$(IMAGE)\" NOWAYOS_EXPERIMENT=\"$(NOWAYOS_EXPERIMENT)\""

# The everyday targets run here inside the dev container or with IN=host;
# otherwise they start the container.
ifneq ($(NOWAYOS_CONTAINER)$(filter host,$(IN)),)
# `cargo fmt` without --all on purpose: from the workspace root it formats
# every workspace member, while --all would also reformat path dependencies,
# which here are NoWayOS's crates in the sibling repo.
fmt:
	$(CARGO) fmt
	$(CARGO) fmt --manifest-path fuzz/Cargo.toml

lint: nowayos-check
	$(CARGO) fmt -- --check
	$(CARGO) clippy --workspace --all-targets $(LOCKED) -- -D warnings

# Each test thread 16 MiB: a test builds the whole showcase - every screen's
# tree - on its stack, as the UI PD does on its 7 MiB one.
test: nowayos-check
	RUST_MIN_STACK=16777216 $(CARGO) test --workspace $(LOCKED)

doc: nowayos-check
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps $(LOCKED)

clean:
	$(CARGO) clean
else ifdef IN_CONTAINER
fmt lint test doc clean:
	$(IN_CONTAINER) make $@ CARGO=cargo $(if $(LOCKED),LOCKED=$(LOCKED)) $(_nowayos_experiment_arg)
else
fmt lint test doc clean:
	@echo "error: make $@ needs ../nowayos/tools/make/container.mk - clone nowayos beside showcase - or IN=host to run on the host" >&2
	@exit 1
endif

# The bodies of nostd, deny and ci, for the dev container (or, with IN=host,
# any host that has the aarch64-unknown-none target, cargo-deny and python3:
# without IN=host, ci-local's lint, test and doc start the container).
#
# Release profile: what ships (panic = abort, overflow checks on).
nostd-local: nowayos-check
	$(CARGO) build $(LOCKED) --release --target $(NOSTD_TARGET) --workspace $(addprefix --exclude ,$(HOST_ONLY_CRATES))

# The NoWayOS baseline (lints, profiles, deny settings and the
# local-crate guard) of all three workspaces first, then cargo-deny for the
# root workspace and the fuzz harness's (pds/: make pds-deny). Only fuzz/'s
# deny.toml allows libFuzzer's licence; the other two ban the crate.
deny-local: nowayos-check
	$(PYTHON) ../nowayos/tools/check-baseline . pds fuzz
	$(CARGO) deny $(LOCKED) check
	$(CARGO) deny --locked --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml check

# The workspace's CycloneDX SBOM, by NoWayOS's generator (tools/sbom),
# with the rule every repo uses: what the kiosk is built from for the device
# target is `required`; dev-dependencies, build-time tools, host-only crates
# and other platforms' dependencies are marked `excluded`, with the reason.
# The fuzz harness is a test harness: nothing in it ships, so everything in
# its SBOM is `excluded`.
sbom-local: nowayos-check
	@mkdir -p $(SBOM_DIR) && { test -f build/.gitignore || printf '*\n' > build/.gitignore; }
	$(PYTHON) ../nowayos/tools/sbom --workspace . --name showcase --target $(NOSTD_TARGET) \
		$(addprefix --host-only ,$(HOST_ONLY_CRATES)) --cargo $(CARGO) --out $(SBOM_DIR)/showcase.cdx.json
	$(PYTHON) ../nowayos/tools/sbom --workspace pds --name showcase-pds --kind application --target aarch64-sel4 \
		--std-crates $(PDS_STD_CRATES) --toolchain $(PDS_TOOLCHAIN) \
		--std-library "$$(rustc +$(PDS_TOOLCHAIN) --print sysroot)/lib/rustlib/src/rust/library" \
		--cargo $(CARGO) --out $(SBOM_DIR)/showcase-pds.cdx.json
	$(PYTHON) ../nowayos/tools/sbom --workspace fuzz --name showcase-fuzz --host-only-workspace \
		--cargo $(CARGO) --out $(SBOM_DIR)/showcase-fuzz.cdx.json

# The secrets scan, by NoWayOS's scanner (../nowayos/tools/secrets-scan),
# with the rule every repo uses: the tree, and every version of
# every file and every commit message on every ref, offline. A finding names
# its file and line, never the text; reviewed false positives go in
# secrets-allow.toml, one exact entry each with its reason. It needs the
# whole history: a shallow clone fails.
secrets-local: nowayos-check
	$(PYTHON) ../nowayos/tools/secrets-scan --repo . --allow secrets-allow.toml

# The kiosk reference screens through Screen Compiler, by tools/screens:
# `screen-compiler check` on every screen in each of $(SCREENS_CRATES), `screen-compiler
# convert` on each it accepts, and both compared with what is checked in -
# the generated Rust in src/generated (which make lint, test and nostd build
# against the recorded NoWayOS) and the ranked gaps in gaps.txt. Screen Compiler is
# built from the clone beside showcase with its own Cargo.lock, and its commit
# is printed: no Screen Compiler revision is recorded (as
# screen-compiler records no NoWayOS revision). The screens are our own synthetic files: never point this at
# untrusted .tsx.
define screen_compiler_build
	@test -f $(SCREEN_COMPILER_DIR)/Cargo.toml || { echo "error: make $@ needs a screen-compiler clone beside showcase ($(SCREEN_COMPILER_DIR))" >&2; exit 1; }
	@rev=$$(git -C $(SCREEN_COMPILER_DIR) rev-parse --verify --quiet --short HEAD) || rev="a commit git cannot read"; \
	if [ -n "$$(git --no-optional-locks -C $(SCREEN_COMPILER_DIR) status --porcelain 2>/dev/null)" ]; then rev="$$rev, with local changes"; fi; \
	echo "Checking the kiosk reference screens with screen-compiler: $$rev"
	cd $(SCREEN_COMPILER_DIR) && CARGO_TARGET_DIR=$(SCREEN_COMPILER_TARGET) $(CARGO) build --locked --quiet -p screen-compiler-cli
endef

# Then the Screen Compiler gallery's coverage (tools/gallery-coverage): its
# screens show every component, prop, listed value and style property that
# `screen-compiler subset` prints, so a gallery that falls behind the subset fails.
screens-local:
	$(call screen_compiler_build)
	@for crate in $(SCREENS_CRATES); do \
		$(PYTHON) tools/screens --screen-compiler $(SCREEN_COMPILER_TARGET)/debug/screen-compiler --screens $$crate || exit 1; \
	done
	$(PYTHON) tools/gallery-coverage --screen-compiler $(SCREEN_COMPILER_TARGET)/debug/screen-compiler --tsx showcase/showcase-app/tsx

screens-bless-local:
	$(call screen_compiler_build)
	@for crate in $(SCREENS_CRATES); do \
		$(PYTHON) tools/screens --screen-compiler $(SCREEN_COMPILER_TARGET)/debug/screen-compiler --screens $$crate --bless || exit 1; \
	done

# In make ci: the screens check when a screen-compiler clone sits beside showcase;
# without one, a notice (the converted screens still build against NoWayOS).
screens-ci:
	@if [ -f $(SCREEN_COMPILER_DIR)/Cargo.toml ]; then \
		$(MAKE) --no-print-directory screens-local CARGO=$(CARGO); \
	else \
		echo "notice: no screen-compiler clone at $(SCREEN_COMPILER_DIR) - skipping the check that the kiosk reference screens convert, or are refused, as checked in; their generated Rust still builds"; \
	fi

# Showcase's images print nothing derived from what people type:
# only Showcase's own programs, the display PD quiet, every expect file
# forbidding a frame's hash, and everything a script types starting with a
# marker its expect files forbid (tools/check-quiet; its own tests first).
quiet-check:
	$(PYTHON) -B -m unittest discover -s tools -p 'test_*.py'
	$(PYTHON) tools/check-quiet

# Cargo.lock records no source or checksum for path dependencies, so the
# NoWayOS commit is checked here (tools/check-nowayos-rev, read-only:
# git takes no lock). nowayos-rev is make ci's check: it reports the revision,
# or fails before anything is built, naming both revisions, and refuses
# NOWAYOS_EXPERIMENT. nowayos-check, before every other target that builds
# against NoWayOS, fails the same way unless NOWAYOS_EXPERIMENT is set, and
# then warns.
nowayos-rev:
	@$(_check_nowayos) ci

nowayos-check:
	@$(_check_nowayos) build

# `override`: --locked even when LOCKED is given on the command line.
ci-local: override LOCKED := --locked
# make kani and make fuzz on their own run every crate and target.
kani fuzz: CI_SCOPE := $(if $(filter ci ci-local,$(MAKECMDGOALS)),$(CI_SCOPE),full)
ci-local: nowayos-rev secrets-local lint test doc nostd-local deny-local sbom-local screens-ci \
	fuzz kani quiet-check pds-lint pds-deny pds-test pds-miri unsafe-audit qemu-test-all std-crates-check

# The system targets build against NoWayOS too: the same check first.
pds image qemu qemu-test qemu-test-all check-serial pds-lint pds-deny pds-test pds-miri \
	unsafe-audit std-crates-check sdcard kani fuzz: nowayos-check

ifeq ($(NOWAYOS_CONTAINER),1)
# Already inside the dev container: run the checks directly.
nostd: nostd-local
deny: deny-local
sbom: sbom-local
secrets: secrets-local
screens: screens-local
screens-bless: screens-bless-local
ci: ci-local
else ifdef IN_CONTAINER
# From the host the secrets scan runs with no network at all.
secrets: override CONTAINER_RUN_ARGS += --network=none
nostd deny sbom secrets screens screens-bless ci:
	$(IN_CONTAINER) make $@-local CARGO=cargo $(if $(LOCKED),LOCKED=$(LOCKED)) $(_nowayos_experiment_arg) \
		CI_SCOPE=$(CI_SCOPE) $(if $(CI_BASE),CI_BASE=$(CI_BASE))
else
nostd deny sbom secrets screens screens-bless ci:
	@echo "error: make $@ needs ../nowayos/tools/make/container.mk - clone nowayos beside showcase" >&2
	@exit 1
endif
