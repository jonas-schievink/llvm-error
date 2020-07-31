use std::task::Waker;

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use std::marker::PhantomData;
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

use crate::future::poll_fn;

struct AtomicWaker {}

impl AtomicWaker {
    /// Create an `AtomicWaker`
    fn new() -> AtomicWaker {
        AtomicWaker {}
    }

    fn register_by_ref(&self, _waker: &Waker) {}
}

/// Channel sender
#[allow(dead_code)]
pub(crate) struct Tx<T> {
    inner: Arc<Chan<T>>,
}

/// Channel receiver
pub(crate) struct Rx<T> {
    inner: Arc<Chan<T>>,
}

#[allow(dead_code)]
struct Chan<T> {
    /// Handle to the push half of the lock-free list.
    tx: PhantomData<T>,

    /// Coordinates access to channel's capacity.
    semaphore: Sema,

    /// Receiver waker. Notified when a value is pushed into the channel.
    rx_waker: AtomicWaker,

    rx_closed: bool,
}

pub(crate) fn channel<T>() -> (Tx<T>, Rx<T>) {
    let chan = Arc::new(Chan {
        tx: PhantomData,
        semaphore: Sema(AtomicUsize::new(0)),
        rx_waker: AtomicWaker::new(),
        rx_closed: false,
    });

    (
        Tx {
            inner: chan.clone(),
        },
        Rx { inner: chan },
    )
}

// ===== impl Rx =====

impl<T> Rx<T> {
    /// Receive the next value
    pub(crate) fn recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.inner.rx_waker.register_by_ref(cx.waker());

        if self.inner.rx_closed && self.inner.semaphore.is_idle() {
            Ready(None)
        } else {
            Pending
        }
    }
}

// ===== impl Semaphore for AtomicUsize =====

struct Sema(AtomicUsize);

impl Sema {
    fn is_idle(&self) -> bool {
        false
    }
}

/// Receive values from the associated `UnboundedSender`.
///
/// Instances are created by the
/// [`unbounded_channel`](unbounded_channel) function.
pub struct UnboundedReceiver<T> {
    /// The channel receiver
    chan: Rx<T>,
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
    let (tx, rx) = channel();

    drop(tx);
    let rx = UnboundedReceiver { chan: rx };

    rx
}

impl<T> UnboundedReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        poll_fn(|cx| self.chan.recv(cx)).await
    }
}
