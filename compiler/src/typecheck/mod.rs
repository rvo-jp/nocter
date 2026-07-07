//! Type checking, ownership, borrowing, move, and drop checks.

mod arrays;
mod bindings;
mod body;
mod calls;
mod controls;
mod diagnostics;
mod entry;
mod environments;
mod expressions;
mod fallible;
mod model;
mod numeric;
mod operations;
mod returns;
mod structs;
mod type_expr;
mod variants;

use crate::ast::{
    ArrayLiteralExpr, AstFile, BinaryExpr, BinaryOperator, BindingStmt, Block, CallExpr, Expr,
    FailStmt, ForRangeStmt, FunctionDecl, IfIsStmt, IfLetStmt, IfStmt, ImplDecl, ImplMember,
    IndexExpr, Item, MemberExpr, ProgramDecl, ReturnStmt, Stmt, StructLiteralExpr,
    StructLiteralField, SwitchArm, SwitchStmt, TypeConversionExpr, TypeExpr, UnaryExpr,
    UnaryOperator, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{
    EnumVariantSignature, FunctionSignature, MethodSignature, ParameterSignature, ResolveOutput,
    StructFieldSignature, TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};
use arrays::*;
use bindings::*;
use body::*;
use calls::*;
use controls::*;
use diagnostics::*;
use entry::*;
use environments::*;
use expressions::*;
use fallible::*;
use model::*;
use numeric::*;
use operations::*;
use returns::*;
use structs::*;
use type_expr::*;
use variants::*;

pub fn check(sources: &SourceMap, ast: &AstFile, resolved: &ResolveOutput) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_program_entry(sources, ast, &mut diagnostics);
    diagnostics.extend(check_module(sources, ast, resolved));

    diagnostics
}

pub fn check_module(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_body_expressions(sources, ast, resolved, &mut diagnostics);
    check_return_types(sources, ast, resolved, &mut diagnostics);

    diagnostics
}

#[cfg(test)]
mod tests;
