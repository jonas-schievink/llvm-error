use crate::loom::cell::UnsafeCell;
use crate::loom::future::AtomicWaker;
use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Arc;
use crate::sync::mpsc::error::ClosedError;
use crate::sync::mpsc::{error, list};

use std::sync::atomic::Ordering::Relaxed;
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

impl<T> From<(T, TrySendError)> for error::SendError<T> {
    fn from(src: (T, TrySendError)) -> error::SendError<T> {
        match src.1 {
            TrySendError::Closed => error::SendError(src.0),
            TrySendError::Full => unreachable!(),
        }
    }
}

impl<T> From<(T, TrySendError)> for error::TrySendError<T> {
    fn from(src: (T, TrySendError)) -> error::TrySendError<T> {
        match src.1 {
            TrySendError::Closed => error::TrySendError::Closed(src.0),
            TrySendError::Full => error::TrySendError::Full(src.0),
        }
    }
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

struct Chan<T, S> {
    /// Handle to the push half of the lock-free list.
    tx: list::Tx<T>,

    /// Coordinates access to channel's capacity.
    semaphore: S,

    /// Receiver waker. Notified when a value is pushed into the channel.
    rx_waker: AtomicWaker,

    /// Tracks the number of outstanding sender handles.
    ///
    /// When this drops to zero, the send half of the channel is closed.
    tx_count: AtomicUsize,

    /// Only accessed by `Rx` handle.
    rx_fields: UnsafeCell<RxFields<T>>,
}

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
        tx_count: AtomicUsize::new(1),
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

impl<T, S> Clone for Tx<T, S>
where
    S: Semaphore,
{
    fn clone(&self) -> Tx<T, S> {
        // Using a Relaxed ordering here is sufficient as the caller holds a
        // strong ref to `self`, preventing a concurrent decrement to zero.
        self.inner.tx_count.fetch_add(1, Relaxed);

        Tx {
            inner: self.inner.clone(),
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
        use super::block::Read::*;

        // Keep track of task budget
        ready!(crate::coop::poll_proceed(cx));

        self.inner.rx_fields.with_mut(|rx_fields_ptr| {
            let rx_fields = unsafe { &mut *rx_fields_ptr };

            macro_rules! try_recv {
                () => {
                    match rx_fields.list.pop(&self.inner.tx) {
                        Some(Value(value)) => {
                            self.inner.semaphore.add_permit();
                            return Ready(Some(value));
                        }
                        Some(Closed) => {
                            // TODO: This check may not be required as it most
                            // likely can only return `true` at this point. A
                            // channel is closed when all tx handles are
                            // dropped. Dropping a tx handle releases memory,
                            // which ensures that if dropping the tx handle is
                            // visible, then all messages sent are also visible.
                            assert!(self.inner.semaphore.is_idle());
                            return Ready(None);
                        }
                        None => {} // fall through
                    }
                };
            }

            try_recv!();

            self.inner.rx_waker.register_by_ref(cx.waker());

            // It is possible that a value was pushed between attempting to read
            // and registering the task, so we have to check the channel a
            // second time here.
            try_recv!();

            if rx_fields.rx_closed && self.inner.semaphore.is_idle() {
                Ready(None)
            } else {
                Pending
            }
        })
    }
}

// ===== impl Semaphore for (::Semaphore, capacity) =====

use crate::sync::semaphore_ll::Permit;

impl Semaphore for (crate::sync::semaphore_ll::Semaphore, usize) {
    type Permit = Permit;

    fn new_permit() -> Permit {
        Permit::new()
    }

    fn drop_permit(&self, _permit: &mut Permit) {}

    fn add_permit(&self) {}

    fn is_idle(&self) -> bool {
        false
    }

    fn poll_acquire(
        &self,
        cx: &mut Context<'_>,
        permit: &mut Permit,
    ) -> Poll<Result<(), ClosedError>> {
        // Keep track of task budget
        ready!(crate::coop::poll_proceed(cx));

        permit
            .poll_acquire(cx, 1, &self.0)
            .map_err(|_| ClosedError::new())
    }

    fn try_acquire(&self, _permit: &mut Permit) -> Result<(), TrySendError> {
        Ok(())
    }

    fn forget(&self, _permit: &mut Self::Permit) {}

    fn close(&self) {}
}

// ===== impl Semaphore for AtomicUsize =====

use std::usize;

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
