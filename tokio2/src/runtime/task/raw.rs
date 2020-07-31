use crate::runtime::task::{Cell, Harness, Header, Schedule, State};

use std::future::Future;
use std::ptr::NonNull;
use std::task::{Poll, Waker};

/// Raw task handle
pub(super) struct RawTask {
    ptr: NonNull<Header>,
}

pub(super) struct Vtable {
    /// Read the task output, if complete
    pub(super) try_read_output: unsafe fn(NonNull<Header>, *mut (), &Waker),

    /// The join handle has been dropped
    pub(super) drop_join_handle_slow: unsafe fn(NonNull<Header>),
}

/// Get the vtable for the requested `T` and `S` generics.
pub(super) fn vtable<T: Future, S: Schedule>() -> &'static Vtable {
    &Vtable {
        try_read_output: try_read_output::<T, S>,
        drop_join_handle_slow: drop_join_handle_slow::<T, S>,
    }
}

impl RawTask {
    pub(super) fn new<T, S>(task: T) -> RawTask
    where
        T: Future,
        S: Schedule,
    {
        let ptr = Box::into_raw(Cell::<_, S>::new(task, State::new()));
        let ptr = unsafe { NonNull::new_unchecked(ptr as *mut Header) };

        RawTask { ptr }
    }

    /// Returns a reference to the task's meta structure.
    ///
    /// Safe as `Header` is `Sync`.
    pub(super) fn header(&self) -> &Header {
        unsafe { self.ptr.as_ref() }
    }

    /// Safety: `dst` must be a `*mut Poll<super::Result<T::Output>>` where `T`
    /// is the future stored by the task.
    pub(super) unsafe fn try_read_output(self, dst: *mut (), waker: &Waker) {
        let vtable = self.header().vtable;
        (vtable.try_read_output)(self.ptr, dst, waker);
    }

    pub(super) fn drop_join_handle_slow(self) {
        let vtable = self.header().vtable;
        unsafe { (vtable.drop_join_handle_slow)(self.ptr) }
    }
}

impl Clone for RawTask {
    fn clone(&self) -> Self {
        RawTask { ptr: self.ptr }
    }
}

impl Copy for RawTask {}

unsafe fn try_read_output<T: Future, S: Schedule>(
    ptr: NonNull<Header>,
    dst: *mut (),
    waker: &Waker,
) {
    let out = &mut *(dst as *mut Poll<super::Result<T::Output>>);

    let harness = Harness::<T, S>::from_raw(ptr);
    harness.try_read_output(out, waker);
}

unsafe fn drop_join_handle_slow<T: Future, S: Schedule>(ptr: NonNull<Header>) {
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.drop_join_handle_slow()
}
