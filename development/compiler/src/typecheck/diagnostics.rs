mod arrays;
mod bindings;
mod callables;
mod calls;
mod closures;
mod control_flow;
mod destructors;
mod entry;
mod fallible;
mod generic_bounds;
mod interfaces;
mod members;
mod operators;
mod optional;
mod ownership;
mod patterns;
mod provenance_contracts;
mod regions;
mod returns;
mod strings;
mod structs;
mod support;
mod types;
mod variants;

use super::calls::CheckedCallSignature;
use super::copyability::NonCopyOwnedValueKind;
use super::model::{ReturnContext, Type};
use super::type_expr::canonical_type_expr;
use crate::ast::{
    AssignmentStmt, BinaryExpr, BindingKind, BindingStmt, Block, BorrowExpr, CallExpr, Expr,
    ForRangeStmt, IfIsStmt, IndexExpr, InterpolatedStringExpression, MemberExpr, OtherwiseExpr,
    ReturnStmt, StructLiteralExpr, StructLiteralField, SwitchArm, SwitchStmt, TypeConversionExpr,
    TypeExpr, UnaryExpr,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::resolve::{
    AssociatedTypeBindingSignature, AssociatedTypeSignature, EnumVariantSignature, MethodSignature,
    ParameterSignature, ResolveOutput, StructFieldSignature, TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};

pub(super) use arrays::*;
pub(super) use bindings::*;
pub(super) use callables::*;
pub(super) use calls::*;
pub(super) use closures::*;
pub(super) use control_flow::*;
pub(super) use destructors::*;
pub(super) use entry::*;
pub(super) use fallible::*;
pub(super) use generic_bounds::*;
pub(super) use interfaces::*;
pub(super) use members::*;
pub(super) use operators::*;
pub(super) use optional::*;
pub(super) use ownership::*;
pub(super) use patterns::*;
pub(super) use provenance_contracts::*;
pub(super) use regions::*;
pub(super) use returns::*;
pub(super) use strings::*;
pub(super) use structs::*;
pub(super) use types::*;
pub(super) use variants::*;

use support::*;
