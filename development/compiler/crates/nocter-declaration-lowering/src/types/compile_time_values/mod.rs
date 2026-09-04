//! Compile-time evaluation boundary for storage-independent constants and immutable statics.

mod evaluator;

pub use evaluator::evaluate;
