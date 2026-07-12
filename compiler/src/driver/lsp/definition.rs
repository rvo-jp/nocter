use super::documents::OpenDocument;
use super::hover::{definition_span_for_ast, module_path_at_offset, resolve_single_file_for_hover};
use super::protocol::{lsp_position_to_byte_offset, position_from_params, range_for_byte_span};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    AstFile, Block, Expr, GenericParamList, ImplMember, Item, Parameter, Stmt, TypeExpr,
};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};

pub(super) fn definition_for_document(
    document: &OpenDocument,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
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
    let resolved = resolve_single_file_for_hover(&document.text, source, &ast);
    type_definition_span_for_ast(&ast, &resolved, offset)
        .or_else(|| definition_span_for_ast(&document.text, &ast, &resolved, offset))
        .and_then(|span| location_for_byte_span(&sources, span))
}

pub(super) fn definition_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let text = sources.get(file.ast.span.source)?.text();
    module_path_definition_location(sources, analysis, file, offset)
        .or_else(|| {
            type_definition_span_for_file_analysis(analysis, file, offset)
                .and_then(|span| location_for_byte_span(sources, span))
        })
        .or_else(|| {
            definition_span_for_ast(text, &file.ast, &file.resolved, offset)
                .and_then(|span| location_for_byte_span(sources, span))
        })
}

fn module_path_definition_location(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let path = module_path_at_offset(&file.ast, offset)?;
    let import_source = analysis.import_sources.get(&path.span)?;
    let imported_file = analysis.file_by_source(import_source.source)?;
    let span = ByteSpan::new(imported_file.ast.span.source, 0, 0);

    location_for_byte_span(sources, span)
}

fn type_definition_span_for_ast(
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    let reference = type_reference_at_offset(ast, offset)?;
    let symbol = resolved.symbols.symbol_by_name(&reference.name)?;
    match symbol.kind {
        SymbolKind::Type(_) => Some(symbol.name_span),
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_) => None,
    }
}

fn type_definition_span_for_file_analysis(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    let reference = type_reference_at_offset(&file.ast, offset)?;
    let symbol = file.resolved.symbols.symbol_by_name(&reference.name)?;
    let SymbolKind::Type(type_symbol) = &symbol.kind else {
        return None;
    };

    if symbol.declaration_span.source != file.ast.span.source
        && let Some(declaration_file) = analysis.file_by_source(symbol.declaration_span.source)
        && let Some(name_span) = declaration_file
            .resolved
            .symbols
            .symbols()
            .find_map(|candidate| match &candidate.kind {
                SymbolKind::Type(_) if candidate.declaration_span == symbol.declaration_span => {
                    Some(candidate.name_span)
                }
                SymbolKind::Function(_)
                | SymbolKind::Primitive(_)
                | SymbolKind::Type(_)
                | SymbolKind::Imported(_) => None,
            })
    {
        return Some(name_span);
    }

    analysis
        .files
        .iter()
        .filter(|candidate_file| candidate_file.ast.span.source != file.ast.span.source)
        .flat_map(|candidate_file| candidate_file.resolved.symbols.symbols())
        .find_map(|candidate| match &candidate.kind {
            SymbolKind::Type(candidate_type)
                if candidate_type.canonical_name == type_symbol.canonical_name =>
            {
                Some(candidate.name_span)
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })
        .or(Some(symbol.name_span))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeReferenceAtOffset {
    name: String,
    span: ByteSpan,
}

fn type_reference_at_offset(ast: &AstFile, offset: usize) -> Option<TypeReferenceAtOffset> {
    let mut candidates = Vec::new();
    for item in &ast.items {
        collect_item_type_references(item, offset, &mut candidates);
    }
    candidates.sort_by_key(|candidate| (candidate.span.len(), candidate.span.start));
    candidates.into_iter().next()
}

fn collect_item_type_references(
    item: &Item,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            collect_generic_param_type_references(&function.generics, offset, candidates);
            collect_parameter_type_references(&function.parameters.parameters, offset, candidates);
            collect_type_expr_references(&function.return_type, offset, candidates);
            collect_block_type_references(&function.body, offset, candidates);
        }
        Item::Primitive(primitive) => {
            collect_generic_param_type_references(&primitive.generics, offset, candidates);
            collect_parameter_type_references(&primitive.parameters.parameters, offset, candidates);
            collect_type_expr_references(&primitive.return_type, offset, candidates);
        }
        Item::TypeAlias(alias) => {
            collect_generic_param_type_references(&alias.generics, offset, candidates);
            collect_type_expr_references(&alias.target, offset, candidates);
        }
        Item::Struct(struct_) => {
            collect_generic_param_type_references(&struct_.generics, offset, candidates);
            for field in &struct_.fields {
                collect_type_expr_references(&field.ty, offset, candidates);
            }
        }
        Item::Enum(enum_) => {
            collect_generic_param_type_references(&enum_.generics, offset, candidates);
            for variant in &enum_.variants {
                collect_parameter_type_references(&variant.payload, offset, candidates);
            }
        }
        Item::Trait(trait_) => {
            collect_generic_param_type_references(&trait_.generics, offset, candidates);
            for method in &trait_.methods {
                collect_method_type_references(method, offset, candidates);
            }
        }
        Item::Impl(impl_) => {
            if let Some(trait_ty) = &impl_.trait_ty {
                collect_type_expr_references(trait_ty, offset, candidates);
            }
            collect_type_expr_references(&impl_.target_ty, offset, candidates);
            for member in &impl_.members {
                match member {
                    ImplMember::Function(function) => {
                        collect_generic_param_type_references(
                            &function.generics,
                            offset,
                            candidates,
                        );
                        collect_parameter_type_references(
                            &function.parameters.parameters,
                            offset,
                            candidates,
                        );
                        collect_type_expr_references(&function.return_type, offset, candidates);
                        collect_block_type_references(&function.body, offset, candidates);
                    }
                    ImplMember::Method(method) => {
                        collect_method_type_references(method, offset, candidates);
                    }
                    ImplMember::Drop(drop_) => {
                        collect_parameter_type_references(
                            std::slice::from_ref(&drop_.binding),
                            offset,
                            candidates,
                        );
                        collect_block_type_references(&drop_.body, offset, candidates);
                    }
                }
            }
        }
    }
}

fn collect_method_type_references(
    method: &crate::ast::MethodDecl,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    collect_parameter_type_references(std::slice::from_ref(&method.receiver), offset, candidates);
    collect_parameter_type_references(&method.parameters.parameters, offset, candidates);
    collect_type_expr_references(&method.return_type, offset, candidates);
    if let Some(body) = &method.body {
        collect_block_type_references(body, offset, candidates);
    }
}

fn collect_generic_param_type_references(
    generics: &GenericParamList,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    for parameter in &generics.parameters {
        if let Some(bound) = &parameter.bound {
            collect_type_expr_references(bound, offset, candidates);
        }
    }
}

fn collect_parameter_type_references(
    parameters: &[Parameter],
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    for parameter in parameters {
        collect_type_expr_references(&parameter.ty, offset, candidates);
    }
}

fn collect_type_expr_references(
    ty: &TypeExpr,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    match ty {
        TypeExpr::Reference(ty) => {
            push_named_type_reference(&ty.name, ty.span, offset, candidates);
        }
        TypeExpr::Generic(ty) => {
            push_named_type_reference(&ty.name, ty.name_span, offset, candidates);
            for argument in &ty.arguments {
                collect_type_expr_references(argument, offset, candidates);
            }
        }
        TypeExpr::Pointer(ty) => collect_type_expr_references(&ty.inner, offset, candidates),
        TypeExpr::Borrow(ty) => collect_type_expr_references(&ty.inner, offset, candidates),
        TypeExpr::View(ty) => collect_type_expr_references(&ty.element, offset, candidates),
        TypeExpr::Array(ty) => collect_type_expr_references(&ty.element, offset, candidates),
        TypeExpr::Optional(ty) => collect_type_expr_references(&ty.inner, offset, candidates),
        TypeExpr::Fallible(ty) => {
            collect_type_expr_references(&ty.success, offset, candidates);
            collect_type_expr_references(&ty.error, offset, candidates);
        }
    }
}

fn collect_block_type_references(
    block: &Block,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    for statement in &block.statements {
        collect_statement_type_references(statement, offset, candidates);
    }
}

fn collect_statement_type_references(
    statement: &Stmt,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_type_references(expression, offset, candidates);
            }
        }
        Stmt::Binding(statement) => {
            if let Some(ty) = &statement.ty {
                collect_type_expr_references(ty, offset, candidates);
            }
            collect_expression_type_references(&statement.initializer, offset, candidates);
            if let Some(block) = &statement.else_block {
                collect_block_type_references(block, offset, candidates);
            }
        }
        Stmt::Assignment(statement) => {
            collect_expression_type_references(&statement.target, offset, candidates);
            collect_expression_type_references(&statement.value, offset, candidates);
        }
        Stmt::If(statement) => {
            collect_expression_type_references(&statement.condition, offset, candidates);
            collect_block_type_references(&statement.then_block, offset, candidates);
            if let Some(block) = &statement.else_block {
                collect_block_type_references(block, offset, candidates);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_type_references(&statement.expression, offset, candidates);
            push_named_type_reference(
                &statement.enum_name,
                statement.enum_name_span,
                offset,
                candidates,
            );
            collect_block_type_references(&statement.then_block, offset, candidates);
            if let Some(block) = &statement.else_block {
                collect_block_type_references(block, offset, candidates);
            }
        }
        Stmt::IfLet(statement) => {
            collect_expression_type_references(&statement.initializer, offset, candidates);
            collect_block_type_references(&statement.then_block, offset, candidates);
            if let Some(block) = &statement.else_block {
                collect_block_type_references(block, offset, candidates);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_type_references(&statement.expression, offset, candidates);
            for arm in &statement.arms {
                push_named_type_reference(&arm.enum_name, arm.enum_name_span, offset, candidates);
                collect_block_type_references(&arm.body, offset, candidates);
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_type_references(&arm.body, offset, candidates);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_type_references(&statement.start, offset, candidates);
            collect_expression_type_references(&statement.end, offset, candidates);
            collect_block_type_references(&statement.body, offset, candidates);
        }
        Stmt::While(statement) => {
            collect_expression_type_references(&statement.condition, offset, candidates);
            collect_block_type_references(&statement.body, offset, candidates);
        }
        Stmt::WhileLet(statement) => {
            collect_expression_type_references(&statement.initializer, offset, candidates);
            collect_block_type_references(&statement.body, offset, candidates);
        }
        Stmt::Loop(statement) => collect_block_type_references(&statement.body, offset, candidates),
        Stmt::Expression(statement) => {
            collect_expression_type_references(&statement.expression, offset, candidates);
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expression_type_references(
    expression: &Expr,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    match expression {
        Expr::StructLiteral(expression) => {
            collect_type_expr_references(&expression.ty, offset, candidates);
            for field in &expression.fields {
                collect_expression_type_references(&field.value, offset, candidates);
            }
        }
        Expr::TypeConversion(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
            collect_type_expr_references(&expression.ty, offset, candidates);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_type_references(element, offset, candidates);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
        }
        Expr::Force(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
        }
        Expr::Catch(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
            collect_block_type_references(&expression.catch_block, offset, candidates);
        }
        Expr::Borrow(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
        }
        Expr::Unary(expression) => {
            collect_expression_type_references(&expression.operand, offset, candidates);
        }
        Expr::Binary(expression) => {
            collect_expression_type_references(&expression.left, offset, candidates);
            collect_expression_type_references(&expression.right, offset, candidates);
        }
        Expr::Call(expression) => {
            collect_expression_type_references(&expression.callee, offset, candidates);
            for argument in &expression.arguments {
                collect_expression_type_references(argument, offset, candidates);
            }
        }
        Expr::Member(expression) => {
            collect_expression_type_references(&expression.object, offset, candidates);
        }
        Expr::Index(expression) => {
            collect_expression_type_references(&expression.object, offset, candidates);
            collect_expression_type_references(&expression.index, offset, candidates);
        }
        Expr::Group(expression) => {
            collect_expression_type_references(&expression.expression, offset, candidates);
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_expression_type_references(&part.expression, offset, candidates);
                }
            }
        }
        Expr::OptionalDefault(expression) => {
            collect_expression_type_references(&expression.value, offset, candidates);
            collect_expression_type_references(&expression.default, offset, candidates);
        }
        Expr::PatternConditional(expression) => {
            collect_expression_type_references(&expression.target, offset, candidates);
            for arm in &expression.arms {
                push_named_type_reference(&arm.enum_name, arm.enum_name_span, offset, candidates);
                collect_expression_type_references(&arm.expression, offset, candidates);
            }
            collect_expression_type_references(&expression.fallback, offset, candidates);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn push_named_type_reference(
    name: &str,
    span: ByteSpan,
    offset: usize,
    candidates: &mut Vec<TypeReferenceAtOffset>,
) {
    if span_contains(span, offset) {
        candidates.push(TypeReferenceAtOffset {
            name: name.to_string(),
            span,
        });
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn location_for_byte_span(sources: &SourceMap, span: ByteSpan) -> Option<Value> {
    let source = sources.get(span.source)?;
    Some(json!({
        "uri": uri_for_source_file(source),
        "range": range_for_byte_span(source.text(), span)
    }))
}

fn uri_for_source_file(source: &crate::source::SourceFile) -> String {
    source
        .absolute_path()
        .map(|path| format!("file://{}", percent_encode_path(&path.to_string_lossy())))
        .unwrap_or_else(|| source.display_path().to_string())
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
