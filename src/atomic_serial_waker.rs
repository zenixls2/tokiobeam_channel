use alloc::sync::Arc;
use core::fmt;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::Waker;
use crossbeam::atomic::AtomicConsume;
use crossbeam::queue::{ArrayQueue, PushError};

#[inline]
pub fn positive_update(a: &AtomicUsize) -> bool {
    let mut prev = a.load_consume();
    while prev > 0 {
        match a.compare_exchange_weak(prev, prev - 1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return true,
            Err(e) => prev = e,
        }
    }
    false
}

pub struct AtomicSerialWaker {
    waker: ArrayQueue<Waker>,
    // increase when wake() or wk() fails to find one waker to wake()
    // decrease when register() and no other thread is waking
    waiting: AtomicUsize,
    // flag shared with Receiver/UnboundedReceiver
    // please refer to the explanation in UnboundedReceiver/Receiver
    from_me: Arc<AtomicUsize>,
}

impl AtomicSerialWaker {
    pub fn new(from_me: Arc<AtomicUsize>) -> Self {
        trait AssertSync: Sync {}
        impl AssertSync for Waker {}
        Self {
            waker: ArrayQueue::new(4),
            waiting: AtomicUsize::new(0),
            from_me: from_me,
        }
    }
    #[inline]
    pub fn register(&self, waker: &Waker) {
        match self.waker.push(waker.clone()) {
            Ok(()) => {
                self.wk();
            }
            Err(PushError(w)) => {
                self.from_me.fetch_add(1, Ordering::Release);
                w.wake_by_ref();
            }
        };
    }

    #[inline]
    fn wk(&self) {
        if let Ok(task) = self.waker.pop() {
            if positive_update(&self.waiting) {
                self.from_me.fetch_add(1, Ordering::Release);
                task.wake();
            } else {
                match self.waker.push(task) {
                    Ok(()) => {}
                    Err(PushError(w)) => {
                        self.from_me.fetch_add(1, Ordering::Release);
                        w.wake_by_ref();
                    }
                }
            }
        }
    }

    #[inline]
    pub fn wake(&self) {
        let task = self.waker.pop();
        if let Ok(task) = task {
            self.from_me.fetch_add(1, Ordering::Release);
            task.wake();
        } else {
            self.waiting.fetch_add(1, Ordering::Release);
        }
    }
}

impl fmt::Debug for AtomicSerialWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AtomicSerialWaker")
    }
}

unsafe impl Send for AtomicSerialWaker {}
unsafe impl Sync for AtomicSerialWaker {}
