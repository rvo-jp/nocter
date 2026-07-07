//! Backend-independent intermediate representation.

mod lower;
mod model;

pub(crate) use lower::lower_program;
pub(crate) use model::{Function, I32Location, I32Value, Instruction, IrModule, Type};
