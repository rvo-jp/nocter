//! Compiler-owned source targets shared by editor features.
//!
//! A target keeps the source range an editor should react to separate from the
//! declaration range that gives the target its semantic identity.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, Block, Expr, FromImportItem, ImplMember, ImportItem, InterpolatedStringPart, Item,
    ModulePath, Stmt,
};
use crate::resolve::{ImportedSymbolKind, ResolveOutput, Symbol, SymbolKind};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceTarget {
    pub(crate) focus_span: ByteSpan,
    pub(crate) declaration_span: ByteSpan,
}

impl SourceTarget {
    pub(crate) const fn new(focus_span: ByteSpan, declaration_span: ByteSpan) -> Self {
        Self {
            focus_span,
            declaration_span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum EditorTargetKind<'a> {
    Module(&'a ModulePath),
    ImportBinding(&'a Symbol),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EditorTarget<'a> {
    pub(crate) focus_span: ByteSpan,
    pub(crate) kind: EditorTargetKind<'a>,
}

impl EditorTarget<'_> {
    pub(crate) fn source_target(self, analysis: &CompileUnitAnalysis) -> Option<SourceTarget> {
        let declaration_span = match self.kind {
            EditorTargetKind::Module(path) => {
                let source = analysis.import_sources.get(&path.span)?.source;
                let imported_file = analysis.file_by_source(source)?;
                ByteSpan::new(imported_file.ast.span.source, 0, 0)
            }
            EditorTargetKind::ImportBinding(symbol) => match &symbol.kind {
                SymbolKind::Imported(imported)
                    if imported.kind == ImportedSymbolKind::Namespace =>
                {
                    let source = imported.source?;
                    let imported_file = analysis.file_by_source(source)?;
                    ByteSpan::new(imported_file.ast.span.source, 0, 0)
                }
                SymbolKind::Function(_)
                | SymbolKind::Primitive(_)
                | SymbolKind::Type(_)
                | SymbolKind::Imported(_) => analysis
                    .file_by_source(symbol.declaration_span.source)
                    .and_then(|file| {
                        file.resolved
                            .symbols
                            .symbols()
                            .find(|candidate| candidate.declaration_span == symbol.declaration_span)
                            .map(|candidate| candidate.name_span)
                    })
                    .unwrap_or(symbol.declaration_span),
            },
        };

        Some(SourceTarget::new(self.focus_span, declaration_span))
    }
}

pub(crate) fn editor_targets(file: &FileAnalysis) -> Vec<EditorTarget<'_>> {
    editor_targets_for_ast(&file.ast, &file.resolved)
}

pub(crate) fn editor_targets_for_ast<'a>(
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
) -> Vec<EditorTarget<'a>> {
    let mut sites = Vec::new();
    collect_import_sites(ast, &mut sites);

    let mut targets = Vec::new();
    for site in sites {
        match site {
            ImportSite::Namespace(import) => {
                targets.push(EditorTarget {
                    focus_span: import.path.span,
                    kind: EditorTargetKind::Module(&import.path),
                });
                if !import.alias_is_default
                    && let Some(symbol) = symbol_at_binding_span(resolved, import.alias.span)
                {
                    targets.push(EditorTarget {
                        focus_span: import.alias.span,
                        kind: EditorTargetKind::ImportBinding(symbol),
                    });
                }
            }
            ImportSite::Names(import) => {
                targets.push(EditorTarget {
                    focus_span: import.path.span,
                    kind: EditorTargetKind::Module(&import.path),
                });
                for name in &import.names {
                    let Some(symbol) = symbol_at_binding_span(resolved, name.local_span()) else {
                        continue;
                    };
                    targets.push(EditorTarget {
                        focus_span: name.name_span,
                        kind: EditorTargetKind::ImportBinding(symbol),
                    });
                    if let Some(alias) = &name.alias {
                        targets.push(EditorTarget {
                            focus_span: alias.span,
                            kind: EditorTargetKind::ImportBinding(symbol),
                        });
                    }
                }
            }
        }
    }
    targets
}

pub(crate) fn editor_target_at_offset(
    file: &FileAnalysis,
    offset: usize,
) -> Option<EditorTarget<'_>> {
    editor_targets(file)
        .into_iter()
        .filter(|target| span_contains(target.focus_span, offset))
        .min_by_key(|target| (target.focus_span.len(), target.focus_span.start))
}

fn symbol_at_binding_span(resolved: &ResolveOutput, span: ByteSpan) -> Option<&Symbol> {
    resolved
        .symbols
        .symbols()
        .find(|symbol| symbol.name_span == span)
}

#[derive(Debug, Clone, Copy)]
enum ImportSite<'a> {
    Namespace(&'a ImportItem),
    Names(&'a FromImportItem),
}

fn collect_import_sites<'a>(ast: &'a AstFile, sites: &mut Vec<ImportSite<'a>>) {
    for item in &ast.items {
        match item {
            Item::Import(import) => sites.push(ImportSite::Namespace(import)),
            Item::FromImport(import) => sites.push(ImportSite::Names(import)),
            Item::Function(function) => collect_block_import_sites(&function.body, sites),
            Item::Impl(impl_) => {
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            if let Some(body) = &method.body {
                                collect_block_import_sites(body, sites);
                            }
                        }
                        ImplMember::Drop(drop_) => collect_block_import_sites(&drop_.body, sites),
                    }
                }
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        collect_block_import_sites(body, sites);
                    }
                }
            }
            Item::Literal(literal) => collect_block_import_sites(&literal.body, sites),
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    collect_block_import_sites(&function.body, sites);
                }
                for (_, literal) in construct.literals() {
                    collect_block_import_sites(&literal.body, sites);
                }
            }
            Item::Primitive(_) | Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) => {}
        }
    }
}

fn collect_block_import_sites<'a>(block: &'a Block, sites: &mut Vec<ImportSite<'a>>) {
    for statement in &block.statements {
        match statement {
            Stmt::Import(import) => sites.push(ImportSite::Namespace(import)),
            Stmt::FromImport(import) => sites.push(ImportSite::Names(import)),
            _ => {}
        }
        collect_statement_import_sites(statement, sites);
    }
    if let Some(result) = &block.result {
        collect_expression_import_sites(result, sites);
    }
}

fn collect_statement_import_sites<'a>(statement: &'a Stmt, sites: &mut Vec<ImportSite<'a>>) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_import_sites(expression, sites);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_import_sites(&statement.initializer, sites);
        }
        Stmt::Assignment(statement) => {
            collect_expression_import_sites(&statement.target, sites);
            collect_expression_import_sites(&statement.value, sites);
        }
        Stmt::If(statement) => {
            collect_expression_import_sites(&statement.condition, sites);
            collect_block_import_sites(&statement.then_block, sites);
            if let Some(block) = &statement.else_block {
                collect_block_import_sites(block, sites);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_import_sites(&statement.expression, sites);
            collect_block_import_sites(&statement.then_block, sites);
            if let Some(block) = &statement.else_block {
                collect_block_import_sites(block, sites);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_import_sites(&statement.expression, sites);
            for arm in &statement.arms {
                collect_block_import_sites(&arm.body, sites);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_import_sites(&arm.body, sites);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_import_sites(&statement.start, sites);
            collect_expression_import_sites(&statement.end, sites);
            collect_block_import_sites(&statement.body, sites);
        }
        Stmt::CollectionFor(statement) => {
            collect_expression_import_sites(&statement.source, sites);
            collect_block_import_sites(&statement.body, sites);
        }
        Stmt::LiteralPackFor(statement) => collect_block_import_sites(&statement.body, sites),
        Stmt::While(statement) => {
            collect_expression_import_sites(&statement.condition, sites);
            collect_block_import_sites(&statement.body, sites);
        }
        Stmt::Loop(statement) => collect_block_import_sites(&statement.body, sites),
        Stmt::Region(statement) => {
            collect_expression_import_sites(&statement.allocator, sites);
            collect_block_import_sites(&statement.body, sites);
        }
        Stmt::Expression(statement) => {
            collect_expression_import_sites(&statement.expression, sites);
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => {}
    }
}

fn collect_expression_import_sites<'a>(expression: &'a Expr, sites: &mut Vec<ImportSite<'a>>) {
    match expression {
        Expr::Closure(expression) => collect_block_import_sites(&expression.body, sites),
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_import_sites(&part.expression, sites);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_import_sites(element, sites);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_import_sites(element, sites);
            }
            if let Some(using) = &expression.using {
                collect_expression_import_sites(&using.allocator, sites);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_import_sites(&using.allocator, sites);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_import_sites(&field.value, sites);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_import_sites(&expression.expression, sites)
        }
        Expr::Force(expression) => collect_expression_import_sites(&expression.expression, sites),
        Expr::Catch(expression) => {
            collect_expression_import_sites(&expression.expression, sites);
            collect_block_import_sites(&expression.catch_block, sites);
        }
        Expr::Borrow(expression) => collect_expression_import_sites(&expression.expression, sites),
        Expr::Unary(expression) => collect_expression_import_sites(&expression.operand, sites),
        Expr::Binary(expression) => {
            collect_expression_import_sites(&expression.left, sites);
            collect_expression_import_sites(&expression.right, sites);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_import_sites(&expression.expression, sites);
        }
        Expr::Call(expression) => {
            collect_expression_import_sites(&expression.callee, sites);
            for argument in &expression.arguments {
                collect_expression_import_sites(argument, sites);
            }
        }
        Expr::Member(expression) => collect_expression_import_sites(&expression.object, sites),
        Expr::Index(expression) => {
            collect_expression_import_sites(&expression.object, sites);
            collect_expression_import_sites(&expression.index, sites);
        }
        Expr::Group(expression) => collect_expression_import_sites(&expression.expression, sites),
        Expr::Otherwise(expression) => {
            collect_expression_import_sites(&expression.value, sites);
            collect_block_import_sites(&expression.fallback, sites);
        }
        Expr::If(expression) => {
            collect_expression_import_sites(&expression.condition, sites);
            collect_block_import_sites(&expression.then_block, sites);
            if let Some(block) = &expression.else_block {
                collect_block_import_sites(block, sites);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_import_sites(&expression.expression, sites);
            collect_block_import_sites(&expression.then_block, sites);
            if let Some(block) = &expression.else_block {
                collect_block_import_sites(block, sites);
            }
        }
        Expr::Match(expression) => {
            collect_expression_import_sites(&expression.expression, sites);
            for arm in &expression.arms {
                collect_block_import_sites(&arm.body, sites);
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_block_import_sites(&arm.body, sites);
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
