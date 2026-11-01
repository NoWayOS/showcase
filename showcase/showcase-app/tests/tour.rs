// SPDX-License-Identifier: MIT OR Apache-2.0
//! The showcase's tour on the Pi 5: `systems/rpi5b_2gb/showcase-tour`
//! is the showcase's own Pi 5 system with a run length added, so a
//! board run of the tour plays and checks the showcase itself, all of its
//! script. These tests keep it so as the showcase changes.
//!
//! * Every line of the showcase's description, from `<system>` on, is in
//!   the tour's, in order, the timer's priority one step up; what the tour adds names only the run's PDs and
//!   regions, and the timer's end of the ticker's channel.
//! * Every grant of the showcase's is in the tour's but `system demo`, the
//!   timer's priority one step up; the
//!   tour adds `system test` and exactly the run's grants.
//! * The tour plays the showcase's script, and checks every line the
//!   showcase's own board run checks.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed step fails the test"
)]

use std::path::PathBuf;

fn read(path: &str) -> String {
    let systems = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../systems");
    std::fs::read_to_string(systems.join(path)).unwrap()
}

/// A description's lines from `<system>` on: its header comment is its own.
fn body(text: &str) -> Vec<&str> {
    let at = text.find("<system>").unwrap();
    text[at..].lines().collect()
}

/// A grants or expect file's lines, without comments and blank lines.
fn facts(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The lines of `tour` beyond `base`, which must be in it whole, in order.
fn added<'a>(base: &[&str], tour: &[&'a str]) -> Vec<&'a str> {
    let mut want = base.iter().peekable();
    let mut extra = Vec::new();
    for line in tour {
        if want.peek() == Some(&line) {
            want.next();
        } else {
            extra.push(*line);
        }
    }
    if let Some(missing) = want.peek() {
        panic!("the showcase's line {missing:?} is not in the tour, in order");
    }
    extra
}

/// The timer's priority in the tour: 13, one step above the showcase's 12,
/// so the ticker that calls it can sit alone at 12 - every priority from 1
/// to 12 is a showcase PD's (the fan at 6), and PDs that share
/// one print their lines mixed.
const TIMER_PD: &str = r#"<protection_domain name="timer" priority="13" stack_size="0x10000">"#;
const TIMER_GRANT: &str = "pd timer priority=13 budget=1000 period=1000";

#[test]
fn the_tour_s_description_is_the_showcase_s_and_the_run() {
    let showcase = read("rpi5b_2gb/showcase.system");
    let tour = read("rpi5b_2gb/showcase-tour.system");
    // The one line the tour changes: the timer one step up, so the ticker
    // can sit alone below it.
    let (theirs, ours) = (TIMER_PD.replace("13", "12"), TIMER_PD);
    assert_eq!(tour.matches(ours).count(), 1);
    let tour = tour.replace(ours, &theirs);
    let extra = added(&body(&showcase), &body(&tour));
    assert!(!extra.is_empty());
    let run = ["run_watchdog", "ticker", "pm_regs", "run_length"];
    for line in extra {
        for attribute in ["name=\"", "pd=\"", "mr=\""] {
            if let Some(at) = line.find(attribute) {
                let rest = &line[at + attribute.len()..];
                let named = &rest[..rest.find('"').unwrap()];
                let timer_end = line.trim() == r#"<end pd="timer" id="10" />"#;
                assert!(
                    run.contains(&named) || timer_end,
                    "the tour adds {line:?}, which is not the run's"
                );
            }
        }
    }
    assert!(tour.contains(r#"prefill_path="test-run-length-300.bin""#));
}

#[test]
fn the_tour_s_grants_are_the_showcase_s_and_the_run_s() {
    let showcase = read("rpi5b_2gb/showcase.grants");
    let tour = read("rpi5b_2gb/showcase-tour.grants");
    let base: Vec<&str> = facts(&showcase)
        .into_iter()
        .filter(|line| *line != "system demo")
        .collect();
    assert_eq!(
        base.len() + 1,
        facts(&showcase).len(),
        "the showcase is a demo system"
    );
    assert_eq!(tour.matches(TIMER_GRANT).count(), 1);
    let tour = tour.replace(TIMER_GRANT, &TIMER_GRANT.replace("13", "12"));
    let mut extra = added(&base, &facts(&tour));
    extra.sort_unstable();
    let mut run = vec![
        "system test",
        "role run_watchdog watchdog",
        "role ticker test",
        "pd run_watchdog priority=250 budget=1000 period=1000",
        "pd ticker priority=12 budget=1000 period=1000",
        "device run_watchdog pm_watchdog rw",
        "map ticker run_length r",
        "channel ticker run_watchdog notify=ticker",
        "channel ticker timer notify=timer",
        "call ticker timer",
    ];
    run.sort_unstable();
    assert_eq!(extra, run);
}

#[test]
fn the_tour_plays_the_showcase_s_script_and_checks_its_board_lines() {
    assert_eq!(read("showcase-tour.qmp"), read("showcase.qmp"));
    let board = read("rpi5b_2gb/showcase.expect");
    let tour = read("rpi5b_2gb/showcase-tour.expect");
    added(&facts(&board), &facts(&tour));
}
