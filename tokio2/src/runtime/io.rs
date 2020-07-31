//! Abstracts out the APIs necessary to `Runtime` for integrating the I/O
//! driver. When the `time` feature flag is **not** enabled. These APIs are
//! shells. This isolates the complexity of dealing with conditional
//! compilation.

/// Re-exported for convenience.
pub(crate) use std::io::Result;

pub(crate) use variant::*;

mod variant {
    use crate::park::ParkThread;

    use std::io;

    /// I/O is not enabled, use a condition variable based parker
    pub(crate) type Driver = ParkThread;

    /// There is no handle
    pub(crate) type Handle = ();

    pub(crate) fn create_driver(_enable: bool) -> io::Result<(Driver, Handle)> {
        let driver = ParkThread::new();

        Ok((driver, ()))
    }
}
