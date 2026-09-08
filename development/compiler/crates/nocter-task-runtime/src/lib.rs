//! Target-independent task lifecycle and readiness-registration authority.
//!
//! This model is shared by deterministic runtime tests and native executor generation. It knows
//! opaque descriptor/timer interests but no compiler IR, computation-frame layout, or native event
//! record.

mod executor;
mod identity;
mod reactor;
mod scheduler;

pub use identity::{RegistrationId, TaskId};
pub use nocter_runtime_contract::{ReactorInterest, ReadinessDirection};
pub use reactor::Reactor;
pub use scheduler::{
    CancellationKind, Scheduler, SchedulerError, SchedulerProgress, TaskCancellation, TaskState,
};

#[cfg(test)]
mod executor_tests;
#[cfg(test)]
mod tests;
pub use executor::{
    Computation, ComputationPoll, Executor, ExecutorError, ExecutorProgress, WaitSet,
};
