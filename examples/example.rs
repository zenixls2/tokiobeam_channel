use std::alloc::System;
#[global_allocator]
static A: System = System;

use futures::prelude::*;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::delay_for;
use tokiobeam_channel::unbounded;

// this example does nothing but help test the cpu usage
fn main() {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    rt.spawn(async move {
        tx.send(0).await.unwrap();
        let d = delay_for(Duration::from_secs(10));
        d.await;
    });
    rt.block_on(async move {
        let items: Vec<i32> = rx.take(2).collect().await;
        println!("{:?}", items);
    });
}
