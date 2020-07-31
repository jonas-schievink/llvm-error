//! An intrusive double linked list of data
//!
//! The data structure supports tracking pinned nodes. Most of the data
//! structure's APIs are `unsafe` as they require the caller to ensure the
//! specified node is actually contained by the list.

use core::ptr::NonNull;

/// Previous / next pointers
pub(crate) struct Pointers<T> {
    /// The previous node in the list. null if there is no previous node.
    _prev: Option<NonNull<T>>,

    /// The next node in the list. null if there is no previous node.
    _next: Option<NonNull<T>>,
}

unsafe impl<T: Send> Send for Pointers<T> {}
unsafe impl<T: Sync> Sync for Pointers<T> {}

// ===== impl Pointers =====

impl<T> Pointers<T> {
    /// Create a new set of empty pointers
    pub(crate) fn new() -> Pointers<T> {
        Pointers {
            _prev: None,
            _next: None,
        }
    }
}
