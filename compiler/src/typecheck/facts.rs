//! Type facts produced from the same environment and expression typing logic as
//! the checker.

use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_method_for_call};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_if_let_binding, environment_for_method,
    environment_for_parameters_with_self_type, environment_for_pattern_conditional_arm,
    environment_for_switch_arm, environment_for_while_let_binding, impl_self_type,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use crate::ast::{
    AstFile, BindingStmt, Block, Expr, GenericParamList, ImplDecl, ImplMember,
    InterpolatedStringPart, Item, MethodDecl, Parameter, Stmt, SwitchPayloadBinding, TypeExpr,
};
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    binding_type_labels: HashMap<ByteSpan, String>,
    binding_readonly: HashMap<ByteSpan, bool>,
    type_references: Vec<TypeReferenceFact>,
    method_call_targets: HashMap<ByteSpan, ByteSpan>,
}

impl TypecheckFacts {
    pub(crate) fn binding_type_label(&self, name_span: ByteSpan) -> Option<&str> {
        self.binding_type_labels.get(&name_span).map(String::as_str)
    }

    pub(crate) fn binding_is_readonly(&self, name_span: ByteSpan) -> Option<bool> {
        self.binding_readonly.get(&name_span).copied()
    }

    pub(crate) fn type_reference_at_offset(&self, offset: usize) -> Option<&TypeReferenceFact> {
        self.type_references
            .iter()
            .filter(|reference| span_contains(reference.span, offset))
            .min_by_key(|reference| (reference.span.len(), reference.span.start))
    }

    pub(crate) fn type_reference_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.type_references.iter().map(|reference| reference.span)
    }

    pub(crate) fn method_call_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.method_call_targets.keys().copied()
    }

    pub(crate) fn method_call_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.method_call_targets.get(&member_span).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeReferenceFact {
    pub(crate) name: String,
    pub(crate) span: ByteSpan,
    pub(crate) symbol_name_span: Option<ByteSpan>,
    pub(crate) symbol_declaration_span: Option<ByteSpan>,
}

pub(crate) fn collect_typecheck_facts(ast: &AstFile, resolved: &ResolveOutput) -> TypecheckFacts {
    let mut collector = TypecheckFactCollector {
        resolved,
        facts: TypecheckFacts::default(),
    };

    for item in &ast.items {
        collector.collect_item_signature_type_references(item);
    }
    for item in &ast.items {
        collector.collect_item_body_facts(item);
    }

    collector.facts
}

struct TypecheckFactCollector<'a> {
    resolved: &'a ResolveOutput,
    facts: TypecheckFacts,
}

impl TypecheckFactCollector<'_> {
    fn collect_item_signature_type_references(&mut self, item: &Item) {
        match item {
            Item::Use(_) | Item::Import(_) | Item::FromImport(_) => {}
            Item::Function(function) => {
                self.collect_generic_param_type_references(&function.generics);
                self.collect_parameter_type_references(&function.parameters.parameters);
                self.collect_type_expr_references(&function.return_type);
            }
            Item::Primitive(primitive) => {
                self.collect_generic_param_type_references(&primitive.generics);
                self.collect_parameter_type_references(&primitive.parameters.parameters);
                self.collect_type_expr_references(&primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                self.collect_generic_param_type_references(&alias.generics);
                self.collect_type_expr_references(&alias.target);
            }
            Item::Struct(struct_) => {
                self.collect_generic_param_type_references(&struct_.generics);
                for field in &struct_.fields {
                    self.collect_type_expr_references(&field.ty);
                }
            }
            Item::Enum(enum_) => {
                self.collect_generic_param_type_references(&enum_.generics);
                for variant in &enum_.variants {
                    self.collect_parameter_type_references(&variant.payload);
                }
            }
            Item::Trait(trait_) => {
                self.collect_generic_param_type_references(&trait_.generics);
                for method in &trait_.methods {
                    self.collect_method_signature_type_references(method);
                }
            }
            Item::Impl(impl_) => {
                if let Some(trait_ty) = &impl_.trait_ty {
                    self.collect_type_expr_references(trait_ty);
                }
                self.collect_type_expr_references(&impl_.target_ty);
                for member in &impl_.members {
                    match member {
                        ImplMember::Function(function) => {
                            self.collect_generic_param_type_references(&function.generics);
                            self.collect_parameter_type_references(&function.parameters.parameters);
                            self.collect_type_expr_references(&function.return_type);
                        }
                        ImplMember::Method(method) => {
                            self.collect_method_signature_type_references(method);
                        }
                        ImplMember::Drop(drop_) => {
                            self.collect_parameter_type_references(std::slice::from_ref(
                                &drop_.binding,
                            ));
                        }
                    }
                }
            }
        }
    }

    fn collect_item_body_facts(&mut self, item: &Item) {
        match item {
            Item::Function(function) => {
                let mut environment = environment_for_function(function, self.resolved);
                self.record_parameter_bindings(&function.parameters.parameters, &environment);
                self.collect_block_facts(&function.body, &mut environment);
            }
            Item::Impl(impl_) => self.collect_impl_member_body_facts(impl_),
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

    fn collect_impl_member_body_facts(&mut self, impl_: &ImplDecl) {
        let self_type = impl_self_type(impl_, self.resolved);

        for member in &impl_.members {
            match member {
                ImplMember::Function(function) => {
                    let mut environment = environment_for_parameters_with_self_type(
                        &function.parameters.parameters,
                        self.resolved,
                        self_type.clone(),
                    );
                    self.record_parameter_bindings(&function.parameters.parameters, &environment);
                    self.collect_block_facts(&function.body, &mut environment);
                }
                ImplMember::Method(method) => {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut environment =
                        environment_for_method(method, self.resolved, self_type.clone());
                    self.record_parameter_bindings(
                        std::slice::from_ref(&method.receiver),
                        &environment,
                    );
                    self.record_parameter_bindings(&method.parameters.parameters, &environment);
                    self.collect_block_facts(body, &mut environment);
                }
                ImplMember::Drop(drop_) => {
                    let mut environment = environment_for_parameters_with_self_type(
                        std::slice::from_ref(&drop_.binding),
                        self.resolved,
                        self_type.clone(),
                    );
                    self.record_parameter_bindings(
                        std::slice::from_ref(&drop_.binding),
                        &environment,
                    );
                    self.collect_block_facts(&drop_.body, &mut environment);
                }
            }
        }
    }

    fn collect_block_facts(&mut self, block: &Block, environment: &mut TypeEnvironment) {
        for statement in &block.statements {
            self.collect_statement_facts(statement, environment);
        }
    }

    fn collect_statement_facts(&mut self, statement: &Stmt, environment: &mut TypeEnvironment) {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.collect_expression_facts(expression, environment);
                }
            }
            Stmt::Binding(statement) => {
                self.collect_binding_statement_facts(statement, environment)
            }
            Stmt::Assignment(statement) => {
                self.collect_expression_facts(&statement.target, environment);
                self.collect_expression_facts(&statement.value, environment);
            }
            Stmt::If(statement) => {
                self.collect_expression_facts(&statement.condition, environment);

                let mut then_environment = environment.clone();
                self.collect_block_facts(&statement.then_block, &mut then_environment);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment);
                }
            }
            Stmt::IfIs(statement) => {
                self.collect_expression_facts(&statement.expression, environment);
                self.record_type_reference(&statement.enum_name, statement.enum_name_span);

                let mut then_environment =
                    environment_for_if_is_binding(statement, self.resolved, environment);
                if let Some(payload) = &statement.payload {
                    self.record_payload_binding(payload, &then_environment);
                }
                self.collect_block_facts(&statement.then_block, &mut then_environment);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment);
                }
            }
            Stmt::IfLet(statement) => {
                self.collect_expression_facts(&statement.initializer, environment);

                let mut then_environment =
                    environment_for_if_let_binding(statement, self.resolved, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &then_environment,
                );
                self.collect_block_facts(&statement.then_block, &mut then_environment);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment);
                }
            }
            Stmt::Switch(statement) => {
                self.collect_expression_facts(&statement.expression, environment);
                for arm in &statement.arms {
                    self.record_type_reference(&arm.enum_name, arm.enum_name_span);
                    let mut arm_environment =
                        environment_for_switch_arm(arm, self.resolved, environment);
                    if let Some(payload) = &arm.payload {
                        self.record_payload_binding(payload, &arm_environment);
                    }
                    self.collect_block_facts(&arm.body, &mut arm_environment);
                }
                if let Some(arm) = &statement.else_arm {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(&arm.body, &mut else_environment);
                }
            }
            Stmt::ForRange(statement) => {
                self.collect_expression_facts(&statement.start, environment);
                self.collect_expression_facts(&statement.end, environment);

                let mut body_environment =
                    environment_for_for_range_binding(statement, self.resolved, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment);
            }
            Stmt::While(statement) => {
                self.collect_expression_facts(&statement.condition, environment);

                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment);
            }
            Stmt::WhileLet(statement) => {
                self.collect_expression_facts(&statement.initializer, environment);

                let mut body_environment =
                    environment_for_while_let_binding(statement, self.resolved, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment);
            }
            Stmt::Loop(statement) => {
                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment);
            }
            Stmt::Expression(statement) => {
                self.collect_expression_facts(&statement.expression, environment);
            }
            Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn collect_binding_statement_facts(
        &mut self,
        statement: &BindingStmt,
        environment: &mut TypeEnvironment,
    ) {
        if let Some(ty) = &statement.ty {
            self.collect_type_expr_references(ty);
        }
        self.collect_expression_facts(&statement.initializer, environment);
        let initializer_type = expression_type(&statement.initializer, self.resolved, environment);

        if let Some(else_block) = &statement.else_block {
            let mut else_environment = environment.clone();
            self.collect_block_facts(else_block, &mut else_environment);
        }

        let binding_type =
            continuing_binding_type(statement, initializer_type, self.resolved, environment);
        let is_mutable = binding_kind_is_mutable(statement.kind);
        self.record_binding(statement.name_span, &binding_type, is_mutable);
        environment.define_binding(statement.name.clone(), binding_type, is_mutable);
    }

    fn collect_expression_facts(&mut self, expression: &Expr, environment: &mut TypeEnvironment) {
        match expression {
            Expr::Propagate(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
            }
            Expr::Force(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
            }
            Expr::Catch(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
                let mut catch_environment = environment_for_catch(
                    expression.error_name.clone(),
                    &expression.expression,
                    self.resolved,
                    environment,
                );
                self.record_environment_binding(
                    expression.error_span,
                    &expression.error_name,
                    &catch_environment,
                );
                self.collect_block_facts(&expression.catch_block, &mut catch_environment);
            }
            Expr::Borrow(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
            }
            Expr::Binary(expression) => {
                self.collect_expression_facts(&expression.left, environment);
                self.collect_expression_facts(&expression.right, environment);
            }
            Expr::Unary(expression) => {
                self.collect_expression_facts(&expression.operand, environment);
            }
            Expr::TypeConversion(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
                self.collect_type_expr_references(&expression.ty);
            }
            Expr::Call(expression) => {
                if let Some(method) = method_member_for_call(expression)
                    && let Some((_, resolved_method)) =
                        resolved_method_for_call(self.resolved, expression, environment)
                {
                    self.facts
                        .method_call_targets
                        .insert(method.member_span, resolved_method.name_span);
                    self.collect_expression_facts(&method.object, environment);
                } else {
                    self.collect_expression_facts(&expression.callee, environment);
                }

                for argument in &expression.arguments {
                    self.collect_expression_facts(argument, environment);
                }
            }
            Expr::Member(expression) => {
                self.collect_expression_facts(&expression.object, environment);
            }
            Expr::Index(expression) => {
                self.collect_expression_facts(&expression.object, environment);
                self.collect_expression_facts(&expression.index, environment);
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression_facts(element, environment);
                }
            }
            Expr::StructLiteral(expression) => {
                self.collect_type_expr_references(&expression.ty);
                for field in &expression.fields {
                    self.collect_expression_facts(&field.value, environment);
                }
            }
            Expr::Group(expression) => {
                self.collect_expression_facts(&expression.expression, environment);
            }
            Expr::InterpolatedString(expression) => {
                for part in &expression.parts {
                    if let InterpolatedStringPart::Expression(part) = part {
                        self.collect_expression_facts(&part.expression, environment);
                    }
                }
            }
            Expr::OptionalDefault(expression) => {
                self.collect_expression_facts(&expression.value, environment);
                self.collect_expression_facts(&expression.default, environment);
            }
            Expr::PatternConditional(expression) => {
                self.collect_expression_facts(&expression.target, environment);
                for arm in &expression.arms {
                    self.record_type_reference(&arm.enum_name, arm.enum_name_span);
                    let mut arm_environment =
                        environment_for_pattern_conditional_arm(arm, self.resolved, environment);
                    if let Some(payload) = &arm.payload {
                        self.record_payload_binding(payload, &arm_environment);
                    }
                    self.collect_expression_facts(&arm.expression, &mut arm_environment);
                }
                self.collect_expression_facts(&expression.fallback, environment);
            }
            Expr::Identifier(identifier) => {
                self.record_environment_binding_readonly(
                    identifier.span,
                    &identifier.name,
                    environment,
                );
            }
            Expr::IntegerLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
        }
    }

    fn collect_method_signature_type_references(&mut self, method: &MethodDecl) {
        self.collect_parameter_type_references(std::slice::from_ref(&method.receiver));
        self.collect_parameter_type_references(&method.parameters.parameters);
        self.collect_type_expr_references(&method.return_type);
    }

    fn collect_generic_param_type_references(&mut self, generics: &GenericParamList) {
        for parameter in &generics.parameters {
            if let Some(bound) = &parameter.bound {
                self.collect_type_expr_references(bound);
            }
        }
    }

    fn collect_parameter_type_references(&mut self, parameters: &[Parameter]) {
        for parameter in parameters {
            self.collect_type_expr_references(&parameter.ty);
        }
    }

    fn collect_type_expr_references(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Reference(ty) => {
                self.record_type_reference(&ty.name, ty.span);
            }
            TypeExpr::Generic(ty) => {
                self.record_type_reference(&ty.name, ty.name_span);
                for argument in &ty.arguments {
                    self.collect_type_expr_references(argument);
                }
            }
            TypeExpr::Pointer(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::Borrow(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::View(ty) => self.collect_type_expr_references(&ty.element),
            TypeExpr::Array(ty) => self.collect_type_expr_references(&ty.element),
            TypeExpr::Optional(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::Fallible(ty) => {
                self.collect_type_expr_references(&ty.success);
                self.collect_type_expr_references(&ty.error);
            }
        }
    }

    fn record_type_reference(&mut self, name: &str, span: ByteSpan) {
        let (symbol_name_span, symbol_declaration_span) =
            match self.resolved.symbols.symbol_by_name(name) {
                Some(symbol) if matches!(symbol.kind, SymbolKind::Type(_)) => {
                    (Some(symbol.name_span), Some(symbol.declaration_span))
                }
                Some(_) | None => (None, None),
            };

        self.facts.type_references.push(TypeReferenceFact {
            name: name.to_string(),
            span,
            symbol_name_span,
            symbol_declaration_span,
        });
    }

    fn record_parameter_bindings(
        &mut self,
        parameters: &[Parameter],
        environment: &TypeEnvironment,
    ) {
        for parameter in parameters {
            self.record_environment_binding(parameter.name_span, &parameter.name, environment);
        }
    }

    fn record_payload_binding(
        &mut self,
        payload: &SwitchPayloadBinding,
        environment: &TypeEnvironment,
    ) {
        self.record_environment_binding(payload.span, &payload.name, environment);
    }

    fn record_environment_binding(
        &mut self,
        name_span: ByteSpan,
        name: &str,
        environment: &TypeEnvironment,
    ) {
        if let Some(ty) = environment.get(name) {
            self.record_binding_type(name_span, ty);
        }
        self.record_environment_binding_readonly(name_span, name, environment);
    }

    fn record_environment_binding_readonly(
        &mut self,
        name_span: ByteSpan,
        name: &str,
        environment: &TypeEnvironment,
    ) {
        if environment.get(name).is_some() {
            self.facts
                .binding_readonly
                .insert(name_span, !environment.is_mutable_binding(name));
        }
    }

    fn record_binding(&mut self, name_span: ByteSpan, ty: &Type, is_mutable: bool) {
        self.record_binding_type(name_span, ty);
        self.facts.binding_readonly.insert(name_span, !is_mutable);
    }

    fn record_binding_type(&mut self, name_span: ByteSpan, ty: &Type) {
        if !ty.is_unknown_or_unresolved() {
            self.facts
                .binding_type_labels
                .insert(name_span, ty.display());
        }
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
