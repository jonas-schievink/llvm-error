pub(crate) mod context;

mod basic_scheduler;
use basic_scheduler::BasicScheduler;

pub(crate) mod task;

mod builder;
pub use self::builder::Builder;

pub(crate) mod enter;
use self::enter::enter;

mod handle;
pub use self::handle::Handle;

mod spawner;
use self::spawner::Spawner;

use std::future::Future;

pub struct Runtime {
    scheduler: BasicScheduler,
    handle: Handle,
}

impl Runtime {
    pub fn block_on<F: Future>(&mut self, future: F) -> F::Output {
        let sched = &mut self.scheduler;

        self.handle.enter(|| sched.block_on(future))
    }
}
