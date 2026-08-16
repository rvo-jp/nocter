mod checker;
mod context;
mod diagnostic;
mod error;
mod literal;
mod ownership;

#[cfg(test)]
mod cleanup_tests;
#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod loop_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod place_tests;

pub use diagnostic::BodyRule;
pub use error::{BodyCheckError, BodyCheckInternalError};

pub use checker::check_prepared_program;
