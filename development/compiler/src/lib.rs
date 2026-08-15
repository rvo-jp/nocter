#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]

pub mod abi;
pub mod analysis;
pub mod ast;
pub mod backend;
mod builtin_types;
mod callable_bodies;
mod callable_parameters;
pub mod comments;
pub mod diagnostics;
pub mod driver;
pub mod entry;
pub mod format;
pub mod frontend;
pub mod home;
mod integer;
mod intrinsics;
pub mod ir;
pub mod lexer;
mod literals;
mod mir;
mod outcomes;
pub mod package;
pub mod parser;
pub mod resolve;
mod semantic;
mod semantics;
pub mod source;
mod source_layout;
mod source_modules;
mod source_scopes;
pub mod target;
mod test_entry;
#[cfg(test)]
mod test_files;
mod timing;
mod type_names;
mod type_notation;
pub mod typecheck;
