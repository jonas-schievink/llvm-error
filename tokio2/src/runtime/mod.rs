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

use crate::task::JoinHandle;

use std::future::Future;
use std::time::Duration;

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
    /// Create a new runtime instance with default configuration values.
    ///
    /// This results in a scheduler, I/O driver, and time driver being
    /// initialized. The type of scheduler used depends on what feature flags
    /// are enabled: if the `rt-threaded` feature is enabled, the [threaded
    /// scheduler] is used, while if only the `rt-core` feature is enabled, the
    /// [basic scheduler] is used instead.
    ///
    /// If the threaded scheduler is selected, it will not spawn
    /// any worker threads until it needs to, i.e. tasks are scheduled to run.
    ///
    /// Most applications will not need to call this function directly. Instead,
    /// they will use the  [`#[tokio::main]` attribute][main]. When more complex
    /// configuration is necessary, the [runtime builder] may be used.
    ///
    /// See [module level][mod] documentation for more details.
    ///
    /// # Examples
    ///
    /// Creating a new `Runtime` with default configuration values.
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// let rt = Runtime::new()
    ///     .unwrap();
    ///
    /// // Use the runtime...
    /// ```
    ///
    /// [mod]: index.html
    /// [main]: ../attr.main.html
    /// [threaded scheduler]: index.html#threaded-scheduler
    /// [basic scheduler]: index.html#basic-scheduler
    /// [runtime builder]: crate::runtime::Builder
    pub fn new() -> io::Result<Runtime> {
        #[cfg(feature = "rt-threaded")]
        let ret = Builder::new().threaded_scheduler().enable_all().build();

        #[cfg(all(not(feature = "rt-threaded"), feature = "rt-core"))]
        let ret = Builder::new().basic_scheduler().enable_all().build();

        #[cfg(not(feature = "rt-core"))]
        let ret = Builder::new().enable_all().build();

        ret
    }

    /// Spawn a future onto the Tokio runtime.
    ///
    /// This spawns the given future onto the runtime's executor, usually a
    /// thread pool. The thread pool is then responsible for polling the future
    /// until it completes.
    ///
    /// See [module level][mod] documentation for more details.
    ///
    /// [mod]: index.html
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = Runtime::new().unwrap();
    ///
    /// // Spawn a future onto the runtime
    /// rt.spawn(async {
    ///     println!("now running on a worker thread");
    /// });
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// This function will not panic unless task execution is disabled on the
    /// executor. This can only happen if the runtime was built using
    /// [`Builder`] without picking either [`basic_scheduler`] or
    /// [`threaded_scheduler`].
    ///
    /// [`Builder`]: struct@Builder
    /// [`threaded_scheduler`]: fn@Builder::threaded_scheduler
    /// [`basic_scheduler`]: fn@Builder::basic_scheduler
    #[cfg(feature = "rt-core")]
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match &self.kind {
            Kind::Basic(exec) => exec.spawn(future),
        }
    }

    /// Run a future to completion on the Tokio runtime. This is the runtime's
    /// entry point.
    ///
    /// This runs the given future on the runtime, blocking until it is
    /// complete, and yielding its resolved result. Any tasks or timers which
    /// the future spawns internally will be executed on the runtime.
    ///
    /// `&mut` is required as calling `block_on` **may** result in advancing the
    /// state of the runtime. The details depend on how the runtime is
    /// configured. [`runtime::Handle::block_on`][handle] provides a version
    /// that takes `&self`.
    ///
    /// This method may not be called from an asynchronous context.
    ///
    /// # Panics
    ///
    /// This function panics if the provided future panics, or if called within an
    /// asynchronous execution context.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::runtime::Runtime;
    ///
    /// // Create the runtime
    /// let mut rt = Runtime::new().unwrap();
    ///
    /// // Execute the future, blocking the current thread until completion
    /// rt.block_on(async {
    ///     println!("hello");
    /// });
    /// ```
    ///
    /// [handle]: fn@Handle::block_on
    pub fn block_on<F: Future>(&mut self, future: F) -> F::Output {
        let kind = &mut self.kind;

        self.handle.enter(|| match kind {
            Kind::Basic(exec) => exec.block_on(future),
        })
    }

    /// Enter the runtime context. This allows you to construct types that must
    /// have an executor available on creation such as [`Delay`] or [`TcpStream`].
    /// It will also allow you to call methods such as [`tokio::spawn`].
    ///
    /// This function is also available as [`Handle::enter`].
    ///
    /// [`Delay`]: struct@crate::time::Delay
    /// [`TcpStream`]: struct@crate::net::TcpStream
    /// [`Handle::enter`]: fn@crate::runtime::Handle::enter
    /// [`tokio::spawn`]: fn@crate::spawn
    ///
    /// # Example
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// fn function_that_spawns(msg: String) {
    ///     // Had we not used `rt.enter` below, this would panic.
    ///     tokio::spawn(async move {
    ///         println!("{}", msg);
    ///     });
    /// }
    ///
    /// fn main() {
    ///     let rt = Runtime::new().unwrap();
    ///
    ///     let s = "Hello World!".to_string();
    ///
    ///     // By entering the context, we tie `tokio::spawn` to this executor.
    ///     rt.enter(|| function_that_spawns(s));
    /// }
    /// ```
    pub fn enter<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.handle.enter(f)
    }

    /// Return a handle to the runtime's spawner.
    ///
    /// The returned handle can be used to spawn tasks that run on this runtime, and can
    /// be cloned to allow moving the `Handle` to other threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// let rt = Runtime::new()
    ///     .unwrap();
    ///
    /// let handle = rt.handle();
    ///
    /// handle.spawn(async { println!("hello"); });
    /// ```
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Shutdown the runtime, waiting for at most `duration` for all spawned
    /// task to shutdown.
    ///
    /// Usually, dropping a `Runtime` handle is sufficient as tasks are able to
    /// shutdown in a timely fashion. However, dropping a `Runtime` will wait
    /// indefinitely for all tasks to terminate, and there are cases where a long
    /// blocking task has been spawned, which can block dropping `Runtime`.
    ///
    /// In this case, calling `shutdown_timeout` with an explicit wait timeout
    /// can work. The `shutdown_timeout` will signal all tasks to shutdown and
    /// will wait for at most `duration` for all spawned tasks to terminate. If
    /// `timeout` elapses before all tasks are dropped, the function returns and
    /// outstanding tasks are potentially leaked.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    /// use tokio::task;
    ///
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// fn main() {
    ///    let mut runtime = Runtime::new().unwrap();
    ///
    ///    runtime.block_on(async move {
    ///        task::spawn_blocking(move || {
    ///            thread::sleep(Duration::from_secs(10_000));
    ///        });
    ///    });
    ///
    ///    runtime.shutdown_timeout(Duration::from_millis(100));
    /// }
    /// ```
    pub fn shutdown_timeout(self, duration: Duration) {
        let Runtime {
            mut blocking_pool, ..
        } = self;
        blocking_pool.shutdown(Some(duration));
    }
}
