#![feature(test)]
#![feature(no_more_cas)]

extern crate alloc;
mod error;
pub use error::*;
mod bounded;
pub use bounded::*;
mod unbounded;
pub use unbounded::*;
pub mod atomic_serial_waker;

#[cfg(test)]
mod test;
