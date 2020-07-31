use crate::runtime::handle::Handle;
use crate::runtime::{io, time, Runtime, Spawner};
use crate::runtime::{BasicScheduler, Kind};

pub struct Builder {
    /// Whether or not to enable the I/O driver
    enable_io: bool,

    /// Whether or not to enable the time driver
    enable_time: bool,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            // I/O defaults to "off"
            enable_io: false,

            // Time defaults to "off"
            enable_time: false,
        }
    }

    pub fn build(&mut self) -> io::Result<Runtime> {
        let clock = time::create_clock();

        // Create I/O driver
        let (io_driver, _) = io::create_driver(self.enable_io)?;

        let (driver, _) = time::create_driver(self.enable_time, io_driver, clock.clone());

        // And now put a single-threaded scheduler on top of the timer. When
        // there are no futures ready to do something, it'll let the timer or
        // the reactor to generate some new stimuli for the futures to continue
        // in their life.
        let scheduler = BasicScheduler::new(driver);
        let spawner = Spawner::Basic(scheduler.spawner().clone());

        // Blocking pool

        Ok(Runtime {
            kind: Kind::Basic(scheduler),
            handle: Handle { spawner, clock },
        })
    }
}
