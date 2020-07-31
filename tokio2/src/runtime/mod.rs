pub(crate) mod context;

mod basic_scheduler;
use basic_scheduler::BasicScheduler;

pub(crate) mod task;

mod blocking;
use blocking::BlockingPool;

mod builder;
pub use self::builder::Builder;

pub(crate) mod enter;
use self::enter::enter;

mod handle;
pub use self::handle::{Handle, TryCurrentError};

mod io;

mod spawner;
use self::spawner::Spawner;

mod time;

use std::future::Future;

/// The Tokio runtime.
///
/// The runtime provides an I/O driver, task scheduler, [timer], and blocking
/// pool, necessary for running asynchronous tasks.
///
/// Instances of `Runtime` can be created using [`new`] or [`Builder`]. However,
/// most users will use the `#[tokio::main]` annotation on their entry point instead.
///
/// See [module level][mod] documentation for more details.
///
/// # Shutdown
///
/// Shutting down the runtime is done by dropping the value. The current thread
/// will block until the shut down operation has completed.
///
/// * Drain any scheduled work queues.
/// * Drop any futures that have not yet completed.
/// * Drop the reactor.
///
/// Once the reactor has dropped, any outstanding I/O resources bound to
/// that reactor will no longer function. Calling any method on them will
/// result in an error.
///
/// [timer]: crate::time
/// [mod]: index.html
/// [`new`]: #method.new
/// [`Builder`]: struct@Builder
/// [`tokio::run`]: fn@run
#[derive(Debug)]
pub struct Runtime {
    /// Task executor
    kind: Kind,

    /// Handle to runtime, also contains driver handles
    handle: Handle,

    /// Blocking pool handle, used to signal shutdown
    blocking_pool: BlockingPool,
}

/// The runtime executor is either a thread-pool or a current-thread executor.
#[derive(Debug)]
enum Kind {
    /// Execute all tasks on the current-thread.
    Basic(BasicScheduler<time::Driver>),
}

/// After thread starts / before thread stops
type Callback = std::sync::Arc<dyn Fn() + Send + Sync>;

impl Runtime {
    pub fn block_on<F: Future>(&mut self, future: F) -> F::Output {
        let kind = &mut self.kind;

        self.handle.enter(|| match kind {
            Kind::Basic(exec) => exec.block_on(future),
        })
    }
}
