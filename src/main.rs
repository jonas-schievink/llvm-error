use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::Poll::{Pending, Ready};

#[allow(dead_code)]
enum Msg {
    A(Vec<()>),
    B,
}

#[allow(dead_code)]
enum Out {
    _0(Option<Msg>),
    Disabled,
}

#[allow(unused_must_use)]
fn main() {
    let mut rx = tokio::unbounded_channel::<Msg>();
    let entity = Mutex::new(());
    tokio::run(async move {
        {
            let output = {
                let mut fut = rx.recv();
                ::tokio::poll_fn(|cx| {
                    loop {
                        let fut = unsafe { Pin::new_unchecked(&mut fut) };
                        let out = match fut.poll(cx) {
                            Ready(out) => out,
                            Pending => {
                                break;
                            }
                        };
                        #[allow(unused_variables)]
                        match &out {
                            Some(_msg) => {}
                            _ => break,
                        }
                        return Ready(Out::_0(out));
                    }
                    Ready(Out::_0(None))
                })
                .await
            };
            match output {
                Out::_0(Some(_msg)) => {
                    entity.lock();
                }
                Out::_0(None) => unreachable!(),
                _ => unreachable!(),
            }
        }
        entity.lock();
    });
}
