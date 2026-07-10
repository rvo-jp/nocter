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
mod strings;
mod structs;
mod support;
mod variants;

use super::calls::CheckedCallSignature;
use super::model::{ReturnContext, Type};
use super::type_expr::type_expr_display_lossy;
use crate::ast::{
    BinaryExpr, BindingKind, BindingStmt, Block, CallExpr, Expr, ForRangeStmt, IfIsStmt, IfLetStmt,
    IndexExpr, InterpolatedStringExpression, MemberExpr, PatternConditionalArm,
    PatternConditionalExpr, ReturnStmt, StructLiteralExpr, StructLiteralField, SwitchArm,
    SwitchStmt, TypeConversionExpr, UnaryExpr, WhileLetStmt,
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
pub(super) use entry::*;
pub(super) use fallible::*;
pub(super) use members::*;
pub(super) use operators::*;
pub(super) use optional::*;
pub(super) use patterns::*;
pub(super) use returns::*;
pub(super) use strings::*;
pub(super) use structs::*;
pub(super) use variants::*;

use support::*;
