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

pub(crate) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    if let Some(hover) = module_path_hover_for_ast(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        return Some(HoverInfo {
            span: symbol.name_span,
            label: symbol.label.clone(),
            documentation: documentation
                .get(symbol.name_span.start)
                .map(str::to_string),
        });
    }

    if let Some(hover) = call_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    if let Some((span, label)) = file.typecheck_facts.field_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: None,
        });
    }

    if let Some((span, label)) = file.typecheck_facts.enum_variant_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: file
                .typecheck_facts
                .enum_variant_target(span)
                .and_then(|target| target_documentation(sources, analysis, target)),
        });
    }

    if let Some(hover) = type_reference_hover_for_file_analysis(sources, analysis, file, offset) {
        return Some(hover);
    }

    resolved_reference_at_offset(&file.resolved, offset).map(|(span, reference)| {
        let (label, documentation) =
            resolved_reference_hover_contents(sources, analysis, &reference);
        HoverInfo {
            span,
            label,
            documentation,
        }
    })
}

pub(crate) fn hover_for_text(text: &str, offset: usize) -> Option<HoverInfo> {
    let parsed = parse_single_file_text("hover.nct", text)?;

    hover_for_ast(text, parsed.source, &parsed.ast, offset)
}

pub(crate) fn hover_for_ast(
    text: &str,
    source: SourceId,
    ast: &AstFile,
    offset: usize,
) -> Option<HoverInfo> {
    let resolved = resolve_single_file_ast("hover.nct", text, source, ast);
    let facts = collect_typecheck_facts(ast, &resolved);
    let mut symbols = hover_symbols_for_ast(text, ast);
    apply_typecheck_hover_facts(text, &facts, &mut symbols);
    let documentation = documentation_for_hover_symbols(source, text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        return Some(HoverInfo {
            span: symbol.name_span,
            label: symbol.label.clone(),
            documentation: documentation
                .get(symbol.name_span.start)
                .map(str::to_string),
        });
    }

    if let Some((span, label)) = facts.call_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: facts
                .function_call_target(span)
                .or_else(|| facts.method_call_target(span))
                .or_else(|| facts.associated_function_target(span))
                .and_then(|target| documentation_for_target_span(&documentation, &symbols, target)),
        });
    }

    if let Some((span, label)) = facts.field_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: None,
        });
    }

    if let Some((span, label)) = facts.enum_variant_hover_at_offset(offset) {
        return Some(HoverInfo {
            span,
            label: label.to_string(),
            documentation: facts
                .enum_variant_target(span)
                .and_then(|target| documentation_for_target_span(&documentation, &symbols, target)),
        });
    }

    if let Some(hover) =
        type_reference_hover_for_ast(text, &resolved, &facts, &symbols, &documentation, offset)
    {
        return Some(hover);
    }

    resolved_reference_at_offset(&resolved, offset).map(|(span, reference)| {
        let (label, documentation) = single_file_resolved_reference_hover_contents(
            text,
            &symbols,
            &documentation,
            &reference,
        );
        HoverInfo {
            span,
            label,
            documentation,
        }
    })
}

pub(crate) fn definition_span_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    let symbols = hover_symbols_for_ast(text, ast);
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| span_contains(symbol.name_span, offset))
    {
        return Some(symbol.name_span);
    }

    resolved_reference_at_offset(resolved, offset)
        .map(|(_, reference)| reference.declaration_span())
}

pub(crate) fn module_path_at_offset(ast: &AstFile, offset: usize) -> Option<&ModulePath> {
    ast.items
        .iter()
        .find_map(|item| module_path_in_item_at_offset(item, offset))
}

fn module_path_in_item_at_offset(item: &Item, offset: usize) -> Option<&ModulePath> {
    match item {
        Item::Import(item) => path_if_at_offset(&item.path, offset),
        Item::FromImport(item) => path_if_at_offset(&item.path, offset),
        Item::Function(function) => module_path_in_block_at_offset(&function.body, offset),
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| match member {
            ImplMember::Method(method) => method
                .body
                .as_ref()
                .and_then(|body| module_path_in_block_at_offset(body, offset)),
            ImplMember::Drop(drop_) => module_path_in_block_at_offset(&drop_.body, offset),
        }),
        Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Interface(_) => None,
    }
}

fn module_path_in_block_at_offset(block: &Block, offset: usize) -> Option<&ModulePath> {
    block
        .statements
        .iter()
        .find_map(|statement| module_path_in_statement_at_offset(statement, offset))
        .or_else(|| {
            block
                .result
                .as_deref()
                .and_then(|result| module_path_in_expression_at_offset(result, offset))
        })
}

fn module_path_in_statement_at_offset(statement: &Stmt, offset: usize) -> Option<&ModulePath> {
    match statement {
        Stmt::Import(statement) => path_if_at_offset(&statement.path, offset),
        Stmt::FromImport(statement) => path_if_at_offset(&statement.path, offset),
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .and_then(|expression| module_path_in_expression_at_offset(expression, offset)),
        Stmt::Binding(statement) => {
            module_path_in_expression_at_offset(&statement.initializer, offset)
        }
        Stmt::Assignment(statement) => {
            module_path_in_expression_at_offset(&statement.target, offset)
                .or_else(|| module_path_in_expression_at_offset(&statement.value, offset))
        }
        Stmt::If(statement) => module_path_in_expression_at_offset(&statement.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.then_block, offset))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Stmt::IfIs(statement) => module_path_in_expression_at_offset(&statement.expression, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.then_block, offset))
            .or_else(|| {
                statement
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Stmt::Switch(statement) => {
            module_path_in_expression_at_offset(&statement.expression, offset)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
                .or_else(|| {
                    statement
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
        }
        Stmt::ForRange(statement) => module_path_in_expression_at_offset(&statement.start, offset)
            .or_else(|| module_path_in_expression_at_offset(&statement.end, offset))
            .or_else(|| module_path_in_block_at_offset(&statement.body, offset)),
        Stmt::While(statement) => module_path_in_expression_at_offset(&statement.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&statement.body, offset)),
        Stmt::Loop(statement) => module_path_in_block_at_offset(&statement.body, offset),
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
        Stmt::Expression(statement) => {
            module_path_in_expression_at_offset(&statement.expression, offset)
        }
    }
}

fn module_path_in_expression_at_offset(expression: &Expr, offset: usize) -> Option<&ModulePath> {
    match expression {
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                InterpolatedStringPart::Expression(part) => {
                    module_path_in_expression_at_offset(&part.expression, offset)
                }
                InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| module_path_in_expression_at_offset(element, offset)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| module_path_in_expression_at_offset(&field.value, offset)),
        Expr::Propagate(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Force(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Catch(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.catch_block, offset))
        }
        Expr::Borrow(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Unary(expression) => module_path_in_expression_at_offset(&expression.operand, offset),
        Expr::Binary(expression) => module_path_in_expression_at_offset(&expression.left, offset)
            .or_else(|| module_path_in_expression_at_offset(&expression.right, offset)),
        Expr::TypeConversion(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Call(expression) => module_path_in_expression_at_offset(&expression.callee, offset)
            .or_else(|| {
                expression
                    .arguments
                    .iter()
                    .find_map(|argument| module_path_in_expression_at_offset(argument, offset))
            }),
        Expr::Member(expression) => module_path_in_expression_at_offset(&expression.object, offset),
        Expr::Index(expression) => module_path_in_expression_at_offset(&expression.object, offset)
            .or_else(|| module_path_in_expression_at_offset(&expression.index, offset)),
        Expr::Group(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Otherwise(expression) => {
            module_path_in_expression_at_offset(&expression.value, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.fallback, offset))
        }
        Expr::If(expression) => module_path_in_expression_at_offset(&expression.condition, offset)
            .or_else(|| module_path_in_block_at_offset(&expression.then_block, offset))
            .or_else(|| {
                expression
                    .else_block
                    .as_ref()
                    .and_then(|block| module_path_in_block_at_offset(block, offset))
            }),
        Expr::IfIs(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| module_path_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| module_path_in_block_at_offset(block, offset))
                })
        }
        Expr::Match(expression) => {
            module_path_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
                .or_else(|| {
                    expression
                        .wildcard_arm
                        .as_ref()
                        .and_then(|arm| module_path_in_block_at_offset(&arm.body, offset))
                })
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn path_if_at_offset(path: &ModulePath, offset: usize) -> Option<&ModulePath> {
    span_contains(path.span, offset).then_some(path)
}

fn module_path_hover_for_ast(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let path = module_path_at_offset(&file.ast, offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;
    let imported_source = sources.get(imported_file.ast.span.source)?;
    let docs = attach_documentation(imported_file.ast.span.source, imported_source.text(), &[]);

    Some(HoverInfo {
        span: path.span,
        label: format!("module {}", path.value),
        documentation: docs.file().map(str::to_string),
    })
}

fn call_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let (span, label) = file.typecheck_facts.call_hover_at_offset(offset)?;
    Some(HoverInfo {
        span,
        label: label.to_string(),
        documentation: call_documentation(sources, analysis, file, span),
    })
}

fn call_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    call_span: ByteSpan,
) -> Option<String> {
    let target_span = file
        .typecheck_facts
        .function_call_target(call_span)
        .or_else(|| file.typecheck_facts.method_call_target(call_span))
        .or_else(|| file.typecheck_facts.associated_function_target(call_span))?;
    target_documentation(sources, analysis, target_span)
}

fn target_documentation(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    target_span: ByteSpan,
) -> Option<String> {
    let target_file = analysis.file_by_source(target_span.source)?;
    let target_source = sources.get(target_file.ast.span.source)?;
    let text = target_source.text();
    let symbols = hover_symbols_for_file_analysis(text, target_file);
    let documentation =
        documentation_for_hover_symbols(target_file.ast.span.source, text, &symbols);
    documentation_for_target_span(&documentation, &symbols, target_span)
}

fn type_reference_hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<HoverInfo> {
    let reference = file.typecheck_facts.type_reference_at_offset(offset)?;
    let declaration_span = reference.symbol_declaration_span?;
    let symbol = type_symbol_for_declaration_span(analysis, declaration_span)?;
    let (label, documentation) = resolved_symbol_hover_contents(sources, analysis, symbol)
        .unwrap_or_else(|| {
            (
                symbol_hover_label_for_sources(sources, symbol),
                None::<String>,
            )
        });

    Some(HoverInfo {
        span: reference.span,
        label,
        documentation,
    })
}

fn type_reference_hover_for_ast(
    text: &str,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    offset: usize,
) -> Option<HoverInfo> {
    let reference = facts.type_reference_at_offset(offset)?;
    let declaration_span = reference.symbol_declaration_span?;
    let symbol = resolved
        .symbols
        .symbols()
        .find(|candidate| is_type_symbol_at_declaration_span(candidate, declaration_span))?;
    let (label, documentation) =
        single_file_symbol_hover_contents(text, symbols, documentation, symbol);

    Some(HoverInfo {
        span: reference.span,
        label,
        documentation,
    })
}

fn type_symbol_for_declaration_span(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> Option<&Symbol> {
    let file = analysis.file_by_source(declaration_span.source)?;
    file.resolved
        .symbols
        .symbols()
        .find(|candidate| is_type_symbol_at_declaration_span(candidate, declaration_span))
}

fn is_type_symbol_at_declaration_span(symbol: &Symbol, declaration_span: ByteSpan) -> bool {
    matches!(symbol.kind, SymbolKind::Type(_)) && symbol.declaration_span == declaration_span
}

fn documentation_for_hover_symbols(
    source: SourceId,
    text: &str,
    symbols: &[HoverSymbol],
) -> crate::comments::AttachedDocumentation {
    let targets = symbols
        .iter()
        .map(|symbol| DocumentationTarget::new(symbol.attach_start, symbol.name_span.start))
        .collect::<Vec<_>>();
    attach_documentation(source, text, &targets)
}

fn documentation_for_target_span(
    documentation: &crate::comments::AttachedDocumentation,
    symbols: &[HoverSymbol],
    target_span: ByteSpan,
) -> Option<String> {
    documentation
        .get(target_span.start)
        .map(str::to_string)
        .or_else(|| {
            symbols
                .iter()
                .find(|symbol| span_contains(symbol.name_span, target_span.start))
                .and_then(|symbol| documentation.get(symbol.name_span.start))
                .map(str::to_string)
        })
}

fn resolved_reference_at_offset(
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let mut candidates = Vec::new();
    if let Some((span, symbol)) = resolved.local_symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::Local(symbol.clone())));
    }
    if let Some((span, symbol)) = resolved.symbol_reference_at_offset(offset) {
        candidates.push((span, ResolvedReference::TopLevel(Box::new(symbol.clone()))));
    }
    candidates.sort_by_key(|(span, _)| (span.len(), span.start));
    candidates.into_iter().next()
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn hover_symbols_for_ast(text: &str, ast: &AstFile) -> Vec<HoverSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.items {
        collect_item_hover_symbols(text, item, &mut symbols);
    }
    symbols
}

fn hover_symbols_for_file_analysis(text: &str, file: &FileAnalysis) -> Vec<HoverSymbol> {
    let mut symbols = hover_symbols_for_ast(text, &file.ast);
    apply_typecheck_hover_facts(text, &file.typecheck_facts, &mut symbols);
    symbols
}

fn apply_typecheck_hover_facts(text: &str, facts: &TypecheckFacts, symbols: &mut [HoverSymbol]) {
    for symbol in symbols {
        if let Some(label) = facts.declaration_hover_label(symbol.name_span) {
            symbol.label = label.to_string();
            continue;
        }

        let Some(ty) = facts.binding_type_label(symbol.name_span) else {
            continue;
        };
        let Some(kind) = binding_hover_label_kind(&symbol.label) else {
            continue;
        };
        let name = source_fragment(text, symbol.name_span);
        symbol.label = format!("{kind} {name}: {ty}");
    }
}

fn binding_hover_label_kind(label: &str) -> Option<&'static str> {
    if label.starts_with("let ") {
        Some("let")
    } else if label.starts_with("var ") {
        Some("var")
    } else if label.starts_with("parameter ") {
        Some("parameter")
    } else {
        None
    }
}

fn collect_item_hover_symbols(text: &str, item: &Item, symbols: &mut Vec<HoverSymbol>) {
    match item {
        Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            push_function_hover_symbol(text, function, symbols);
            collect_parameter_hover_symbols(text, &function.parameters.parameters, symbols);
            collect_block_hover_symbols(text, &function.body, symbols);
        }
        Item::Primitive(primitive) => {
            push_primitive_hover_symbol(text, primitive, symbols);
            collect_parameter_hover_symbols(text, &primitive.parameters.parameters, symbols);
        }
        Item::TypeAlias(alias) => push_hover_symbol(
            text,
            alias.name_span,
            alias.span.start,
            format!(
                "type {} = {}",
                alias.name,
                source_fragment(text, alias.target.span())
            ),
            symbols,
        ),
        Item::Struct(struct_) => collect_struct_hover_symbols(text, struct_, symbols),
        Item::Enum(enum_) => collect_enum_hover_symbols(text, enum_, symbols),
        Item::Interface(interface) => collect_interface_hover_symbols(text, interface, symbols),
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::Method(method) => {
                        collect_method_hover_symbols(text, method, symbols)
                    }
                    ImplMember::Drop(drop_) => collect_drop_hover_symbols(text, drop_, symbols),
                }
            }
        }
    }
}

fn collect_struct_hover_symbols(text: &str, struct_: &StructDecl, symbols: &mut Vec<HoverSymbol>) {
    let copy_prefix = if struct_.is_copy { "copy " } else { "" };
    push_hover_symbol(
        text,
        struct_.name_span,
        struct_.span.start,
        format!("{copy_prefix}struct {}", struct_.name),
        symbols,
    );
    for field in &struct_.fields {
        push_struct_field_hover_symbol(text, field, symbols);
    }
}

fn collect_enum_hover_symbols(text: &str, enum_: &EnumDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        enum_.name_span,
        enum_.span.start,
        format!("enum {}", enum_.name),
        symbols,
    );
    for variant in &enum_.variants {
        let payload = if variant.payload.is_empty() {
            String::new()
        } else {
            format!("({})", parameters_label(text, &variant.payload))
        };
        push_hover_symbol(
            text,
            variant.name_span,
            variant.span.start,
            format!("variant {}{}", variant.name, payload),
            symbols,
        );
        collect_parameter_hover_symbols(text, &variant.payload, symbols);
    }
}

fn collect_interface_hover_symbols(
    text: &str,
    interface: &InterfaceDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        interface.name_span,
        interface.span.start,
        format!("interface {}", interface.name),
        symbols,
    );
    for method in &interface.methods {
        collect_method_hover_symbols(text, method, symbols);
    }
}

fn collect_drop_hover_symbols(
    text: &str,
    drop_: &crate::ast::DropDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        drop_.binding.name_span,
        drop_.span.start,
        function_like_header(text, drop_.span, Some(drop_.body.span.start)),
        symbols,
    );
    collect_parameter_hover_symbols(text, std::slice::from_ref(&drop_.binding), symbols);
    collect_block_hover_symbols(text, &drop_.body, symbols);
}

fn collect_method_hover_symbols(text: &str, method: &MethodDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        method.name_span,
        method.span.start,
        function_like_header(
            text,
            method.span,
            method.body.as_ref().map(|body| body.span.start),
        ),
        symbols,
    );
    collect_parameter_hover_symbols(text, std::slice::from_ref(&method.receiver), symbols);
    collect_parameter_hover_symbols(text, &method.parameters.parameters, symbols);
    if let Some(body) = &method.body {
        collect_block_hover_symbols(text, body, symbols);
    }
}

fn push_function_hover_symbol(text: &str, function: &FunctionDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        function.name_span,
        function.span.start,
        function_like_header(text, function.span, Some(function.body.span.start)),
        symbols,
    );
}

fn push_primitive_hover_symbol(
    text: &str,
    primitive: &PrimitiveDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        primitive.name_span,
        primitive.span.start,
        function_like_header(text, primitive.span, None),
        symbols,
    );
}

fn push_struct_field_hover_symbol(text: &str, field: &StructField, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        field.name_span,
        field.span.start,
        format!(
            "field {}: {}",
            field.name,
            source_fragment(text, field.ty.span())
        ),
        symbols,
    );
}

fn collect_parameter_hover_symbols(
    text: &str,
    parameters: &[Parameter],
    symbols: &mut Vec<HoverSymbol>,
) {
    for parameter in parameters {
        push_hover_symbol_with_attach_start(
            parameter.name_span,
            parameter.span.start,
            format!(
                "parameter {}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            ),
            symbols,
        );
    }
}

fn collect_block_hover_symbols(text: &str, block: &Block, symbols: &mut Vec<HoverSymbol>) {
    for statement in &block.statements {
        collect_statement_hover_symbols(text, statement, symbols);
    }
    if let Some(result) = &block.result {
        collect_expression_hover_symbols(text, result, symbols);
    }
}

fn collect_statement_hover_symbols(text: &str, statement: &Stmt, symbols: &mut Vec<HoverSymbol>) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_hover_symbols(text, expression, symbols);
            }
        }
        Stmt::Binding(statement) => {
            push_binding_hover_symbol(text, statement, symbols);
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
        }
        Stmt::Assignment(statement) => {
            collect_expression_hover_symbols(text, &statement.target, symbols);
            collect_expression_hover_symbols(text, &statement.value, symbols);
        }
        Stmt::If(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
            if let Some(payload) = statement
                .payload
                .as_ref()
                .and_then(|payload| payload.binding())
            {
                push_hover_symbol(
                    text,
                    payload.span,
                    statement.span.start,
                    format!("payload {}", payload.name),
                    symbols,
                );
            }
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
            for arm in &statement.arms {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
        }
        Stmt::ForRange(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("let {}", statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.start, symbols);
            collect_expression_hover_symbols(text, &statement.end, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::While(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::Loop(statement) => collect_block_hover_symbols(text, &statement.body, symbols),
        Stmt::Drop(_) => {}
        Stmt::Expression(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn push_binding_hover_symbol(text: &str, statement: &BindingStmt, symbols: &mut Vec<HoverSymbol>) {
    let ty = statement
        .ty
        .as_ref()
        .map(|ty| format!(": {}", source_fragment(text, ty.span())))
        .unwrap_or_default();
    push_hover_symbol(
        text,
        statement.name_span,
        statement.span.start,
        format!(
            "{} {}{}",
            binding_kind_label(statement.kind),
            statement.name,
            ty
        ),
        symbols,
    );
}

fn collect_expression_hover_symbols(text: &str, expression: &Expr, symbols: &mut Vec<HoverSymbol>) {
    match expression {
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_hover_symbols(text, element, symbols);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_hover_symbols(text, &field.value, symbols);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Force(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Catch(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            collect_block_hover_symbols(text, &expression.catch_block, symbols);
        }
        Expr::Borrow(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Unary(expression) => {
            collect_expression_hover_symbols(text, &expression.operand, symbols)
        }
        Expr::Binary(expression) => {
            collect_expression_hover_symbols(text, &expression.left, symbols);
            collect_expression_hover_symbols(text, &expression.right, symbols);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Call(expression) => {
            collect_expression_hover_symbols(text, &expression.callee, symbols);
            for argument in &expression.arguments {
                collect_expression_hover_symbols(text, argument, symbols);
            }
        }
        Expr::Member(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols)
        }
        Expr::Index(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols);
            collect_expression_hover_symbols(text, &expression.index, symbols);
        }
        Expr::Group(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_hover_symbols(text, &part.expression, symbols);
                }
            }
        }
        Expr::Otherwise(expression) => {
            collect_expression_hover_symbols(text, &expression.value, symbols);
            collect_block_hover_symbols(text, &expression.fallback, symbols);
        }
        Expr::If(expression) => {
            collect_expression_hover_symbols(text, &expression.condition, symbols);
            collect_block_hover_symbols(text, &expression.then_block, symbols);
            if let Some(block) = &expression.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            if let Some(payload) = expression
                .payload
                .as_ref()
                .and_then(|payload| payload.binding())
            {
                push_hover_symbol(
                    text,
                    payload.span,
                    expression.span.start,
                    format!("payload {}", payload.name),
                    symbols,
                );
            }
            collect_block_hover_symbols(text, &expression.then_block, symbols);
            if let Some(block) = &expression.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Expr::Match(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            for arm in &expression.arms {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_block_hover_symbols(text, &arm.body, symbols);
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

fn push_hover_symbol(
    text: &str,
    name_span: ByteSpan,
    declaration_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol_with_attach_start(
        name_span,
        declaration_line_start(text, declaration_start),
        label,
        symbols,
    );
}

fn push_hover_symbol_with_attach_start(
    name_span: ByteSpan,
    attach_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    symbols.push(HoverSymbol {
        name_span,
        attach_start,
        label,
    });
}

fn function_like_header(text: &str, span: ByteSpan, body_start: Option<usize>) -> String {
    let end = body_start.unwrap_or(span.end).min(span.end);
    source_fragment(text, ByteSpan::new(span.source, span.start, end))
        .trim_end_matches('{')
        .trim()
        .to_string()
}

fn parameters_label(text: &str, parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn binding_kind_label(kind: crate::ast::BindingKind) -> &'static str {
    match kind {
        crate::ast::BindingKind::Let => "let",
        crate::ast::BindingKind::Var => "var",
    }
}

fn single_file_resolved_reference_hover_contents(
    text: &str,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            single_file_symbol_hover_contents(text, symbols, documentation, symbol)
        }
        ResolvedReference::Local(symbol) => {
            local_symbol_hover_contents(symbols, documentation, symbol)
        }
    }
}

fn single_file_symbol_hover_contents(
    text: &str,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    symbol: &Symbol,
) -> (String, Option<String>) {
    let referenced = symbols
        .iter()
        .find(|candidate| candidate.name_span == symbol.name_span);
    let label = referenced
        .map(|symbol| symbol.label.clone())
        .unwrap_or_else(|| symbol_hover_label(text, symbol));
    let docs = referenced
        .and_then(|symbol| documentation.get(symbol.name_span.start))
        .map(str::to_string);
    (label, docs)
}

fn resolved_reference_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            resolved_symbol_hover_contents(sources, analysis, symbol).unwrap_or_else(|| {
                (
                    symbol_hover_label_for_sources(sources, symbol),
                    None::<String>,
                )
            })
        }
        ResolvedReference::Local(symbol) => {
            resolved_local_symbol_hover_contents(sources, analysis, symbol)
                .unwrap_or_else(|| (local_symbol_hover_label(symbol), None))
        }
    }
}

fn local_symbol_hover_contents(
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    symbol: &LocalSymbol,
) -> (String, Option<String>) {
    let referenced = symbols
        .iter()
        .find(|candidate| candidate.name_span == symbol.name_span);
    let label = referenced
        .map(|symbol| symbol.label.clone())
        .unwrap_or_else(|| local_symbol_hover_label(symbol));
    let docs = referenced
        .and_then(|symbol| documentation.get(symbol.name_span.start))
        .map(str::to_string);

    (label, docs)
}

fn resolved_local_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &LocalSymbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.name_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    Some(local_symbol_hover_contents(
        &symbols,
        &documentation,
        symbol,
    ))
}

fn local_symbol_hover_label(symbol: &LocalSymbol) -> String {
    match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter {}", symbol.name),
        LocalSymbolKind::Binding(kind) => format!("{} {}", binding_kind_label(kind), symbol.name),
        LocalSymbolKind::PatternPayload => format!("payload {}", symbol.name),
        LocalSymbolKind::CatchError => format!("catch {}", symbol.name),
        LocalSymbolKind::ForRange => format!("for {}", symbol.name),
    }
}

fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let file = analysis.file_by_source(symbol.declaration_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_file_analysis(text, file);
    let hover_symbol = symbols
        .iter()
        .find(|candidate| candidate.name_span == symbol.declaration_span)
        .or_else(|| {
            symbols
                .iter()
                .find(|candidate| candidate.name_span == symbol.name_span)
        })?;
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);
    let docs = documentation
        .get(hover_symbol.name_span.start)
        .map(str::to_string);

    Some((hover_symbol.label.clone(), docs))
}

fn symbol_hover_label(text: &str, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => format!(
            "{} {}({}): {}",
            if matches!(&symbol.kind, SymbolKind::Primitive(_)) {
                "primitive"
            } else {
                "func"
            },
            symbol.name,
            parameter_signatures_label(text, &signature.parameters),
            source_fragment(text, signature.return_type.span())
        ),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => type_symbol
                .alias_target
                .as_ref()
                .map(|target| {
                    format!(
                        "type {} = {}",
                        symbol.name,
                        source_fragment(text, target.span())
                    )
                })
                .unwrap_or_else(|| format!("type {}", symbol.name)),
            TypeSymbolKind::Struct => format!("struct {}", symbol.name),
            TypeSymbolKind::Enum => format!("enum {}", symbol.name),
            TypeSymbolKind::Interface => format!("interface {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn symbol_hover_label_for_sources(sources: &SourceMap, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => format!(
            "{} {}({}): {}",
            if matches!(&symbol.kind, SymbolKind::Primitive(_)) {
                "primitive"
            } else {
                "func"
            },
            symbol.name,
            parameter_signatures_label_for_sources(sources, &signature.parameters),
            source_fragment_from_sources(sources, signature.return_type.span())
        ),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => type_symbol
                .alias_target
                .as_ref()
                .map(|target| {
                    format!(
                        "type {} = {}",
                        symbol.name,
                        source_fragment_from_sources(sources, target.span())
                    )
                })
                .unwrap_or_else(|| format!("type {}", symbol.name)),
            TypeSymbolKind::Struct => format!("struct {}", symbol.name),
            TypeSymbolKind::Enum => format!("enum {}", symbol.name),
            TypeSymbolKind::Interface => format!("interface {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn parameter_signatures_label_for_sources(
    sources: &SourceMap,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment_from_sources(sources, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn source_fragment_from_sources(sources: &SourceMap, span: ByteSpan) -> String {
    sources
        .get(span.source)
        .map(|source| source_fragment(source.text(), span).to_string())
        .unwrap_or_default()
}

fn parameter_signatures_label(
    text: &str,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    let mut start = line_start;
    while start < node_start && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }

    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::{analyze_namespace_import_text, analyze_text};

    #[test]
    fn workspace_hover_uses_typecheck_facts_and_documentation() {
        let text = "func main(): i32 {\n    /// Exit code.\n    var code = 0\n    return code\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("return code").expect("expected reference") + "return ".len();

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "var code: i32");
        assert_eq!(hover.documentation.as_deref(), Some("Exit code."));
    }

    #[test]
    fn workspace_hover_uses_normalized_typecheck_facts_for_function_reference() {
        let text = "type Exit = i32\n\nfunc answer(value: Exit): Exit {\n    return value\n}\n\nfunc main(): i32 {\n    return answer(1)\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("answer(1)").expect("expected reference");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "func answer(value: i32): i32");
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_namespace_imported_function_member_call() {
        let root_text = "use lib/math\n\nfunc main(): i32 {\n    return math.answer()\n}\n";
        let module_text = "/// Computes an answer.\npub func answer(): i32 {\n    return 7\n}\n";
        let (sources, analysis) = analyze_namespace_import_text(root_text, module_text);
        let file = analysis.root_file().expect("expected root file");
        let offset = root_text.find("answer()").expect("expected namespace call");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "func answer(): i32");
        assert_eq!(hover.documentation.as_deref(), Some("Computes an answer."));
    }

    #[test]
    fn workspace_hover_uses_normalized_typecheck_facts_for_method_call() {
        let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n\nimpl File {\n    /// Reads a count.\n    method self.read(amount: Count): Count {\n        return amount\n    }\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.read(1)\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("read(1)").expect("expected method call");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "method self.read(amount: i32): i32");
        assert_eq!(hover.documentation.as_deref(), Some("Reads a count."));
    }

    #[test]
    fn workspace_hover_uses_normalized_typecheck_facts_for_associated_function_call() {
        let text = "struct File {\n    fd: i32\n}\n\n/// Opens a file.\nfunc File.open(): Self {\n    return Self { fd: 1 }\n}\n\nfunc main(): i32 {\n    return File.open().fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("open()").expect("expected associated call");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "func File.open(): File");
        assert_eq!(hover.documentation.as_deref(), Some("Opens a file."));
    }

    #[test]
    fn workspace_hover_uses_normalized_typecheck_facts_for_struct_field() {
        let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("fd:").expect("expected field");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "field fd: i32");
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_struct_field_reference() {
        let text = "type Count = i32\n\nstruct File {\n    fd: Count\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.fd\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("fd").expect("expected field reference");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "field File.fd: i32");
    }

    #[test]
    fn workspace_hover_uses_normalized_typecheck_facts_for_enum_variant() {
        let text = "type Count = i32\n\nenum Event {\n    count(value: Count)\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("count(value").expect("expected variant");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "variant count(value: i32)");
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_enum_variant_reference() {
        let text = "type Count = i32\n\nenum Event {\n    ready\n    count(value: Count)\n}\n\nfunc main(): i32 {\n    let event = Event.count(1)\n    return 0\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("count(1)").expect("expected variant reference");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "variant Event.count(value: i32)");
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_enum_pattern_variant_reference() {
        let text = r#"enum Choice {
    /// Selected hit.
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text
            .find("hit(_)")
            .expect("expected pattern variant reference");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "variant Choice.hit(value: i32)");
        assert_eq!(hover.documentation.as_deref(), Some("Selected hit."));
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_payloadless_enum_variant_reference() {
        let text = "enum Event {\n    /// Ready to run.\n    ready\n}\n\nfunc main(): i32 {\n    let event = Event.ready\n    return 0\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("ready").expect("expected variant reference");

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "variant Event.ready");
        assert_eq!(hover.documentation.as_deref(), Some("Ready to run."));
    }

    #[test]
    fn workspace_hover_uses_typecheck_facts_for_type_reference() {
        let text = "/// Request header.\nstruct Header {\n    code: i32\n}\n\nfunc inspect(value: Header): i32 {\n    return value.code\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("value: Header").expect("expected type reference") + "value: ".len();

        let hover = hover_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected hover info");

        assert_eq!(hover.label, "struct Header");
        assert_eq!(hover.documentation.as_deref(), Some("Request header."));
    }
}
