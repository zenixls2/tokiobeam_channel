use crate::atomic_serial_waker::{positive_update, AtomicSerialWaker};
use crate::error::*;
use alloc::sync::Arc;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering::*};
use core::task::{Context, Poll};
use crossbeam::crossbeam_channel::{self, TryRecvError};
use futures::prelude::*;

pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (s, r) = crossbeam_channel::unbounded();
    let tx_count = Arc::new(AtomicUsize::new(1));
    let from_me = Arc::new(AtomicUsize::new(0));
    let waker = Arc::new(AtomicSerialWaker::new(from_me.clone()));
    (
        UnboundedSender {
            tx: Some(s),
            tx_count: tx_count.clone(),
            waker: waker.clone(),
        },
        UnboundedReceiver {
            first: true,
            rx: r,
            tx_count: tx_count,
            waker: waker,
            from_me: from_me,
        },
    )
}

#[derive(Debug)]
pub struct UnboundedSender<T> {
    tx: Option<crossbeam_channel::Sender<T>>,
    tx_count: Arc<AtomicUsize>,
    waker: Arc<AtomicSerialWaker>,
}

#[derive(Debug)]
pub struct UnboundedReceiver<T> {
    // this flag is set to false when first NotReady happen,
    // ensure that the first register() is executed
    // Notice that each cloned instance is an independent stream,
    // this flag would be set to true in the cloned receiver
    first: bool,
    rx: crossbeam_channel::Receiver<T>,
    tx_count: Arc<AtomicUsize>,
    waker: Arc<AtomicSerialWaker>,
    // this flag will be set to true when any sender triggers Waker::wake()
    // the executed receiver could then be decided by this flag whether to register() again
    // and no sender triggers Waker::wake().
    // we don't need to re-register() again in such case.
    from_me: Arc<AtomicUsize>,
}

impl<T: Unpin> UnboundedReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        use futures::future::poll_fn;
        match self.rx.try_recv() {
            Ok(t) => return Some(t),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return None,
        };
        poll_fn(|cx| Stream::poll_next(Pin::new(self), cx)).await
    }
}

impl<T: Unpin> Stream for UnboundedReceiver<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pinned = Pin::get_mut(self);
        match pinned.rx.try_recv() {
            Ok(t) => return Poll::Ready(Some(t)),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return Poll::Ready(None),
        };

        if pinned.first {
            pinned.first = false;
            pinned.waker.register(cx.waker());

            match pinned.rx.try_recv() {
                Ok(t) => Poll::Ready(Some(t)),
                Err(TryRecvError::Empty) => Poll::Pending,
                Err(TryRecvError::Disconnected) => Poll::Ready(None),
            }
        } else if positive_update(&pinned.from_me) {
            pinned.waker.register(cx.waker());
            match pinned.rx.try_recv() {
                Ok(t) => Poll::Ready(Some(t)),
                Err(TryRecvError::Empty) => Poll::Pending,
                Err(TryRecvError::Disconnected) => Poll::Ready(None),
            }
        } else {
            Poll::Pending
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<T: Unpin> futures::stream::FusedStream for UnboundedReceiver<T> {
    fn is_terminated(&self) -> bool {
        self.tx_count.load(Acquire) == 0
    }
}

impl<T> Clone for UnboundedReceiver<T> {
    fn clone(&self) -> Self {
        Self {
            first: true,
            rx: self.rx.clone(),
            tx_count: self.tx_count.clone(),
            waker: self.waker.clone(),
            from_me: self.from_me.clone(),
        }
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        // Using a Relaxed ordering here is sufficient as the caller holds a
        // strong ref to `self`, preventing a concurrent decrement to zero.
        self.tx_count.fetch_add(1, Relaxed);
        Self {
            tx: self.tx.clone(),
            waker: self.waker.clone(),
            tx_count: self.tx_count.clone(),
        }
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        // force trigger disconnect in crossbeam unbounded
        // and wake up receiver when the last instance is going to be destroyed
        // so that stream could end (by sending None)
        self.tx.take();
        if self.tx_count.fetch_sub(1, AcqRel) != 1 {
            return;
        }
        self.waker.wake();
    }
}

impl<T: Unpin> Sink<T> for UnboundedSender<T> {
    type Error = SendError;
    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), SendError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), SendError> {
        self.try_send(item).map_err(|_| SendError)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), SendError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), SendError>> {
        Poll::Ready(Ok(()))
    }
}

impl<T> UnboundedSender<T> {
    pub fn try_send(&mut self, message: T) -> Result<(), TrySendError<T>> {
        self.tx.as_ref().unwrap().try_send(message)?;
        self.waker.wake();
        Ok(())
    }
}
