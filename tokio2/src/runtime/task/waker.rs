use crate::runtime::task::Header;

use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops;
use std::task::Waker;

pub(super) struct WakerRef<'a, S: 'static> {
    waker: ManuallyDrop<Waker>,
    _p: PhantomData<(&'a Header, S)>,
}

impl<S> ops::Deref for WakerRef<'_, S> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}
