#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]

pub mod abi;
pub mod analysis;
pub mod ast;
pub mod backend;
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
pub mod parser;
pub mod resolve;
mod semantics;
pub mod source;
pub mod target;
pub mod typecheck;
