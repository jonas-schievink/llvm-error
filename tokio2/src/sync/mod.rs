use std::task::Waker;

pub mod mpsc;

struct AtomicWaker {}

impl AtomicWaker {
    /// Create an `AtomicWaker`
    fn new() -> AtomicWaker {
        AtomicWaker {}
    }

    fn register_by_ref(&self, _waker: &Waker) {}
}
