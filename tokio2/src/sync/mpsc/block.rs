use crate::loom::{
    cell::UnsafeCell,
    sync::atomic::{AtomicPtr, AtomicUsize},
};

use std::ptr::{self, NonNull};
use std::sync::atomic::Ordering;

/// A block in a linked list.
///
/// Each block in the list can hold up to `BLOCK_CAP` messages.
#[allow(dead_code)]
pub(crate) struct Block<T> {
    /// The start index of this block.
    ///
    /// Slots in this block have indices in `start_index .. start_index + BLOCK_CAP`.
    start_index: usize,

    /// The next block in the linked list.
    next: AtomicPtr<Block<T>>,

    /// Bitfield tracking slots that are ready to have their values consumed.
    ready_slots: AtomicUsize,

    /// The observed `tail_position` value *after* the block has been passed by
    /// `block_tail`.
    observed_tail_position: UnsafeCell<usize>,
}

#[allow(dead_code)]
pub(crate) enum Read<T> {
    Value(T),
    Closed,
}

impl<T> Block<T> {
    pub(crate) fn new(start_index: usize) -> Block<T> {
        Block {
            // The absolute index in the channel of the first slot in the block.
            start_index,

            // Pointer to the next block in the linked list.
            next: AtomicPtr::new(ptr::null_mut()),

            ready_slots: AtomicUsize::new(0),

            observed_tail_position: UnsafeCell::new(0),
        }
    }

    /// Reads the value at the given offset.
    ///
    /// Returns `None` if the slot is empty.
    ///
    /// # Safety
    ///
    /// To maintain safety, the caller must ensure:
    ///
    /// * No concurrent access to the slot.
    pub(crate) unsafe fn read(&self, _slot_index: usize) -> Option<Read<T>> {
        None
    }

    /// Resets the block to a blank state. This enables reusing blocks in the
    /// channel.
    ///
    /// # Safety
    ///
    /// To maintain safety, the caller must ensure:
    ///
    /// * All slots are empty.
    /// * The caller holds a unique pointer to the block.
    pub(crate) unsafe fn reclaim(&mut self) {}

    /// Returns the `observed_tail_position` value, if set
    pub(crate) fn observed_tail_position(&self) -> Option<usize> {
        None
    }

    /// Loads the next block
    pub(crate) fn load_next(&self, _ordering: Ordering) -> Option<NonNull<Block<T>>> {
        None
    }

    /// Pushes `block` as the next block in the link.
    ///
    /// Returns Ok if successful, otherwise, a pointer to the next block in
    /// the list is returned.
    ///
    /// This requires that the next pointer is null.
    ///
    /// # Ordering
    ///
    /// This performs a compare-and-swap on `next` using AcqRel ordering.
    ///
    /// # Safety
    ///
    /// To maintain safety, the caller must ensure:
    ///
    /// * `block` is not freed until it has been removed from the list.
    pub(crate) unsafe fn try_push(
        &self,
        _block: &mut NonNull<Block<T>>,
        _ordering: Ordering,
    ) -> Result<(), NonNull<Block<T>>> {
        Ok(())
    }
}
