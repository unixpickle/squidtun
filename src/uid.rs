use std::fmt::Write;

use rand::thread_rng;
use rand::distributions::{Range, IndependentSample};

pub fn generate_session_id() -> String {
    let mut res = String::new();
    let mut rng = thread_rng();
    let range = Range::new(0u8, 16u8);
    for _ in 0..32 {
        write!(res, "{:x}", range.ind_sample(&mut rng)).unwrap();
    }
    res
}
