use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_method_for_call};
use super::diagnostics::{invalid_drop_target_diagnostic, uninitialized_binding_diagnostic};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_if_is_binding,
    environment_for_if_let_binding, environment_for_method, environment_for_parameters,
    environment_for_parameters_with_self_type, environment_for_pattern_conditional_arm,
    environment_for_switch_arm, environment_for_while_let_binding, impl_self_type,
};
use super::expressions::{collection_len_call_type, expression_type};
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use crate::ast::{
    AstFile, Block, Expr, IdentifierExpr, ImplDecl, ImplMember, Item, Stmt, TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_ownership_states(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let mut environment =
                    environment_for_parameters(&function.parameters.parameters, resolved);
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    &function.parameters.parameters,
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_ownership(sources, impl_, resolved, diagnostics);
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
}

fn check_impl_member_ownership(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let self_type = impl_self_type(impl_, resolved);

    for member in &impl_.members {
        match member {
            ImplMember::Function(function) => {
                let mut environment = environment_for_parameters_with_self_type(
                    &function.parameters.parameters,
                    resolved,
                    self_type.clone(),
                );
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    &function.parameters.parameters,
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, self_type.clone());
                let mut ownership = OwnershipState::default();
                ownership.define_binding_from_environment(
                    &method.receiver.name,
                    method.receiver.name_span,
                    &environment,
                    resolved,
                );
                ownership.define_parameters(&method.parameters.parameters, &environment, resolved);
                check_block_ownership(
                    sources,
                    body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            ImplMember::Drop(drop_) => {
                let mut environment = environment_for_parameters_with_self_type(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    self_type.clone(),
                );
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    std::slice::from_ref(&drop_.binding),
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &drop_.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
        }
    }
}

fn check_block_ownership(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    for statement in &block.statements {
        check_statement_ownership(
            sources,
            statement,
            resolved,
            diagnostics,
            environment,
            ownership,
        );
    }
}

fn check_statement_ownership(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_ownership(
                    sources,
                    expression,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Stmt::Binding(statement) => {
            check_expression_ownership(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
            }
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            ownership.define_binding(
                statement.name.clone(),
                statement.name_span,
                &binding_type,
                resolved,
            );
        }
        Stmt::Assignment(statement) => {
            check_assignment_target_ownership(
                sources,
                &statement.target,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.value,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(ty) = environment.get(&identifier.name)
            {
                ownership.define_binding(identifier.name.clone(), identifier.span, ty, resolved);
            }
        }
        Stmt::If(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment = environment.clone();
            let mut then_ownership = ownership.clone();
            check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_ownership = ownership.clone();
            if let Some(payload) = &statement.payload {
                then_ownership.define_binding_from_environment(
                    &payload.name,
                    payload.span,
                    &then_environment,
                    resolved,
                );
            }
            check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
            }
        }
        Stmt::IfLet(statement) => {
            check_expression_ownership(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment =
                environment_for_if_let_binding(statement, resolved, environment);
            let mut then_ownership = ownership.clone();
            then_ownership.define_binding_from_environment(
                &statement.name,
                statement.name_span,
                &then_environment,
                resolved,
            );
            check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
            }
        }
        Stmt::Switch(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            for arm in &statement.arms {
                let mut arm_environment = environment_for_switch_arm(arm, resolved, environment);
                let mut arm_ownership = ownership.clone();
                if let Some(payload) = &arm.payload {
                    arm_ownership.define_binding_from_environment(
                        &payload.name,
                        payload.span,
                        &arm_environment,
                        resolved,
                    );
                }
                check_block_ownership(
                    sources,
                    &arm.body,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_ownership,
                );
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                check_block_ownership(
                    sources,
                    &else_arm.body,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
            }
        }
        Stmt::While(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
        }
        Stmt::WhileLet(statement) => {
            check_expression_ownership(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut body_environment =
                environment_for_while_let_binding(statement, resolved, environment);
            let mut body_ownership = ownership.clone();
            body_ownership.define_binding_from_environment(
                &statement.name,
                statement.name_span,
                &body_environment,
                resolved,
            );
            check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_ownership(
                sources,
                &statement.start,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.end,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_ownership = ownership.clone();
            check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
        }
        Stmt::Drop(statement) => {
            let Some(ty) = environment.get(&statement.name) else {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    None,
                ));
                return;
            };
            if non_copy_struct_type_name(ty, resolved).is_none() {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    Some(ty),
                ));
                return;
            }
            ownership.ensure_binding_from_environment(
                &statement.name,
                statement.name_span,
                environment,
                resolved,
            );
            ownership.drop_binding(sources, &statement.name, statement.name_span, diagnostics);
        }
        Stmt::Expression(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn check_expression_ownership(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    match expression {
        Expr::Identifier(identifier) => {
            ownership.require_initialized(sources, identifier, "use", diagnostics);
        }
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = expression.operand.as_ref() {
                if let Some(ty) = environment.get(&identifier.name)
                    && non_copy_struct_type_name(ty, resolved).is_some()
                {
                    ownership.ensure_binding_from_environment(
                        &identifier.name,
                        identifier.span,
                        environment,
                        resolved,
                    );
                    ownership.move_binding(sources, identifier, diagnostics);
                }
            } else {
                check_expression_ownership(
                    sources,
                    &expression.operand,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Propagate(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Force(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Catch(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            let mut catch_ownership = ownership.clone();
            catch_ownership.define_binding_from_environment(
                &expression.error_name,
                expression.error_span,
                &catch_environment,
                resolved,
            );
            check_block_ownership(
                sources,
                &expression.catch_block,
                resolved,
                diagnostics,
                &mut catch_environment,
                &mut catch_ownership,
            );
        }
        Expr::Borrow(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Binary(expression) => {
            check_expression_ownership(
                sources,
                &expression.left,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.right,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Unary(expression) => {
            check_expression_ownership(
                sources,
                &expression.operand,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Call(expression) => {
            if let Some(identifier) =
                owned_method_receiver_identifier(expression, resolved, environment)
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            } else if collection_len_call_type(expression, resolved, environment).is_some() {
                if let Some(method) = method_member_for_call(expression) {
                    check_expression_ownership(
                        sources,
                        &method.object,
                        resolved,
                        diagnostics,
                        environment,
                        ownership,
                    );
                }
            } else if let Some(method) = method_member_for_call(expression)
                && resolved_method_for_call(resolved, expression, environment).is_some()
            {
                check_expression_ownership(
                    sources,
                    &method.object,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            } else {
                check_expression_ownership(
                    sources,
                    &expression.callee,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }

            for argument in &expression.arguments {
                check_expression_ownership(
                    sources,
                    argument,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Index(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.index,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_ownership(
                    sources,
                    element,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_ownership(
                    sources,
                    &field.value,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    check_expression_ownership(
                        sources,
                        &part.expression,
                        resolved,
                        diagnostics,
                        environment,
                        ownership,
                    );
                }
            }
        }
        Expr::OptionalDefault(expression) => {
            check_expression_ownership(
                sources,
                &expression.value,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.default,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::PatternConditional(expression) => {
            check_expression_ownership(
                sources,
                &expression.target,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_pattern_conditional_arm(arm, resolved, environment);
                let mut arm_ownership = ownership.clone();
                if let Some(payload) = &arm.payload {
                    arm_ownership.define_binding_from_environment(
                        &payload.name,
                        payload.span,
                        &arm_environment,
                        resolved,
                    );
                }
                check_expression_ownership(
                    sources,
                    &arm.expression,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_ownership,
                );
            }
            check_expression_ownership(
                sources,
                &expression.fallback,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_assignment_target_ownership(
    sources: &SourceMap,
    target: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    if whole_identifier(target).is_some() {
        return;
    }
    check_expression_ownership(
        sources,
        target,
        resolved,
        diagnostics,
        environment,
        ownership,
    );
}

fn whole_identifier(expression: &Expr) -> Option<&IdentifierExpr> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Group(group) => whole_identifier(&group.expression),
        _ => None,
    }
}

fn owned_method_receiver_identifier<'a>(
    call: &'a crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<&'a IdentifierExpr> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    if !matches!(signature.receiver.ty, TypeExpr::Reference(ref reference) if reference.name == "Self")
    {
        return None;
    }

    let Expr::Identifier(identifier) = method.object.as_ref() else {
        return None;
    };
    let receiver_type = expression_type(&method.object, resolved, environment);
    non_copy_struct_type_name(&receiver_type, resolved)?;
    Some(identifier)
}

fn non_copy_struct_type_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let Type::Named(canonical_name) = ty else {
        return None;
    };
    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && !symbol.is_copy)
        .map(|symbol| symbol.canonical_name.as_str())
}

#[derive(Debug, Clone, Default)]
struct OwnershipState {
    bindings: HashMap<String, OwnedBinding>,
}

impl OwnershipState {
    fn define_parameters(
        &mut self,
        parameters: &[crate::ast::Parameter],
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        for parameter in parameters {
            self.define_binding_from_environment(
                &parameter.name,
                parameter.name_span,
                environment,
                resolved,
            );
        }
    }

    fn define_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if let Some(ty) = environment.get(name) {
            self.define_binding(name.to_string(), span, ty, resolved);
        } else {
            self.bindings.remove(name);
        }
    }

    fn ensure_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if self.bindings.contains_key(name) {
            return;
        }
        self.define_binding_from_environment(name, span, environment, resolved);
    }

    fn define_binding(
        &mut self,
        name: String,
        span: ByteSpan,
        ty: &Type,
        resolved: &ResolveOutput,
    ) {
        if non_copy_struct_type_name(ty, resolved).is_some() {
            self.bindings.insert(
                name,
                OwnedBinding {
                    state: BindingState::Initialized { span },
                },
            );
        } else {
            self.bindings.remove(&name);
        }
    }

    fn require_initialized(
        &self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        action: &'static str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(binding) = self.bindings.get(&identifier.name) else {
            return true;
        };
        let BindingState::Initialized { .. } = binding.state else {
            diagnostics.push(uninitialized_binding_diagnostic(
                sources,
                &identifier.name,
                identifier.span,
                action,
                binding.state.previous_action(),
                binding.state.previous_span(),
            ));
            return false;
        };
        true
    }

    fn move_binding(
        &mut self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.require_initialized(sources, identifier, "move", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(&identifier.name) {
            binding.state = BindingState::Moved {
                span: identifier.span,
            };
        }
    }

    fn drop_binding(
        &mut self,
        sources: &SourceMap,
        name: &str,
        span: ByteSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let identifier = IdentifierExpr {
            span,
            name: name.to_string(),
        };
        if !self.require_initialized(sources, &identifier, "drop", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(name) {
            binding.state = BindingState::Dropped { span };
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedBinding {
    state: BindingState,
}

#[derive(Debug, Clone, Copy)]
enum BindingState {
    Initialized { span: ByteSpan },
    Moved { span: ByteSpan },
    Dropped { span: ByteSpan },
}

impl BindingState {
    fn previous_action(self) -> &'static str {
        match self {
            BindingState::Moved { .. } => "moved",
            BindingState::Dropped { .. } => "dropped",
            BindingState::Initialized { .. } => "initialized",
        }
    }

    fn previous_span(self) -> ByteSpan {
        match self {
            BindingState::Initialized { span }
            | BindingState::Moved { span }
            | BindingState::Dropped { span } => span,
        }
    }
}
