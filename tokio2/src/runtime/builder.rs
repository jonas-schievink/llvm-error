use crate::runtime::handle::Handle;
use crate::runtime::BasicScheduler;
use crate::runtime::{Runtime, Spawner};
use std::io;

pub struct Builder {}

impl Builder {
    pub fn new() -> Builder {
        Builder {}
    }

    pub fn build(&mut self) -> io::Result<Runtime> {
        let scheduler = BasicScheduler::new();
        let spawner = Spawner::Basic(scheduler.spawner().clone());

        Ok(Runtime {
            scheduler,
            handle: Handle { spawner },
        })
    }
}
