use crate::atomic_serial_waker::{positive_update, AtomicSerialWaker};
use crate::error::*;
use alloc::sync::Arc;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering::*};
use core::task::{Context, Poll};
use crossbeam::crossbeam_channel::{self, TryRecvError, TrySendError as TSE};
use futures::prelude::*;

pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let (s, r) = crossbeam_channel::bounded(cap);
    let tx_count = Arc::new(AtomicUsize::new(1));
    let rx_count = Arc::new(AtomicUsize::new(1));
    let from_me = Arc::new(AtomicUsize::new(0));
    let waker = Arc::new(AtomicSerialWaker::new(from_me.clone()));
    // dummy flag, not actually used at this moment,
    // TODO: use this flag for further improvement
    let to_me = Arc::new(AtomicUsize::new(0));
    let send_waker = Arc::new(AtomicSerialWaker::new(to_me));
    (
        Sender {
            tx: Some(s),
            tx_count: tx_count.clone(),
            waker: waker.clone(),
            send_waker: send_waker.clone(),
            #[cfg(feature = "zerocap")]
            is_zero: cap == 0,
        },
        Receiver {
            first: true,
            rx: r,
            rx_count: rx_count,
            tx_count: tx_count,
            waker: waker,
            send_waker: send_waker,
            #[cfg(feature = "zerocap")]
            is_zero: cap == 0,
            from_me: from_me,
        },
    )
}

pub struct Sender<T> {
    tx: Option<crossbeam_channel::Sender<T>>,
    tx_count: Arc<AtomicUsize>,
    waker: Arc<AtomicSerialWaker>,
    send_waker: Arc<AtomicSerialWaker>,
    #[cfg(feature = "zerocap")]
    is_zero: bool, // for zero-capacity channel
}

pub struct Receiver<T> {
    // this flag is set to false when first NotReady happen,
    // ensure that the first register() is executed.
    // Notice that each cloned instance is an independent stream.
    // this flag would be set to true in the cloned receiver.
    first: bool,
    rx: crossbeam_channel::Receiver<T>,
    rx_count: Arc<AtomicUsize>,
    tx_count: Arc<AtomicUsize>,
    waker: Arc<AtomicSerialWaker>,
    send_waker: Arc<AtomicSerialWaker>,
    // if this is a zero capacity channel
    #[cfg(feature = "zerocap")]
    is_zero: bool,
    // this flag will be set to true when any sender triggers Waker::wake()
    // the executed receiver could then be decided by this flag whether to register() again
    // if this flag is positive, meaning that we already register() at least once,
    // and no sender triggers Waker::wake().
    // we don't need to re-register() again in such case.
    from_me: Arc<AtomicUsize>,
}

impl<T: Unpin> Receiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        use futures::future::poll_fn;
        match self.rx.try_recv() {
            Ok(t) => {
                self.send_waker.wake();
                return Some(t);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return None,
        };
        #[cfg(feature = "zerocap")]
        {
            if self.is_zero {
                // block the thread
                self.send_waker.wake();
                match self.rx.recv() {
                    Ok(t) => Some(t),
                    Err(_) => None,
                }
            } else {
                poll_fn(|cx| Stream::poll_next(Pin::new(self), cx)).await
            }
        }
        #[cfg(not(feature = "zerocap"))]
        poll_fn(|cx| Stream::poll_next(Pin::new(self), cx)).await
    }
}

// supports new futures
impl<T: Unpin> Stream for Receiver<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pinned = Pin::get_mut(self);
        match pinned.rx.try_recv() {
            Ok(t) => {
                pinned.send_waker.wake();
                return Poll::Ready(Some(t));
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return Poll::Ready(None),
        };
        // detects:
        // 1. first register()
        // 2. register() triggered by our Waker::wake()
        if pinned.first {
            pinned.first = false;
            pinned.waker.register(cx.waker());
            #[cfg(feature = "zerocap")]
            {
                if pinned.is_zero {
                    // block the thread
                    pinned.send_waker.wake();
                    match pinned.rx.recv() {
                        Ok(t) => Poll::Ready(Some(t)),
                        Err(_) => Poll::Ready(None),
                    }
                } else {
                    match pinned.rx.try_recv() {
                        Ok(t) => {
                            pinned.send_waker.wake();
                            Poll::Ready(Some(t))
                        }
                        Err(TryRecvError::Empty) => {
                            pinned.send_waker.wake();
                            Poll::Pending
                        }
                        Err(TryRecvError::Disconnected) => Poll::Ready(None),
                    }
                }
            }
            #[cfg(not(feature = "zerocap"))]
            match pinned.rx.try_recv() {
                Ok(t) => {
                    pinned.send_waker.wake();
                    Poll::Ready(Some(t))
                }
                Err(TryRecvError::Empty) => {
                    pinned.send_waker.wake();
                    Poll::Pending
                }
                Err(TryRecvError::Disconnected) => Poll::Ready(None),
            }
        } else if positive_update(&pinned.from_me) {
            pinned.waker.register(cx.waker());
            #[cfg(feature = "zerocap")]
            {
                if pinned.is_zero {
                    // block the thread
                    pinned.send_waker.wake();
                    match pinned.rx.recv() {
                        Ok(t) => Poll::Ready(Some(t)),
                        Err(_) => Poll::Ready(None),
                    }
                } else {
                    match pinned.rx.try_recv() {
                        Ok(t) => {
                            pinned.send_waker.wake();
                            Poll::Ready(Some(t))
                        }
                        Err(TryRecvError::Empty) => {
                            pinned.send_waker.wake();
                            Poll::Pending
                        }
                        Err(TryRecvError::Disconnected) => Poll::Ready(None),
                    }
                }
            }
            #[cfg(not(feature = "zerocap"))]
            match pinned.rx.try_recv() {
                Ok(t) => {
                    pinned.send_waker.wake();
                    Poll::Ready(Some(t))
                }
                Err(TryRecvError::Empty) => {
                    pinned.send_waker.wake();
                    Poll::Pending
                }
                Err(TryRecvError::Disconnected) => Poll::Ready(None),
            }
        } else {
            // otherwise return notready directly
            Poll::Pending
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.rx.capacity())
    }
}

impl<T: Unpin> futures::stream::FusedStream for Receiver<T> {
    fn is_terminated(&self) -> bool {
        self.tx_count.load(Acquire) == 0
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.rx_count.fetch_add(1, Relaxed);
        Self {
            first: true,
            rx: self.rx.clone(),
            waker: self.waker.clone(),
            rx_count: self.rx_count.clone(),
            tx_count: self.tx_count.clone(),
            send_waker: self.send_waker.clone(),
            #[cfg(feature = "zerocap")]
            is_zero: self.is_zero,
            from_me: self.from_me.clone(),
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if self.rx_count.fetch_sub(1, AcqRel) != 1 {
            return;
        }
        self.send_waker.wake();
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        // Using a Relaxed ordering here is sufficient as the caller holds a
        // strong ref to `self`, preventing a concurrent decrement to zero.
        self.tx_count.fetch_add(1, Relaxed);
        Self {
            tx: self.tx.clone(),
            waker: self.waker.clone(),
            tx_count: self.tx_count.clone(),
            send_waker: self.send_waker.clone(),
            #[cfg(feature = "zerocap")]
            is_zero: self.is_zero,
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // force trigger disconnect in crossbeam bounded
        // and wake up receiver when the last instance is going to be destroyed
        // so that stream could end (by sending None)
        self.tx.take();
        if self.tx_count.fetch_sub(1, AcqRel) != 1 {
            return;
        }
        self.waker.wake();
    }
}

impl<T: Unpin> Sink<T> for Sender<T> {
    type Error = SendError;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), SendError>> {
        let pinned = Pin::get_mut(self);
        if !pinned.tx.as_ref().unwrap().is_full() {
            return Poll::Ready(Ok(()));
        }
        pinned.send_waker.register(cx.waker());
        if pinned.tx.as_ref().unwrap().is_full() {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
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

impl<T: Unpin> Sender<T> {
    pub fn try_send(&mut self, message: T) -> Result<(), TrySendError<T>> {
        self.tx.as_ref().unwrap().try_send(message)?;
        self.waker.wake();
        Ok(())
    }
}
impl<T: Unpin + Clone> Sender<T> {
    fn poll_send(&mut self, message: T, cx: &mut Context) -> Poll<Result<(), SendError>> {
        let item = match self.tx.as_ref().unwrap().try_send(message) {
            Ok(()) => {
                self.waker.wake();
                return Poll::Ready(Ok(()));
            }
            Err(TSE::Full(e)) => e,
            Err(_) => return Poll::Ready(Err(SendError)),
        };
        #[cfg(feature = "zerocap")]
        {
            if self.is_zero {
                // block the thread
                self.send_waker.register(cx.waker());
                self.waker.wake();
                match self.tx.as_ref().unwrap().send(item) {
                    Ok(()) => Poll::Ready(Ok(())),
                    Err(_) => Poll::Ready(Err(SendError)),
                }
            } else {
                // TODO: we may have to handle to_me.
                // currently since we use ArrayQueue, waker won't have memory leak issue
                self.send_waker.register(cx.waker());
                match self.tx.as_ref().unwrap().try_send(item) {
                    Ok(()) => {
                        self.waker.wake();
                        Poll::Ready(Ok(()))
                    }
                    Err(TSE::Full(_)) => Poll::Pending,
                    Err(_) => Poll::Ready(Err(SendError)),
                }
            }
        }
        #[cfg(not(feature = "zerocap"))]
        {
            // TODO: we may have to handle to_me.
            // currently since we use ArrayQueue, waker won't have memory leak issue
            self.send_waker.register(cx.waker());
            match self.tx.as_ref().unwrap().try_send(item) {
                Ok(()) => {
                    self.waker.wake();
                    Poll::Ready(Ok(()))
                }
                Err(TSE::Full(_)) => Poll::Pending,
                Err(_) => Poll::Ready(Err(SendError)),
            }
        }
    }
    // provides a faster send than SinkExt::send if message is Cloneable
    pub async fn send(&mut self, message: T) -> Result<(), SendError> {
        use futures::future::poll_fn;
        let item = match self.tx.as_ref().unwrap().try_send(message) {
            Ok(()) => {
                self.waker.wake();
                return Ok(());
            }
            Err(TSE::Full(e)) => e,
            Err(_) => return Err(SendError),
        };
        poll_fn(move |cx| self.poll_send(item.clone(), cx)).await
    }
}
