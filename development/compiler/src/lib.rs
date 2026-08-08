#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]

pub mod abi;
pub mod analysis;
pub mod ast;
pub mod backend;
mod builtin_types;
pub mod comments;
pub mod diagnostics;
pub mod driver;
pub mod entry;
pub mod format;
pub mod frontend;
pub mod home;
pub mod ir;
pub mod lexer;
mod literals;
mod outcomes;
pub mod package;
pub mod parser;
pub mod resolve;
mod semantics;
pub mod source;
pub mod target;
mod test_entry;
mod type_notation;
pub mod typecheck;
