//! Target-independent owned blocking-job lifecycle.
//!
//! This crate models capacity and ownership. Native worker execution and completion notification
//! are adapters over this contract rather than alternate lifecycle authorities.

mod identity;
mod service;

pub use identity::{CapacityEpoch, JobId};
pub use service::{
    Backpressure, BlockingJobService, Cancellation, JobOutcome, JobStatus, RunningJob,
    ServiceCapacity, ServiceError, ServiceSnapshot, ShutdownCleanup, SubmitError,
};

#[cfg(test)]
mod tests;
