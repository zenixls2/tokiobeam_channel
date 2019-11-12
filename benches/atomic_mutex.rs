#![feature(test)]

extern crate test;
use std::sync::Mutex;
use test::Bencher;

#[bench]
fn test_sth(b: &mut Bencher) {
    let a = Mutex::new(1000000);
    b.iter(|| {
        for _ in 0..1000 {
            {
                let mut aa = a.lock().unwrap();
                if *aa > 0 {
                    *aa = *aa - 1;
                    true
                } else {
                    false
                }
            };
        }
    })
}
