#![feature(test)]

extern crate test;
use futures::prelude::*;
use std::time::Instant;
use test::Bencher;
use tokio::runtime::Runtime;
use tokiobeam_channel::*;

#[bench]
fn test_channel_bounded1000_new(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = bounded(1000);
    rt.spawn(async move {
        for i in 0..200000_i32 {
            tx.send(i).await.unwrap();
        }
    });
    rt.block_on(async move {
        let now = Instant::now();
        let c: Vec<i32> = rx.collect().await;
        let elapsed = now.elapsed().as_nanos();
        println!(
            "bounded(1000) new: {}ns/iter, total {}ns",
            elapsed / 200000,
            elapsed
        );
        assert_eq!(c.len(), 200000);
    });
}
