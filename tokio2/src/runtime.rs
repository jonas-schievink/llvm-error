use crate::util::{waker_ref, Wake};
use std::future::Future;
use std::sync::Arc;
use std::task::Poll::Ready;

pub fn run<F: Future>(future: F) -> F::Output {
    BasicScheduler.block_on(future)
}

/// Executes tasks on the current thread
pub(crate) struct BasicScheduler;

impl BasicScheduler {
    pub(crate) fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
    {
        let waker = Arc::new(Waker);
        let waker = waker_ref(&waker);
        let mut cx = std::task::Context::from_waker(&waker);

        pin!(future);

        loop {
            if let Ready(v) = future.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }
}

// ===== impl Spawner =====

struct Waker;

impl Wake for Waker {
    fn wake(self: Arc<Self>) {}
    fn wake_by_ref(_: &Arc<Self>) {}
}
