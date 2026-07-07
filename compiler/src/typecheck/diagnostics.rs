mod arrays;
mod bindings;
mod calls;
mod control_flow;
mod entry;
mod fallible;
mod members;
mod operators;
mod optional;
mod patterns;
mod returns;
mod structs;
mod support;
mod variants;

use super::*;
use crate::ast::BindingKind;
use crate::diagnostics::{Diagnostic, DiagnosticNote};

pub(super) use arrays::*;
pub(super) use bindings::*;
pub(super) use calls::*;
pub(super) use control_flow::*;
pub(super) use entry::*;
pub(super) use fallible::*;
pub(super) use members::*;
pub(super) use operators::*;
pub(super) use optional::*;
pub(super) use patterns::*;
pub(super) use returns::*;
pub(super) use structs::*;
pub(super) use variants::*;

use support::*;
