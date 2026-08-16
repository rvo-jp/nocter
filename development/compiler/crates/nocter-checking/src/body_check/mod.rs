mod checker;
mod diagnostic;
mod error;
mod literal;

#[cfg(test)]
mod move_tests;

pub use diagnostic::BodyRule;
pub use error::{BodyCheckError, BodyCheckInternalError};

pub use checker::check_prepared_program;
