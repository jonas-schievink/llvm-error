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
            tokio::select! {
                Some(_msg) = rx.recv() => {
                    entity.lock();
                }
            }
            entity.lock();
        });
}
