//! Errors from use cases. `kind` says how an inbound adapter reports it (an HTTP status, an exit code).
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input is malformed. Not retried.
    BadRequest,
    /// A signature does not match, or the signed time is out of range.
    Unauthorized,
    /// The caller may not do this (an invite that is used or expired).
    Forbidden,
    /// No such thing, or the caller may not know it exists.
    NotFound,
    /// It conflicts with what this node already has.
    Conflict,
    /// A team runtime or another node cannot act now. Retried.
    Unavailable,
    /// Storage or another failure inside this node.
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    /// Why the request was really refused, when `message` hides it (see `unseen`). For the node's own
    /// log; never sent to the caller.
    pub reason: Option<Reason>,
}

/// The real reason behind a refusal that looks like "not found" to the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
    pub text: String,
    /// The request carried a valid signature by a member of the org (or by its root): a peer that
    /// ought to get through, for example one whose clock is off. Always logged; other reasons may be
    /// dropped when they come in a flood.
    pub is_from_member: bool,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            reason: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadRequest, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
