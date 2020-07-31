use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

#[derive(Clone, Copy)]
pub(crate) enum EnterContext {
    Entered {
        #[allow(dead_code)]
        allow_blocking: bool,
    },
    NotEntered,
}

impl EnterContext {
    pub(crate) fn is_entered(self) -> bool {
        if let EnterContext::Entered { .. } = self {
            true
        } else {
            false
        }
    }
}

thread_local!(static ENTERED: Cell<EnterContext> = Cell::new(EnterContext::NotEntered));

/// Represents an executor context.
pub(crate) struct Enter {
    _p: PhantomData<RefCell<()>>,
}

/// Marks the current thread as being within the dynamic extent of an
/// executor.
pub(crate) fn enter(allow_blocking: bool) -> Enter {
    if let Some(enter) = try_enter(allow_blocking) {
        return enter;
    }

    panic!(
        "Cannot start a runtime from within a runtime. This happens \
         because a function (like `block_on`) attempted to block the \
         current thread while the thread is being used to drive \
         asynchronous tasks."
    );
}

/// Tries to enter a runtime context, returns `None` if already in a runtime
/// context.
pub(crate) fn try_enter(allow_blocking: bool) -> Option<Enter> {
    ENTERED.with(|c| {
        if c.get().is_entered() {
            None
        } else {
            c.set(EnterContext::Entered { allow_blocking });
            Some(Enter { _p: PhantomData })
        }
    })
}
