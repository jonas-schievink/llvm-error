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

mod io;

mod spawner;
use self::spawner::Spawner;

mod time;

use std::future::Future;

pub struct Runtime {
    kind: Kind,
    handle: Handle,
}

enum Kind {
    Basic(BasicScheduler<time::Driver>),
}

impl Runtime {
    pub fn block_on<F: Future>(&mut self, future: F) -> F::Output {
        let kind = &mut self.kind;

        self.handle.enter(|| match kind {
            Kind::Basic(exec) => exec.block_on(future),
        })
    }
}
