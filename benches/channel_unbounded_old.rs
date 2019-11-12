#![feature(test)]

extern crate test;
use futures::prelude::*;
use std::time::Instant;
use test::Bencher;
use tokio::runtime::Runtime;

#[bench]
fn test_channel_unbounded_old(_b: &mut Bencher) {
    use tokio::sync::mpsc::unbounded_channel;
    let mut rt = Runtime::new().unwrap();
    let (tx, rx) = unbounded_channel();
    rt.spawn(async move {
        for i in 0..200000_i32 {
            tx.send(i).unwrap();
        }
    });
    rt.block_on(async move {
        let now = Instant::now();
        let c: Vec<i32> = rx.collect().await;
        let elapsed = now.elapsed().as_nanos();
        println!(
            "unbounded old: {}ns/iter, total {}ns",
            elapsed / 200000,
            elapsed
        );
        assert_eq!(c.len(), 200000);
    });
}
