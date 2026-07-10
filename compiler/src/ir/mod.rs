//! Backend-independent intermediate representation.

mod lower;
mod model;

pub(crate) use lower::lower_executable_with_entry;
pub(crate) use model::{
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, CallTarget, Function,
    I32ComparisonOperator, I32Location, I32Value, Instruction, IrModule, Type,
};
