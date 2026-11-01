// SPDX-License-Identifier: MIT OR Apache-2.0
//! Actions: the records the UI sends the application, one kind for each
//! thing the person can do.
//!
//! Every action carries `seq`, the UI's number for it (wrapping at 2³²):
//! a report that answers an action quotes it, so the UI can tell which of
//! its actions a refusal is about. Only [`SetText`] carries entered text.

use core::fmt;

use nowayos_bounded::{BoundedStr, CapacityError, Wipe};
use nowayos_ipc::{DecodeError, EncodeError, Header, Message, Reader, Writer, decode, encode};
use showcase_kiosk_flow::FieldId;

use crate::{ContractError, VERSION};

/// The longest text one [`SetText`] carries, in bytes of UTF-8. A form's
/// text fields may allow no more ([`AppEnd::new`](crate::AppEnd::new)).
pub const MAX_TEXT_LEN: usize = 64;

/// Kind of [`Start`].
pub const START_KIND: u16 = 0x4B01;
/// Kind of [`SetText`].
pub const SET_TEXT_KIND: u16 = 0x4B02;
/// Kind of [`Select`].
pub const SELECT_KIND: u16 = 0x4B03;
/// Kind of [`SetChecked`].
pub const SET_CHECKED_KIND: u16 = 0x4B04;
/// Kind of [`ClearField`].
pub const CLEAR_FIELD_KIND: u16 = 0x4B05;
/// Kind of [`Next`].
pub const NEXT_KIND: u16 = 0x4B06;
/// Kind of [`Back`].
pub const BACK_KIND: u16 = 0x4B07;
/// Kind of [`Submit`].
pub const SUBMIT_KIND: u16 = 0x4B08;
/// Kind of [`Retry`].
pub const RETRY_KIND: u16 = 0x4B09;
/// Kind of [`Cancel`].
pub const CANCEL_KIND: u16 = 0x4B0A;
/// Kind of [`Finish`].
pub const FINISH_KIND: u16 = 0x4B0B;
/// Kind of [`StillHere`].
pub const STILL_HERE_KIND: u16 = 0x4B0C;
/// Kind of [`Abandon`].
pub const ABANDON_KIND: u16 = 0x4B0D;
/// Kind of [`SimulateFailure`].
pub const SIMULATE_FAILURE_KIND: u16 = 0x4B0E;

/// Every action kind, in the order of [`Action`]'s variants.
pub const ACTION_KINDS: [u16; 14] = [
    START_KIND,
    SET_TEXT_KIND,
    SELECT_KIND,
    SET_CHECKED_KIND,
    CLEAR_FIELD_KIND,
    NEXT_KIND,
    BACK_KIND,
    SUBMIT_KIND,
    RETRY_KIND,
    CANCEL_KIND,
    FINISH_KIND,
    STILL_HERE_KIND,
    ABANDON_KIND,
    SIMULATE_FAILURE_KIND,
];

/// An action that carries only the UI's sequence number.
macro_rules! seq_action {
    ($(#[$doc:meta])* $name:ident = $kind:ident) => {
        $(#[$doc])*
        ///
        /// Wire layout, 4 bytes: `seq` (`u32`, little-endian).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            /// The UI's number for this action.
            pub seq: u32,
        }

        impl Message for $name {
            const KIND: u16 = $kind;
            const VERSION: u16 = VERSION;
            const PAYLOAD_LEN: usize = 4;

            fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
                out.write_u32(self.seq)
            }

            fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
                Ok(Self {
                    seq: input.read_u32()?,
                })
            }
        }
    };
}

seq_action!(
    /// Start a session from the welcome screen
    /// ([`KioskFlow::start`](showcase_kiosk_flow::KioskFlow::start)).
    Start = START_KIND
);
seq_action!(
    /// Go on from a complete step
    /// ([`KioskFlow::next`](showcase_kiosk_flow::KioskFlow::next)).
    Next = NEXT_KIND
);
seq_action!(
    /// Go back one screen, keeping every answer
    /// ([`KioskFlow::back`](showcase_kiosk_flow::KioskFlow::back)).
    Back = BACK_KIND
);
seq_action!(
    /// Send the answers from the review screen
    /// ([`KioskFlow::submit`](showcase_kiosk_flow::KioskFlow::submit)).
    Submit = SUBMIT_KIND
);
seq_action!(
    /// Send the same answers again after a failure
    /// ([`KioskFlow::retry`](showcase_kiosk_flow::KioskFlow::retry)).
    Retry = RETRY_KIND
);
seq_action!(
    /// Abandon the session
    /// ([`KioskFlow::cancel`](showcase_kiosk_flow::KioskFlow::cancel)).
    Cancel = CANCEL_KIND
);
seq_action!(
    /// End the confirmation early
    /// ([`KioskFlow::finish`](showcase_kiosk_flow::KioskFlow::finish)).
    Finish = FINISH_KIND
);
seq_action!(
    /// The person is still there: input the app does not otherwise see,
    /// or "I'm still here" on the idle warning
    /// ([`KioskFlow::note_activity`](showcase_kiosk_flow::KioskFlow::note_activity)).
    StillHere = STILL_HERE_KIND
);
seq_action!(
    /// The UI can no longer trust what it shows - a report it could not
    /// decode, a screen it could not show - or is giving up: it has wiped
    /// everything it holds, and asks the application to end any session at
    /// once, whatever its phase, a submission in flight included
    /// ([`KioskFlow::end_session`](showcase_kiosk_flow::KioskFlow::end_session)),
    /// so the next person never joins this one's. The session ends as
    /// [`EndReason::Abandoned`](crate::EndReason::Abandoned) (the
    /// showcase's Home).
    Abandon = ABANDON_KIND
);
seq_action!(
    /// A demonstration's switch: the person pressed a visible
    /// *Demo: service down* button, and asks that the example's stand-in
    /// for the service fail every submission of this session, so the
    /// failure can be shown. It changes nothing in the flow, and only a
    /// demonstration's application acts on it ([`Replies::simulate_failure`](crate::Replies::simulate_failure));
    /// a product's ignores it. Outside a session it does nothing.
    SimulateFailure = SIMULATE_FAILURE_KIND
);

/// The person typed into a text field: its whole value, replacing the
/// last one
/// ([`KioskFlow::set_text`](showcase_kiosk_flow::KioskFlow::set_text)).
/// Empty text clears the field.
///
/// The only record that carries entered text, and it travels only from the
/// UI to the application. `Debug` output shows its length, never the text,
/// and dropping a `SetText` zeroes its storage (hygiene, not a guarantee:
/// see [`nowayos_bounded::Wipe`]). Deliberately not `Copy`.
///
/// Wire layout, 72 bytes, little-endian:
///
/// | Bytes | Field | Rule |
/// | --- | --- | --- |
/// | 0–3 | `seq` (`u32`) | any |
/// | 4–5 | field (`u16`) | any; the form decides |
/// | 6–7 | length in bytes (`u16`) | at most [`MAX_TEXT_LEN`] |
/// | 8–71 | the text, then zeros | the first `length` bytes UTF-8; the rest zero |
///
/// Which characters a field accepts is the form's rule, checked by the
/// application ([`showcase_kiosk_flow::TextRule`]); the record only has to be
/// UTF-8.
#[derive(Clone, PartialEq, Eq)]
pub struct SetText {
    /// The UI's number for this action.
    pub seq: u32,
    /// The text field.
    pub field: FieldId,
    text: BoundedStr<MAX_TEXT_LEN>,
}

impl SetText {
    /// The record setting `field` to `text`.
    ///
    /// # Errors
    ///
    /// [`CapacityError`] if `text` is longer than [`MAX_TEXT_LEN`] bytes.
    pub fn new(seq: u32, field: FieldId, text: &str) -> Result<Self, CapacityError> {
        Ok(Self {
            seq,
            field,
            text: BoundedStr::try_from_str(text)?,
        })
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
}

impl Drop for SetText {
    fn drop(&mut self) {
        self.text.clear();
    }
}

/// Zeroes the text in place, leaving it empty: for a copy that must not
/// linger once sent, before the memory holding it is reused.
impl Wipe for SetText {
    fn wipe(&mut self) {
        self.text.wipe();
    }
}

/// Shows the length of the text, never the text.
impl fmt::Debug for SetText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetText")
            .field("seq", &self.seq)
            .field("field", &self.field)
            .field("text_len", &self.text.len())
            .finish()
    }
}

impl Message for SetText {
    const KIND: u16 = SET_TEXT_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8 + MAX_TEXT_LEN;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        let len = u16::try_from(self.text.len())
            .map_err(|_| EncodeError::InvalidValue { field: "len" })?;
        out.write_u32(self.seq)?;
        out.write_u16(self.field.0)?;
        out.write_u16(len)?;
        out.write_bytes(self.text.as_bytes())?;
        out.write_zeros(MAX_TEXT_LEN.saturating_sub(self.text.len()))
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let field = FieldId(input.read_u16()?);
        let len = usize::from(input.read_u16()?);
        let bytes = input.read_bytes(MAX_TEXT_LEN)?;
        let (text, padding) = bytes
            .split_at_checked(len)
            .ok_or(DecodeError::InvalidValue { field: "len" })?;
        if padding.iter().any(|&byte| byte != 0) {
            return Err(DecodeError::NonZeroPadding);
        }
        let text =
            BoundedStr::from_utf8(text).map_err(|_| DecodeError::InvalidValue { field: "text" })?;
        Ok(Self { seq, field, text })
    }
}

/// The person chose an option of a choice field, numbered from 0
/// ([`KioskFlow::select`](showcase_kiosk_flow::KioskFlow::select)).
///
/// Wire layout, 8 bytes: `seq` (`u32`), field (`u16`), option (`u8`), then
/// one zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Select {
    /// The UI's number for this action.
    pub seq: u32,
    /// The choice field.
    pub field: FieldId,
    /// The option chosen, from 0.
    pub option: u8,
}

impl Message for Select {
    const KIND: u16 = SELECT_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.seq)?;
        out.write_u16(self.field.0)?;
        out.write_u8(self.option)?;
        out.write_zeros(1)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let field = FieldId(input.read_u16()?);
        let option = input.read_u8()?;
        input.read_zeros(1)?;
        Ok(Self { seq, field, option })
    }
}

/// The person ticked or unticked a checkbox
/// ([`KioskFlow::set_checked`](showcase_kiosk_flow::KioskFlow::set_checked)).
///
/// Wire layout, 8 bytes: `seq` (`u32`), field (`u16`), checked (`0` or `1`),
/// then one zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SetChecked {
    /// The UI's number for this action.
    pub seq: u32,
    /// The checkbox.
    pub field: FieldId,
    /// Whether it is now ticked.
    pub checked: bool,
}

impl Message for SetChecked {
    const KIND: u16 = SET_CHECKED_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.seq)?;
        out.write_u16(self.field.0)?;
        out.write_bool(self.checked)?;
        out.write_zeros(1)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let field = FieldId(input.read_u16()?);
        let checked = input.read_bool()?;
        input.read_zeros(1)?;
        Ok(Self {
            seq,
            field,
            checked,
        })
    }
}

/// The person cleared a field, whatever its kind
/// ([`KioskFlow::clear_field`](showcase_kiosk_flow::KioskFlow::clear_field)).
///
/// Wire layout, 8 bytes: `seq` (`u32`), field (`u16`), then two zero bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClearField {
    /// The UI's number for this action.
    pub seq: u32,
    /// The field.
    pub field: FieldId,
}

impl Message for ClearField {
    const KIND: u16 = CLEAR_FIELD_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 8;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        out.write_u32(self.seq)?;
        out.write_u16(self.field.0)?;
        out.write_zeros(2)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let field = FieldId(input.read_u16()?);
        input.read_zeros(2)?;
        Ok(Self { seq, field })
    }
}

/// Any action, as [`decode_action`] returns it.
///
/// `Debug` output never shows entered text (see [`SetText`]), and
/// [`Wipe`] zeroes a [`SetText`]'s text in place; no other action holds any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// See [`Start`].
    Start(Start),
    /// See [`SetText`].
    SetText(SetText),
    /// See [`Select`].
    Select(Select),
    /// See [`SetChecked`].
    SetChecked(SetChecked),
    /// See [`ClearField`].
    ClearField(ClearField),
    /// See [`Next`].
    Next(Next),
    /// See [`Back`].
    Back(Back),
    /// See [`Submit`].
    Submit(Submit),
    /// See [`Retry`].
    Retry(Retry),
    /// See [`Cancel`].
    Cancel(Cancel),
    /// See [`Finish`].
    Finish(Finish),
    /// See [`StillHere`].
    StillHere(StillHere),
    /// See [`Abandon`].
    Abandon(Abandon),
    /// See [`SimulateFailure`].
    SimulateFailure(SimulateFailure),
}

impl Wipe for Action {
    fn wipe(&mut self) {
        if let Self::SetText(set) = self {
            set.wipe();
        }
    }
}

impl Action {
    /// The UI's number for the action.
    #[must_use]
    pub const fn seq(&self) -> u32 {
        match self {
            Self::Start(a) => a.seq,
            Self::SetText(a) => a.seq,
            Self::Select(a) => a.seq,
            Self::SetChecked(a) => a.seq,
            Self::ClearField(a) => a.seq,
            Self::Next(a) => a.seq,
            Self::Back(a) => a.seq,
            Self::Submit(a) => a.seq,
            Self::Retry(a) => a.seq,
            Self::Cancel(a) => a.seq,
            Self::Finish(a) => a.seq,
            Self::StillHere(a) => a.seq,
            Self::Abandon(a) => a.seq,
            Self::SimulateFailure(a) => a.seq,
        }
    }

    /// The record kind.
    #[must_use]
    pub const fn kind(&self) -> u16 {
        match self {
            Self::Start(_) => START_KIND,
            Self::SetText(_) => SET_TEXT_KIND,
            Self::Select(_) => SELECT_KIND,
            Self::SetChecked(_) => SET_CHECKED_KIND,
            Self::ClearField(_) => CLEAR_FIELD_KIND,
            Self::Next(_) => NEXT_KIND,
            Self::Back(_) => BACK_KIND,
            Self::Submit(_) => SUBMIT_KIND,
            Self::Retry(_) => RETRY_KIND,
            Self::Cancel(_) => CANCEL_KIND,
            Self::Finish(_) => FINISH_KIND,
            Self::StillHere(_) => STILL_HERE_KIND,
            Self::Abandon(_) => ABANDON_KIND,
            Self::SimulateFailure(_) => SIMULATE_FAILURE_KIND,
        }
    }

    /// Encodes the action as a record at the start of `out`, returning its
    /// length; [`MAX_RECORD_LEN`](crate::MAX_RECORD_LEN) bytes always
    /// suffice.
    ///
    /// # Errors
    ///
    /// As for [`nowayos_ipc::encode`]: [`EncodeError::BufferTooSmall`] if
    /// `out` is too short.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        match self {
            Self::Start(a) => encode(a, out),
            Self::SetText(a) => encode(a, out),
            Self::Select(a) => encode(a, out),
            Self::SetChecked(a) => encode(a, out),
            Self::ClearField(a) => encode(a, out),
            Self::Next(a) => encode(a, out),
            Self::Back(a) => encode(a, out),
            Self::Submit(a) => encode(a, out),
            Self::Retry(a) => encode(a, out),
            Self::Cancel(a) => encode(a, out),
            Self::Finish(a) => encode(a, out),
            Self::StillHere(a) => encode(a, out),
            Self::Abandon(a) => encode(a, out),
            Self::SimulateFailure(a) => encode(a, out),
        }
    }
}

/// Decodes one action record, already copied out of the ring into private
/// memory (nowayos-ipc's `Consumer` does that). Bytes after the record, such
/// as the rest of a ring slot, are ignored.
///
/// # Errors
///
/// [`ContractError::UnexpectedKind`] for a kind that is not an action
/// (including every report kind); [`ContractError::Decode`] for a malformed
/// record, a wrong version or length among them. Never panics, whatever the
/// bytes.
pub fn decode_action(record: &[u8]) -> Result<Action, ContractError> {
    let action = match Header::parse(record)?.kind {
        START_KIND => Action::Start(decode(record)?),
        SET_TEXT_KIND => Action::SetText(decode(record)?),
        SELECT_KIND => Action::Select(decode(record)?),
        SET_CHECKED_KIND => Action::SetChecked(decode(record)?),
        CLEAR_FIELD_KIND => Action::ClearField(decode(record)?),
        NEXT_KIND => Action::Next(decode(record)?),
        BACK_KIND => Action::Back(decode(record)?),
        SUBMIT_KIND => Action::Submit(decode(record)?),
        RETRY_KIND => Action::Retry(decode(record)?),
        CANCEL_KIND => Action::Cancel(decode(record)?),
        FINISH_KIND => Action::Finish(decode(record)?),
        STILL_HERE_KIND => Action::StillHere(decode(record)?),
        ABANDON_KIND => Action::Abandon(decode(record)?),
        SIMULATE_FAILURE_KIND => Action::SimulateFailure(decode(record)?),
        kind => return Err(ContractError::UnexpectedKind { kind }),
    };
    Ok(action)
}
