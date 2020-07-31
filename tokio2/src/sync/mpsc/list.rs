//! A concurrent, lock-free, FIFO list.

use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize};
use crate::sync::mpsc::block::Block;

use std::ptr::NonNull;

/// List queue transmit handle
#[allow(dead_code)]
pub(crate) struct Tx<T> {
    /// Tail in the `Block` mpmc list.
    block_tail: AtomicPtr<Block<T>>,

    /// Position to push the next message. This reference a block and offset
    /// into the block.
    tail_position: AtomicUsize,
}

/// List queue receive handle
#[allow(dead_code)]
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
