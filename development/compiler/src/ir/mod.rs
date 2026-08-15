//! Backend-independent intermediate representation.

mod lower;
pub(crate) use lower::coercion_symbol_name;
mod model;

pub(crate) use lower::{lower_executable, lower_test};
pub(crate) use model::{
    AggregateArgument, AggregateArgumentSource, AggregateIndex, AggregateLocation, AggregateRange,
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, BorrowArgument,
    BorrowSource, CallTarget, ComposedOutcomeDestination, DirectAggregateArgument, Function,
    I32ComparisonOperator, I32Location, I32Value, Instruction, IntegerBinaryOperator, IrModule,
    OutcomeFailureMode, ScalarArgument, SliceElementAddressKind, SliceElementIndex, SliceLocation,
    SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation, UsizeValue,
};
