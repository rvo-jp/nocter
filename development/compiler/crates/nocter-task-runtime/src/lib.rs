//! Target-independent task lifecycle and readiness-registration authority.
//!
//! This model is shared by deterministic runtime tests and native executor generation. It knows
//! opaque descriptor/timer interests but no compiler IR, computation-frame layout, or native event
//! record.

mod identity;
mod reactor;
mod scheduler;

pub use identity::{RegistrationId, TaskId};
pub use reactor::{Reactor, ReactorInterest, ReadinessDirection};
pub use scheduler::{
    CancellationKind, Scheduler, SchedulerError, SchedulerProgress, TaskCancellation, TaskState,
};

#[cfg(test)]
mod tests;
