#![feature(test)]

extern crate test;
use std::sync::atomic::AtomicUsize;
use test::Bencher;
use tokiobeam_channel::atomic_serial_waker::*;

#[bench]
fn test_positive_update(b: &mut Bencher) {
    let a = AtomicUsize::new(1000000);
    b.iter(|| {
        for _ in 0..1000 {
            positive_update(&a);
        }
    });
}
