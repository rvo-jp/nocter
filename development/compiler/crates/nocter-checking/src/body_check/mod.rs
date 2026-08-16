mod checker;
mod context;
mod diagnostic;
mod error;
mod literal;

#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod place_tests;

pub use diagnostic::BodyRule;
pub use error::{BodyCheckError, BodyCheckInternalError};

pub use checker::check_prepared_program;
