use core::fmt;
use crossbeam::crossbeam_channel::TrySendError as TSE;

#[derive(Debug)]
pub struct SendError;

#[derive(Debug)]
pub struct TrySendError<T> {
    kind: ErrorKind,
    value: T,
}

#[derive(Debug)]
enum ErrorKind {
    Closed,
    NoCapacity,
}

impl fmt::Display for SendError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "channel closed")
    }
}
impl std::error::Error for SendError {}

impl<T> TrySendError<T> {
    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
    #[inline]
    pub fn is_closed(&self) -> bool {
        if let ErrorKind::Closed = self.kind {
            true
        } else {
            false
        }
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        if let ErrorKind::NoCapacity = self.kind {
            true
        } else {
            false
        }
    }
}

impl<T: fmt::Debug> fmt::Display for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let descr = match self.kind {
            ErrorKind::Closed => "channel closed",
            ErrorKind::NoCapacity => "no available capacity",
        };
        write!(fmt, "{}", descr)
    }
}
impl<T: fmt::Debug> std::error::Error for TrySendError<T> {}

impl<T> From<TSE<T>> for TrySendError<T> {
    fn from(err: TSE<T>) -> TrySendError<T> {
        match err {
            TSE::Full(i) => TrySendError {
                value: i,
                kind: ErrorKind::NoCapacity,
            },
            TSE::Disconnected(i) => TrySendError {
                value: i,
                kind: ErrorKind::Closed,
            },
        }
    }
}
