use tokiobeam_channel::*;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures::future;
use futures::prelude::*;
use tokio::runtime::Runtime;

struct Stream1 {
    counter: i32,
    sender: std::mem::ManuallyDrop<UnboundedSender<i32>>,
}

impl Stream for Stream1 {
    type Item = Result<(), ()>;
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        let pinned = Pin::get_mut(self);
        if pinned.counter < 250_000 {
            pinned.sender.try_send(pinned.counter).unwrap();
            pinned.counter += 1;
            for g in 0..100_i32 {
                assert!(g >= 0);
            }
            Poll::Ready(Some(Ok(())))
        } else {
            unsafe { std::mem::ManuallyDrop::drop(&mut pinned.sender) }
            println!("finished");
            Poll::Ready(Some(Err(())))
        }
    }
}

struct Stream2 {
    receiver: UnboundedReceiver<i32>,
}

impl Stream for Stream2 {
    type Item = i32;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<i32>> {
        let pinned = Pin::get_mut(self);
        match Pin::new(&mut pinned.receiver).poll_next(cx) {
            Poll::Ready(None) => {
                println!("none callled");
                Poll::Ready(None)
            }
            Poll::Ready(Some(i)) => Poll::Ready(Some(i)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[test]
fn test_unbounded_drop() {
    println!("start test_unbounded_drop");
    let mut rt = Runtime::new().unwrap();
    let (tx, rx) = unbounded();
    let tx = std::mem::ManuallyDrop::new(tx);
    let mut s1 = Stream1 {
        counter: 0,
        sender: tx,
    };
    let mut s2 = Stream2 { receiver: rx };
    let join1 = rt.spawn(async move {
        while let Some(v) = s1.next().await {
            if v.is_err() {
                println!("bye");
                break;
            }
        }
    });
    let join2 = rt.spawn(async move {
        let mut counter = 0;
        while let Some(i) = s2.next().await {
            assert_eq!(counter, i);
            counter += 1;
        }
    });
    rt.block_on(async {
        join1.await.unwrap();
        join2.await.unwrap();
    });
    println!("finally finished");
}

#[test]
fn test_unbounded() {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    let mut txx = tx.clone();
    let a = future::lazy(move |_| {
        for i in 0..1000_i32 {
            tx.try_send(i).unwrap();
        }
    });
    rt.spawn(a);
    rt.block_on(async move {
        let mut rxx = rx.clone();
        let items: Vec<i32> = rx.take(1000).collect().await;
        assert_eq!(items.len(), 1000);
        let mut counter = 0_i32;
        for i in items {
            assert_eq!(i, counter);
            counter += 1;
        }
        txx.try_send(3).unwrap();
        let mut item = rxx.next().await;
        assert_eq!(item.take().unwrap(), 3);
    });
}

#[test]
fn test_forward() {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, rx) = unbounded();
    let (txx, rxx) = unbounded();
    let a = future::lazy(move |_| {
        for i in 0..1000_i32 {
            tx.try_send(i).unwrap();
        }
    });
    rt.spawn(a);
    let task = rx.map(|i| Ok(i)).forward(txx).map(|_| ());
    rt.spawn(task);
    rt.block_on(async move {
        let result: Vec<i32> = rxx.collect().await;
        assert_eq!(result.len(), 1000);
    });
}

#[test]
fn test_bounded() {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, mut rx) = bounded(1);
    let mut txx = tx.clone();
    rt.spawn(async move {
        for i in 0..10_i32 {
            tx.send(i).await.unwrap();
        }
    });
    rt.block_on(async move {
        let item = rx.recv().await;
        assert_eq!(item, Some(0));
        match txx.try_send(3) {
            Err(e) => {
                // 1 injected first, block this try_send
                assert_eq!(e.into_inner(), 3);
                let items: Vec<i32> = rx.take(9).collect().await;
                assert_eq!(items, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
            }
            Ok(()) => {
                // inject successfully
                let items: Vec<i32> = rx.take(10).collect().await;
                assert_eq!(items, vec![3, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
            }
        };
    });
}

#[cfg(feature = "zerocap")]
#[test]
fn test_bounded_0() {
    let mut rt = Runtime::new().unwrap();
    let (mut tx, mut rx) = bounded(0);
    let mut txx = tx.clone();
    rt.spawn(async move {
        for i in 0..10_i32 {
            tx.send(i).await.unwrap();
        }
    });
    rt.block_on(async move {
        let item = rx.recv().await;
        assert_eq!(item, Some(0));
        match txx.try_send(3) {
            Err(e) => {
                // 1 injected first, block this try_send
                assert_eq!(e.into_inner(), 3);
                let items: Vec<i32> = rx.take(9).collect().await;
                assert_eq!(items, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
            }
            Ok(()) => {
                assert_eq!("should not be here", "");
            }
        };
    });
}

#[test]
fn test_race() {
    let mut rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        txx.try_send(2_i32).unwrap();
        rx.next().await;
        rxx.next().await;
        drop(rx);
        drop(rxx);
        drop(tx);
        drop(txx);
    });
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        txx.try_send(2_i32).unwrap();
        drop(tx);
        drop(txx);
        rx.next().await;
        rxx.next().await;
        drop(rx);
        drop(rxx);
    });
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        txx.try_send(2_i32).unwrap();
        drop(tx);
        drop(txx);
        rx.next().await;
        drop(rx);
        rxx.next().await;
        drop(rxx);
    });
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        drop(tx);
        txx.try_send(2_i32).unwrap();
        drop(txx);
        rx.next().await;
        drop(rx);
        rxx.next().await;
        drop(rxx);
    });
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        drop(tx);
        txx.try_send(2_i32).unwrap();
        drop(txx);
        rx.next().await;
        rxx.next().await;
        drop(rx);
        drop(rxx);
    });
    rt.block_on(async move {
        let (mut tx, mut rx) = unbounded();
        let mut txx = tx.clone();
        let mut rxx = rx.clone();
        tx.try_send(1_i32).unwrap();
        rx.next().await;
        drop(tx);
        txx.try_send(2_i32).unwrap();
        drop(txx);
        rxx.next().await;
        drop(rx);
        drop(rxx);
    });
}
