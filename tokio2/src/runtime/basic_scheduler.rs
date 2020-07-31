use crate::park::Park;
use crate::runtime;
use crate::runtime::task::{self, JoinHandle, Schedule, Task};
use crate::util::linked_list::LinkedList;
use crate::util::{waker_ref, Wake};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::task::Poll::Ready;
use std::time::Duration;

/// Executes tasks on the current thread
pub(crate) struct BasicScheduler<P>
where
    P: Park,
{
    /// Scheduler run queue
    ///
    /// When the scheduler is executed, the queue is removed from `self` and
    /// moved into `Context`.
    ///
    /// This indirection is to allow `BasicScheduler` to be `Send`.
    tasks: Option<Tasks>,

    /// Sendable task spawner
    spawner: Spawner,

    /// Current tick
    tick: u8,

    /// Thread park handle
    park: P,
}

#[derive(Clone)]
pub(crate) struct Spawner {
    shared: Arc<Shared>,
}

struct Tasks {
    /// Collection of all active tasks spawned onto this executor.
    owned: LinkedList<Task<Arc<Shared>>>,

    /// Local run queue.
    ///
    /// Tasks notified from the current thread are pushed into this queue.
    queue: VecDeque<task::Notified<Arc<Shared>>>,
}

/// Scheduler state shared between threads.
struct Shared {}

/// Thread-local context
struct Context {
    /// Shared scheduler state
    shared: Arc<Shared>,

    /// Local queue
    tasks: RefCell<Tasks>,
}

/// Initial queue capacity
const INITIAL_CAPACITY: usize = 64;

/// Max number of tasks to poll per tick.
const MAX_TASKS_PER_TICK: usize = 61;

/// How often ot check the remote queue first
const REMOTE_FIRST_INTERVAL: u8 = 31;

// Tracks the current BasicScheduler
scoped_thread_local!(static CURRENT: Context);

impl<P> BasicScheduler<P>
where
    P: Park,
{
    pub(crate) fn new(park: P) -> BasicScheduler<P> {
        BasicScheduler {
            tasks: Some(Tasks {
                owned: LinkedList::new(),
                queue: VecDeque::with_capacity(INITIAL_CAPACITY),
            }),
            spawner: Spawner {
                shared: Arc::new(Shared {}),
            },
            tick: 0,
            park,
        }
    }

    pub(crate) fn spawner(&self) -> &Spawner {
        &self.spawner
    }

    pub(crate) fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
    {
        enter(self, |scheduler, context| {
            let _enter = runtime::enter(false);
            let waker = waker_ref(&scheduler.spawner.shared);
            let mut cx = std::task::Context::from_waker(&waker);

            pin!(future);

            'outer: loop {
                if let Ready(v) = crate::coop::budget(|| future.as_mut().poll(&mut cx)) {
                    return v;
                }

                for _ in 0..MAX_TASKS_PER_TICK {
                    // Get and increment the current tick
                    let tick = scheduler.tick;
                    scheduler.tick = scheduler.tick.wrapping_add(1);

                    let next = if tick % REMOTE_FIRST_INTERVAL == 0 {
                        scheduler
                            .spawner
                            .pop()
                            .or_else(|| context.tasks.borrow_mut().queue.pop_front())
                    } else {
                        context
                            .tasks
                            .borrow_mut()
                            .queue
                            .pop_front()
                            .or_else(|| scheduler.spawner.pop())
                    };

                    match next {
                        Some(task) => crate::coop::budget(|| task.run()),
                        None => {
                            // Park until the thread is signaled
                            scheduler.park.park().ok().expect("failed to park");

                            // Try polling the `block_on` future next
                            continue 'outer;
                        }
                    }
                }

                // Yield to the park, this drives the timer and pulls any pending
                // I/O events.
                scheduler
                    .park
                    .park_timeout(Duration::from_millis(0))
                    .ok()
                    .expect("failed to park");
            }
        })
    }
}

/// Enter the scheduler context. This sets the queue and other necessary
/// scheduler state in the thread-local
fn enter<F, R, P>(scheduler: &mut BasicScheduler<P>, f: F) -> R
where
    F: FnOnce(&mut BasicScheduler<P>, &Context) -> R,
    P: Park,
{
    // Ensures the run queue is placed back in the `BasicScheduler` instance
    // once `block_on` returns.`
    struct Guard<'a, P: Park> {
        context: Option<Context>,
        scheduler: &'a mut BasicScheduler<P>,
    }

    // Remove `tasks` from `self` and place it in a `Context`.
    let tasks = scheduler.tasks.take().expect("invalid state");

    let guard = Guard {
        context: Some(Context {
            shared: scheduler.spawner.shared.clone(),
            tasks: RefCell::new(tasks),
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

    fn pop(&self) -> Option<task::Notified<Arc<Shared>>> {
        None
    }
}

// ===== impl Shared =====

impl Schedule for Arc<Shared> {
    fn bind(task: Task<Self>) -> Arc<Shared> {
        CURRENT.with(|maybe_cx| {
            let cx = maybe_cx.expect("scheduler context missing");
            cx.tasks.borrow_mut().owned.push_front(task);
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
