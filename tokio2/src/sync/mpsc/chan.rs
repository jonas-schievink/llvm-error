use crate::loom::cell::UnsafeCell;
use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Arc;
use crate::sync::mpsc::error::ClosedError;
use crate::sync::mpsc::list;
use crate::sync::AtomicWaker;

use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// Channel sender
#[allow(dead_code)]
pub(crate) struct Tx<T, S: Semaphore> {
    inner: Arc<Chan<T, S>>,
    permit: S::Permit,
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
    type Permit;

    fn new_permit() -> Self::Permit;

    /// The permit is dropped without a value being sent. In this case, the
    /// permit must be returned to the semaphore.
    fn drop_permit(&self, permit: &mut Self::Permit);

    fn is_idle(&self) -> bool;

    fn add_permit(&self);

    fn poll_acquire(
        &self,
        cx: &mut Context<'_>,
        permit: &mut Self::Permit,
    ) -> Poll<Result<(), ClosedError>>;

    fn try_acquire(&self, permit: &mut Self::Permit) -> Result<(), TrySendError>;

    /// A value was sent into the channel and the permit held by `tx` is
    /// dropped. In this case, the permit should not immeditely be returned to
    /// the semaphore. Instead, the permit is returnred to the semaphore once
    /// the sent value is read by the rx handle.
    fn forget(&self, permit: &mut Self::Permit);

    fn close(&self);
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
    rx_fields: UnsafeCell<RxFields<T>>,
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

pub(crate) fn channel<T, S>(semaphore: S) -> (Tx<T, S>, Rx<T, S>)
where
    S: Semaphore,
{
    let (tx, rx) = list::channel();

    let chan = Arc::new(Chan {
        tx,
        semaphore,
        rx_waker: AtomicWaker::new(),
        rx_fields: UnsafeCell::new(RxFields {
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
        Tx {
            inner: chan,
            permit: S::new_permit(),
        }
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
        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            self.inner.rx_waker.register_by_ref(cx.waker());

            if rx_fields.rx_closed && self.inner.semaphore.is_idle() {
                Ready(None)
            } else {
                Pending
            }
        })
    }
}

// ===== impl Semaphore for AtomicUsize =====

impl Semaphore for AtomicUsize {
    type Permit = ();

    fn new_permit() {}

    fn drop_permit(&self, _permit: &mut ()) {}

    fn add_permit(&self) {}

    fn is_idle(&self) -> bool {
        false
    }

    fn poll_acquire(
        &self,
        _cx: &mut Context<'_>,
        permit: &mut (),
    ) -> Poll<Result<(), ClosedError>> {
        Ready(self.try_acquire(permit).map_err(|_| ClosedError::new()))
    }

    fn try_acquire(&self, _permit: &mut ()) -> Result<(), TrySendError> {
        Ok(())
    }

    fn forget(&self, _permit: &mut ()) {}

    fn close(&self) {}
}
