#![feature(test)]
#![feature(no_more_cas)]

extern crate test;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use test::Bencher;

#[bench]
fn test_fetch_update(b: &mut Bencher) {
    let a = AtomicUsize::new(1000000);
    b.iter(|| {
        for _ in 0..1000 {
            match a.fetch_update(
                |x| {
                    if x > 0 {
                        Some(x - 1)
                    } else {
                        None
                    }
                },
                SeqCst,
                SeqCst,
            ) {
                Ok(_) => true,
                Err(_) => false,
            };
        }
    });
}
