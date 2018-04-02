use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use sha1::Sha1;

pub fn proof_for_time(password: &str, time: u64) -> String {
    let mut sh = Sha1::new();
    sh.update(format!("{}{}{}", password, time, password).as_bytes());
    let hash = sh.digest().bytes();
    let mut res = String::new();
    for b in hash.iter() {
        write!(res, "{:02x}", b).unwrap();
    }
    res
}

pub fn current_proof(password: &str) -> String {
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    proof_for_time(password, epoch)
}

pub fn check_proof(password: &str, proof: &str, allowed_diff: u64) -> bool {
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    for time in (epoch - allowed_diff)..(epoch + allowed_diff) {
        if proof_for_time(password, time) == proof {
            return true;
        }
    }
    false
}
