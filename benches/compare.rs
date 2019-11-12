#![feature(test)]

extern crate test;
use futures::prelude::*;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use test::Bencher;
use tokio::runtime::Runtime;
use tokiobeam_channel::*;

struct Dual<T> {
    r1: T,
    r2: T,
}

impl<T: std::marker::Unpin + Stream<Item = i32>> Stream for Dual<T> {
    type Item = i32;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut end = false;
        let pinned = Pin::get_mut(self);
        match Pin::new(&mut pinned.r1).poll_next(cx) {
            x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {
                end = true;
            }
            Poll::Pending => (),
        };

        match Pin::new(&mut pinned.r2).poll_next(cx) {
            x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {
                if end {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[bench]
fn test_unbounded_dual_poll(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    let (mut txx, rxx) = unbounded();
    rt.spawn(async move {
        for i in 0..100000_i32 {
            tx.try_send(i).unwrap();
        }
    });

    rt.spawn(async move {
        for i in 100000..200000_i32 {
            txx.try_send(i).unwrap();
        }
    });

    let dual = Dual { r1: rxx, r2: rx };
    let now = Instant::now();
    rt.block_on(async move {
        let v: Vec<i32> = dual.take(200000).collect().await;
        assert_eq!(v.len(), 200000);
    });
    let elapsed = now.elapsed().as_nanos();
    println!(
        "dual poll unbounded: {}ns/iter, total: {}ns",
        elapsed / 200000,
        elapsed
    );
}

#[bench]
fn test_bounded_dual_poll(_b: &mut Bencher) {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = bounded(1000);
    let (mut txx, rxx) = bounded(1000);
    rt.spawn(async move {
        for i in 0..100000_i32 {
            tx.send(i).await.unwrap();
        }
    });

    rt.spawn(async move {
        for i in 100000..200000_i32 {
            txx.send(i).await.unwrap();
        }
    });

    let dual = Dual { r1: rxx, r2: rx };
    let now = Instant::now();
    rt.block_on(async move {
        let v: Vec<i32> = dual.take(200000).collect().await;
        assert_eq!(v.len(), 200000);
    });
    let elapsed = now.elapsed().as_nanos();
    println!(
        "dual poll bounded: {}ns/iter, total: {}ns",
        elapsed / 200000,
        elapsed
    );
}

#[bench]
fn test_unbounded_dual_poll_old(_b: &mut Bencher) {
    use tokio::sync::mpsc::unbounded_channel;
    let mut rt = Runtime::new().unwrap();
    let (tx, rx) = unbounded_channel();
    let (txx, rxx) = unbounded_channel();
    rt.spawn(async move {
        for i in 0..100000_i32 {
            tx.send(i).unwrap();
        }
    });

    rt.spawn(async move {
        for i in 100000..200000_i32 {
            txx.send(i).unwrap();
        }
    });

    let dual = Dual { r1: rxx, r2: rx };
    let now = Instant::now();
    rt.block_on(async move {
        let v: Vec<i32> = dual.take(200000).collect().await;
        assert_eq!(v.len(), 200000);
    });
    let elapsed = now.elapsed().as_nanos();
    println!(
        "dual poll unbounded old: {}ns/iter, total: {}ns",
        elapsed / 200000,
        elapsed
    );
}

#[bench]
fn test_bounded_dual_poll_old(_b: &mut Bencher) {
    use tokio::sync::mpsc::channel;
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = channel(1000);
    let (mut txx, rxx) = channel(1000);
    rt.spawn(async move {
        for i in 0..100000_i32 {
            tx.send(i).await.unwrap();
        }
    });

    rt.spawn(async move {
        for i in 100000..200000_i32 {
            txx.send(i).await.unwrap();
        }
    });

    let dual = Dual { r1: rxx, r2: rx };
    let now = Instant::now();
    rt.block_on(async move {
        let v: Vec<i32> = dual.take(200000).collect().await;
        assert_eq!(v.len(), 200000);
    });
    let elapsed = now.elapsed().as_nanos();
    println!(
        "dual poll bounded old: {}ns/iter, total: {}ns",
        elapsed / 200000,
        elapsed
    );
}
