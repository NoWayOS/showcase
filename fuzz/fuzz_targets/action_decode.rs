// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fuzz target: the kiosk's actions, which the UI PD sends the application
//! PD - any bytes decoded by `decode_action`, as they are or behind a valid
//! header for one of the fourteen actions, as the fuzzer chooses.
//!
//! Checks (`showcase_fuzz::check_decoder`): decoding never panics; every kind
//! but an action's, the reports' among them, is refused as unexpected; an
//! action that decodes re-encodes to exactly the bytes it came from; the
//! bytes after it change nothing, and the record cut short is refused.

#![no_main]

use libfuzzer_sys::fuzz_target;
use showcase_fuzz::{Decoder, FRAME_MAX, Framing, check_decoder, choose, frame};
use showcase_kiosk_contract::{
    ACTION_KINDS, Abandon, Action, Back, Cancel, ClearField, Finish, Next, Retry, Select,
    SetChecked, SetText, SimulateFailure, Start, StillHere, Submit, decode_action,
};

const FRAMINGS: [Framing; 14] = [
    frame::<Start>,
    frame::<SetText>,
    frame::<Select>,
    frame::<SetChecked>,
    frame::<ClearField>,
    frame::<Next>,
    frame::<Back>,
    frame::<Submit>,
    frame::<Retry>,
    frame::<Cancel>,
    frame::<Finish>,
    frame::<StillHere>,
    frame::<Abandon>,
    frame::<SimulateFailure>,
];

const DECODER: Decoder<Action> = Decoder {
    name: "decode_action",
    kinds: &ACTION_KINDS,
    decode: decode_action,
    kind: Action::kind,
    encode: Action::encode,
};

fuzz_target!(|data: &[u8]| {
    let mut buf = [0u8; FRAME_MAX];
    check_decoder(&DECODER, choose(data, &FRAMINGS, &mut buf));
});
