// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fuzz target: the kiosk's reports, which the application PD sends the UI
//! PD - any bytes decoded by `decode_report`, as they are or behind a valid
//! header for one of the fourteen reports, as the fuzzer chooses.
//!
//! Checks (`showcase_fuzz::check_decoder`): decoding never panics; every kind
//! but a report's, the actions' among them, is refused as unexpected; a
//! report that decodes re-encodes to exactly the bytes it came from; the
//! bytes after it change nothing, and the record cut short is refused.

#![no_main]

use libfuzzer_sys::fuzz_target;
use showcase_fuzz::{Decoder, FRAME_MAX, Framing, check_decoder, choose, frame};
use showcase_kiosk_contract::{
    Countdown, IdleWarning, Incomplete, Issued, OutOfService, REPORT_KINDS, Refused, Report,
    SessionEnded, ShowFailed, ShowReview, ShowStep, ShowSubmitted, ShowSubmitting, ShowWelcome,
    ValueRefused, decode_report,
};

const FRAMINGS: [Framing; 14] = [
    frame::<ShowWelcome>,
    frame::<ShowStep>,
    frame::<ShowReview>,
    frame::<ShowSubmitting>,
    frame::<ShowSubmitted>,
    frame::<ShowFailed>,
    frame::<IdleWarning>,
    frame::<ValueRefused>,
    frame::<Incomplete>,
    frame::<Refused>,
    frame::<SessionEnded>,
    frame::<Countdown>,
    frame::<Issued>,
    frame::<OutOfService>,
];

const DECODER: Decoder<Report> = Decoder {
    name: "decode_report",
    kinds: &REPORT_KINDS,
    decode: decode_report,
    kind: Report::kind,
    encode: Report::encode,
};

fuzz_target!(|data: &[u8]| {
    let mut buf = [0u8; FRAME_MAX];
    check_decoder(&DECODER, choose(data, &FRAMINGS, &mut buf));
});
