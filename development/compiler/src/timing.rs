//! Opt-in internal timing events for compiler performance work.

use std::sync::OnceLock;
use std::time::Instant;

const TIMING_ENV: &str = "NOCTER_INTERNAL_TIMINGS";

pub(crate) fn measure<T>(phase: &'static str, operation: impl FnOnce() -> T) -> T {
    measure_at_level(1, phase, operation)
}

pub(crate) fn measure_detail<T>(phase: &'static str, operation: impl FnOnce() -> T) -> T {
    measure_at_level(2, phase, operation)
}

fn measure_at_level<T>(level: u8, phase: &'static str, operation: impl FnOnce() -> T) -> T {
    if timing_level() < level {
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

fn timing_level() -> u8 {
    static LEVEL: OnceLock<u8> = OnceLock::new();
    *LEVEL.get_or_init(|| {
        std::env::var(TIMING_ENV)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    })
}
