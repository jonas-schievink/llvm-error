use std::future::Future;
use std::pin::Pin;
use std::{
    ptr,
    task::{Poll::Ready, RawWaker, RawWakerVTable, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    BasicScheduler.block_on(future)
}

/// Executes tasks on the current thread
pub(crate) struct BasicScheduler;

impl BasicScheduler {
    pub(crate) fn block_on<F>(&mut self, mut future: F) -> F::Output
    where
        F: Future,
    {
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut cx = std::task::Context::from_waker(&waker);

        let mut future = unsafe { Pin::new_unchecked(&mut future) };

        loop {
            if let Ready(v) = future.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }
}

// ===== impl Spawner =====

fn raw_waker() -> RawWaker {
    RawWaker::new(ptr::null(), waker_vtable())
}

fn waker_vtable() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw,
        wake_arc_raw,
        wake_by_ref_arc_raw,
        drop_arc_raw,
    )
}

unsafe fn clone_arc_raw(_: *const ()) -> RawWaker {
    raw_waker()
}

unsafe fn wake_arc_raw(_: *const ()) {}

unsafe fn wake_by_ref_arc_raw(_: *const ()) {}

unsafe fn drop_arc_raw(_: *const ()) {}
