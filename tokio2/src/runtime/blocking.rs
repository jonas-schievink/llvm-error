//! Abstracts out the APIs necessary to `Runtime` for integrating the blocking
//! pool. When the `blocking` feature flag is **not** enabled, these APIs are
//! shells. This isolates the complexity of dealing with conditional
//! compilation.

use crate::runtime::Builder;

#[derive(Debug, Clone)]
pub(crate) struct BlockingPool {}

pub(crate) use BlockingPool as Spawner;

pub(crate) fn create_blocking_pool(_builder: &Builder, _thread_cap: usize) -> BlockingPool {
    BlockingPool {}
}

impl BlockingPool {
    pub(crate) fn spawner(&self) -> &BlockingPool {
        self
    }
}
