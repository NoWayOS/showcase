// SPDX-License-Identifier: MIT OR Apache-2.0
//! The application's records with the network path: what the App
//! PD sends towards the backend and what comes back - outcomes only.
//! Their mapping onto requests to the backend is
//! the network PD's, and the submission request itself is still to come.
//!
//! | Ring | Records |
//! | --- | --- |
//! | app → network | [`ToNetwork`]: a [`CheckIn`] |
//! | network → app | [`FromNetwork`]: a [`CheckedIn`] answer, or a [`SubmissionResult`] naming its ticket |
//!
//! Each record has a kind of its own, apart from every action's and
//! report's, so a record on the wrong ring is refused; each is checked on
//! receipt like the app-UI contract's, with zero padding and every code
//! known, and decoding never panics, whatever the bytes.

use nowayos_ipc::{DecodeError, EncodeError, Header, Message, Reader, Writer, decode, encode};
use showcase_kiosk_flow::SubmissionTicket;

use crate::report::encode_error;
use crate::{ContractError, ISSUED_LEN, Issued, VERSION};

/// Kind of [`CheckIn`].
pub const CHECK_IN_KIND: u16 = 0x4B41;
/// Kind of [`CheckedIn`].
pub const CHECKED_IN_KIND: u16 = 0x4BC1;
/// Kind of [`SubmissionResult`].
pub const SUBMISSION_RESULT_KIND: u16 = 0x4BC2;

/// Every kind the App sends the network path.
pub const TO_NETWORK_KINDS: [u16; 1] = [CHECK_IN_KIND];
/// Every kind the network path sends the App.
pub const FROM_NETWORK_KINDS: [u16; 2] = [CHECKED_IN_KIND, SUBMISSION_RESULT_KIND];

/// The App checks in with the backend: once at each
/// start, and again until it has an epoch. Its `seq` is the App's own, and
/// the answer quotes it. It carries nothing else: the connection carrying
/// it authenticates the kiosk.
///
/// Wire layout, 4 bytes: `seq` (`u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckIn {
    /// The App's number for this check-in.
    pub seq: u32,
}

impl Message for CheckIn {
    const KIND: u16 = CHECK_IN_KIND;
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

/// How the backend answered a check-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckInAnswer {
    /// The kiosk's key is approved, and this start's epoch is the
    /// backend's next count of check-ins, never 0.
    Approved {
        /// The epoch the App makes its flow with.
        epoch: u32,
    },
    /// The backend does not know the key, or has not approved it yet: the
    /// kiosk waits, showing its approval code.
    NotApproved,
    /// No answer from the backend: unreachable, or the connection failed.
    Failed,
}

/// The network path's answer to a [`CheckIn`], quoting its `seq`.
///
/// Wire layout, 12 bytes: `seq` (`u32`), the answer (`u8`: `1` approved,
/// `2` not approved, `3` failed), three zero bytes, then the epoch (`u32`)
/// - not 0 when approved, 0 otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckedIn {
    /// The `seq` of the check-in answered.
    pub seq: u32,
    /// The answer.
    pub answer: CheckInAnswer,
}

impl Message for CheckedIn {
    const KIND: u16 = CHECKED_IN_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 12;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        let (answer, epoch) = match self.answer {
            CheckInAnswer::Approved { epoch: 0 } => {
                return Err(EncodeError::InvalidValue { field: "epoch" });
            }
            CheckInAnswer::Approved { epoch } => (1, epoch),
            CheckInAnswer::NotApproved => (2, 0),
            CheckInAnswer::Failed => (3, 0),
        };
        out.write_u32(self.seq)?;
        out.write_u8(answer)?;
        out.write_zeros(3)?;
        out.write_u32(epoch)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let seq = input.read_u32()?;
        let answer = input.read_u8()?;
        input.read_zeros(3)?;
        let epoch = input.read_u32()?;
        let answer = match (answer, epoch) {
            (1..=3, _) if (answer == 1) == (epoch == 0) => {
                return Err(DecodeError::InvalidValue { field: "epoch" });
            }
            (1, epoch) => CheckInAnswer::Approved { epoch },
            (2, _) => CheckInAnswer::NotApproved,
            (3, _) => CheckInAnswer::Failed,
            _ => return Err(DecodeError::InvalidValue { field: "answer" }),
        };
        Ok(Self { seq, answer })
    }
}

/// What the backend did with a submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubmissionOutcome {
    /// Accepted, with what the service issued for it, if anything: a pass
    /// or queue number.
    Accepted {
        /// What was issued.
        issued: Option<Issued>,
    },
    /// The service turned the answers down.
    Rejected,
    /// The service could not take them now.
    Unavailable,
    /// The kiosk's key is no longer approved: revoked, or expired.
    NotApproved,
}

/// The result of one submission, naming its ticket: the flow refuses one
/// whose ticket names another epoch, session or attempt as stale.
///
/// Wire layout, 20 bytes: the ticket's epoch (`u32`, not 0), session
/// (`u32`) and attempt (`u8`, not 0); the outcome (`u8`: `1` accepted, `2`
/// rejected, `3` unavailable, `4` not approved); two zero bytes; then what
/// was issued, as [`Issued`]'s 8 bytes - zeros unless accepted with a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmissionResult {
    /// The submission's ticket.
    pub ticket: SubmissionTicket,
    /// What became of it.
    pub outcome: SubmissionOutcome,
}

impl Message for SubmissionResult {
    const KIND: u16 = SUBMISSION_RESULT_KIND;
    const VERSION: u16 = VERSION;
    const PAYLOAD_LEN: usize = 12 + ISSUED_LEN;

    fn encode_payload(&self, out: &mut Writer<'_>) -> Result<(), EncodeError> {
        let ticket = self.ticket;
        if ticket.epoch == 0 {
            return Err(EncodeError::InvalidValue { field: "epoch" });
        }
        if ticket.attempt == 0 {
            return Err(EncodeError::InvalidValue { field: "attempt" });
        }
        let (outcome, issued) = match self.outcome {
            SubmissionOutcome::Accepted { issued } => (1, issued),
            SubmissionOutcome::Rejected => (2, None),
            SubmissionOutcome::Unavailable => (3, None),
            SubmissionOutcome::NotApproved => (4, None),
        };
        let code = match issued {
            Some(issued) => Issued::checked(issued.code).map_err(encode_error)?.code,
            None => [0; ISSUED_LEN],
        };
        out.write_u32(ticket.epoch)?;
        out.write_u32(ticket.session)?;
        out.write_u8(ticket.attempt)?;
        out.write_u8(outcome)?;
        out.write_zeros(2)?;
        out.write_bytes(&code)
    }

    fn decode_payload(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let epoch = input.read_u32()?;
        let session = input.read_u32()?;
        let attempt = input.read_u8()?;
        let outcome = input.read_u8()?;
        input.read_zeros(2)?;
        let code: [u8; ISSUED_LEN] = input.read_array()?;
        if epoch == 0 {
            return Err(DecodeError::InvalidValue { field: "epoch" });
        }
        if attempt == 0 {
            return Err(DecodeError::InvalidValue { field: "attempt" });
        }
        let none = code == [0; ISSUED_LEN];
        let outcome = match outcome {
            1 if none => SubmissionOutcome::Accepted { issued: None },
            1 => SubmissionOutcome::Accepted {
                issued: Some(Issued::checked(code)?),
            },
            2..=4 if !none => return Err(DecodeError::InvalidValue { field: "code" }),
            2 => SubmissionOutcome::Rejected,
            3 => SubmissionOutcome::Unavailable,
            4 => SubmissionOutcome::NotApproved,
            _ => return Err(DecodeError::InvalidValue { field: "outcome" }),
        };
        Ok(Self {
            ticket: SubmissionTicket {
                epoch,
                session,
                attempt,
            },
            outcome,
        })
    }
}

/// A record the App sends the network path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToNetwork {
    /// See [`CheckIn`].
    CheckIn(CheckIn),
}

impl ToNetwork {
    /// The record's kind.
    #[must_use]
    pub const fn kind(&self) -> u16 {
        match self {
            Self::CheckIn(_) => CHECK_IN_KIND,
        }
    }

    /// Encodes the record into the start of `out`, returning its length.
    ///
    /// # Errors
    ///
    /// As for [`nowayos_ipc::encode`].
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        match self {
            Self::CheckIn(r) => encode(r, out),
        }
    }
}

/// A record the network path sends the App.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FromNetwork {
    /// See [`CheckedIn`].
    CheckedIn(CheckedIn),
    /// See [`SubmissionResult`].
    SubmissionResult(SubmissionResult),
}

impl FromNetwork {
    /// The record's kind.
    #[must_use]
    pub const fn kind(&self) -> u16 {
        match self {
            Self::CheckedIn(_) => CHECKED_IN_KIND,
            Self::SubmissionResult(_) => SUBMISSION_RESULT_KIND,
        }
    }

    /// Encodes the record into the start of `out`, returning its length.
    ///
    /// # Errors
    ///
    /// As for [`nowayos_ipc::encode`]: also [`EncodeError::InvalidValue`]
    /// for an epoch or attempt of 0, or an issued code that is not one.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        match self {
            Self::CheckedIn(r) => encode(r, out),
            Self::SubmissionResult(r) => encode(r, out),
        }
    }
}

/// Decodes one record for the network path, already copied out of its
/// ring. Bytes after the record are ignored.
///
/// # Errors
///
/// [`ContractError::UnexpectedKind`] for any other kind;
/// [`ContractError::Decode`] for a malformed record. Never panics.
pub fn decode_to_network(record: &[u8]) -> Result<ToNetwork, ContractError> {
    match Header::parse(record)?.kind {
        CHECK_IN_KIND => Ok(ToNetwork::CheckIn(decode(record)?)),
        kind => Err(ContractError::UnexpectedKind { kind }),
    }
}

/// Decodes one record from the network path, already copied out of its
/// ring. Bytes after the record are ignored.
///
/// # Errors
///
/// [`ContractError::UnexpectedKind`] for any other kind;
/// [`ContractError::Decode`] for a malformed record - an approval with
/// epoch 0 or a ticket with epoch or attempt 0 among them. Never panics.
pub fn decode_from_network(record: &[u8]) -> Result<FromNetwork, ContractError> {
    match Header::parse(record)?.kind {
        CHECKED_IN_KIND => Ok(FromNetwork::CheckedIn(decode(record)?)),
        SUBMISSION_RESULT_KIND => Ok(FromNetwork::SubmissionResult(decode(record)?)),
        kind => Err(ContractError::UnexpectedKind { kind }),
    }
}
