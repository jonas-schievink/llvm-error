use crate::future::poll_fn;
use crate::sync::mpsc::chan;

/// Receive values from the associated `UnboundedSender`.
///
/// Instances are created by the
/// [`unbounded_channel`](unbounded_channel) function.
pub struct UnboundedReceiver<T> {
    /// The channel receiver
    chan: chan::Rx<T>,
}

/// Creates an unbounded mpsc channel for communicating between asynchronous
/// tasks.
///
/// A `send` on this channel will always succeed as long as the receive half has
/// not been closed. If the receiver falls behind, messages will be arbitrarily
/// buffered.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded_channel<T>() -> UnboundedReceiver<T> {
    let (tx, rx) = chan::channel();

    drop(tx);
    let rx = UnboundedReceiver { chan: rx };

    rx
}

impl<T> UnboundedReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        poll_fn(|cx| self.chan.recv(cx)).await
    }
}
