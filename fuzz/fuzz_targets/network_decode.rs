// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fuzz target: the records between the application PD and the network
//! path - any bytes decoded by `decode_from_network`, which reads
//! what comes from the backend, or by `decode_to_network`, as the fuzzer's
//! first byte chooses, as they are or behind a valid header for one of
//! their records.
//!
//! Checks (`showcase_fuzz::check_decoder`): decoding never panics; every
//! other kind is refused as unexpected; a record that decodes re-encodes to
//! exactly the bytes it came from - so an approval or ticket with epoch or
//! attempt 0, which cannot be encoded, is never accepted; the bytes after it
//! change nothing, and the record cut short is refused.

#![no_main]

use libfuzzer_sys::fuzz_target;
use showcase_fuzz::{Decoder, FRAME_MAX, Framing, check_decoder, choose, frame};
use showcase_kiosk_contract::{
    CheckIn, CheckedIn, FROM_NETWORK_KINDS, FromNetwork, SubmissionResult, TO_NETWORK_KINDS,
    ToNetwork, decode_from_network, decode_to_network,
};

const FROM_FRAMINGS: [Framing; 2] = [frame::<CheckedIn>, frame::<SubmissionResult>];
const TO_FRAMINGS: [Framing; 1] = [frame::<CheckIn>];

const FROM_NETWORK: Decoder<FromNetwork> = Decoder {
    name: "decode_from_network",
    kinds: &FROM_NETWORK_KINDS,
    decode: decode_from_network,
    kind: FromNetwork::kind,
    encode: FromNetwork::encode,
};

const TO_NETWORK: Decoder<ToNetwork> = Decoder {
    name: "decode_to_network",
    kinds: &TO_NETWORK_KINDS,
    decode: decode_to_network,
    kind: ToNetwork::kind,
    encode: ToNetwork::encode,
};

fuzz_target!(|data: &[u8]| {
    let mut buf = [0u8; FRAME_MAX];
    match data.split_first() {
        Some((&to, rest)) if to & 1 == 1 => {
            check_decoder(&TO_NETWORK, choose(rest, &TO_FRAMINGS, &mut buf));
        }
        Some((_, rest)) => {
            check_decoder(&FROM_NETWORK, choose(rest, &FROM_FRAMINGS, &mut buf));
        }
        None => {
            check_decoder(&FROM_NETWORK, data);
            check_decoder(&TO_NETWORK, data);
        }
    }
});
