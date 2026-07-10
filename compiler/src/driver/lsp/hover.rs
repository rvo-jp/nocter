use super::documents::OpenDocument;
use super::protocol::{
    LspRange, byte_offset_to_lsp_position, lsp_position_to_byte_offset, position_from_params,
    range_for_byte_span,
};
use super::semantic::{SEMANTIC_DECLARATION_MODIFIER, classified_identifiers};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, BindingStmt, Block, EnumDecl, Expr, FunctionDecl, ImplMember, InterpolatedStringPart,
    Item, MethodDecl, Parameter, PrimitiveDecl, Stmt, StructDecl, StructField, TraitDecl,
};
use crate::comments::{DocumentationTarget, attach_documentation};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{
    LocalSymbol, LocalSymbolKind, ResolveOutput, Symbol, SymbolKind, TypeSymbolKind, resolve,
};
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

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
    pub(super) fn declaration_span(&self) -> ByteSpan {
        match self {
            ResolvedReference::TopLevel(symbol) => symbol.declaration_span,
            ResolvedReference::Local(symbol) => symbol.name_span,
        }
    }
}

pub(super) fn hover_for_document(document: &OpenDocument, params: Option<&Value>) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    if let Some(hover) = documented_hover_for_document(document, offset) {
        return Some(hover);
    }

    let identifier = classified_identifiers(document)
        .into_iter()
        .find(|identifier| identifier.start_byte <= offset && offset < identifier.end_byte)?;
    let lexeme = document
        .text
        .get(identifier.start_byte..identifier.end_byte)?;
    let range = LspRange {
        start: byte_offset_to_lsp_position(&document.text, identifier.start_byte),
        end: byte_offset_to_lsp_position(&document.text, identifier.end_byte),
    };
    let declaration = if identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0 {
        " declaration"
    } else {
        ""
    };

    Some(json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```nocter\n{}{} {}\n```", identifier.kind.hover_label(), declaration, lexeme)
        },
        "range": range
    }))
}

pub(super) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_ast(text, &file.ast);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        let docs = documentation.get(symbol.name_span.start);
        return Some(json!({
            "contents": {
                "kind": "markdown",
                "value": hover_markdown(&symbol.label, docs)
            },
            "range": range_for_byte_span(text, symbol.name_span)
        }));
    }

    find_resolved_hover_symbol(&file.ast, &file.resolved, offset).map(|(name_span, reference)| {
        let (label, docs) = resolved_reference_hover_contents(sources, analysis, &reference);

        json!({
            "contents": {
                "kind": "markdown",
                "value": hover_markdown(&label, docs.as_deref())
            },
            "range": range_for_byte_span(text, name_span)
        })
    })
}

pub(super) fn definition_span_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    let symbols = hover_symbols_for_ast(text, ast);
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        return Some(symbol.name_span);
    }

    find_resolved_hover_symbol(ast, resolved, offset)
        .map(|(_, reference)| reference.declaration_span())
}

pub(super) fn resolve_single_file_for_hover(
    text: &str,
    source: crate::source::SourceId,
    ast: &AstFile,
) -> ResolveOutput {
    let mut sources = SourceMap::new();
    let hover_source = sources.add_source("hover.nct", None, text.to_string());
    debug_assert_eq!(hover_source.raw(), source.raw());
    resolve(&sources, ast)
}

fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

fn documented_hover_for_document(document: &OpenDocument, offset: usize) -> Option<Value> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let symbols = hover_symbols_for_ast(&document.text, &ast);
    let documentation = documentation_for_hover_symbols(source, &document.text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        let docs = documentation.get(symbol.name_span.start);
        let value = hover_markdown(&symbol.label, docs);

        return Some(json!({
            "contents": {
                "kind": "markdown",
                "value": value
            },
            "range": range_for_byte_span(&document.text, symbol.name_span)
        }));
    }

    resolved_reference_hover_for_ast(&document.text, source, &ast, offset).map(
        |(name_span, reference)| {
            let (label, docs) = single_file_resolved_reference_hover_contents(
                &document.text,
                &symbols,
                &documentation,
                &reference,
            );

            json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover_markdown(&label, docs.as_deref())
                },
                "range": range_for_byte_span(&document.text, name_span)
            })
        },
    )
}

fn documentation_for_hover_symbols(
    source: crate::source::SourceId,
    text: &str,
    symbols: &[HoverSymbol],
) -> crate::comments::AttachedDocumentation {
    let targets = symbols
        .iter()
        .map(|symbol| DocumentationTarget::new(symbol.attach_start, symbol.name_span.start))
        .collect::<Vec<_>>();
    attach_documentation(source, text, &targets)
}

fn resolved_reference_hover_for_ast(
    text: &str,
    source: crate::source::SourceId,
    ast: &AstFile,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let resolved = resolve_single_file_for_hover(text, source, ast);
    find_resolved_hover_symbol(ast, &resolved, offset)
}

fn find_resolved_hover_symbol(
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let mut candidates = Vec::new();
    for item in &ast.items {
        collect_item_resolved_hover_symbols(item, resolved, offset, &mut candidates);
    }
    candidates.sort_by_key(|(span, _)| (span.end - span.start, span.start));
    candidates.into_iter().next()
}

fn collect_item_resolved_hover_symbols(
    item: &Item,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match item {
        Item::Function(function) => {
            collect_block_resolved_hover_symbols(&function.body, resolved, offset, candidates);
        }
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::Function(function) => {
                        collect_block_resolved_hover_symbols(
                            &function.body,
                            resolved,
                            offset,
                            candidates,
                        );
                    }
                    ImplMember::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_block_resolved_hover_symbols(
                                body, resolved, offset, candidates,
                            );
                        }
                    }
                }
            }
        }
        Item::Use(_)
        | Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Trait(_) => {}
    }
}

fn collect_block_resolved_hover_symbols(
    block: &Block,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    for statement in &block.statements {
        collect_statement_resolved_hover_symbols(statement, resolved, offset, candidates);
    }
}

fn collect_statement_resolved_hover_symbols(
    statement: &Stmt,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_resolved_hover_symbols(expression, resolved, offset, candidates);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::Assignment(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.target,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &statement.value,
                resolved,
                offset,
                candidates,
            );
        }
        Stmt::If(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.condition,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::IfLet(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
            for arm in &statement.arms {
                collect_block_resolved_hover_symbols(&arm.body, resolved, offset, candidates);
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_resolved_hover_symbols(&arm.body, resolved, offset, candidates);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.start,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(&statement.end, resolved, offset, candidates);
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::While(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.condition,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::WhileLet(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::Loop(statement) => {
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::Expression(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expression_resolved_hover_symbols(
    expression: &Expr,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match expression {
        Expr::Identifier(expression) => {
            if span_contains(expression.span, offset) {
                if let Some(symbol) = resolved.local_symbol_for_identifier(expression) {
                    candidates.push((expression.span, ResolvedReference::Local(symbol.clone())));
                } else if let Some(symbol) = resolved.symbol_for_identifier(expression) {
                    candidates.push((
                        expression.span,
                        ResolvedReference::TopLevel(Box::new(symbol.clone())),
                    ));
                }
            }
        }
        Expr::Call(expression) => {
            if let Expr::Identifier(callee) = expression.callee.as_ref()
                && span_contains(callee.span, offset)
                && let Some(symbol) = resolved.symbol_for_call(expression)
            {
                candidates.push((
                    callee.span,
                    ResolvedReference::TopLevel(Box::new(symbol.clone())),
                ));
            }
            collect_expression_resolved_hover_symbols(
                &expression.callee,
                resolved,
                offset,
                candidates,
            );
            for argument in &expression.arguments {
                collect_expression_resolved_hover_symbols(argument, resolved, offset, candidates);
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_resolved_hover_symbols(element, resolved, offset, candidates);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_resolved_hover_symbols(
                    &field.value,
                    resolved,
                    offset,
                    candidates,
                );
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Force(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Catch(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &expression.catch_block,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Unary(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.operand,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Binary(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.left,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.right,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::TypeConversion(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Member(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.object,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Index(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.object,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.index,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Group(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_resolved_hover_symbols(
                        &part.expression,
                        resolved,
                        offset,
                        candidates,
                    );
                }
            }
        }
        Expr::OptionalDefault(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.value,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.default,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::PatternConditional(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.target,
                resolved,
                offset,
                candidates,
            );
            for arm in &expression.arms {
                collect_expression_resolved_hover_symbols(
                    &arm.expression,
                    resolved,
                    offset,
                    candidates,
                );
            }
            collect_expression_resolved_hover_symbols(
                &expression.fallback,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
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

fn collect_item_hover_symbols(text: &str, item: &Item, symbols: &mut Vec<HoverSymbol>) {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => {}
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
        Item::Trait(trait_) => collect_trait_hover_symbols(text, trait_, symbols),
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::Function(function) => {
                        push_function_hover_symbol(text, function, symbols);
                        collect_parameter_hover_symbols(
                            text,
                            &function.parameters.parameters,
                            symbols,
                        );
                        collect_block_hover_symbols(text, &function.body, symbols);
                    }
                    ImplMember::Method(method) => {
                        collect_method_hover_symbols(text, method, symbols)
                    }
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

fn collect_trait_hover_symbols(text: &str, trait_: &TraitDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        trait_.name_span,
        trait_.span.start,
        format!("trait {}", trait_.name),
        symbols,
    );
    for method in &trait_.methods {
        collect_method_hover_symbols(text, method, symbols);
    }
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
        push_hover_symbol(
            text,
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
}

fn collect_statement_hover_symbols(text: &str, statement: &Stmt, symbols: &mut Vec<HoverSymbol>) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_hover_symbols(text, expression, symbols);
            }
        }
        Stmt::Binding(statement) => {
            push_binding_hover_symbol(text, statement, symbols);
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
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
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::IfLet(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("{} {}", binding_kind_label(statement.kind), statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
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
            if let Some(arm) = &statement.else_arm {
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
        Stmt::WhileLet(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("{} {}", binding_kind_label(statement.kind), statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::Loop(statement) => collect_block_hover_symbols(text, &statement.body, symbols),
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
        Expr::OptionalDefault(expression) => {
            collect_expression_hover_symbols(text, &expression.value, symbols);
            collect_expression_hover_symbols(text, &expression.default, symbols);
        }
        Expr::PatternConditional(expression) => {
            collect_expression_hover_symbols(text, &expression.target, symbols);
            for arm in &expression.arms {
                collect_expression_hover_symbols(text, &arm.expression, symbols);
            }
            collect_expression_hover_symbols(text, &expression.fallback, symbols);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
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
    symbols.push(HoverSymbol {
        name_span,
        attach_start: declaration_line_start(text, declaration_start),
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

fn hover_markdown(label: &str, documentation: Option<&str>) -> String {
    let mut value = format!("```nocter\n{label}\n```");
    if let Some(documentation) = documentation
        && !documentation.trim().is_empty()
    {
        value.push_str("\n\n");
        value.push_str(documentation.trim());
    }
    value
}

fn symbol_hover_label(text: &str, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => format!(
            "func {}({}): {}",
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
            TypeSymbolKind::Trait => format!("trait {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
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
        ResolvedReference::Local(symbol) => (local_symbol_hover_label(symbol), None),
    }
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
        ResolvedReference::Local(symbol) => (local_symbol_hover_label(symbol), None),
    }
}

fn local_symbol_hover_label(symbol: &LocalSymbol) -> String {
    match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter {}", symbol.name),
        LocalSymbolKind::Binding(kind) => {
            format!("{} {}", binding_kind_label(kind), symbol.name)
        }
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
    let file = analysis
        .files
        .iter()
        .find(|file| file.ast.span.source == symbol.declaration_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_ast(text, &file.ast);
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

fn symbol_hover_label_for_sources(sources: &SourceMap, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => format!(
            "func {}({}): {}",
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
            TypeSymbolKind::Trait => format!("trait {}", symbol.name),
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
