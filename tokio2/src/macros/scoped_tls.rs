/// Set a reference as a thread-local
macro_rules! scoped_thread_local {
    ($(#[$attrs:meta])* $vis:vis static $name:ident: $ty:ty) => (
        $(#[$attrs])*
        $vis static $name: $crate::macros::scoped_tls::ScopedKey<$ty>
            = $crate::macros::scoped_tls::ScopedKey {
                inner: {
                    thread_local!(static FOO: ::std::cell::Cell<*const ()> = {
                        std::cell::Cell::new(::std::ptr::null())
                    });
                    &FOO
                },
                _marker: ::std::marker::PhantomData,
            };
    )
}
