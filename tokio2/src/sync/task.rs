#![cfg_attr(any(loom, not(feature = "sync")), allow(dead_code, unreachable_pub))]

use std::fmt;
use std::task::Waker;

/// A synchronization primitive for task waking.
///
/// `AtomicWaker` will coordinate concurrent wakes with the consumer
/// potentially "waking" the underlying task. This is useful in scenarios
/// where a computation completes in another thread and wants to wake the
/// consumer, but the consumer is in the process of being migrated to a new
/// logical task.
///
/// Consumers should call `register` before checking the result of a computation
/// and producers should call `wake` after producing the computation (this
/// differs from the usual `thread::park` pattern). It is also permitted for
/// `wake` to be called **before** `register`. This results in a no-op.
///
/// A single `AtomicWaker` may be reused for any number of calls to `register` or
/// `wake`.
pub(crate) struct AtomicWaker {}

impl AtomicWaker {
    /// Create an `AtomicWaker`
    pub(crate) fn new() -> AtomicWaker {
        AtomicWaker {}
    }

    /// Registers the provided waker to be notified on calls to `wake`.
    ///
    /// The new waker will take place of any previous wakers that were registered
    /// by previous calls to `register`. Any calls to `wake` that happen after
    /// a call to `register` (as defined by the memory ordering rules), will
    /// wake the `register` caller's task.
    ///
    /// It is safe to call `register` with multiple other threads concurrently
    /// calling `wake`. This will result in the `register` caller's current
    /// task being woken once.
    ///
    /// This function is safe to call concurrently, but this is generally a bad
    /// idea. Concurrent calls to `register` will attempt to register different
    /// tasks to be woken. One of the callers will win and have its task set,
    /// but there is no guarantee as to which caller will succeed.
    pub(crate) fn register_by_ref(&self, _waker: &Waker) {}
}

impl Default for AtomicWaker {
    fn default() -> Self {
        AtomicWaker::new()
    }
}

impl fmt::Debug for AtomicWaker {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "AtomicWaker")
    }
}
