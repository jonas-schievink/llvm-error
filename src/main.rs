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
    let (_tx, rx) = async_channel::unbounded::<Msg>();
    let entity = Mutex::new(());
    llvm_error::run(async move {
        {
            let output = {
                let mut fut = rx.recv();
                llvm_error::poll_fn(|cx| {
                    loop {
                        let fut = unsafe { Pin::new_unchecked(&mut fut) };
                        let out = match fut.poll(cx) {
                            Ready(out) => out,
                            Pending => {
                                break;
                            }
                        };
                        #[allow(unused_variables)]
                        if out.is_err() {
                            break
                        };
                        return Ready(Out::_0(out.ok()));
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
