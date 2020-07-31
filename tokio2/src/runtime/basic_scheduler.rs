use crate::runtime;
use crate::runtime::task::{self, JoinHandle, Schedule, Task};
use crate::util::{waker_ref, Wake};

use std::future::Future;
use std::sync::Arc;
use std::task::Poll::Ready;

/// Executes tasks on the current thread
pub(crate) struct BasicScheduler {
    /// Sendable task spawner
    spawner: Spawner,
}

#[derive(Clone)]
pub(crate) struct Spawner {
    shared: Arc<Shared>,
}

/// Scheduler state shared between threads.
struct Shared {}

/// Thread-local context
struct Context {
    /// Shared scheduler state
    shared: Arc<Shared>,
}

// Tracks the current BasicScheduler
scoped_thread_local!(static CURRENT: Context);

impl BasicScheduler {
    pub(crate) fn new() -> BasicScheduler {
        BasicScheduler {
            spawner: Spawner {
                shared: Arc::new(Shared {}),
            },
        }
    }

    pub(crate) fn spawner(&self) -> &Spawner {
        &self.spawner
    }

    pub(crate) fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
    {
        enter(self, |scheduler, _| {
            let _enter = runtime::enter(false);
            let waker = waker_ref(&scheduler.spawner.shared);
            let mut cx = std::task::Context::from_waker(&waker);

            pin!(future);

            loop {
                if let Ready(v) = future.as_mut().poll(&mut cx) {
                    return v;
                }
            }
        })
    }
}

/// Enter the scheduler context. This sets the queue and other necessary
/// scheduler state in the thread-local
fn enter<F, R>(scheduler: &mut BasicScheduler, f: F) -> R
where
    F: FnOnce(&mut BasicScheduler, &Context) -> R,
{
    // Ensures the run queue is placed back in the `BasicScheduler` instance
    // once `block_on` returns.`
    struct Guard<'a> {
        context: Option<Context>,
        scheduler: &'a mut BasicScheduler,
    }

    let guard = Guard {
        context: Some(Context {
            shared: scheduler.spawner.shared.clone(),
        }),
        scheduler,
    };

    let context = guard.context.as_ref().unwrap();
    let scheduler = &mut *guard.scheduler;

    CURRENT.set(context, || f(scheduler, context))
}

// ===== impl Spawner =====

impl Spawner {
    /// Spawns a future onto the thread pool
    pub(crate) fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (task, handle) = task::joinable(future);
        self.shared.schedule(task);
        handle
    }
}

// ===== impl Shared =====

impl Schedule for Arc<Shared> {
    fn bind(_: Task<Self>) -> Arc<Shared> {
        CURRENT.with(|maybe_cx| {
            let cx = maybe_cx.expect("scheduler context missing");
            cx.shared.clone()
        })
    }

    fn release(&self, _: &Task<Self>) -> Option<Task<Self>> {
        None
    }

    fn schedule(&self, _: task::Notified<Self>) {}
}

impl Wake for Shared {
    fn wake(self: Arc<Self>) {}

    /// Wake by reference
    fn wake_by_ref(_: &Arc<Self>) {}
}
