use super::allocation::type_is_aborting_allocator_capability;
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::{is_assignable, is_expression_assignable};
use super::type_expr::{
    infer_type_expr_substitutions, type_expr_to_type_in_environment,
    type_expr_to_type_with_substitutions,
};
use crate::ast::{
    AstFile, Block, Expr, Item, LiteralDecl, LiteralShape, Stmt, TypeExpr,
    TypedSequenceLiteralExpr, TypedStringLiteralExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{LiteralSignature, LocalSymbolKind, ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};

pub(super) fn literal_expression_type(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    literal_expression_type_with_expected(expression, None, resolved, environment)
}

pub(super) fn literal_expression_type_with_expected(
    expression: &Expr,
    expected: Option<&Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression {
        Expr::TypedSequenceLiteral(literal) => typed_literal_result_type(
            literal.span,
            &literal.target,
            &literal.elements,
            expected,
            resolved,
            environment,
        ),
        Expr::TypedStringLiteral(literal) => typed_literal_result_type(
            literal.span,
            &literal.target,
            &[],
            expected,
            resolved,
            environment,
        ),
        _ => expression_type(expression, resolved, environment),
    }
}

pub(super) fn check_literal_declarations(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Literal(literal) => {
                check_literal_definition(sources, literal, resolved, diagnostics)
            }
            Item::Construct(construct) => {
                for (_, literal) in construct.literals() {
                    check_literal_definition(sources, literal, resolved, diagnostics);
                }
            }
            _ => {}
        }
    }
}

fn check_literal_definition(
    sources: &SourceMap,
    literal: &LiteralDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if literal.shape == LiteralShape::String {
        let parameter = &literal.parameters.parameters[0];
        let environment = super::environments::environment_for_literal(literal, resolved);
        if type_expr_to_type_in_environment(&parameter.ty, resolved, &environment) != Type::Str {
            diagnostics.push(literal_diagnostic(
                sources,
                "E0520",
                parameter.ty.span(),
                "string literal parameter must have type `&str`",
                "use exactly one `(text: &str)` parameter",
            ));
        }
    }

    let mut pack_loops = HashSet::new();
    check_pack_uses_in_block(
        sources,
        &literal.body,
        resolved,
        &mut pack_loops,
        diagnostics,
    );
}

pub(super) fn check_typed_sequence_literal(
    sources: &SourceMap,
    literal: &TypedSequenceLiteralExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((signature, parameters, result_type)) = literal_signature_and_parameters(
        literal.span,
        &literal.target,
        &literal.elements,
        None,
        resolved,
        environment,
    ) else {
        return;
    };
    let Some(capture) = &signature.capture else {
        return;
    };
    let expected_element = type_expr_to_type_with_substitutions(
        &capture.element_type,
        resolved,
        Some(&result_type),
        &parameters,
    );
    for element in &literal.elements {
        let spread = sequence_spread(element);
        let actual = match spread {
            Some(spread) => {
                match super::iteration::resolve_sequence_spread(spread, resolved, environment) {
                    Ok(resolution) => resolution.pack_item_type,
                    Err(error) => {
                        diagnostics.push(super::iteration::sequence_spread_diagnostic(
                            sources, spread, error,
                        ));
                        continue;
                    }
                }
            }
            None => expression_type(element, resolved, environment),
        };
        if !actual.is_unknown_or_unresolved()
            && !expected_element.is_unknown_or_unresolved()
            && match spread {
                Some(_) => !is_assignable(&expected_element, &actual),
                None => {
                    !is_expression_assignable(&expected_element, element, resolved, environment)
                }
            }
        {
            diagnostics.push(literal_diagnostic(
                sources,
                "E0521",
                element.span(),
                &format!(
                    "typed sequence element has type `{}`, expected `{}`",
                    actual.display(),
                    expected_element.display()
                ),
                "make every element compatible with the literal capture type",
            ));
        }
    }
    check_using_context(
        sources,
        literal.using.as_ref(),
        resolved,
        environment,
        diagnostics,
    );
}

pub(super) fn check_unconstrained_literal_initializer(
    sources: &SourceMap,
    expression: &Expr,
    has_expected_type: bool,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = unwrap_groups(expression);
    let Expr::TypedSequenceLiteral(literal) = expression else {
        return;
    };
    if has_expected_type
        || !literal.elements.is_empty()
        || matches!(literal.target, TypeExpr::Generic(_))
    {
        return;
    }
    let unresolved_target_parameter = resolved
        .literal_resolution(literal.span)
        .and_then(|resolution| resolved.symbols.get(resolution.type_symbol))
        .is_some_and(|symbol| match &symbol.kind {
            SymbolKind::Type(target) => !target.generic_parameters.is_empty(),
            _ => false,
        });
    if unresolved_target_parameter {
        diagnostics.push(literal_diagnostic(
            sources,
            "E0522",
            literal.target.span(),
            "empty typed sequence literal does not determine all generic arguments",
            "write explicit target arguments or provide an expected result type",
        ));
    }
}

pub(super) fn check_literal_pack_for_statement(
    sources: &SourceMap,
    statement: &crate::ast::LiteralPackForStmt,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if environment
        .literal_pack_element(&statement.pack_name)
        .is_none()
    {
        diagnostics.push(literal_diagnostic(
            sources,
            "E0524",
            statement.pack_span,
            "literal-pack `for` source is not a literal capture",
            "use this loop only with the current literal definition's `...items` capture",
        ));
    }
}

fn unwrap_groups(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
}

pub(super) fn check_typed_string_literal(
    sources: &SourceMap,
    literal: &TypedStringLiteralExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_using_context(
        sources,
        literal.using.as_ref(),
        resolved,
        environment,
        diagnostics,
    );
}

fn check_using_context(
    sources: &SourceMap,
    using: Option<&crate::ast::LiteralContextOverride>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(using) = using else {
        return;
    };
    if !super::places::expression_is_established_place(&using.allocator) {
        diagnostics.push(literal_diagnostic(
            sources,
            "E0527",
            using.allocator.span(),
            "typed literal `using` requires an established allocator place",
            "bind the allocator capability first, then pass that binding or one of its fields",
        ));
        return;
    }
    let actual = expression_type(&using.allocator, resolved, environment);
    if actual.is_unknown_or_unresolved() {
        return;
    }
    if !type_is_aborting_allocator_capability(&actual, resolved) {
        diagnostics.push(literal_diagnostic(
            sources,
            "E0523",
            using.allocator.span(),
            &format!(
                "typed literal `using` requires an aborting allocator context, found `{}`",
                actual.display()
            ),
            "use an `Allocator` capability; recoverable `TryAllocator` literals are not implicit",
        ));
    }
}

fn typed_literal_result_type(
    span: ByteSpan,
    target: &TypeExpr,
    elements: &[Expr],
    expected: Option<&Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    literal_signature_and_parameters(span, target, elements, expected, resolved, environment)
        .map(|(_, _, result)| result)
        .unwrap_or(Type::Unknown)
}

fn literal_signature_and_parameters<'a>(
    span: ByteSpan,
    target: &TypeExpr,
    elements: &[Expr],
    expected: Option<&Type>,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<(&'a LiteralSignature, HashMap<String, Type>, Type)> {
    let resolution = resolved.literal_resolution(span)?;
    let signature = resolved.literal_signature(resolution)?;
    let SymbolKind::Type(target_symbol) = &resolved.symbols.get(resolution.type_symbol)?.kind
    else {
        return None;
    };
    let mut substitutions = HashMap::new();
    if let Some(expected) = expected {
        match expected {
            Type::Generic { name, arguments } if name == &target_symbol.canonical_name => {
                substitutions.extend(
                    target_symbol
                        .generic_parameters
                        .iter()
                        .cloned()
                        .zip(arguments.iter().cloned()),
                );
            }
            Type::Named(name) if name == &target_symbol.canonical_name => {}
            _ => {}
        }
    }
    if let TypeExpr::Generic(generic) = target {
        for (parameter, argument) in target_symbol
            .generic_parameters
            .iter()
            .zip(&generic.arguments)
        {
            substitutions.insert(
                parameter.clone(),
                type_expr_to_type_in_environment(argument, resolved, environment),
            );
        }
    }
    if let Some(capture) = &signature.capture {
        let parameters = target_symbol
            .generic_parameters
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for element in elements {
            let actual = sequence_spread(element)
                .and_then(|spread| {
                    super::iteration::resolve_sequence_spread(spread, resolved, environment)
                        .ok()
                        .map(|resolution| resolution.pack_item_type)
                })
                .unwrap_or_else(|| expression_type(element, resolved, environment));
            infer_type_expr_substitutions(
                &capture.element_type,
                &actual,
                resolved,
                None,
                &parameters,
                &mut substitutions,
            );
        }
    }
    let result = if target_symbol.generic_parameters.is_empty() {
        Type::Named(target_symbol.canonical_name.clone())
    } else {
        Type::Generic {
            name: target_symbol.canonical_name.clone(),
            arguments: target_symbol
                .generic_parameters
                .iter()
                .map(|parameter| {
                    substitutions
                        .get(parameter)
                        .cloned()
                        .unwrap_or(Type::Unknown)
                })
                .collect(),
        }
    };
    Some((signature, substitutions, result))
}

pub(crate) fn sequence_spread(expression: &Expr) -> Option<&crate::ast::UnaryExpr> {
    let Expr::Unary(unary) = expression.without_groups() else {
        return None;
    };
    (unary.operator == crate::ast::UnaryOperator::Spread).then_some(unary)
}

fn check_pack_uses_in_block(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    pack_loops: &mut HashSet<crate::resolve::LocalSymbolId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        if let Stmt::LiteralPackFor(loop_) = statement {
            let Some(symbol) = resolved
                .local_symbol_reference_at_offset(loop_.pack_span.start)
                .map(|(_, symbol)| symbol)
            else {
                continue;
            };
            if symbol.kind != LocalSymbolKind::LiteralCapture {
                diagnostics.push(literal_diagnostic(
                    sources,
                    "E0524",
                    loop_.pack_span,
                    "literal-pack `for` source is not a literal capture",
                    "use `for item in items` only with this literal definition's `...items` capture",
                ));
            } else if !pack_loops.insert(symbol.id) {
                diagnostics.push(literal_diagnostic(
                    sources,
                    "E0525",
                    loop_.pack_span,
                    "literal element pack is consumed by more than one loop",
                    "consume the pack in exactly one `for` loop",
                ));
            }
            check_pack_loop_control(sources, &loop_.body, diagnostics);
            check_pack_uses_in_block(sources, &loop_.body, resolved, pack_loops, diagnostics);
            continue;
        }
        visit_statement_expressions(statement, &mut |expression| {
            check_pack_expression(sources, expression, resolved, diagnostics)
        });
        match statement {
            Stmt::If(statement) => {
                check_pack_uses_in_block(
                    sources,
                    &statement.then_block,
                    resolved,
                    pack_loops,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    check_pack_uses_in_block(sources, block, resolved, pack_loops, diagnostics);
                }
            }
            Stmt::IfIs(statement) => {
                check_pack_uses_in_block(
                    sources,
                    &statement.then_block,
                    resolved,
                    pack_loops,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    check_pack_uses_in_block(sources, block, resolved, pack_loops, diagnostics);
                }
            }
            Stmt::Switch(statement) => {
                for arm in &statement.arms {
                    check_pack_uses_in_block(sources, &arm.body, resolved, pack_loops, diagnostics);
                }
                if let Some(arm) = &statement.wildcard_arm {
                    check_pack_uses_in_block(sources, &arm.body, resolved, pack_loops, diagnostics);
                }
            }
            Stmt::ForRange(statement) => check_pack_uses_in_block(
                sources,
                &statement.body,
                resolved,
                pack_loops,
                diagnostics,
            ),
            Stmt::CollectionFor(statement) => check_pack_uses_in_block(
                sources,
                &statement.body,
                resolved,
                pack_loops,
                diagnostics,
            ),
            Stmt::While(statement) => check_pack_uses_in_block(
                sources,
                &statement.body,
                resolved,
                pack_loops,
                diagnostics,
            ),
            Stmt::Loop(statement) => check_pack_uses_in_block(
                sources,
                &statement.body,
                resolved,
                pack_loops,
                diagnostics,
            ),
            Stmt::Region(statement) => check_pack_uses_in_block(
                sources,
                &statement.body,
                resolved,
                pack_loops,
                diagnostics,
            ),
            Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Return(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::LiteralPackFor(_)
            | Stmt::Expression(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Drop(_) => {}
        }
    }
    if let Some(result) = &block.result {
        check_pack_expression(sources, result, resolved, diagnostics);
    }
}

fn check_pack_loop_control(sources: &SourceMap, block: &Block, diagnostics: &mut Vec<Diagnostic>) {
    for statement in &block.statements {
        match statement {
            Stmt::Break(_) | Stmt::Continue(_) => diagnostics.push(literal_diagnostic(
                sources,
                "E0528",
                statement.span(),
                "Phase 1 literal pack iteration does not support `break` or `continue`",
                "use conditional statements in the body, or return from the literal definition",
            )),
            Stmt::If(statement) => {
                check_pack_loop_control(sources, &statement.then_block, diagnostics);
                if let Some(block) = &statement.else_block {
                    check_pack_loop_control(sources, block, diagnostics);
                }
            }
            Stmt::IfIs(statement) => {
                check_pack_loop_control(sources, &statement.then_block, diagnostics);
                if let Some(block) = &statement.else_block {
                    check_pack_loop_control(sources, block, diagnostics);
                }
            }
            Stmt::Switch(statement) => {
                for arm in &statement.arms {
                    check_pack_loop_control(sources, &arm.body, diagnostics);
                }
                if let Some(arm) = &statement.wildcard_arm {
                    check_pack_loop_control(sources, &arm.body, diagnostics);
                }
            }
            Stmt::Region(statement) => {
                check_pack_loop_control(sources, &statement.body, diagnostics)
            }
            Stmt::ForRange(_)
            | Stmt::CollectionFor(_)
            | Stmt::LiteralPackFor(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
            | Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Return(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Expression(_)
            | Stmt::Drop(_) => {}
        }
    }
}

fn check_pack_expression(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Call(call) = expression
        && call.arguments.is_empty()
        && let Expr::Member(member) = call.callee.as_ref()
        && member.member == "len"
        && let Expr::Identifier(identifier) = member.object.as_ref()
        && resolved
            .local_symbol_for_identifier(identifier)
            .is_some_and(|symbol| symbol.kind == LocalSymbolKind::LiteralCapture)
    {
        return;
    }
    if let Expr::Identifier(identifier) = expression
        && resolved
            .local_symbol_for_identifier(identifier)
            .is_some_and(|symbol| symbol.kind == LocalSymbolKind::LiteralCapture)
    {
        diagnostics.push(literal_diagnostic(
            sources,
            "E0526",
            identifier.span,
            "literal element pack cannot be used as an ordinary value",
            "use only `items.len()` or consuming `for item in items`",
        ));
        return;
    }
    visit_expression_children(expression, &mut |child| {
        check_pack_expression(sources, child, resolved, diagnostics)
    });
}

fn visit_statement_expressions(statement: &Stmt, visitor: &mut impl FnMut(&Expr)) {
    match statement {
        Stmt::Return(statement) => statement.expression.as_ref().into_iter().for_each(visitor),
        Stmt::Binding(statement) => visitor(&statement.initializer),
        Stmt::Assignment(statement) => {
            visitor(&statement.target);
            visitor(&statement.value);
        }
        Stmt::If(statement) => visitor(&statement.condition),
        Stmt::IfIs(statement) => visitor(&statement.expression),
        Stmt::Switch(statement) => visitor(&statement.expression),
        Stmt::ForRange(statement) => {
            visitor(&statement.start);
            visitor(&statement.end);
        }
        Stmt::CollectionFor(statement) => visitor(&statement.source),
        Stmt::While(statement) => visitor(&statement.condition),
        Stmt::Region(statement) => visitor(&statement.allocator),
        Stmt::Expression(statement) => visitor(&statement.expression),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
    }
}

fn visit_expression_children(expression: &Expr, visitor: &mut impl FnMut(&Expr)) {
    match expression {
        Expr::Closure(closure) => {
            for statement in &closure.body.statements {
                visit_statement_expressions(statement, visitor);
            }
            if let Some(result) = &closure.body.result {
                visitor(result);
            }
        }
        Expr::Unary(expression) => visitor(&expression.operand),
        Expr::Binary(expression) => {
            visitor(&expression.left);
            visitor(&expression.right);
        }
        Expr::TypeConversion(expression) => visitor(&expression.expression),
        Expr::Propagate(expression) => visitor(&expression.expression),
        Expr::Force(expression) => visitor(&expression.expression),
        Expr::Catch(expression) => visitor(&expression.expression),
        Expr::Borrow(expression) => visitor(&expression.expression),
        Expr::Call(expression) => {
            visitor(&expression.callee);
            expression.arguments.iter().for_each(visitor);
        }
        Expr::Member(expression) => visitor(&expression.object),
        Expr::Index(expression) => {
            visitor(&expression.object);
            visitor(&expression.index);
        }
        Expr::Group(expression) => visitor(&expression.expression),
        Expr::ArrayLiteral(expression) => expression.elements.iter().for_each(visitor),
        Expr::TypedSequenceLiteral(expression) => expression.elements.iter().for_each(visitor),
        Expr::TypedStringLiteral(_) | Expr::StringLiteral(_) => {}
        Expr::StructLiteral(expression) => {
            expression
                .fields
                .iter()
                .for_each(|field| visitor(&field.value));
        }
        Expr::InterpolatedString(expression) => expression.parts.iter().for_each(|part| {
            if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                visitor(&part.expression);
            }
        }),
        Expr::Otherwise(expression) => visitor(&expression.value),
        Expr::If(expression) => visitor(&expression.condition),
        Expr::IfIs(expression) => visitor(&expression.expression),
        Expr::Match(expression) => visitor(&expression.expression),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(super) fn literal_pack_len_call_type(
    expression: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    if !expression.arguments.is_empty() {
        return None;
    }
    let Expr::Member(member) = expression.callee.as_ref() else {
        return None;
    };
    if member.member != "len" {
        return None;
    }
    let Expr::Identifier(identifier) = member.object.as_ref() else {
        return None;
    };
    let symbol = resolved.local_symbol_for_identifier(identifier)?;
    (symbol.kind == LocalSymbolKind::LiteralCapture
        && environment.literal_pack_element(&identifier.name).is_some())
    .then(|| Type::Primitive("usize".to_string()))
}

fn literal_diagnostic(
    sources: &SourceMap,
    code: &'static str,
    span: ByteSpan,
    message: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(code, message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}
