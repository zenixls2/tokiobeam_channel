#![feature(test)]

extern crate test;
use crossbeam::channel::after;
use futures::future;
use futures::prelude::*;
use std::time::Duration;
use std::time::Instant;
use test::Bencher;
use tokio::runtime::Runtime;
use tokiobeam_channel::*;

#[bench]
fn test_channel_single_receiver(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    rt.spawn(async move {
        for i in 0..80000_i32 {
            tx.try_send(i).unwrap();
        }
    });
    rt.block_on(async move {
        let now = Instant::now();
        rx.for_each(|_| {
            after(Duration::from_nanos(200)).recv().unwrap();
            future::ready(())
        })
        .await;
        let elapsed = now.elapsed().as_nanos();
        println!("single: {}ns/iter, total {}ns", elapsed / 80000, elapsed);
    });
}
