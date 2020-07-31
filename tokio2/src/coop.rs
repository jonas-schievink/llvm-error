use std::cell::Cell;

thread_local! {
    static CURRENT: Cell<Budget> = Cell::new(Budget::unconstrained());
}

/// Opaque type tracking the amount of "work" a task may still do before
/// yielding back to the scheduler.
#[derive(Debug, Copy, Clone)]
pub(crate) struct Budget(Option<u8>);

impl Budget {
    /// Returns an unconstrained budget. Operations will not be limited.
    const fn unconstrained() -> Budget {
        Budget(None)
    }
}

cfg_blocking_impl! {
    /// Forcibly remove the budgeting constraints early.
    ///
    /// Returns the remaining budget
    pub(crate) fn stop() -> Budget {
        CURRENT.with(|cell| {
            let prev = cell.get();
            cell.set(Budget::unconstrained());
            prev
        })
    }
}

cfg_coop! {
    use std::task::{Context, Poll};

    /// Returns `Poll::Pending` if the current task has exceeded its budget and should yield.
    #[inline]
    pub(crate) fn poll_proceed(cx: &mut Context<'_>) -> Poll<()> {
        CURRENT.with(|cell| {
            let mut budget = cell.get();

            if budget.decrement() {
                cell.set(budget);
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
    }

    impl Budget {
        /// Decrement the budget. Returns `true` if successful. Decrementing fails
        /// when there is not enough remaining budget.
        fn decrement(&mut self) -> bool {
            if let Some(num) = &mut self.0 {
                if *num > 0 {
                    *num -= 1;
                    true
                } else {
                    false
                }
            } else {
                true
            }
    }
    }
}
