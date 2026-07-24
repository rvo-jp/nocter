//! Type facts produced from the same environment and expression typing logic as
//! the checker.

use super::bindings::continuing_binding_type;
use super::calls::{
    infer_generic_substitutions, method_member_for_call, resolved_call_signature,
    resolved_method_for_call,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_if_let_binding, environment_for_method,
    environment_for_parameters_in_impl, environment_for_pattern_conditional_arm,
    environment_for_switch_arm, environment_for_while_let_binding, function_self_type,
    impl_self_type,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::places::field_member_is_writable_place;
use super::structs::{resolved_struct_field_for_literal_field, resolved_struct_field_for_member};
use super::type_expr::{simple_type_from_display_name, type_expr_to_type_with_self_type};
use super::variants::resolved_enum_variant_for_member;
use crate::ast::{
    ArrayLength, ArrayType, AstFile, BindingStmt, Block, BorrowType, EnumDecl, EnumVariant, Expr,
    FallibleType, GenericParamList, GenericType, ImplDecl, ImplMember, InterpolatedStringPart,
    Item, MemberExpr, MethodDecl, OptionalType, Parameter, PointerType, Stmt, StructDecl,
    StructField, StructLiteralExpr, StructLiteralField, SwitchPayloadBinding, TypeAliasDecl,
    TypeExpr, TypeReference, ViewType,
};
use crate::resolve::{
    AssociatedFunctionSignature, FunctionSignature, MethodSignature, ParameterSignature,
    ResolveOutput, SymbolKind, TypeSymbol,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    binding_type_labels: HashMap<ByteSpan, String>,
    binding_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    binding_readonly: HashMap<ByteSpan, bool>,
    declaration_hover_labels: HashMap<ByteSpan, String>,
    call_hover_labels: HashMap<ByteSpan, String>,
    field_hover_labels: HashMap<ByteSpan, String>,
    enum_variant_hover_labels: HashMap<ByteSpan, String>,
    type_references: Vec<TypeReferenceFact>,
    field_targets: HashMap<ByteSpan, ByteSpan>,
    field_readonly: HashMap<ByteSpan, bool>,
    associated_function_targets: HashMap<ByteSpan, ByteSpan>,
    enum_variant_targets: HashMap<ByteSpan, ByteSpan>,
    method_call_targets: HashMap<ByteSpan, ByteSpan>,
    generic_function_call_spans: HashMap<ByteSpan, ByteSpan>,
    function_call_specializations: HashMap<ByteSpan, FunctionCallSpecialization>,
    generic_method_call_spans: HashMap<ByteSpan, ByteSpan>,
    method_call_specializations: HashMap<ByteSpan, MethodCallSpecialization>,
}

impl TypecheckFacts {
    pub(crate) fn binding_type_label(&self, name_span: ByteSpan) -> Option<&str> {
        self.binding_type_labels.get(&name_span).map(String::as_str)
    }

    pub(crate) fn binding_scalar_view_kind(
        &self,
        name_span: ByteSpan,
    ) -> Option<TypecheckScalarViewKind> {
        self.binding_scalar_view_kinds.get(&name_span).copied()
    }

    pub(crate) fn binding_is_readonly(&self, name_span: ByteSpan) -> Option<bool> {
        self.binding_readonly.get(&name_span).copied()
    }

    pub(crate) fn declaration_hover_label(&self, name_span: ByteSpan) -> Option<&str> {
        self.declaration_hover_labels
            .get(&name_span)
            .map(String::as_str)
    }

    pub(crate) fn call_hover_at_offset(&self, offset: usize) -> Option<(ByteSpan, &str)> {
        self.call_hover_labels
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, label)| (*span, label.as_str()))
    }

    pub(crate) fn field_hover_at_offset(&self, offset: usize) -> Option<(ByteSpan, &str)> {
        self.field_hover_labels
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, label)| (*span, label.as_str()))
    }

    pub(crate) fn enum_variant_hover_at_offset(&self, offset: usize) -> Option<(ByteSpan, &str)> {
        self.enum_variant_hover_labels
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, label)| (*span, label.as_str()))
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

    pub(crate) fn type_references(&self) -> impl Iterator<Item = &TypeReferenceFact> + '_ {
        self.type_references.iter()
    }

    pub(crate) fn method_call_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.method_call_targets.keys().copied()
    }

    pub(crate) fn field_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.field_targets.keys().copied()
    }

    pub(crate) fn field_is_readonly(&self, span: ByteSpan) -> Option<bool> {
        self.field_readonly.get(&span).copied()
    }

    pub(crate) fn field_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.field_targets.get(&member_span).copied()
    }

    pub(crate) fn associated_function_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.associated_function_targets.keys().copied()
    }

    pub(crate) fn enum_variant_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.enum_variant_targets.keys().copied()
    }

    pub(crate) fn method_call_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.method_call_targets.get(&member_span).copied()
    }

    pub(crate) fn generic_function_call_target(&self, call_span: ByteSpan) -> Option<ByteSpan> {
        self.generic_function_call_spans.get(&call_span).copied()
    }

    pub(crate) fn function_call_specialization(
        &self,
        call_span: ByteSpan,
    ) -> Option<&FunctionCallSpecialization> {
        self.function_call_specializations.get(&call_span)
    }

    pub(crate) fn function_call_specializations(
        &self,
    ) -> impl Iterator<Item = &FunctionCallSpecialization> + '_ {
        self.function_call_specializations.values()
    }

    pub(crate) fn generic_method_call_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.generic_method_call_spans.get(&member_span).copied()
    }

    pub(crate) fn method_call_specialization(
        &self,
        member_span: ByteSpan,
    ) -> Option<&MethodCallSpecialization> {
        self.method_call_specializations.get(&member_span)
    }

    pub(crate) fn method_call_specializations(
        &self,
    ) -> impl Iterator<Item = &MethodCallSpecialization> + '_ {
        self.method_call_specializations.values()
    }

    pub(crate) fn associated_function_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.associated_function_targets.get(&member_span).copied()
    }

    pub(crate) fn enum_variant_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.enum_variant_targets.get(&member_span).copied()
    }

    pub(crate) fn field_target_at_offset(&self, offset: usize) -> Option<(ByteSpan, ByteSpan)> {
        self.field_targets
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, target)| (*span, *target))
    }

    pub(crate) fn associated_function_target_at_offset(
        &self,
        offset: usize,
    ) -> Option<(ByteSpan, ByteSpan)> {
        self.associated_function_targets
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, target)| (*span, *target))
    }

    pub(crate) fn enum_variant_target_at_offset(
        &self,
        offset: usize,
    ) -> Option<(ByteSpan, ByteSpan)> {
        self.enum_variant_targets
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .map(|(span, target)| (*span, *target))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckScalarViewKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    U8Slice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeReferenceFact {
    pub(crate) name: String,
    pub(crate) span: ByteSpan,
    pub(crate) symbol_name_span: Option<ByteSpan>,
    pub(crate) symbol_declaration_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionCallSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodCallSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
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
                self.facts.declaration_hover_labels.insert(
                    function.name_span,
                    function_declaration_hover_label(function, self.resolved),
                );
                self.collect_generic_param_type_references(&function.generics);
                self.collect_parameter_type_references(&function.parameters.parameters);
                self.collect_type_expr_references(&function.return_type);
            }
            Item::Primitive(primitive) => {
                self.facts.declaration_hover_labels.insert(
                    primitive.name_span,
                    primitive_declaration_hover_label(primitive, self.resolved),
                );
                self.collect_generic_param_type_references(&primitive.generics);
                self.collect_parameter_type_references(&primitive.parameters.parameters);
                self.collect_type_expr_references(&primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                self.facts.declaration_hover_labels.insert(
                    alias.name_span,
                    type_alias_declaration_hover_label(alias, self.resolved),
                );
                self.collect_generic_param_type_references(&alias.generics);
                self.collect_type_expr_references(&alias.target);
            }
            Item::Struct(struct_) => {
                self.facts.declaration_hover_labels.insert(
                    struct_.name_span,
                    struct_declaration_hover_label(struct_, self.resolved),
                );
                self.collect_generic_param_type_references(&struct_.generics);
                for field in &struct_.fields {
                    self.facts.declaration_hover_labels.insert(
                        field.name_span,
                        struct_field_declaration_hover_label(field, self.resolved),
                    );
                    self.collect_type_expr_references(&field.ty);
                }
            }
            Item::Enum(enum_) => {
                self.facts.declaration_hover_labels.insert(
                    enum_.name_span,
                    enum_declaration_hover_label(enum_, self.resolved),
                );
                self.collect_generic_param_type_references(&enum_.generics);
                for variant in &enum_.variants {
                    self.facts.declaration_hover_labels.insert(
                        variant.name_span,
                        enum_variant_declaration_hover_label(variant, self.resolved),
                    );
                    self.collect_parameter_type_references(&variant.payload);
                }
            }
            Item::Interface(interface) => {
                self.collect_generic_param_type_references(&interface.generics);
                for method in &interface.methods {
                    self.facts.declaration_hover_labels.insert(
                        method.name_span,
                        method_declaration_hover_label(method, self.resolved, None),
                    );
                    self.collect_method_signature_type_references(method);
                }
            }
            Item::Impl(impl_) => {
                let self_type = impl_self_type(impl_, self.resolved);
                if let Some(interface_ty) = &impl_.interface_ty {
                    self.collect_type_expr_references(interface_ty);
                }
                self.collect_type_expr_references(&impl_.target_ty);
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            self.facts.declaration_hover_labels.insert(
                                method.name_span,
                                method_declaration_hover_label(
                                    method,
                                    self.resolved,
                                    Some(&self_type),
                                ),
                            );
                            self.collect_method_signature_type_references(method);
                        }
                        ImplMember::Drop(drop_) => {
                            self.facts.declaration_hover_labels.insert(
                                drop_.binding.name_span,
                                drop_declaration_hover_label(drop_, self.resolved, &self_type),
                            );
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
            | Item::Interface(_) => {}
        }
    }

    fn collect_impl_member_body_facts(&mut self, impl_: &ImplDecl) {
        for member in &impl_.members {
            match member {
                ImplMember::Method(method) => {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut environment = environment_for_method(method, self.resolved, impl_);
                    self.record_parameter_bindings(
                        std::slice::from_ref(&method.receiver),
                        &environment,
                    );
                    self.record_parameter_bindings(&method.parameters.parameters, &environment);
                    self.collect_block_facts(body, &mut environment);
                }
                ImplMember::Drop(drop_) => {
                    let mut environment = environment_for_parameters_in_impl(
                        std::slice::from_ref(&drop_.binding),
                        self.resolved,
                        impl_,
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
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &statement.expression,
                        self.resolved,
                        environment,
                    );
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
                    && let Some((owner, resolved_method)) =
                        resolved_method_for_call(self.resolved, expression, environment)
                {
                    self.facts
                        .method_call_targets
                        .insert(method.member_span, resolved_method.name_span);
                    if !resolved_method.signature.generic_parameters.is_empty() {
                        self.facts
                            .generic_method_call_spans
                            .insert(method.member_span, resolved_method.name_span);
                        if let Some(specialization) = method_call_specialization(
                            expression,
                            method,
                            resolved_method,
                            self.resolved,
                            environment,
                        ) {
                            self.facts
                                .method_call_specializations
                                .insert(method.member_span, specialization);
                        }
                    }
                    self.facts.call_hover_labels.insert(
                        method.member_span,
                        method_signature_hover_label(resolved_method, owner, self.resolved),
                    );
                    self.collect_expression_facts(&method.object, environment);
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, resolved_function)) =
                        self.resolved.associated_function_for_call(expression)
                {
                    self.facts
                        .associated_function_targets
                        .insert(method.member_span, resolved_function.name_span);
                    self.record_generic_function_call_specialization(
                        expression,
                        resolved_function.name_span,
                        &resolved_function.target_name,
                        &resolved_function.signature,
                        environment,
                    );
                    self.facts.call_hover_labels.insert(
                        method.member_span,
                        associated_function_signature_hover_label(
                            owner,
                            resolved_function,
                            self.resolved,
                        ),
                    );
                    self.collect_expression_facts(&method.object, environment);
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, variant)) =
                        resolved_enum_variant_for_member(method, self.resolved)
                {
                    self.record_enum_variant_reference(method.member_span, owner, variant);
                    self.collect_expression_facts(&method.object, environment);
                } else {
                    if let Some(symbol) = self.resolved.symbol_for_call(expression)
                        && let SymbolKind::Function(signature) = &symbol.kind
                    {
                        self.record_generic_function_call_specialization(
                            expression,
                            symbol.declaration_span,
                            &symbol.name,
                            signature,
                            environment,
                        );
                    }
                    self.collect_expression_facts(&expression.callee, environment);
                }

                for argument in &expression.arguments {
                    self.collect_expression_facts(argument, environment);
                }
            }
            Expr::Member(expression) => {
                self.collect_expression_facts(&expression.object, environment);
                self.record_struct_field_member_reference(expression, environment);
                if let Some((owner, variant)) =
                    resolved_enum_variant_for_member(expression, self.resolved)
                {
                    self.record_enum_variant_reference(expression.member_span, owner, variant);
                }
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
                    self.record_struct_literal_field_reference(expression, field, environment);
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
                    let mut arm_environment = environment_for_pattern_conditional_arm(
                        arm,
                        &expression.target,
                        self.resolved,
                        environment,
                    );
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
                .insert(name_span, type_hover_label(ty, self.resolved));
        }
        if let Some(kind) = scalar_view_kind(ty) {
            self.facts.binding_scalar_view_kinds.insert(name_span, kind);
        }
    }

    fn record_struct_field_member_reference(
        &mut self,
        member: &MemberExpr,
        environment: &TypeEnvironment,
    ) {
        let Some((owner, field)) =
            resolved_struct_field_for_member(member, self.resolved, environment)
        else {
            return;
        };

        self.facts.field_readonly.insert(
            member.member_span,
            !field_member_is_writable_place(member, self.resolved, environment),
        );
        self.record_struct_field_reference(member.member_span, owner, field, environment);
    }

    fn record_struct_literal_field_reference(
        &mut self,
        literal: &StructLiteralExpr,
        field: &StructLiteralField,
        environment: &TypeEnvironment,
    ) {
        let Some((owner, expected_field)) =
            resolved_struct_field_for_literal_field(literal, field, self.resolved, environment)
        else {
            return;
        };

        self.record_struct_field_reference(field.name_span, owner, expected_field, environment);
    }

    fn record_struct_field_reference(
        &mut self,
        span: ByteSpan,
        owner: &TypeSymbol,
        field: &crate::resolve::StructFieldSignature,
        environment: &TypeEnvironment,
    ) {
        self.facts.field_targets.insert(span, field.name_span);
        self.facts.field_hover_labels.insert(
            span,
            format!(
                "field {}.{}: {}",
                type_owner_hover_label(owner, self.resolved),
                field.name,
                type_hover_label(
                    &type_expr_to_type_with_self_type(
                        &field.ty,
                        self.resolved,
                        environment.self_type()
                    ),
                    self.resolved
                )
            ),
        );
    }

    fn record_enum_variant_reference(
        &mut self,
        span: ByteSpan,
        owner: &TypeSymbol,
        variant: &crate::resolve::EnumVariantSignature,
    ) {
        self.facts
            .enum_variant_targets
            .insert(span, variant.name_span);
        self.facts.enum_variant_hover_labels.insert(
            span,
            enum_variant_signature_hover_label(owner, variant, self.resolved),
        );
    }

    fn record_generic_function_call_specialization(
        &mut self,
        call: &crate::ast::CallExpr,
        declaration_span: ByteSpan,
        base_target_name: &str,
        signature: &FunctionSignature,
        environment: &TypeEnvironment,
    ) {
        if signature.generic_parameters.is_empty() {
            return;
        }
        self.facts
            .generic_function_call_spans
            .insert(call.span, declaration_span);
        if let Some(specialization) = function_call_specialization(
            call,
            declaration_span,
            base_target_name,
            signature,
            self.resolved,
            environment,
        ) {
            self.facts
                .function_call_specializations
                .insert(call.span, specialization);
        }
    }
}

fn function_call_specialization(
    call: &crate::ast::CallExpr,
    declaration_span: ByteSpan,
    base_target_name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<FunctionCallSpecialization> {
    let checked = resolved_call_signature(resolved, call, environment)?;
    let substitution_types = infer_generic_substitutions(call, &checked, resolved, environment);
    if !signature
        .generic_parameters
        .iter()
        .all(|parameter| substitution_types.contains_key(parameter))
    {
        return None;
    }
    let type_arguments = signature
        .generic_parameters
        .iter()
        .map(|parameter| substitution_types.get(parameter).map(Type::display))
        .collect::<Option<Vec<_>>>()?;
    let substitutions = substitution_types
        .into_iter()
        .map(|(name, ty)| type_to_type_expr(&ty, call.span).map(|ty| (name, ty)))
        .collect::<Option<HashMap<_, _>>>()?;

    Some(FunctionCallSpecialization {
        declaration_span,
        target_name: format!("{base_target_name}<{}>", type_arguments.join(", ")),
        substitutions,
    })
}

fn method_call_specialization(
    call: &crate::ast::CallExpr,
    member: &MemberExpr,
    method: &MethodSignature,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<MethodCallSpecialization> {
    let receiver_type = expression_type(&member.object, resolved, environment);
    let self_ty = type_to_type_expr(&receiver_type, member.object.span())?;
    let checked = resolved_call_signature(resolved, call, environment)?;
    let substitutions = infer_generic_substitutions(call, &checked, resolved, environment)
        .into_iter()
        .map(|(name, ty)| type_to_type_expr(&ty, member.member_span).map(|ty| (name, ty)))
        .collect::<Option<HashMap<_, _>>>()?;
    if !method
        .signature
        .generic_parameters
        .iter()
        .all(|parameter| substitutions.contains_key(parameter))
    {
        return None;
    }

    Some(MethodCallSpecialization {
        declaration_span: method.name_span,
        target_name: format!("{}.{}", receiver_type.display(), method.name),
        self_ty,
        substitutions,
    })
}

fn type_to_type_expr(ty: &Type, span: ByteSpan) -> Option<TypeExpr> {
    match ty {
        Type::I32 => Some(type_reference("i32", span)),
        Type::Primitive(name) => Some(type_reference(name, span)),
        Type::Named(name) if name.starts_with("&+") => {
            borrowed_display_type_to_type_expr(name.strip_prefix("&+")?, true, span)
        }
        Type::Named(name) if name.starts_with('&') => {
            borrowed_display_type_to_type_expr(name.strip_prefix('&')?, false, span)
        }
        Type::Named(name) => Some(type_reference(name, span)),
        Type::StrData => Some(type_reference("str", span)),
        Type::Str => Some(TypeExpr::Borrow(BorrowType {
            span,
            is_readwrite: false,
            inner: Box::new(type_reference("str", span)),
        })),
        Type::Error => Some(type_reference("error", span)),
        Type::Void => Some(type_reference("void", span)),
        Type::Never => Some(type_reference("never", span)),
        Type::ArrayData { element } => Some(TypeExpr::View(ViewType {
            span,
            is_readwrite: false,
            element: Box::new(type_to_type_expr(element, span)?),
        })),
        Type::View {
            is_readwrite,
            element,
        } => Some(TypeExpr::Borrow(BorrowType {
            span,
            is_readwrite: *is_readwrite,
            inner: Box::new(TypeExpr::View(ViewType {
                span,
                is_readwrite: false,
                element: Box::new(type_to_type_expr(element, span)?),
            })),
        })),
        Type::Array { element, length } => Some(TypeExpr::Array(ArrayType {
            span,
            element: Box::new(type_to_type_expr(element, span)?),
            length: ArrayLength {
                span,
                value: length.clone(),
            },
        })),
        Type::Pointer(inner) => Some(TypeExpr::Pointer(PointerType {
            span,
            inner: Box::new(type_to_type_expr(inner, span)?),
        })),
        Type::Optional(inner) => Some(TypeExpr::Optional(OptionalType {
            span,
            inner: Box::new(type_to_type_expr(inner, span)?),
        })),
        Type::Fallible { success, error } => Some(TypeExpr::Fallible(FallibleType {
            span,
            success: Box::new(type_to_type_expr(success, span)?),
            error: Box::new(type_to_type_expr(error, span)?),
        })),
        Type::Generic { name, arguments } => Some(TypeExpr::Generic(GenericType {
            span,
            name: name.clone(),
            name_span: span,
            arguments: arguments
                .iter()
                .map(|argument| type_to_type_expr(argument, span))
                .collect::<Option<Vec<_>>>()?,
        })),
        Type::None | Type::Parameter(_) | Type::Unresolved(_) | Type::Unknown => None,
    }
}

fn type_reference(name: impl Into<String>, span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: name.into(),
    })
}

fn borrowed_display_type_to_type_expr(
    inner: &str,
    is_readwrite: bool,
    span: ByteSpan,
) -> Option<TypeExpr> {
    Some(TypeExpr::Borrow(BorrowType {
        span,
        is_readwrite,
        inner: Box::new(type_to_type_expr(
            &simple_type_from_display_name(inner),
            span,
        )?),
    }))
}

fn scalar_view_kind(ty: &Type) -> Option<TypecheckScalarViewKind> {
    match ty {
        Type::I32 => Some(TypecheckScalarViewKind::I32),
        Type::Primitive(name) if name == "u8" => Some(TypecheckScalarViewKind::U8),
        Type::Primitive(name) if name == "usize" => Some(TypecheckScalarViewKind::Usize),
        Type::Primitive(name) if name == "bool" => Some(TypecheckScalarViewKind::Bool),
        Type::Str => Some(TypecheckScalarViewKind::Str),
        Type::View { element, .. } if matches!(element.as_ref(), Type::Primitive(name) if name == "u8") => {
            Some(TypecheckScalarViewKind::U8Slice)
        }
        _ => None,
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn function_declaration_hover_label(
    function: &crate::ast::FunctionDecl,
    resolved: &ResolveOutput,
) -> String {
    let self_type = function_self_type(function, resolved);
    format!(
        "func {}{}({}): {}",
        function.name,
        generic_parameters_label(&function.generics, resolved, self_type.as_ref()),
        parameters_label(
            &function.parameters.parameters,
            resolved,
            self_type.as_ref()
        ),
        type_label(&function.return_type, resolved, self_type.as_ref())
    )
}

fn primitive_declaration_hover_label(
    primitive: &crate::ast::PrimitiveDecl,
    resolved: &ResolveOutput,
) -> String {
    format!(
        "primitive {}{}({}): {}",
        primitive.name,
        generic_parameters_label(&primitive.generics, resolved, None),
        parameters_label(&primitive.parameters.parameters, resolved, None),
        type_label(&primitive.return_type, resolved, None)
    )
}

fn type_alias_declaration_hover_label(alias: &TypeAliasDecl, resolved: &ResolveOutput) -> String {
    format!(
        "type {}{} = {}",
        alias.name,
        generic_parameters_label(&alias.generics, resolved, None),
        type_label(&alias.target, resolved, None)
    )
}

fn struct_declaration_hover_label(struct_: &StructDecl, resolved: &ResolveOutput) -> String {
    let copy_prefix = if struct_.is_copy { "copy " } else { "" };
    format!(
        "{copy_prefix}struct {}{}",
        struct_.name,
        generic_parameters_label(&struct_.generics, resolved, None)
    )
}

fn struct_field_declaration_hover_label(field: &StructField, resolved: &ResolveOutput) -> String {
    format!(
        "field {}: {}",
        field.name,
        type_label(&field.ty, resolved, None)
    )
}

fn enum_declaration_hover_label(enum_: &EnumDecl, resolved: &ResolveOutput) -> String {
    format!(
        "enum {}{}",
        enum_.name,
        generic_parameters_label(&enum_.generics, resolved, None)
    )
}

fn enum_variant_declaration_hover_label(variant: &EnumVariant, resolved: &ResolveOutput) -> String {
    if variant.payload.is_empty() {
        return format!("variant {}", variant.name);
    }

    format!(
        "variant {}({})",
        variant.name,
        parameters_label(&variant.payload, resolved, None)
    )
}

fn enum_variant_signature_hover_label(
    owner: &TypeSymbol,
    variant: &crate::resolve::EnumVariantSignature,
    resolved: &ResolveOutput,
) -> String {
    if variant.payload.is_empty() {
        return format!(
            "variant {}.{}",
            type_owner_hover_label(owner, resolved),
            variant.name
        );
    }

    format!(
        "variant {}.{}({})",
        type_owner_hover_label(owner, resolved),
        variant.name,
        parameter_signatures_label(&variant.payload, resolved, None)
    )
}

fn method_declaration_hover_label(
    method: &MethodDecl,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    format!(
        "method {}.{}({}): {}",
        method_receiver_label(&method.receiver, resolved, self_type),
        method.name,
        parameters_label(&method.parameters.parameters, resolved, self_type),
        type_label(&method.return_type, resolved, self_type)
    )
}

fn drop_declaration_hover_label(
    drop_: &crate::ast::DropDecl,
    resolved: &ResolveOutput,
    self_type: &Type,
) -> String {
    format!(
        "drop {}",
        method_receiver_label(&drop_.binding, resolved, Some(self_type))
    )
}

fn associated_function_signature_hover_label(
    owner: &TypeSymbol,
    function: &AssociatedFunctionSignature,
    resolved: &ResolveOutput,
) -> String {
    let self_type = Type::Named(owner.canonical_name.clone());
    let name = format!(
        "{}.{}",
        type_owner_hover_label(owner, resolved),
        function.name
    );
    function_signature_hover_label(
        "func",
        &name,
        &function.signature,
        resolved,
        Some(&self_type),
    )
}

fn method_signature_hover_label(
    method: &MethodSignature,
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    let self_type = Type::Named(owner.canonical_name.clone());
    format!(
        "method {}.{}({}): {}",
        method_signature_receiver_label(&method.receiver, resolved, Some(&self_type)),
        method.name,
        parameter_signatures_label(&method.signature.parameters, resolved, Some(&self_type)),
        type_label(&method.signature.return_type, resolved, Some(&self_type))
    )
}

fn method_receiver_label(
    receiver: &Parameter,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    match self_receiver_prefix(&receiver.ty) {
        Some(prefix) => format!("{prefix}{}", receiver.name),
        None => format!(
            "{}: {}",
            receiver.name,
            type_label(&receiver.ty, resolved, self_type)
        ),
    }
}

fn method_signature_receiver_label(
    receiver: &ParameterSignature,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    if let Some(prefix) = self_receiver_prefix(&receiver.ty) {
        return format!("{prefix}{}", receiver.name);
    }
    format!(
        "{}: {}",
        receiver.name,
        parameter_signature_type_label(receiver, resolved, self_type)
    )
}

fn self_receiver_prefix(ty: &TypeExpr) -> Option<&'static str> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => Some(""),
        TypeExpr::Borrow(borrow) => match borrow.inner.as_ref() {
            TypeExpr::Reference(reference) if reference.name == "Self" => {
                Some(if borrow.is_readwrite { "&+" } else { "&" })
            }
            _ => None,
        },
        _ => None,
    }
}

fn function_signature_hover_label(
    kind: &str,
    name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    format!(
        "{kind} {name}({}): {}",
        parameter_signatures_label(&signature.parameters, resolved, self_type),
        type_label(&signature.return_type, resolved, self_type)
    )
}

fn generic_parameters_label(
    generics: &GenericParamList,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    if generics.parameters.is_empty() {
        return String::new();
    }

    let parameters = generics
        .parameters
        .iter()
        .map(|parameter| match &parameter.bound {
            Some(bound) => format!(
                "{}: {}",
                parameter.name,
                type_label(bound, resolved, self_type)
            ),
            None => parameter.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("<{parameters}>")
}

fn parameters_label(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                type_label(&parameter.ty, resolved, self_type)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parameter_signatures_label(
    parameters: &[ParameterSignature],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                parameter_signature_type_label(parameter, resolved, self_type)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parameter_signature_type_label(
    parameter: &ParameterSignature,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    type_label(&parameter.ty, resolved, self_type)
}

fn type_label(ty: &TypeExpr, resolved: &ResolveOutput, self_type: Option<&Type>) -> String {
    type_hover_label(
        &type_expr_to_type_with_self_type(ty, resolved, self_type),
        resolved,
    )
}

fn type_hover_label(ty: &Type, resolved: &ResolveOutput) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Primitive(name) => name.clone(),
        Type::StrData => "str".to_string(),
        Type::Str => "&str".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::None => "none".to_string(),
        Type::ArrayData { element } => format!("[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: true,
            element,
        } => format!("&+[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: false,
            element,
        } => format!("&[{}]", type_hover_label(element, resolved)),
        Type::Array { element, length } => {
            format!("[{}; {}]", type_hover_label(element, resolved), length)
        }
        Type::Pointer(inner) => format!("*{}", type_hover_label(inner, resolved)),
        Type::Optional(inner) => format!("{}?", type_hover_label(inner, resolved)),
        Type::Fallible { success, .. } => format!("{}!", type_hover_label(success, resolved)),
        Type::Named(name) => display_type_name(name, resolved).to_string(),
        Type::Generic { name, arguments } => {
            let name = display_type_name(name, resolved);
            let arguments = arguments
                .iter()
                .map(|argument| type_hover_label(argument, resolved))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{arguments}>")
        }
        Type::Parameter(name) => name.clone(),
        Type::Unresolved(name) => name.clone(),
        Type::Unknown => "<unknown>".to_string(),
    }
}

fn type_owner_hover_label<'a>(owner: &'a TypeSymbol, resolved: &'a ResolveOutput) -> &'a str {
    display_type_name(&owner.canonical_name, resolved)
}

fn display_type_name<'a>(canonical_name: &'a str, resolved: &'a ResolveOutput) -> &'a str {
    visible_type_name(canonical_name, resolved).unwrap_or_else(|| short_type_name(canonical_name))
}

fn short_type_name(canonical_name: &str) -> &str {
    canonical_name
        .rsplit_once('.')
        .map(|(_, name)| name)
        .unwrap_or(canonical_name)
}

fn visible_type_name<'a>(canonical_name: &str, resolved: &'a ResolveOutput) -> Option<&'a str> {
    resolved
        .symbols
        .symbols()
        .filter_map(|symbol| match &symbol.kind {
            SymbolKind::Type(type_symbol)
                if type_symbol.canonical_name == canonical_name
                    && symbol.name != canonical_name =>
            {
                Some(symbol.name.as_str())
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })
        .min_by_key(|name| name.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::source::SourceMap;

    #[test]
    fn type_hover_label_shortens_hidden_canonical_names() {
        let resolved = resolve_text("func main(): i32 {\n    return 0\n}\n");

        assert_eq!(
            type_hover_label(&Type::Named("std/string.String".to_string()), &resolved),
            "String"
        );
        assert_eq!(
            type_hover_label(
                &Type::Generic {
                    name: "std/vec.Vec".to_string(),
                    arguments: vec![Type::Named("std/string.String".to_string())],
                },
                &resolved,
            ),
            "Vec<String>"
        );
    }

    fn resolve_text(text: &str) -> ResolveOutput {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let lex_output = lex(&sources, source);
        assert!(
            lex_output.diagnostics.is_empty(),
            "unexpected lex diagnostics: {:?}",
            lex_output.diagnostics
        );
        let parse_output = parse(&sources, source, &lex_output.tokens);
        assert!(
            parse_output.diagnostics.is_empty(),
            "unexpected parse diagnostics: {:?}",
            parse_output.diagnostics
        );
        let ast = parse_output.ast.expect("expected ast");
        resolve(&sources, &ast)
    }
}
