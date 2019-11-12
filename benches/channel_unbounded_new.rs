#![feature(test)]

extern crate test;
use futures::prelude::*;
use std::time::Instant;
use test::Bencher;
use tokio::runtime::Runtime;
use tokiobeam_channel::*;

#[bench]
fn test_channel_unbounded_new(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    rt.spawn(async move {
        for i in 0..200000_i32 {
            tx.try_send(i).unwrap();
        }
    });
    rt.block_on(async move {
        let now = Instant::now();
        let c: Vec<i32> = rx.collect().await;
        let elapsed = now.elapsed().as_nanos();
        println!(
            "unbounded new: {}ns/iter, total {}ns",
            elapsed / 200000,
            elapsed
        );
        assert_eq!(c.len(), 200000);
    });
}
