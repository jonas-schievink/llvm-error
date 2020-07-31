use crate::runtime::task::core::{Cell, Core, Header, Trailer};
use crate::runtime::task::state::Snapshot;
use crate::runtime::task::Schedule;

use std::future::Future;
use std::panic;
use std::ptr::NonNull;
use std::task::{Poll, Waker};

/// Typed raw task handle
pub(super) struct Harness<T: Future, S: 'static> {
    cell: NonNull<Cell<T, S>>,
}

impl<T, S> Harness<T, S>
where
    T: Future,
    S: 'static,
{
    pub(super) unsafe fn from_raw(ptr: NonNull<Header>) -> Harness<T, S> {
        Harness {
            cell: ptr.cast::<Cell<T, S>>(),
        }
    }

    fn header(&self) -> &Header {
        unsafe { &self.cell.as_ref().header }
    }

    fn trailer(&self) -> &Trailer {
        unsafe { &self.cell.as_ref().trailer }
    }

    fn core(&self) -> &Core<T, S> {
        unsafe { &self.cell.as_ref().core }
    }
}

impl<T, S> Harness<T, S>
where
    T: Future,
    S: Schedule,
{
    pub(super) fn dealloc(self) {
        // Release the join waker, if there is one.
        self.trailer().waker.with_mut(|_| ());

        // Check causality
        self.core().stage.with_mut(|_| {});
        self.core().scheduler.with_mut(|_| {});

        unsafe {
            drop(Box::from_raw(self.cell.as_ptr()));
        }
    }

    // ===== join handle =====

    /// Read the task output into `dst`.
    pub(super) fn try_read_output(self, dst: &mut Poll<super::Result<T::Output>>, waker: &Waker) {
        // Load a snapshot of the current task state
        let snapshot = self.header().state.load();

        debug_assert!(snapshot.is_join_interested());

        if !snapshot.is_complete() {
            // The waker must be stored in the task struct.
            let res = if snapshot.has_join_waker() {
                // There already is a waker stored in the struct. If it matches
                // the provided waker, then there is no further work to do.
                // Otherwise, the waker must be swapped.
                let will_wake = unsafe {
                    // Safety: when `JOIN_INTEREST` is set, only `JOIN_HANDLE`
                    // may mutate the `waker` field.
                    self.trailer()
                        .waker
                        .with(|ptr| (*ptr).as_ref().unwrap().will_wake(waker))
                };

                if will_wake {
                    // The task is not complete **and** the waker is up to date,
                    // there is nothing further that needs to be done.
                    return;
                }

                // Unset the `JOIN_WAKER` to gain mutable access to the `waker`
                // field then update the field with the new join worker.
                //
                // This requires two atomic operations, unsetting the bit and
                // then resetting it. If the task transitions to complete
                // concurrently to either one of those operations, then setting
                // the join waker fails and we proceed to reading the task
                // output.
                self.header()
                    .state
                    .unset_waker()
                    .and_then(|snapshot| self.set_join_waker(waker.clone(), snapshot))
            } else {
                self.set_join_waker(waker.clone(), snapshot)
            };

            match res {
                Ok(_) => return,
                Err(snapshot) => {
                    assert!(snapshot.is_complete());
                }
            }
        }

        *dst = Poll::Ready(self.core().take_output());
    }

    fn set_join_waker(&self, waker: Waker, snapshot: Snapshot) -> Result<Snapshot, Snapshot> {
        assert!(snapshot.is_join_interested());
        assert!(!snapshot.has_join_waker());

        // Safety: Only the `JoinHandle` may set the `waker` field. When
        // `JOIN_INTEREST` is **not** set, nothing else will touch the field.
        unsafe {
            self.trailer().waker.with_mut(|ptr| {
                *ptr = Some(waker);
            });
        }

        // Update the `JoinWaker` state accordingly
        let res = self.header().state.set_join_waker();

        // If the state could not be updated, then clear the join waker
        if res.is_err() {
            unsafe {
                self.trailer().waker.with_mut(|ptr| {
                    *ptr = None;
                });
            }
        }

        res
    }

    pub(super) fn drop_join_handle_slow(self) {
        // Try to unset `JOIN_INTEREST`. This must be done as a first step in
        // case the task concurrently completed.
        if self.header().state.unset_join_interested().is_err() {
            // It is our responsibility to drop the output. This is critical as
            // the task output may not be `Send` and as such must remain with
            // the scheduler or `JoinHandle`. i.e. if the output remains in the
            // task structure until the task is deallocated, it may be dropped
            // by a Waker on any arbitrary thread.
            self.core().drop_future_or_output();
        }

        // Drop the `JoinHandle` reference, possibly deallocating the task
        self.drop_reference();
    }

    pub(super) fn drop_reference(self) {
        if self.header().state.ref_dec() {
            self.dealloc();
        }
    }
}
