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
fn test_channel_multiple_receiver(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    let rxx = rx.clone();
    rt.spawn(async move {
        for i in 0..80000_i32 {
            tx.try_send(i).unwrap();
        }
    });

    let now = Instant::now();
    rt.spawn(async move {
        rx.take(40000)
            .for_each(|_| {
                after(Duration::from_nanos(200)).recv().unwrap();
                future::ready(())
            })
            .await;
    });
    rt.block_on(async move {
        rxx.take(40000)
            .for_each(|_| {
                after(Duration::from_nanos(200)).recv().unwrap();
                future::ready(())
            })
            .await;
    });
    let elapsed = now.elapsed().as_nanos();
    println!("multiple: {}ns/iter, total: {}ns", elapsed / 80000, elapsed);
}
