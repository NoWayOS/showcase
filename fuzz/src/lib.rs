// SPDX-License-Identifier: MIT OR Apache-2.0
//! What Showcase's fuzz targets share (`fuzz_targets/`, run by `make fuzz`):
//! [`frame`] and [`choose`], which put the fuzzer's bytes behind a
//! valid header for one record type, and [`check_decoder`], the checks every decoder of
//! the kiosk's contract must pass, whatever the bytes.
//!
//! A target's checks are assertions: a failed one aborts the run like a
//! panic in the code under test, and libFuzzer keeps the input that caused
//! it. A crash, once fixed, becomes an ordinary regression test in the crate
//! that had the bug.

#![forbid(unsafe_code)]

use core::fmt::Debug;

use nowayos_ipc::{EncodeError, HEADER_LEN, Header, Message};
use showcase_kiosk_contract::{ContractError, MAX_RECORD_LEN};

/// The longest record [`frame`] builds: the contract's longest, with room
/// for bytes after it, which every decoder must ignore.
pub const FRAME_MAX: usize = 2 * MAX_RECORD_LEN;

/// `payload` behind a valid header for `M` - its kind, version and payload
/// length - in `buf`, cut to [`FRAME_MAX`] bytes: so that a target reaches
/// `M`'s payload checks from its first input, without first having to find
/// the header's three values.
pub fn frame<'b, M: Message>(payload: &[u8], buf: &'b mut [u8; FRAME_MAX]) -> &'b [u8] {
    let header = Header {
        kind: M::KIND,
        version: M::VERSION,
        payload_len: u16::try_from(M::PAYLOAD_LEN).unwrap_or(u16::MAX),
    };
    let Some((head, body)) = buf.split_at_mut_checked(HEADER_LEN) else {
        return &[];
    };
    if header.write(head).is_err() {
        return &[];
    }
    let len = payload.len().min(body.len());
    for (to, from) in body.iter_mut().zip(payload) {
        *to = *from;
    }
    buf.get(..HEADER_LEN.saturating_add(len))
        .unwrap_or_default()
}

/// [`frame`] for one record type.
pub type Framing = for<'b> fn(&[u8], &'b mut [u8; FRAME_MAX]) -> &'b [u8];

/// The record a target decodes, as the fuzzer's first byte chooses: the
/// rest of its bytes as they are (an even byte), or behind a valid header
/// for one of `framings`, in `buf` (an odd one).
pub fn choose<'b>(data: &'b [u8], framings: &[Framing], buf: &'b mut [u8; FRAME_MAX]) -> &'b [u8] {
    let Some((&choice, rest)) = data.split_first() else {
        return data;
    };
    let framing = usize::from(choice >> 1)
        .checked_rem(framings.len())
        .and_then(|index| framings.get(index));
    match framing {
        Some(framing) if choice & 1 == 1 => framing(rest, buf),
        _ => rest,
    }
}

/// One of the contract's decoders, with what [`check_decoder`] needs to
/// check it: the kinds it reads, and how a record it returns gives its kind
/// and encodes again.
pub struct Decoder<T> {
    /// Its name, for a failure's message.
    pub name: &'static str,
    /// The record kinds it reads; any other is [`ContractError::UnexpectedKind`].
    pub kinds: &'static [u16],
    /// The decoder: `decode_action`, `decode_report` and so on.
    pub decode: fn(&[u8]) -> Result<T, ContractError>,
    /// A record's kind.
    pub kind: fn(&T) -> u16,
    /// A record's encoder.
    pub encode: fn(&T, &mut [u8]) -> Result<usize, EncodeError>,
}

/// Decodes `bytes` with `decoder` and checks what its documentation
/// promises, whatever the bytes:
///
/// * a kind it does not read is refused as one, naming the kind the
///   header gives;
/// * a record it returns is of a kind it reads, the one the header gives,
///   and re-encodes to exactly the bytes it came from, as long as the
///   header says and no longer than [`MAX_RECORD_LEN`], so every value has
///   one encoding;
/// * bytes after the record change nothing, and the record cut by a byte is
///   refused.
///
/// Returns the record, if one decodes.
pub fn check_decoder<T: Debug + PartialEq>(decoder: &Decoder<T>, bytes: &[u8]) -> Option<T> {
    let name = decoder.name;
    let decoded = (decoder.decode)(bytes);
    let record = match decoded {
        Ok(record) => record,
        Err(ContractError::UnexpectedKind { kind }) => {
            let header = Header::parse(bytes).ok();
            assert!(
                header.map(|h| h.kind) == Some(kind) && !decoder.kinds.contains(&kind),
                "{name}: kind {kind:#06x} refused as unexpected, for {bytes:02x?}"
            );
            return None;
        }
        Err(ContractError::Decode(_)) => return None,
    };
    let kind = (decoder.kind)(&record);
    let header = Header::parse(bytes).ok();
    assert!(
        decoder.kinds.contains(&kind) && header.map(|h| h.kind) == Some(kind),
        "{name}: {record:?} decoded from a header of another kind, {bytes:02x?}"
    );
    let len = header.map(|h| h.record_len()).unwrap_or_default();
    assert!(
        len <= MAX_RECORD_LEN,
        "{name}: a {len}-byte record is longer than MAX_RECORD_LEN"
    );
    let mut out = [0u8; MAX_RECORD_LEN];
    assert_eq!(
        (decoder.encode)(&record, &mut out),
        Ok(len),
        "{name}: {record:?} does not re-encode to its own length"
    );
    assert_eq!(
        out.get(..len),
        bytes.get(..len),
        "{name}: {record:?} re-encodes to other bytes than it came from"
    );
    let own = bytes.get(..len).unwrap_or_default();
    assert!(
        (decoder.decode)(own).as_ref() == Ok(&record),
        "{name}: the bytes after {record:?} changed it"
    );
    let cut = own.get(..len.saturating_sub(1)).unwrap_or_default();
    assert!(
        (decoder.decode)(cut).is_err(),
        "{name}: {record:?} decoded from a record cut short"
    );
    Some(record)
}
