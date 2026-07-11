use super::diagnostics::unsized_value_type_diagnostic;
use super::model::Type;
use super::type_expr::type_expr_to_type_with_self_type;
use crate::ast::{
    AstFile, Block, Expr, FunctionDecl, ImplDecl, ImplMember, Item, MethodDecl, Parameter,
    PrimitiveDecl, Stmt, TraitDecl, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_sized_value_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                check_function(sources, function, resolved, None, diagnostics);
            }
            Item::Primitive(primitive) => {
                check_primitive(sources, primitive, resolved, diagnostics);
            }
            Item::Struct(struct_) => {
                for field in &struct_.fields {
                    let subject = format!("struct field `{}.{}`", struct_.name, field.name);
                    check_value_type(sources, &field.ty, &subject, resolved, None, diagnostics);
                }
            }
            Item::Enum(enum_) => {
                for variant in &enum_.variants {
                    for payload in &variant.payload {
                        let subject =
                            format!("enum variant payload `{}.{}`", enum_.name, variant.name);
                        check_parameter_type(
                            sources,
                            payload,
                            &subject,
                            resolved,
                            None,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Trait(trait_) => {
                check_trait(sources, trait_, resolved, diagnostics);
            }
            Item::Impl(impl_) => {
                check_impl(sources, impl_, resolved, diagnostics);
            }
            Item::Use(_) | Item::Import(_) | Item::FromImport(_) | Item::TypeAlias(_) => {}
        }
    }
}

fn check_function(
    sources: &SourceMap,
    function: &FunctionDecl,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let prefix = format!("function `{}`", function.name);
    check_parameter_list(
        sources,
        &function.parameters.parameters,
        &prefix,
        resolved,
        self_type,
        diagnostics,
    );
    check_value_type(
        sources,
        &function.return_type,
        &format!("{prefix} return type"),
        resolved,
        self_type,
        diagnostics,
    );
    check_block(sources, &function.body, resolved, self_type, diagnostics);
}

fn check_primitive(
    sources: &SourceMap,
    primitive: &PrimitiveDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let prefix = format!("primitive `{}`", primitive.name);
    check_parameter_list(
        sources,
        &primitive.parameters.parameters,
        &prefix,
        resolved,
        None,
        diagnostics,
    );
    check_value_type(
        sources,
        &primitive.return_type,
        &format!("{prefix} return type"),
        resolved,
        None,
        diagnostics,
    );
}

fn check_trait(
    sources: &SourceMap,
    trait_: &TraitDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in &trait_.methods {
        let prefix = format!("trait method `{}.{}`", trait_.name, method.name);
        check_method_with_prefix(sources, method, &prefix, resolved, None, diagnostics);
    }
}

fn check_impl(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let self_type = type_expr_to_type_with_self_type(&impl_.target_ty, resolved, None);
    for member in &impl_.members {
        match member {
            ImplMember::Function(function) => {
                check_function(sources, function, resolved, Some(&self_type), diagnostics);
            }
            ImplMember::Method(method) => {
                let prefix = format!("method `{}`", method.name);
                check_method_with_prefix(
                    sources,
                    method,
                    &prefix,
                    resolved,
                    Some(&self_type),
                    diagnostics,
                );
            }
        }
    }
}

fn check_method_with_prefix(
    sources: &SourceMap,
    method: &MethodDecl,
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_parameter_type(
        sources,
        &method.receiver,
        &format!("{prefix} receiver"),
        resolved,
        self_type,
        diagnostics,
    );
    check_parameter_list(
        sources,
        &method.parameters.parameters,
        prefix,
        resolved,
        self_type,
        diagnostics,
    );
    check_value_type(
        sources,
        &method.return_type,
        &format!("{prefix} return type"),
        resolved,
        self_type,
        diagnostics,
    );
    if let Some(body) = &method.body {
        check_block(sources, body, resolved, self_type, diagnostics);
    }
}

fn check_parameter_list(
    sources: &SourceMap,
    parameters: &[Parameter],
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        check_parameter_type(sources, parameter, prefix, resolved, self_type, diagnostics);
    }
}

fn check_parameter_type(
    sources: &SourceMap,
    parameter: &Parameter,
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_value_type(
        sources,
        &parameter.ty,
        &format!("{prefix} parameter `{}`", parameter.name),
        resolved,
        self_type,
        diagnostics,
    );
}

fn check_block(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement(sources, statement, resolved, self_type, diagnostics);
    }
}

fn check_statement(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Binding(statement) => {
            if let Some(ty) = &statement.ty {
                check_value_type(
                    sources,
                    ty,
                    &format!("binding `{}` annotation", statement.name),
                    resolved,
                    self_type,
                    diagnostics,
                );
            }
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::Assignment(statement) => {
            check_expression(sources, &statement.target, resolved, self_type, diagnostics);
            check_expression(sources, &statement.value, resolved, self_type, diagnostics);
        }
        Stmt::If(statement) => {
            check_expression(
                sources,
                &statement.condition,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::IfLet(statement) => {
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &statement.then_block,
                resolved,
                self_type,
                diagnostics,
            );
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, self_type, diagnostics);
            }
        }
        Stmt::Switch(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
            for arm in &statement.arms {
                check_block(sources, &arm.body, resolved, self_type, diagnostics);
            }
            if let Some(else_arm) = &statement.else_arm {
                check_block(sources, &else_arm.body, resolved, self_type, diagnostics);
            }
        }
        Stmt::ForRange(statement) => {
            check_expression(sources, &statement.start, resolved, self_type, diagnostics);
            check_expression(sources, &statement.end, resolved, self_type, diagnostics);
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::While(statement) => {
            check_expression(
                sources,
                &statement.condition,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::WhileLet(statement) => {
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::Loop(statement) => {
            check_block(sources, &statement.body, resolved, self_type, diagnostics);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression(sources, expression, resolved, self_type, diagnostics);
            }
        }
        Stmt::Expression(statement) => {
            check_expression(
                sources,
                &statement.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn check_expression(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    check_expression(sources, &part.expression, resolved, self_type, diagnostics);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression(sources, element, resolved, self_type, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression(sources, &field.value, resolved, self_type, diagnostics);
            }
        }
        Expr::Propagate(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Force(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Catch(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
            check_block(
                sources,
                &expression.catch_block,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Unary(expression) => {
            check_expression(
                sources,
                &expression.operand,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Binary(expression) => {
            check_expression(sources, &expression.left, resolved, self_type, diagnostics);
            check_expression(sources, &expression.right, resolved, self_type, diagnostics);
        }
        Expr::TypeConversion(expression) => {
            check_value_type(
                sources,
                &expression.ty,
                "type conversion target",
                resolved,
                self_type,
                diagnostics,
            );
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Call(expression) => {
            check_expression(
                sources,
                &expression.callee,
                resolved,
                self_type,
                diagnostics,
            );
            for argument in &expression.arguments {
                check_expression(sources, argument, resolved, self_type, diagnostics);
            }
        }
        Expr::Member(expression) => {
            check_expression(
                sources,
                &expression.object,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Index(expression) => {
            check_expression(
                sources,
                &expression.object,
                resolved,
                self_type,
                diagnostics,
            );
            check_expression(sources, &expression.index, resolved, self_type, diagnostics);
        }
        Expr::Group(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression(sources, &expression.value, resolved, self_type, diagnostics);
            check_expression(
                sources,
                &expression.default,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::PatternConditional(expression) => {
            check_expression(
                sources,
                &expression.target,
                resolved,
                self_type,
                diagnostics,
            );
            for arm in &expression.arms {
                check_expression(sources, &arm.expression, resolved, self_type, diagnostics);
            }
            check_expression(
                sources,
                &expression.fallback,
                resolved,
                self_type,
                diagnostics,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_value_type(
    sources: &SourceMap,
    ty: &TypeExpr,
    subject: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(unsized_part) = first_unsized_value_part(ty, resolved, self_type) {
        diagnostics.push(unsized_value_type_diagnostic(
            sources,
            ty,
            subject,
            &unsized_part,
        ));
    }
}

fn first_unsized_value_part(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    let resolved_type = type_expr_to_type_with_self_type(ty, resolved, self_type);
    resolved_type
        .first_unsized_part()
        .cloned()
        .or_else(|| first_unsized_generic_argument(ty, resolved, self_type))
}

fn first_unsized_generic_argument(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    match ty {
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .find_map(|argument| first_unsized_value_part(argument, resolved, self_type)),
        TypeExpr::Array(array) => first_unsized_value_part(&array.element, resolved, self_type),
        TypeExpr::Optional(optional) => {
            first_unsized_value_part(&optional.inner, resolved, self_type)
        }
        TypeExpr::Fallible(fallible) => {
            first_unsized_value_part(&fallible.success, resolved, self_type)
                .or_else(|| first_unsized_value_part(&fallible.error, resolved, self_type))
        }
        TypeExpr::Reference(_) | TypeExpr::Pointer(_) | TypeExpr::Borrow(_) | TypeExpr::View(_) => {
            None
        }
    }
}
