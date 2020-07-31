//! Core task module.
//!
//! # Safety
//!
//! The functions in this module are private to the `task` module. All of them
//! should be considered `unsafe` to use, but are not marked as such since it
//! would be too noisy.
//!
//! Make sure to consult the relevant safety section of each function before
//! use.

use crate::loom::cell::UnsafeCell;
use crate::runtime::task::raw::{self, Vtable};
use crate::runtime::task::state::State;
use crate::runtime::task::Schedule;
use crate::util::linked_list;

use std::future::Future;
use std::ptr::NonNull;
use std::task::Waker;

/// The task cell. Contains the components of the task.
///
/// It is critical for `Header` to be the first field as the task structure will
/// be referenced by both *mut Cell and *mut Header.
#[repr(C)]
pub(super) struct Cell<T: Future, S> {
    /// Hot task state data
    pub(super) header: Header,

    /// Either the future or output, depending on the execution stage.
    pub(super) core: Core<T, S>,

    /// Cold data
    pub(super) trailer: Trailer,
}

/// The core of the task.
///
/// Holds the future or output, depending on the stage of execution.
pub(super) struct Core<T: Future, S> {
    /// Scheduler used to drive this future
    pub(super) scheduler: UnsafeCell<Option<S>>,

    /// Either the future or the output
    pub(super) stage: UnsafeCell<Stage<T>>,
}

/// Crate public as this is also needed by the pool.
#[repr(C)]
pub(crate) struct Header {
    /// Task state
    pub(super) state: State,

    pub(crate) owned: UnsafeCell<linked_list::Pointers<Header>>,

    /// Pointer to next task, used with the injection queue
    pub(crate) queue_next: UnsafeCell<Option<NonNull<Header>>>,

    /// Pointer to the next task in the transfer stack
    pub(super) stack_next: UnsafeCell<Option<NonNull<Header>>>,

    /// Table of function pointers for executing actions on the task.
    pub(super) vtable: &'static Vtable,
}

unsafe impl Send for Header {}
unsafe impl Sync for Header {}

/// Cold data is stored after the future.
pub(super) struct Trailer {
    /// Consumer task waiting on completion of this task.
    pub(super) waker: UnsafeCell<Option<Waker>>,
}

/// Either the future or the output.
pub(super) enum Stage<T: Future> {
    Running(T),
    Consumed,
}

impl<T: Future, S: Schedule> Cell<T, S> {
    /// Allocates a new task cell, containing the header, trailer, and core
    /// structures.
    pub(super) fn new(future: T, state: State) -> Box<Cell<T, S>> {
        Box::new(Cell {
            header: Header {
                state,
                owned: UnsafeCell::new(linked_list::Pointers::new()),
                queue_next: UnsafeCell::new(None),
                stack_next: UnsafeCell::new(None),
                vtable: raw::vtable::<T, S>(),
            },
            core: Core {
                scheduler: UnsafeCell::new(None),
                stage: UnsafeCell::new(Stage::Running(future)),
            },
            trailer: Trailer {
                waker: UnsafeCell::new(None),
            },
        })
    }
}

impl<T: Future, S: Schedule> Core<T, S> {
    /// Drop the future
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `stage` field.
    pub(super) fn drop_future_or_output(&self) {
        self.stage.with_mut(|ptr| {
            // Safety: The caller ensures mutal exclusion to the field.
            unsafe { *ptr = Stage::Consumed };
        });
    }

    /// Take the task output
    ///
    /// # Safety
    ///
    /// The caller must ensure it is safe to mutate the `stage` field.
    pub(super) fn take_output(&self) -> super::Result<T::Output> {
        use std::mem;

        self.stage.with_mut(|ptr| {
            // Safety:: the caller ensures mutal exclusion to the field.
            match mem::replace(unsafe { &mut *ptr }, Stage::Consumed) {
                _ => panic!("unexpected task state"),
            }
        })
    }
}
