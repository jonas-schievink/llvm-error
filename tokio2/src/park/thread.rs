use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::{Arc, Condvar, Mutex};
use crate::park::{Park, Unpark};

use std::time::Duration;

#[derive(Debug)]
pub(crate) struct ParkThread {
    inner: Arc<Inner>,
}

pub(crate) type ParkError = ();

/// Unblocks a thread that was blocked by `ParkThread`.
#[derive(Clone, Debug)]
pub(crate) struct UnparkThread {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    state: AtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
}

const EMPTY: usize = 0;

thread_local! {
    static CURRENT_PARKER: ParkThread = ParkThread::new();
}

// ==== impl ParkThread ====

impl ParkThread {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: AtomicUsize::new(EMPTY),
                mutex: Mutex::new(()),
                condvar: Condvar::new(),
            }),
        }
    }
}

impl Park for ParkThread {
    type Unpark = UnparkThread;
    type Error = ParkError;

    fn unpark(&self) -> Self::Unpark {
        let inner = self.inner.clone();
        UnparkThread { inner }
    }

    fn park(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn park_timeout(&mut self, _: Duration) -> Result<(), Self::Error> {
        Ok(())
    }
}

// ==== impl Inner ====

impl Inner {
    fn unpark(&self) {}
}

// ===== impl UnparkThread =====

impl Unpark for UnparkThread {
    fn unpark(&self) {
        self.inner.unpark();
    }
}

cfg_block_on! {
    use std::marker::PhantomData;
    use std::rc::Rc;

    /// Blocks the current thread using a condition variable.
    pub(crate) struct CachedParkThread {
        _anchor: PhantomData<Rc<()>>,
    }

    impl CachedParkThread {
        pub(crate) fn get_unpark(&self) -> Result<UnparkThread, ParkError> {
            self.with_current(|park_thread| park_thread.unpark())
        }

        /// Get a reference to the `ParkThread` handle for this thread.
        fn with_current<F, R>(&self, f: F) -> Result<R, ParkError>
        where
            F: FnOnce(&ParkThread) -> R,
        {
            CURRENT_PARKER.try_with(|inner| f(inner))
                .map_err(|_| ())
        }
    }

    impl Park for CachedParkThread {
        type Unpark = UnparkThread;
        type Error = ParkError;

        fn unpark(&self) -> Self::Unpark {
            self.get_unpark().unwrap()
        }

        fn park(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn park_timeout(&mut self, _: Duration) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}
