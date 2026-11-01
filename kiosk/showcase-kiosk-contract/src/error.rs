// SPDX-License-Identifier: MIT OR Apache-2.0
//! Errors: records refused on receipt, and flows that cannot use the
//! contract.
//!
//! No error carries entered text: only kinds, field ids, lengths and the
//! static names of the fields that failed validation.

use core::fmt;

use nowayos_ipc::DecodeError;
use showcase_kiosk_flow::FieldId;

/// Why a record was refused. Nothing in this crate panics instead, whatever
/// the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    /// The record is malformed: truncated, the wrong version or length for
    /// its kind, non-zero padding, an unknown code, text that is not UTF-8
    /// or is longer than [`MAX_TEXT_LEN`](crate::MAX_TEXT_LEN). See the
    /// [`DecodeError`]; it names fields, never their contents.
    Decode(DecodeError),
    /// A well-formed header with a kind this end never receives: an
    /// unknown kind, or a record meant for the other ring.
    UnexpectedKind {
        /// The kind in the record's header.
        kind: u16,
    },
}

impl From<DecodeError> for ContractError {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(err) => write!(f, "malformed record: {err}"),
            Self::UnexpectedKind { kind } => write!(f, "unexpected record kind {kind:#06x}"),
        }
    }
}

impl core::error::Error for ContractError {}

/// Why a flow cannot be driven through the contract
/// ([`AppEnd::new`](crate::AppEnd::new)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    /// The form has more steps than a [`ShowStep`](crate::ShowStep) can
    /// number (`u16::MAX`).
    TooManySteps {
        /// How many steps the form has.
        steps: usize,
    },
    /// A text field allows more bytes than a [`SetText`](crate::SetText)
    /// carries ([`MAX_TEXT_LEN`](crate::MAX_TEXT_LEN)), so the person could
    /// never fill it.
    TextTooLong {
        /// The field concerned.
        field: FieldId,
        /// The field's maximum length in bytes.
        max_len: u16,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySteps { steps } => {
                write!(
                    f,
                    "the form has {steps} steps, more than the contract numbers"
                )
            }
            Self::TextTooLong { field, max_len } => write!(
                f,
                "field {} allows {max_len} bytes, more than a record carries ({})",
                field.0,
                crate::MAX_TEXT_LEN
            ),
        }
    }
}

impl core::error::Error for SetupError {}
