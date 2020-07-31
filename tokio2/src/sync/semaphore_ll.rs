#![cfg_attr(not(feature = "sync"), allow(dead_code, unreachable_pub))]

//! Thread-safe, asynchronous counting semaphore.
//!
//! A `Semaphore` instance holds a set of permits. Permits are used to
//! synchronize access to a shared resource.
//!
//! Before accessing the shared resource, callers acquire a permit from the
//! semaphore. Once the permit is acquired, the caller then enters the critical
//! section. If no permits are available, then acquiring the semaphore returns
//! `Pending`. The task is woken once a permit becomes available.

use crate::loom::future::AtomicWaker;
use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize};

use std::task::Poll::Pending;
use std::task::{Context, Poll};
use std::usize;

/// Futures-aware semaphore.
pub(crate) struct Semaphore {}

/// A semaphore permit
///
/// Tracks the lifecycle of a semaphore permit.
///
/// An instance of `Permit` is intended to be used with a **single** instance of
/// `Semaphore`. Using a single instance of `Permit` with multiple semaphore
/// instances will result in unexpected behavior.
///
/// `Permit` does **not** release the permit back to the semaphore on drop. It
/// is the user's responsibility to ensure that `Permit::release` is called
/// before dropping the permit.
#[derive(Debug)]
pub(crate) struct Permit {
    waiter: Option<Box<Waiter>>,
}

/// Error returned by `Permit::poll_acquire`.
#[derive(Debug)]
pub(crate) struct AcquireError(());

/// Error returned by `Permit::try_acquire`.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum TryAcquireError {
    Closed,
    NoPermits,
}

/// Node used to notify the semaphore waiter when permit is available.
#[derive(Debug)]
struct Waiter {
    /// Stores waiter state.
    ///
    /// See `WaiterState` for more details.
    state: AtomicUsize,

    /// Task to wake when a permit is made available.
    waker: AtomicWaker,

    /// Next pointer in the queue of waiting senders.
    next: AtomicPtr<Waiter>,
}

/// Semaphore state
///
/// The 2 low bits track the modes.
///
/// - Closed
/// - Full
///
/// When not full, the rest of the `usize` tracks the total number of messages
/// in the channel. When full, the rest of the `usize` is a pointer to the tail
/// of the "waiting senders" queue.
#[derive(Copy, Clone)]
struct SemState(usize);

/// State for an individual waker node
#[derive(Debug, Copy, Clone)]
struct WaiterState(usize);

// ===== impl Permit =====

impl Permit {
    /// Creates a new `Permit`.
    ///
    /// The permit begins in the "unacquired" state.
    pub(crate) fn new() -> Permit {
        Permit { waiter: None }
    }

    /// Tries to acquire the permit. If no permits are available, the current task
    /// is notified once a new permit becomes available.
    pub(crate) fn poll_acquire(
        &mut self,
        _cx: &mut Context<'_>,
        _num_permits: u16,
        _semaphore: &Semaphore,
    ) -> Poll<Result<(), AcquireError>> {
        Pending
    }
}

impl Default for Permit {
    fn default() -> Self {
        Self::new()
    }
}
