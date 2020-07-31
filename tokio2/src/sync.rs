use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::task::Poll::{Pending, Ready};
use std::task::Waker;
use std::task::{Context, Poll};

/// Future for the [`poll_fn`] function.
pub struct PollFn<F> {
    f: F,
}

impl<F> Unpin for PollFn<F> {}

/// Creates a new future wrapping around a function returning [`Poll`].
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    PollFn { f }
}

impl<T, F> Future for PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        (&mut self.f)(cx)
    }
}

struct AtomicWaker {}

impl AtomicWaker {
    /// Create an `AtomicWaker`
    fn new() -> AtomicWaker {
        AtomicWaker {}
    }

    fn register_by_ref(&self, _waker: &Waker) {}
}

#[allow(dead_code)]
struct Tx<T> {
    inner: Arc<Chan<T>>,
}

struct Rx<T> {
    inner: Arc<Chan<T>>,
}

#[allow(dead_code)]
struct Chan<T> {
    tx: PhantomData<T>,
    semaphore: Sema,
    rx_waker: AtomicWaker,
    rx_closed: bool,
}

fn channel<T>() -> (Tx<T>, Rx<T>) {
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
    fn recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.inner.rx_waker.register_by_ref(cx.waker());

        if self.inner.rx_closed && self.inner.semaphore.is_idle() {
            Ready(None)
        } else {
            Pending
        }
    }
}

struct Sema(AtomicUsize);

impl Sema {
    fn is_idle(&self) -> bool {
        false
    }
}

pub struct UnboundedReceiver<T> {
    chan: Rx<T>,
}

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
