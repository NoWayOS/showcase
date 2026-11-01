// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kani proofs: the app-UI contract's two decoders,
//! [`decode_action`] and [`decode_report`], and the App's two for the
//! network path, [`decode_to_network`] and [`decode_from_network`],
//! given any byte string up to [`MAX_RECORD_LEN`] bytes and one more -
//! every record of each ring, anything short of one, and a byte past the
//! longest - never panic, overflow or read out of bounds. And a [`SetText`], the one record that
//! carries entered text, is accepted only with a length within its text
//! field and nothing but zeros after the text.
//!
//! **UTF-8 is modelled.** Checking UTF-8 over 64 symbolic bytes - which
//! `SetText` does once to decode and `BoundedStr` again on every read - is
//! more than CBMC finishes in a CI run. Each proof replaces
//! `core::str::from_utf8`, core's own function and not this crate's code,
//! with [`from_utf8_model`]: for any input it may pass or fail, so every
//! outcome the real check can have is covered, and a pass gives a string of
//! the input's length. The decoders look only at whether the check passed
//! and at the text's length, never at what the text says; so these proofs
//! cover every path through them. That `from_utf8` itself never panics is
//! core's, not proven here.
//!
//! `make kani` runs them, then a copy of this crate with `SetText`'s length
//! check removed, whose proof must fail (`tools/kani-mutations.json`,
//! through NoWayOS's `tools/kani-mutation`). Compiled only by Kani
//! (`cfg(kani)`); stubbing is switched on in this crate's `Cargo.toml`.

use nowayos_ipc::{HEADER_LEN, Header, Message, decode_exact};

use crate::{
    MAX_RECORD_LEN, MAX_TEXT_LEN, SetText, decode_action, decode_from_network, decode_report,
    decode_to_network,
};

/// Any byte string of at most this many bytes: every record, short of it,
/// and one byte past the longest.
const INPUT_LEN: usize = MAX_RECORD_LEN + 1;

/// Zero bytes, as long as any text the model is given: the string a pass
/// returns is a prefix of it.
const ZEROS: &str = "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

/// The model of `core::str::from_utf8` (see the module documentation): for
/// any input, either a pass - a string of the input's length - or a fail.
fn from_utf8_model(bytes: &[u8]) -> Result<&str, core::str::Utf8Error> {
    match ZEROS.get(..bytes.len()) {
        Some(text) if kani::any() => Ok(text),
        // A real `Utf8Error`, from the one other way core makes one.
        _ => match core::str::from_utf8_mut(&mut [0xff]) {
            Err(error) => Err(error),
            Ok(_) => Ok(""),
        },
    }
}

/// `buf`'s first `len` bytes, for any `len` up to `N`.
fn any_prefix<const N: usize>(buf: &[u8; N]) -> &[u8] {
    let len: usize = kani::any_where(|len: &usize| *len <= N);
    &buf[..len]
}

/// Any bytes up to the longest record and one more: `decode_action` returns
/// an action or a typed error, and never panics, overflows or reads out of
/// bounds.
#[kani::proof]
#[kani::stub(core::str::from_utf8, from_utf8_model)]
#[kani::unwind(82)]
fn decode_action_any_input() {
    let buf: [u8; INPUT_LEN] = kani::any();
    let _ = decode_action(any_prefix(&buf));
}

/// The same for `decode_report`.
#[kani::proof]
#[kani::stub(core::str::from_utf8, from_utf8_model)]
#[kani::unwind(82)]
fn decode_report_any_input() {
    let buf: [u8; INPUT_LEN] = kani::any();
    let _ = decode_report(any_prefix(&buf));
}

/// The same for the network path's two decoders, whose records carry no
/// text.
#[kani::proof]
#[kani::unwind(82)]
fn decode_from_network_any_input() {
    let buf: [u8; INPUT_LEN] = kani::any();
    let _ = decode_from_network(any_prefix(&buf));
}

/// And for what the App sends it.
#[kani::proof]
#[kani::unwind(82)]
fn decode_to_network_any_input() {
    let buf: [u8; INPUT_LEN] = kani::any();
    let _ = decode_to_network(any_prefix(&buf));
}

/// Any payload behind a `SetText`'s own header: one that is accepted has
/// a length field within [`MAX_TEXT_LEN`] and only zeros after its text.
#[kani::proof]
#[kani::stub(core::str::from_utf8, from_utf8_model)]
#[kani::unwind(82)]
fn set_text_accepts_only_its_own_length_and_zeros() {
    let mut record = [0u8; MAX_RECORD_LEN];
    let header = Header {
        kind: SetText::KIND,
        version: SetText::VERSION,
        payload_len: SetText::PAYLOAD_LEN as u16,
    };
    assert!(header.write(&mut record).is_ok());
    let payload: [u8; MAX_RECORD_LEN] = kani::any();
    record[HEADER_LEN..].copy_from_slice(&payload[HEADER_LEN..]);
    if decode_exact::<SetText>(&record).is_ok() {
        // The payload: seq (4 bytes), field (2), length (2), then the text.
        let at = HEADER_LEN + 6;
        let len = usize::from(u16::from_le_bytes([record[at], record[at + 1]]));
        assert!(
            len <= MAX_TEXT_LEN,
            "a text longer than its field was accepted"
        );
        let text_end = HEADER_LEN + 8 + len;
        assert!(
            record[text_end..].iter().all(|&byte| byte == 0),
            "a SetText with bytes after its text was accepted"
        );
    }
}
