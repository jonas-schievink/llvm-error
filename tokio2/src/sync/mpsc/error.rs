//! Channel error types

/// Error returned by the `Sender`.
#[derive(Debug)]
pub struct SendError<T>(pub T);

// ===== TrySendError =====

/// This enumeration is the list of the possible error outcomes for the
/// [try_send](super::Sender::try_send) method.
#[derive(Debug)]
pub enum TrySendError<T> {
    /// The data could not be sent on the channel because the channel is
    /// currently full and sending would require blocking.
    Full(T),

    /// The receive half of the channel was explicitly closed or has been
    /// dropped.
    Closed(T),
}

// ===== ClosedError =====

/// Error returned by [`Sender::poll_ready`](super::Sender::poll_ready).
#[derive(Debug)]
pub struct ClosedError(());

impl ClosedError {
    pub(crate) fn new() -> ClosedError {
        ClosedError(())
    }
}
