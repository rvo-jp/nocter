use super::diagnostics::{
    generic_type_argument_count_diagnostic, self_type_outside_context_diagnostic,
    unresolved_type_reference_diagnostic,
};
use crate::ast::{
    AstFile, Block, Expr, GenericParamList, ImplMember, InterpolatedStringPart, Item, MethodDecl,
    Parameter, Stmt, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_generic_type_arities(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let scope = if function.owner.is_some() {
                    GenericScope::new(&function.generics).with_self_type()
                } else {
                    GenericScope::new(&function.generics)
                };
                check_parameters(
                    sources,
                    &function.parameters.parameters,
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_type_expr(
                    sources,
                    &function.return_type,
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_block(sources, &function.body, resolved, &scope, diagnostics);
            }
            Item::Primitive(primitive) => {
                let scope = GenericScope::new(&primitive.generics);
                check_parameters(
                    sources,
                    &primitive.parameters.parameters,
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_type_expr(
                    sources,
                    &primitive.return_type,
                    resolved,
                    &scope,
                    diagnostics,
                );
            }
            Item::TypeAlias(alias) => {
                let scope = GenericScope::new(&alias.generics);
                check_type_expr(sources, &alias.target, resolved, &scope, diagnostics);
            }
            Item::Struct(struct_) => {
                let scope = GenericScope::new(&struct_.generics);
                for field in &struct_.fields {
                    check_type_expr(sources, &field.ty, resolved, &scope, diagnostics);
                }
            }
            Item::Enum(enum_) => {
                let scope = GenericScope::new(&enum_.generics);
                for variant in &enum_.variants {
                    check_parameters(sources, &variant.payload, resolved, &scope, diagnostics);
                }
            }
            Item::Interface(interface) => {
                let scope = GenericScope::new(&interface.generics).with_self_type();
                for method in &interface.methods {
                    check_method_signature(sources, method, resolved, &scope, diagnostics);
                }
            }
            Item::Impl(impl_) => {
                let scope = GenericScope::new(&impl_.generics);
                if let Some(interface_ty) = &impl_.interface_ty {
                    check_type_expr(sources, interface_ty, resolved, &scope, diagnostics);
                }
                check_type_expr(sources, &impl_.target_ty, resolved, &scope, diagnostics);
                let member_scope = scope.clone().with_self_type();
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            check_method_signature(
                                sources,
                                method,
                                resolved,
                                &member_scope,
                                diagnostics,
                            );
                            if let Some(body) = &method.body {
                                check_block(sources, body, resolved, &member_scope, diagnostics);
                            }
                        }
                        ImplMember::Drop(drop_) => {
                            check_type_expr(
                                sources,
                                &drop_.binding.ty,
                                resolved,
                                &member_scope,
                                diagnostics,
                            );
                            check_block(sources, &drop_.body, resolved, &member_scope, diagnostics);
                        }
                    }
                }
            }
            Item::Import(_) | Item::FromImport(_) | Item::Literal(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
struct GenericScope<'a> {
    parameters: HashMap<&'a str, ByteSpan>,
    allows_self_type: bool,
}

impl<'a> GenericScope<'a> {
    fn new(generics: &'a GenericParamList) -> Self {
        Self {
            parameters: generics
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), parameter.span))
                .collect(),
            allows_self_type: false,
        }
    }

    fn with_self_type(mut self) -> Self {
        self.allows_self_type = true;
        self
    }

    fn allows_self_type(&self) -> bool {
        self.allows_self_type
    }

    fn contains(&self, name: &str) -> bool {
        self.parameters.contains_key(name)
    }

    fn parameter_span(&self, name: &str) -> Option<ByteSpan> {
        self.parameters.get(name).copied()
    }
}

fn check_method_signature(
    sources: &SourceMap,
    method: &MethodDecl,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_type_expr(sources, &method.receiver.ty, resolved, scope, diagnostics);
    check_parameters(
        sources,
        &method.parameters.parameters,
        resolved,
        scope,
        diagnostics,
    );
    check_type_expr(sources, &method.return_type, resolved, scope, diagnostics);
}

fn check_parameters(
    sources: &SourceMap,
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        check_type_expr(sources, &parameter.ty, resolved, scope, diagnostics);
    }
}

fn check_type_expr(
    sources: &SourceMap,
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ty {
        TypeExpr::Reference(reference) => {
            if reference.name == "Self" {
                if !scope.allows_self_type() {
                    diagnostics.push(self_type_outside_context_diagnostic(
                        sources,
                        reference.span,
                    ));
                }
                return;
            }
            if scope.contains(&reference.name) {
                return;
            }
            if builtin_type_argument_arity(&reference.name).is_some() {
                return;
            }
            match resolved.type_symbol_definition_by_reference_name(&reference.name) {
                Some((symbol, type_symbol)) if type_symbol.generic_arity > 0 => {
                    diagnostics.push(generic_type_argument_count_diagnostic(
                        sources,
                        &reference.name,
                        reference.span,
                        Some(symbol.declaration_span),
                        type_symbol.generic_arity,
                        0,
                    ));
                }
                Some(_) => {}
                None => {
                    diagnostics.push(unresolved_type_reference_diagnostic(
                        sources,
                        &reference.name,
                        reference.span,
                    ));
                }
            }
        }
        TypeExpr::Generic(generic) => {
            if generic.name == "Self" {
                if scope.allows_self_type() {
                    diagnostics.push(generic_type_argument_count_diagnostic(
                        sources,
                        &generic.name,
                        generic.name_span,
                        None,
                        0,
                        generic.arguments.len(),
                    ));
                } else {
                    diagnostics.push(self_type_outside_context_diagnostic(
                        sources,
                        generic.name_span,
                    ));
                }
            } else if builtin_type_argument_arity(&generic.name).is_some() {
                diagnostics.push(generic_type_argument_count_diagnostic(
                    sources,
                    &generic.name,
                    generic.name_span,
                    None,
                    0,
                    generic.arguments.len(),
                ));
            } else if let Some(parameter_span) = scope.parameter_span(&generic.name) {
                diagnostics.push(generic_type_argument_count_diagnostic(
                    sources,
                    &generic.name,
                    generic.name_span,
                    Some(parameter_span),
                    0,
                    generic.arguments.len(),
                ));
            } else {
                match resolved.type_symbol_definition_by_reference_name(&generic.name) {
                    Some((symbol, type_symbol))
                        if type_symbol.generic_arity != generic.arguments.len() =>
                    {
                        diagnostics.push(generic_type_argument_count_diagnostic(
                            sources,
                            &generic.name,
                            generic.name_span,
                            Some(symbol.declaration_span),
                            type_symbol.generic_arity,
                            generic.arguments.len(),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        diagnostics.push(unresolved_type_reference_diagnostic(
                            sources,
                            &generic.name,
                            generic.name_span,
                        ));
                    }
                }
            }
            for argument in &generic.arguments {
                check_type_expr(sources, argument, resolved, scope, diagnostics);
            }
        }
        TypeExpr::Pointer(pointer) => {
            check_type_expr(sources, &pointer.inner, resolved, scope, diagnostics)
        }
        TypeExpr::Borrow(borrow) => {
            check_type_expr(sources, &borrow.inner, resolved, scope, diagnostics)
        }
        TypeExpr::View(view) => {
            check_type_expr(sources, &view.element, resolved, scope, diagnostics)
        }
        TypeExpr::Array(array) => {
            check_type_expr(sources, &array.element, resolved, scope, diagnostics)
        }
        TypeExpr::Optional(optional) => {
            check_type_expr(sources, &optional.inner, resolved, scope, diagnostics)
        }
        TypeExpr::Fallible(fallible) => {
            check_type_expr(sources, &fallible.success, resolved, scope, diagnostics);
            check_type_expr(sources, &fallible.error, resolved, scope, diagnostics);
        }
    }
}

fn builtin_type_argument_arity(name: &str) -> Option<usize> {
    matches!(
        name,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "str"
            | "error"
            | "void"
            | "never"
    )
    .then_some(0)
}

fn check_block(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement(sources, statement, resolved, scope, diagnostics);
    }
    if let Some(result) = &block.result {
        check_expression(sources, result, resolved, scope, diagnostics);
    }
}

fn check_statement(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Binding(statement) => {
            if let Some(ty) = &statement.ty {
                check_type_expr(sources, ty, resolved, scope, diagnostics);
            }
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                scope,
                diagnostics,
            );
        }
        Stmt::Assignment(statement) => {
            check_expression(sources, &statement.target, resolved, scope, diagnostics);
            check_expression(sources, &statement.value, resolved, scope, diagnostics);
        }
        Stmt::If(statement) => {
            check_expression(sources, &statement.condition, resolved, scope, diagnostics);
            check_block(sources, &statement.then_block, resolved, scope, diagnostics);
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, scope, diagnostics);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
            check_block(sources, &statement.then_block, resolved, scope, diagnostics);
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, scope, diagnostics);
            }
        }
        Stmt::Switch(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
            for arm in &statement.arms {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                check_block(sources, &wildcard_arm.body, resolved, scope, diagnostics);
            }
        }
        Stmt::ForRange(statement) => {
            check_expression(sources, &statement.start, resolved, scope, diagnostics);
            check_expression(sources, &statement.end, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::LiteralPackFor(statement) => {
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::While(statement) => {
            check_expression(sources, &statement.condition, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::Loop(statement) => {
            check_block(sources, &statement.body, resolved, scope, diagnostics)
        }
        Stmt::Region(statement) => {
            check_expression(sources, &statement.allocator, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression(sources, expression, resolved, scope, diagnostics);
            }
        }
        Stmt::Expression(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn check_expression(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression(sources, &part.expression, resolved, scope, diagnostics);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression(sources, element, resolved, scope, diagnostics);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            if matches!(expression.target, TypeExpr::Generic(_)) {
                check_type_expr(sources, &expression.target, resolved, scope, diagnostics);
            }
            for element in &expression.elements {
                check_expression(sources, element, resolved, scope, diagnostics);
            }
            if let Some(using) = &expression.using {
                check_expression(sources, &using.allocator, resolved, scope, diagnostics);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if matches!(expression.target, TypeExpr::Generic(_)) {
                check_type_expr(sources, &expression.target, resolved, scope, diagnostics);
            }
            if let Some(using) = &expression.using {
                check_expression(sources, &using.allocator, resolved, scope, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            check_type_expr(sources, &expression.ty, resolved, scope, diagnostics);
            for field in &expression.fields {
                check_expression(sources, &field.value, resolved, scope, diagnostics);
            }
        }
        Expr::Propagate(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Force(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Catch(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_block(
                sources,
                &expression.catch_block,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Unary(expression) => {
            check_expression(sources, &expression.operand, resolved, scope, diagnostics);
        }
        Expr::Binary(expression) => {
            check_expression(sources, &expression.left, resolved, scope, diagnostics);
            check_expression(sources, &expression.right, resolved, scope, diagnostics);
        }
        Expr::TypeConversion(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_type_expr(sources, &expression.ty, resolved, scope, diagnostics);
        }
        Expr::Call(expression) => {
            check_expression(sources, &expression.callee, resolved, scope, diagnostics);
            for argument in &expression.arguments {
                check_expression(sources, argument, resolved, scope, diagnostics);
            }
        }
        Expr::Member(expression) => {
            check_expression(sources, &expression.object, resolved, scope, diagnostics);
        }
        Expr::Index(expression) => {
            check_expression(sources, &expression.object, resolved, scope, diagnostics);
            check_expression(sources, &expression.index, resolved, scope, diagnostics);
        }
        Expr::Group(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Otherwise(expression) => {
            check_expression(sources, &expression.value, resolved, scope, diagnostics);
            check_block(sources, &expression.fallback, resolved, scope, diagnostics);
        }
        Expr::If(expression) => {
            check_expression(sources, &expression.condition, resolved, scope, diagnostics);
            check_block(
                sources,
                &expression.then_block,
                resolved,
                scope,
                diagnostics,
            );
            if let Some(block) = &expression.else_block {
                check_block(sources, block, resolved, scope, diagnostics);
            }
        }
        Expr::IfIs(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_block(
                sources,
                &expression.then_block,
                resolved,
                scope,
                diagnostics,
            );
            if let Some(block) = &expression.else_block {
                check_block(sources, block, resolved, scope, diagnostics);
            }
        }
        Expr::Match(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            for arm in &expression.arms {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
            }
            if let Some(arm) = &expression.wildcard_arm {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
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
