//! Abstracts out the APIs necessary to `Runtime` for integrating the time
//! driver. When the `time` feature flag is **not** enabled. These APIs are
//! shells. This isolates the complexity of dealing with conditional
//! compilation.

pub(crate) use variant::*;

mod variant {
    use crate::runtime::io;

    pub(crate) type Clock = ();
    pub(crate) type Driver = io::Driver;
    pub(crate) type Handle = ();

    pub(crate) fn create_clock() -> Clock {
        ()
    }

    /// Create a new timer driver / handle pair
    pub(crate) fn create_driver(
        _enable: bool,
        io_driver: io::Driver,
        _clock: Clock,
    ) -> (Driver, Handle) {
        (io_driver, ())
    }
}
