#![no_main]

use libfuzzer_sys::fuzz_target;
use medplum_mcp_core::token_diet::{distill_raw_slice, DetailLevel};

fuzz_target!(|data: &[u8]| {
    let mut buf = data.to_vec();
    for level in [
        DetailLevel::Compact,
        DetailLevel::Standard,
        DetailLevel::Executive,
    ] {
        let _ = distill_raw_slice(&mut buf, level);
    }
});
