//! Backend-independent intermediate representation.

mod lower;
mod model;

pub(crate) use lower::lower_executable_with_entry;
pub(crate) use model::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    BorrowArgument, BorrowSource, CallTarget, FallibleFailureMode, Function, I32ComparisonOperator,
    I32Location, I32Value, Instruction, IrModule, ScalarArgument, SliceLocation, SliceValue,
    StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation, UsizeValue,
};
