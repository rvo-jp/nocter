//! Darwin readiness adapter for the target-independent task runtime.

#[cfg(target_os = "macos")]
mod reactor;

#[cfg(target_os = "macos")]
pub use reactor::{DarwinReactor, DarwinReactorError, ProcessMonotonicClock, ReactorClock};
