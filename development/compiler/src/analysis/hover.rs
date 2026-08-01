//! Hover information derived from compile-unit analysis.

use super::single_file::{parse_single_file_text, resolve_single_file_ast};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, BindingStmt, Block, EnumDecl, Expr, FunctionDecl, ImplMember, InterfaceDecl,
    InterpolatedStringPart, Item, MethodDecl, ModulePath, Parameter, PrimitiveDecl, Stmt,
    StructDecl, StructField,
};
use crate::comments::{DocumentationTarget, attach_documentation};
use crate::resolve::{
    LocalSymbol, LocalSymbolKind, ResolveOutput, Symbol, SymbolKind, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{TypecheckFacts, collect_typecheck_facts};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HoverInfo {
    pub(crate) span: ByteSpan,
    pub(crate) label: String,
    pub(crate) documentation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HoverSymbol {
    name_span: ByteSpan,
    attach_start: usize,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedReference {
    TopLevel(Box<Symbol>),
    Local(LocalSymbol),
}

impl ResolvedReference {
    fn declaration_span(&self) -> ByteSpan {
        match self {
            ResolvedReference::TopLevel(symbol) => symbol.declaration_span,
            ResolvedReference::Local(symbol) => symbol.name_span,
        }
    }
}

mod contents;
mod entry;
mod module_paths;
mod symbols;
mod targets;

pub(in crate::analysis::hover) use contents::*;
#[allow(unused_imports)]
pub(crate) use entry::hover_for_ast;
pub(crate) use entry::{definition_span_for_ast, hover_for_file_analysis, hover_for_text};
pub(crate) use module_paths::module_path_at_offset;
pub(in crate::analysis::hover) use module_paths::*;
pub(in crate::analysis::hover) use symbols::*;
pub(in crate::analysis::hover) use targets::*;

#[cfg(test)]
mod tests;
