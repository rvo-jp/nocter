//! Opt-in internal timing events for compiler performance work.

use std::sync::OnceLock;
use std::time::Instant;

const TIMING_ENV: &str = "NOCTER_INTERNAL_TIMINGS";

pub(crate) fn measure<T>(phase: &'static str, operation: impl FnOnce() -> T) -> T {
    if !enabled() {
        return operation();
    }

    let start = Instant::now();
    let result = operation();
    let elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
    eprintln!(
        "{}",
        serde_json::json!({
            "event": "nocter_timing",
            "phase": phase,
            "elapsed_us": elapsed_us,
            "pid": std::process::id(),
        })
    );
    result
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os(TIMING_ENV).is_some_and(|value| value == "1"))
}
