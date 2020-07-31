use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::usize;

pub(super) struct State {
    val: AtomicUsize,
}

/// Current state value
#[derive(Copy, Clone)]
pub(super) struct Snapshot(usize);

type UpdateResult = Result<Snapshot, Snapshot>;

/// The task is complete.
///
/// Once this bit is set, it is never unset
const COMPLETE: usize = 0b0010;

/// Extracts the task's lifecycle value from the state
const LIFECYCLE_MASK: usize = 0b11;

/// Flag tracking if the task has been pushed into a run queue.
const NOTIFIED: usize = 0b100;

/// The join handle is still around
const JOIN_INTEREST: usize = 0b1_000;

/// A join handle waker has been set
const JOIN_WAKER: usize = 0b10_000;

/// The task has been forcibly cancelled.
const CANCELLED: usize = 0b100_000;

/// All bits
const STATE_MASK: usize = LIFECYCLE_MASK | NOTIFIED | JOIN_INTEREST | JOIN_WAKER | CANCELLED;

/// Bits used by the ref count portion of the state.
const REF_COUNT_MASK: usize = !STATE_MASK;

/// Number of positions to shift the ref count
const REF_COUNT_SHIFT: usize = REF_COUNT_MASK.count_zeros() as usize;

/// One ref count
const REF_ONE: usize = 1 << REF_COUNT_SHIFT;

/// State a task is initialized with
///
/// A task is initialized with two references: one for the scheduler and one for
/// the `JoinHandle`. As the task starts with a `JoinHandle`, `JOIN_INTERST` is
/// set. A new task is immediately pushed into the run queue for execution and
/// starts with the `NOTIFIED` flag set.
const INITIAL_STATE: usize = (REF_ONE * 2) | JOIN_INTEREST | NOTIFIED;

/// All transitions are performed via RMW operations. This establishes an
/// unambiguous modification order.
impl State {
    /// Return a task's initial state
    pub(super) fn new() -> State {
        // A task is initialized with three references: one for the scheduler,
        // one for the `JoinHandle`, one for the task handle made available in
        // release. As the task starts with a `JoinHandle`, `JOIN_INTERST` is
        // set. A new task is immediately pushed into the run queue for
        // execution and starts with the `NOTIFIED` flag set.
        State {
            val: AtomicUsize::new(INITIAL_STATE),
        }
    }

    /// Loads the current state, establishes `Acquire` ordering.
    pub(super) fn load(&self) -> Snapshot {
        Snapshot(self.val.load(Acquire))
    }

    /// Optimistically tries to swap the state assuming the join handle is
    /// __immediately__ dropped on spawn
    pub(super) fn drop_join_handle_fast(&self) -> Result<(), ()> {
        use std::sync::atomic::Ordering::Relaxed;

        // Relaxed is acceptable as if this function is called and succeeds,
        // then nothing has been done w/ the join handle.
        //
        // The moment the join handle is used (polled), the `JOIN_WAKER` flag is
        // set, at which point the CAS will fail.
        //
        // Given this, there is no risk if this operation is reordered.
        self.val
            .compare_exchange_weak(
                INITIAL_STATE,
                (INITIAL_STATE - REF_ONE) & !JOIN_INTEREST,
                Release,
                Relaxed,
            )
            .map(|_| ())
            .map_err(|_| ())
    }

    /// Try to unset the JOIN_INTEREST flag.
    ///
    /// Returns `Ok` if the operation happens before the task transitions to a
    /// completed state, `Err` otherwise.
    pub(super) fn unset_join_interested(&self) -> UpdateResult {
        self.fetch_update(|curr| {
            assert!(curr.is_join_interested());

            if curr.is_complete() {
                return None;
            }

            let mut next = curr;
            next.unset_join_interested();

            Some(next)
        })
    }

    /// Set the `JOIN_WAKER` bit.
    ///
    /// Returns `Ok` if the bit is set, `Err` otherwise. This operation fails if
    /// the task has completed.
    pub(super) fn set_join_waker(&self) -> UpdateResult {
        self.fetch_update(|curr| {
            assert!(curr.is_join_interested());
            assert!(!curr.has_join_waker());

            if curr.is_complete() {
                return None;
            }

            let mut next = curr;
            next.set_join_waker();

            Some(next)
        })
    }

    /// Unsets the `JOIN_WAKER` bit.
    ///
    /// Returns `Ok` has been unset, `Err` otherwise. This operation fails if
    /// the task has completed.
    pub(super) fn unset_waker(&self) -> UpdateResult {
        self.fetch_update(|curr| {
            assert!(curr.is_join_interested());
            assert!(curr.has_join_waker());

            if curr.is_complete() {
                return None;
            }

            let mut next = curr;
            next.unset_join_waker();

            Some(next)
        })
    }

    /// Returns `true` if the task should be released.
    pub(super) fn ref_dec(&self) -> bool {
        let prev = Snapshot(self.val.fetch_sub(REF_ONE, AcqRel));
        prev.ref_count() == 1
    }

    fn fetch_update<F>(&self, mut f: F) -> Result<Snapshot, Snapshot>
    where
        F: FnMut(Snapshot) -> Option<Snapshot>,
    {
        let mut curr = self.load();

        loop {
            let next = match f(curr) {
                Some(next) => next,
                None => return Err(curr),
            };

            let res = self.val.compare_exchange(curr.0, next.0, AcqRel, Acquire);

            match res {
                Ok(_) => return Ok(next),
                Err(actual) => curr = Snapshot(actual),
            }
        }
    }
}

// ===== impl Snapshot =====

impl Snapshot {
    /// Returns `true` if the task's future has completed execution.
    pub(super) fn is_complete(self) -> bool {
        self.0 & COMPLETE == COMPLETE
    }

    pub(super) fn is_join_interested(self) -> bool {
        self.0 & JOIN_INTEREST == JOIN_INTEREST
    }

    fn unset_join_interested(&mut self) {
        self.0 &= !JOIN_INTEREST
    }

    pub(super) fn has_join_waker(self) -> bool {
        self.0 & JOIN_WAKER == JOIN_WAKER
    }

    fn set_join_waker(&mut self) {
        self.0 |= JOIN_WAKER;
    }

    fn unset_join_waker(&mut self) {
        self.0 &= !JOIN_WAKER
    }

    pub(super) fn ref_count(self) -> usize {
        (self.0 & REF_COUNT_MASK) >> REF_COUNT_SHIFT
    }
}
