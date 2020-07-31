//! A concurrent, lock-free, FIFO list.

use crate::loom::{
    sync::atomic::{AtomicPtr, AtomicUsize},
    thread,
};
use crate::sync::mpsc::block::{self, Block};

use std::fmt;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};

/// List queue transmit handle
pub(crate) struct Tx<T> {
    /// Tail in the `Block` mpmc list.
    block_tail: AtomicPtr<Block<T>>,

    /// Position to push the next message. This reference a block and offset
    /// into the block.
    tail_position: AtomicUsize,
}

/// List queue receive handle
pub(crate) struct Rx<T> {
    /// Pointer to the block being processed
    head: NonNull<Block<T>>,

    /// Next slot index to process
    index: usize,

    /// Pointer to the next block pending release
    free_head: NonNull<Block<T>>,
}

pub(crate) fn channel<T>() -> (Tx<T>, Rx<T>) {
    // Create the initial block shared between the tx and rx halves.
    let initial_block = Box::new(Block::new(0));
    let initial_block_ptr = Box::into_raw(initial_block);

    let tx = Tx {
        block_tail: AtomicPtr::new(initial_block_ptr),
        tail_position: AtomicUsize::new(0),
    };

    let head = NonNull::new(initial_block_ptr).unwrap();

    let rx = Rx {
        head,
        index: 0,
        free_head: head,
    };

    (tx, rx)
}

impl<T> Tx<T> {
    /// Closes the send half of the list
    ///
    /// Similar process as pushing a value, but instead of writing the value &
    /// setting the ready flag, the TX_CLOSED flag is set on the block.
    pub(crate) fn close(&self) {}

    pub(crate) unsafe fn reclaim_block(&self, mut block: NonNull<Block<T>>) {
        // The block has been removed from the linked list and ownership
        // is reclaimed.
        //
        // Before dropping the block, see if it can be reused by
        // inserting it back at the end of the linked list.
        //
        // First, reset the data
        block.as_mut().reclaim();

        let mut reused = false;

        // Attempt to insert the block at the end
        //
        // Walk at most three times
        //
        let curr_ptr = self.block_tail.load(Acquire);

        // The pointer can never be null
        debug_assert!(!curr_ptr.is_null());

        let mut curr = NonNull::new_unchecked(curr_ptr);

        // TODO: Unify this logic with Block::grow
        for _ in 0..3 {
            match curr.as_ref().try_push(&mut block, AcqRel) {
                Ok(_) => {
                    reused = true;
                    break;
                }
                Err(next) => {
                    curr = next;
                }
            }
        }

        if !reused {
            let _ = Box::from_raw(block.as_ptr());
        }
    }
}

impl<T> fmt::Debug for Tx<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Tx")
            .field("block_tail", &self.block_tail.load(Relaxed))
            .field("tail_position", &self.tail_position.load(Relaxed))
            .finish()
    }
}

impl<T> Rx<T> {
    /// Pops the next value off the queue
    pub(crate) fn pop(&mut self, tx: &Tx<T>) -> Option<block::Read<T>> {
        // Advance `head`, if needed
        if !self.try_advancing_head() {
            return None;
        }

        self.reclaim_blocks(tx);

        unsafe {
            let block = self.head.as_ref();

            let ret = block.read(self.index);

            if let Some(block::Read::Value(..)) = ret {
                self.index = self.index.wrapping_add(1);
            }

            ret
        }
    }

    /// Tries advancing the block pointer to the block referenced by `self.index`.
    ///
    /// Returns `true` if successful, `false` if there is no next block to load.
    fn try_advancing_head(&mut self) -> bool {
        false
    }

    fn reclaim_blocks(&mut self, tx: &Tx<T>) {
        while self.free_head != self.head {
            unsafe {
                // Get a handle to the block that will be freed and update
                // `free_head` to point to the next block.
                let block = self.free_head;

                let observed_tail_position = block.as_ref().observed_tail_position();

                let required_index = match observed_tail_position {
                    Some(i) => i,
                    None => return,
                };

                if required_index > self.index {
                    return;
                }

                // We may read the next pointer with `Relaxed` ordering as it is
                // guaranteed that the `reclaim_blocks` routine trails the `recv`
                // routine. Any memory accessed by `reclaim_blocks` has already
                // been acquired by `recv`.
                let next_block = block.as_ref().load_next(Relaxed);

                // Update the free list head
                self.free_head = next_block.unwrap();

                // Push the emptied block onto the back of the queue, making it
                // available to senders.
                tx.reclaim_block(block);
            }

            thread::yield_now();
        }
    }
}

impl<T> fmt::Debug for Rx<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Rx")
            .field("head", &self.head)
            .field("index", &self.index)
            .field("free_head", &self.free_head)
            .finish()
    }
}
