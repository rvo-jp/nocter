mod arrays;
mod bindings;
mod calls;
mod control_flow;
mod drop_members;
mod entry;
mod fallible;
mod interfaces;
mod members;
mod operators;
mod optional;
mod ownership;
mod patterns;
mod returns;
mod strings;
mod structs;
mod support;
mod types;
mod variants;

use super::calls::CheckedCallSignature;
use super::model::{ReturnContext, Type};
use super::type_expr::type_expr_display_lossy;
use crate::ast::{
    AssignmentStmt, BinaryExpr, BindingKind, BindingStmt, Block, BorrowExpr, CallExpr, Expr,
    ForRangeStmt, IfIsStmt, IfLetStmt, ImplDecl, IndexExpr, InterpolatedStringExpression,
    MemberExpr, OptionalDefaultExpr, PatternConditionalArm, PatternConditionalExpr, ReturnStmt,
    StructLiteralExpr, StructLiteralField, SwitchArm, SwitchStmt, TypeConversionExpr, TypeExpr,
    UnaryExpr, WhileLetStmt,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::resolve::{
    EnumVariantSignature, MethodSignature, ParameterSignature, ResolveOutput, StructFieldSignature,
    TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};

pub(super) use arrays::*;
pub(super) use bindings::*;
pub(super) use calls::*;
pub(super) use control_flow::*;
pub(super) use drop_members::*;
pub(super) use entry::*;
pub(super) use fallible::*;
pub(super) use interfaces::*;
pub(super) use members::*;
pub(super) use operators::*;
pub(super) use optional::*;
pub(super) use ownership::*;
pub(super) use patterns::*;
pub(super) use returns::*;
pub(super) use strings::*;
pub(super) use structs::*;
pub(super) use types::*;
pub(super) use variants::*;

use support::*;
