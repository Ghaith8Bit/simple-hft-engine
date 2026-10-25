#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Any byte sequence is valid input. Errors are fine; panics and UB are bugs.
    let _ = hft_feed::decode::decode(data);
});
