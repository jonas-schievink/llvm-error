use ::tokio::macros::support::Future;
use ::tokio::macros::support::Pin;
use ::tokio::macros::support::Poll::{Pending, Ready};
use std::sync::Mutex;

#[allow(dead_code)]
enum Msg {
    A(Vec<()>),
    B,
}

#[allow(unused_must_use)]
fn main() {
    let (_, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    let entity = Mutex::new(());
    tokio::runtime::Builder::new()
        .build()
        .unwrap()
        .block_on(async move {
            {
                mod util {
                    pub(super) enum Out<_0> {
                        _0(_0),
                        Disabled,
                    }
                }
                let output = {
                    let mut futures = (rx.recv(),);
                    ::tokio::future::poll_fn(|cx| {
                        let mut is_pending = false;
                        for _ in 0..1 {
                            match 0 {
                                0 => {
                                    let (fut, ..) = &mut futures;
                                    let fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match fut.poll(cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    #[allow(unused_variables)]
                                    match &out {
                                        Some(_msg) => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_0(out));
                                }
                                _ => unreachable!(
                                    "reaching this means there probably is an off by one bug"
                                ),
                            }
                        }
                        if is_pending {
                            Pending
                        } else {
                            Ready(util::Out::Disabled)
                        }
                    })
                    .await
                };
                match output {
                    util::Out::_0(Some(_msg)) => {
                        entity.lock();
                    }
                    util::Out::Disabled => unreachable!(),
                    _ => unreachable!("failed to match bind"),
                }
            }
            entity.lock();
        });
}
