use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Arc;
use crate::sync::mpsc::list;
use crate::sync::AtomicWaker;

use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// Channel sender
#[allow(dead_code)]
pub(crate) struct Tx<T, S: Semaphore> {
    inner: Arc<Chan<T, S>>,
}

/// Channel receiver
pub(crate) struct Rx<T, S: Semaphore> {
    inner: Arc<Chan<T, S>>,
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TrySendError {
    Closed,
    Full,
}

pub(crate) trait Semaphore {
    fn is_idle(&self) -> bool;
}

#[allow(dead_code)]
struct Chan<T, S> {
    /// Handle to the push half of the lock-free list.
    tx: list::Tx<T>,

    /// Coordinates access to channel's capacity.
    semaphore: S,

    /// Receiver waker. Notified when a value is pushed into the channel.
    rx_waker: AtomicWaker,

    /// Only accessed by `Rx` handle.
    rx_fields: RxFields<T>,
}

#[allow(dead_code)]
/// Fields only accessed by `Rx` handle.
struct RxFields<T> {
    /// Channel receiver. This field is only accessed by the `Receiver` type.
    list: list::Rx<T>,

    /// `true` if `Rx::close` is called.
    rx_closed: bool,
}

unsafe impl<T: Send, S: Send> Send for Chan<T, S> {}
unsafe impl<T: Send, S: Sync> Sync for Chan<T, S> {}

pub(crate) fn channel<T>(semaphore: AtomicUsize) -> (Tx<T, AtomicUsize>, Rx<T, AtomicUsize>) {
    let (tx, rx) = list::channel();

    let chan = Arc::new(Chan {
        tx,
        semaphore,
        rx_waker: AtomicWaker::new(),
        rx_fields: (RxFields {
            list: rx,
            rx_closed: false,
        }),
    });

    (Tx::new(chan.clone()), Rx::new(chan))
}

// ===== impl Tx =====

impl<T, S> Tx<T, S>
where
    S: Semaphore,
{
    fn new(chan: Arc<Chan<T, S>>) -> Tx<T, S> {
        Tx { inner: chan }
    }
}

// ===== impl Rx =====

impl<T, S> Rx<T, S>
where
    S: Semaphore,
{
    fn new(chan: Arc<Chan<T, S>>) -> Rx<T, S> {
        Rx { inner: chan }
    }

    /// Receive the next value
    pub(crate) fn recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.inner.rx_waker.register_by_ref(cx.waker());

        if self.inner.rx_fields.rx_closed && self.inner.semaphore.is_idle() {
            Ready(None)
        } else {
            Pending
        }
    }
}

// ===== impl Semaphore for AtomicUsize =====

impl Semaphore for AtomicUsize {
    fn is_idle(&self) -> bool {
        false
    }
}
