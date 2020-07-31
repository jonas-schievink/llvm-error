use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Arc;
use crate::sync::AtomicWaker;

use std::marker::PhantomData;
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

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
    semaphore: AtomicUsize,

    /// Receiver waker. Notified when a value is pushed into the channel.
    rx_waker: AtomicWaker,

    rx_closed: bool,
}

pub(crate) fn channel<T>() -> (Tx<T>, Rx<T>) {
    let chan = Arc::new(Chan {
        tx: PhantomData,
        semaphore: AtomicUsize::new(0),
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

impl AtomicUsize {
    fn is_idle(&self) -> bool {
        false
    }
}
