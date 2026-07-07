//! Backend-independent intermediate representation.

mod lower;
mod model;

pub(crate) use lower::lower_program;
pub(crate) use model::{Function, Instruction, IrModule, Type};
