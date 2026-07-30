//! Type facts produced from the same environment and expression typing logic as
//! the checker.

use super::bindings::continuing_binding_type;
use super::calls::{
    infer_generic_substitutions, method_member_for_call, method_self_type_for_receiver,
    resolved_call_signature, resolved_method_for_call,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_method, environment_for_parameters_in_impl,
    environment_for_switch_arm, function_self_type, impl_self_type,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::places::field_member_is_writable_place;
use super::structs::{
    resolved_struct_field_for_literal_field, resolved_struct_field_for_member,
    struct_literal_field_type, struct_member_type,
};
use super::type_expr::{
    infer_type_expr_substitutions, simple_type_from_display_name, type_expr_display_lossy,
    type_expr_to_type_in_environment, type_expr_to_type_with_self_type,
    type_expr_to_type_with_substitutions,
};
use super::variants::resolved_enum_variant_for_member;
use crate::ast::{
    ArrayLength, ArrayType, AstFile, BindingStmt, Block, BorrowType, CallExpr, EnumDecl,
    EnumVariant, Expr, FallibleType, GenericParamList, GenericType, IfIsStmt, ImplDecl, ImplMember,
    InterpolatedStringPart, Item, MemberExpr, MethodDecl, OptionalType, Parameter, PointerType,
    Stmt, StructDecl, StructField, StructLiteralExpr, StructLiteralField, SwitchArm,
    SwitchPayloadBinding, TypeAliasDecl, TypeExpr, TypeReference, ViewType,
    substitute_type_expr_parameters,
};
use crate::resolve::{
    AssociatedFunctionSignature, FunctionSignature, MethodSignature, ParameterSignature,
    ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind,
};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    binding_type_labels: HashMap<ByteSpan, String>,
    binding_type_exprs: HashMap<ByteSpan, TypeExpr>,
    expression_type_exprs: HashMap<ByteSpan, TypeExpr>,
    binding_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    binding_readonly: HashMap<ByteSpan, bool>,
    declaration_hover_labels: HashMap<ByteSpan, String>,
    call_hover_labels: HashMap<ByteSpan, String>,
    field_hover_labels: HashMap<ByteSpan, String>,
    enum_variant_hover_labels: HashMap<ByteSpan, String>,
    type_references: Vec<TypeReferenceFact>,
    field_targets: HashMap<ByteSpan, ByteSpan>,
    field_type_exprs: HashMap<ByteSpan, TypeExpr>,
    field_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    field_readonly: HashMap<ByteSpan, bool>,
    function_call_targets: HashMap<ByteSpan, ByteSpan>,
    associated_function_targets: HashMap<ByteSpan, ByteSpan>,
    enum_variant_targets: HashMap<ByteSpan, ByteSpan>,
    method_call_targets: HashMap<ByteSpan, ByteSpan>,
    method_call_receiver_kinds: HashMap<ByteSpan, TypecheckMethodReceiverKind>,
    generic_function_call_spans: HashMap<ByteSpan, ByteSpan>,
    function_call_specializations: HashMap<ByteSpan, FunctionCallSpecialization>,
    generic_method_call_spans: HashMap<ByteSpan, ByteSpan>,
    method_call_specializations: HashMap<ByteSpan, MethodCallSpecialization>,
    drop_type_specializations: Vec<DropTypeSpecialization>,
    field_drop_type_specializations: HashMap<ByteSpan, DropTypeSpecialization>,
}

impl TypecheckFacts {
    pub(crate) fn binding_type_label(&self, name_span: ByteSpan) -> Option<&str> {
        self.binding_type_labels.get(&name_span).map(String::as_str)
    }

    pub(crate) fn binding_type_expr(&self, name_span: ByteSpan) -> Option<&TypeExpr> {
        self.binding_type_exprs.get(&name_span)
    }

    pub(crate) fn expression_type_expr(&self, expression_span: ByteSpan) -> Option<&TypeExpr> {
        self.expression_type_exprs.get(&expression_span)
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

    pub(crate) fn field_type_expr(&self, field_span: ByteSpan) -> Option<&TypeExpr> {
        self.field_type_exprs.get(&field_span)
    }

    pub(crate) fn field_scalar_view_kind(
        &self,
        member_span: ByteSpan,
    ) -> Option<TypecheckScalarViewKind> {
        self.field_scalar_view_kinds.get(&member_span).copied()
    }

    pub(crate) fn associated_function_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.associated_function_targets.keys().copied()
    }

    pub(crate) fn function_call_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.function_call_targets.keys().copied()
    }

    pub(crate) fn enum_variant_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.enum_variant_targets.keys().copied()
    }

    pub(crate) fn function_call_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.function_call_targets.get(&member_span).copied()
    }

    pub(crate) fn method_call_target(&self, member_span: ByteSpan) -> Option<ByteSpan> {
        self.method_call_targets.get(&member_span).copied()
    }

    pub(crate) fn method_call_receiver_kind(
        &self,
        member_span: ByteSpan,
    ) -> Option<TypecheckMethodReceiverKind> {
        self.method_call_receiver_kinds.get(&member_span).copied()
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

    pub(crate) fn function_call_specialization_entries(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &FunctionCallSpecialization)> + '_ {
        self.function_call_specializations
            .iter()
            .map(|(span, specialization)| (*span, specialization))
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

    pub(crate) fn method_call_specialization_entries(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &MethodCallSpecialization)> + '_ {
        self.method_call_specializations
            .iter()
            .map(|(span, specialization)| (*span, specialization))
    }

    pub(crate) fn drop_type_specializations(
        &self,
    ) -> impl Iterator<Item = &DropTypeSpecialization> + '_ {
        self.drop_type_specializations.iter()
    }

    pub(crate) fn field_drop_type_specialization(
        &self,
        member_span: ByteSpan,
    ) -> Option<&DropTypeSpecialization> {
        self.field_drop_type_specializations.get(&member_span)
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

    pub(crate) fn function_call_target_at_offset(
        &self,
        offset: usize,
    ) -> Option<(ByteSpan, ByteSpan)> {
        self.function_call_targets
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
    Slice(TypecheckSliceElementKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckSliceElementKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckMethodReceiverKind {
    Owned,
    ReadonlyBorrow,
    ReadwriteBorrow,
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
    base_target_name: String,
    generic_parameters: Vec<String>,
    pub(crate) target_name: String,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    free_type_parameters: HashSet<String>,
}

impl FunctionCallSpecialization {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let mut substitutions = HashMap::new();
        for parameter in &self.generic_parameters {
            let ty = self.substitutions.get(parameter)?;
            substitutions.insert(
                parameter.clone(),
                substitute_type_expr_parameters(ty, context_substitutions),
            );
        }
        if substitutions
            .values()
            .any(|ty| type_expr_contains_free_parameters(ty, &self.free_type_parameters))
        {
            return None;
        }
        let target_name = specialized_target_name(
            &self.base_target_name,
            &self.generic_parameters,
            &substitutions,
        )?;

        Some(Self {
            declaration_span: self.declaration_span,
            base_target_name: self.base_target_name.clone(),
            generic_parameters: self.generic_parameters.clone(),
            target_name,
            substitutions,
            free_type_parameters: HashSet::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodCallSpecialization {
    pub(crate) declaration_span: ByteSpan,
    method_name: String,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    generic_parameters: Vec<String>,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    free_type_parameters: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DropTypeSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    base_target_name: String,
    free_type_parameters: HashSet<String>,
}

impl DropTypeSpecialization {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let self_ty = substitute_type_expr_parameters(&self.self_ty, context_substitutions);
        if type_expr_contains_free_parameters(&self_ty, &self.free_type_parameters) {
            return None;
        }

        Some(Self {
            declaration_span: self.declaration_span,
            target_name: drop_target_name_from_base_and_self_ty(&self.base_target_name, &self_ty),
            self_ty,
            base_target_name: self.base_target_name.clone(),
            free_type_parameters: HashSet::new(),
        })
    }
}

impl MethodCallSpecialization {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let self_ty = substitute_type_expr_parameters(&self.self_ty, context_substitutions);
        let mut substitutions = HashMap::new();
        for parameter in &self.generic_parameters {
            let ty = self.substitutions.get(parameter)?;
            substitutions.insert(
                parameter.clone(),
                substitute_type_expr_parameters(ty, context_substitutions),
            );
        }
        if type_expr_contains_free_parameters(&self_ty, &self.free_type_parameters)
            || substitutions
                .values()
                .any(|ty| type_expr_contains_free_parameters(ty, &self.free_type_parameters))
        {
            return None;
        }
        let target_name = method_target_name_from_self_ty(&self_ty, &self.method_name);

        Some(Self {
            declaration_span: self.declaration_span,
            method_name: self.method_name.clone(),
            target_name,
            self_ty,
            generic_parameters: self.generic_parameters.clone(),
            substitutions,
            free_type_parameters: HashSet::new(),
        })
    }
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
            Item::Import(_) | Item::FromImport(_) => {}
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
                let return_type = type_expr_to_type_in_environment(
                    &function.return_type,
                    self.resolved,
                    &environment,
                );
                let return_success_type = return_type.success_type().clone();
                self.collect_block_facts(
                    &function.body,
                    &mut environment,
                    Some(&return_success_type),
                );
            }
            Item::Impl(impl_) => self.collect_impl_member_body_facts(impl_),
            Item::Import(_)
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
                    let return_type = type_expr_to_type_in_environment(
                        &method.return_type,
                        self.resolved,
                        &environment,
                    );
                    let return_success_type = return_type.success_type().clone();
                    self.collect_block_facts(body, &mut environment, Some(&return_success_type));
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
                    let return_type = Type::Void;
                    self.collect_block_facts(&drop_.body, &mut environment, Some(&return_type));
                }
            }
        }
    }

    fn collect_block_facts(
        &mut self,
        block: &Block,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        for statement in &block.statements {
            self.collect_statement_facts(statement, environment, return_type);
        }
        if let Some(result) = &block.result {
            if let Some(return_type) = return_type {
                self.collect_expression_facts_with_expected(
                    result,
                    return_type,
                    environment,
                    Some(return_type),
                );
            } else {
                self.collect_expression_facts(result, environment);
            }
        }
    }

    fn collect_statement_facts(
        &mut self,
        statement: &Stmt,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    if let Some(return_type) = return_type {
                        self.collect_expression_facts_with_expected(
                            expression,
                            return_type,
                            environment,
                            Some(return_type),
                        );
                    } else {
                        self.collect_expression_facts(expression, environment);
                    }
                }
            }
            Stmt::Binding(statement) => {
                self.collect_binding_statement_facts(statement, environment, return_type)
            }
            Stmt::Assignment(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.target,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &statement.value,
                    environment,
                    return_type,
                );
            }
            Stmt::If(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.condition,
                    environment,
                    return_type,
                );

                let mut then_environment = environment.clone();
                self.collect_block_facts(&statement.then_block, &mut then_environment, return_type);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Stmt::IfIs(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
                self.record_if_is_pattern_references(statement);

                let mut then_environment =
                    environment_for_if_is_binding(statement, self.resolved, environment);
                if let Some(payload) = statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.record_payload_binding(payload, &then_environment);
                }
                self.collect_block_facts(&statement.then_block, &mut then_environment, return_type);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Stmt::Switch(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
                for arm in &statement.arms {
                    self.record_switch_arm_pattern_references(arm);
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &statement.expression,
                        self.resolved,
                        environment,
                    );
                    if let Some(payload) =
                        arm.payload.as_ref().and_then(|payload| payload.binding())
                    {
                        self.record_payload_binding(payload, &arm_environment);
                    }
                    self.collect_block_facts(&arm.body, &mut arm_environment, return_type);
                }
                if let Some(arm) = &statement.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(&arm.body, &mut else_environment, return_type);
                }
            }
            Stmt::ForRange(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.start,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(&statement.end, environment, return_type);

                let mut body_environment =
                    environment_for_for_range_binding(statement, self.resolved, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::While(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.condition,
                    environment,
                    return_type,
                );

                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::Loop(statement) => {
                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::Expression(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
            }
            Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn collect_binding_statement_facts(
        &mut self,
        statement: &BindingStmt,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        let expected_initializer_type = statement.ty.as_ref().map(|ty| {
            self.collect_type_expr_references(ty);
            type_expr_to_type_in_environment(ty, self.resolved, environment)
        });
        if let Some(expected) = &expected_initializer_type {
            self.collect_expression_facts_with_expected(
                &statement.initializer,
                expected,
                environment,
                return_type,
            );
        } else {
            self.collect_expression_facts_in_context(
                &statement.initializer,
                environment,
                return_type,
            );
        }
        let initializer_type = expression_type(&statement.initializer, self.resolved, environment);

        let binding_type =
            continuing_binding_type(statement, initializer_type, self.resolved, environment);
        let is_mutable = binding_kind_is_mutable(statement.kind);
        self.record_binding(statement.name_span, &binding_type, is_mutable);
        if let Some(ty) = &statement.ty {
            self.facts
                .binding_type_exprs
                .insert(statement.name_span, ty.clone());
        }
        environment.define_binding(statement.name.clone(), binding_type, is_mutable);
    }

    fn collect_expression_facts_with_expected(
        &mut self,
        expression: &Expr,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        self.collect_expression_facts_in_context(expression, environment, return_type);
        self.collect_expected_expression_facts(expression, expected, environment, return_type);
    }

    fn collect_expected_expression_facts(
        &mut self,
        expression: &Expr,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        match expression {
            Expr::Group(expression) => {
                self.collect_expected_expression_facts(
                    &expression.expression,
                    expected,
                    environment,
                    return_type,
                );
            }
            Expr::Propagate(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Force(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Catch(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Call(call) => {
                self.record_expected_generic_function_call_specialization(
                    call,
                    expected,
                    environment,
                );
                self.collect_call_argument_facts(call, Some(expected), environment, return_type);
            }
            Expr::If(expression) => {
                let mut then_environment = environment.clone();
                self.collect_expected_block_result_facts(
                    &expression.then_block,
                    expected,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        else_block,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            Expr::IfIs(expression) => {
                let mut then_environment =
                    environment_for_if_is_binding(expression, self.resolved, environment);
                self.collect_expected_block_result_facts(
                    &expression.then_block,
                    expected,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        else_block,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            Expr::Match(expression) => {
                for arm in &expression.arms {
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        self.resolved,
                        environment,
                    );
                    self.collect_expected_block_result_facts(
                        &arm.body,
                        expected,
                        &mut arm_environment,
                        return_type,
                    );
                }
                if let Some(wildcard_arm) = &expression.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        &wildcard_arm.body,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            _ => {}
        }
    }

    fn collect_expected_block_result_facts(
        &mut self,
        block: &Block,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        for statement in &block.statements {
            self.collect_statement_facts(statement, environment, return_type);
        }
        if let Some(result) = &block.result {
            self.collect_expression_facts_with_expected(result, expected, environment, return_type);
        }
    }

    fn collect_expression_facts(&mut self, expression: &Expr, environment: &mut TypeEnvironment) {
        self.collect_expression_facts_in_context(expression, environment, None);
    }

    fn collect_expression_facts_in_context(
        &mut self,
        expression: &Expr,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        self.record_expression_type(
            expression.span(),
            &expression_type(expression, self.resolved, environment),
        );
        match expression {
            Expr::Propagate(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Force(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Catch(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
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
                self.collect_block_facts(
                    &expression.catch_block,
                    &mut catch_environment,
                    return_type,
                );
            }
            Expr::Borrow(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Binary(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.left,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &expression.right,
                    environment,
                    return_type,
                );
            }
            Expr::Unary(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.operand,
                    environment,
                    return_type,
                );
            }
            Expr::TypeConversion(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
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
                    if let Some(kind) = method_receiver_kind(&resolved_method.receiver.ty) {
                        self.facts
                            .method_call_receiver_kinds
                            .insert(method.member_span, kind);
                    }
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
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
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
                        None,
                        environment,
                        true,
                    );
                    self.facts.call_hover_labels.insert(
                        method.member_span,
                        associated_function_signature_hover_label(
                            owner,
                            resolved_function,
                            self.resolved,
                        ),
                    );
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, variant)) =
                        resolved_enum_variant_for_member(method, self.resolved)
                {
                    self.record_enum_variant_reference(method.member_span, owner, variant);
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else {
                    if let Some(symbol) = self.resolved.symbol_for_call(expression) {
                        match &symbol.kind {
                            SymbolKind::Function(signature) => {
                                self.record_function_call_reference(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    "func",
                                    signature,
                                );
                                self.record_generic_function_call_specialization(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    signature,
                                    None,
                                    environment,
                                    true,
                                );
                            }
                            SymbolKind::Primitive(signature) => {
                                self.record_function_call_reference(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    "primitive",
                                    signature,
                                );
                                self.record_generic_function_call_specialization(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    signature,
                                    None,
                                    environment,
                                    false,
                                );
                            }
                            SymbolKind::Type(_) | SymbolKind::Imported(_) => {}
                        }
                    }
                    self.collect_expression_facts_in_context(
                        &expression.callee,
                        environment,
                        return_type,
                    );
                }

                self.collect_call_argument_facts(expression, None, environment, return_type);
            }
            Expr::Member(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.object,
                    environment,
                    return_type,
                );
                self.record_struct_field_member_reference(expression, environment);
                if let Some((owner, variant)) =
                    resolved_enum_variant_for_member(expression, self.resolved)
                {
                    self.record_enum_variant_reference(expression.member_span, owner, variant);
                }
            }
            Expr::Index(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.object,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &expression.index,
                    environment,
                    return_type,
                );
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression_facts_in_context(element, environment, return_type);
                }
            }
            Expr::StructLiteral(expression) => {
                self.collect_type_expr_references(&expression.ty);
                for field in &expression.fields {
                    self.record_struct_literal_field_reference(expression, field, environment);
                    self.collect_expression_facts_in_context(
                        &field.value,
                        environment,
                        return_type,
                    );
                }
            }
            Expr::Group(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::InterpolatedString(expression) => {
                for part in &expression.parts {
                    if let InterpolatedStringPart::Expression(part) = part {
                        self.collect_expression_facts_in_context(
                            &part.expression,
                            environment,
                            return_type,
                        );
                    }
                }
            }
            Expr::Otherwise(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.value,
                    environment,
                    return_type,
                );
                let mut fallback_environment = environment.clone();
                self.collect_block_facts(
                    &expression.fallback,
                    &mut fallback_environment,
                    return_type,
                );
            }
            Expr::If(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.condition,
                    environment,
                    return_type,
                );

                let mut then_environment = environment.clone();
                self.collect_block_facts(
                    &expression.then_block,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Expr::IfIs(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                self.record_if_is_pattern_references(expression);

                let mut then_environment =
                    environment_for_if_is_binding(expression, self.resolved, environment);
                if let Some(payload) = expression
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.record_payload_binding(payload, &then_environment);
                }
                self.collect_block_facts(
                    &expression.then_block,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Expr::Match(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                for arm in &expression.arms {
                    self.record_switch_arm_pattern_references(arm);
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        self.resolved,
                        environment,
                    );
                    if let Some(payload) =
                        arm.payload.as_ref().and_then(|payload| payload.binding())
                    {
                        self.record_payload_binding(payload, &arm_environment);
                    }
                    self.collect_block_facts(&arm.body, &mut arm_environment, return_type);
                }
                if let Some(arm) = &expression.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(&arm.body, &mut else_environment, return_type);
                }
            }
            Expr::Identifier(identifier) => {
                self.record_environment_binding_readonly(
                    identifier.span,
                    &identifier.name,
                    environment,
                );
            }
            Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
        }
    }

    fn collect_call_argument_facts(
        &mut self,
        call: &crate::ast::CallExpr,
        expected_return_type: Option<&Type>,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        let Some(checked) = resolved_call_signature(self.resolved, call, environment) else {
            for argument in &call.arguments {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            }
            return;
        };
        if call.arguments.len() != checked.signature.parameters.len() {
            for argument in &call.arguments {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            }
            return;
        }

        let mut substitutions =
            infer_generic_substitutions(call, &checked, self.resolved, environment);
        if let Some(expected_return_type) = expected_return_type {
            let parameters = checked
                .signature
                .generic_parameters
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            infer_type_expr_substitutions(
                &checked.signature.return_type,
                expected_return_type,
                self.resolved,
                checked.self_type.as_ref(),
                &parameters,
                &mut substitutions,
            );
        }

        for (argument, parameter) in call
            .arguments
            .iter()
            .zip(checked.signature.parameters.iter())
        {
            let expected = type_expr_to_type_with_substitutions(
                &parameter.ty,
                self.resolved,
                checked.self_type.as_ref(),
                &substitutions,
            );
            if expected.is_unknown_or_unresolved() || expected.first_unsized_part().is_some() {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            } else {
                self.collect_expression_facts_with_expected(
                    argument,
                    &expected,
                    environment,
                    return_type,
                );
            }
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

    fn record_if_is_pattern_references(&mut self, statement: &IfIsStmt) {
        self.record_enum_pattern_references(
            &statement.enum_name,
            statement.enum_name_span,
            &statement.variant_name,
            statement.variant_name_span,
        );
    }

    fn record_switch_arm_pattern_references(&mut self, arm: &SwitchArm) {
        self.record_enum_pattern_references(
            &arm.enum_name,
            arm.enum_name_span,
            &arm.variant_name,
            arm.variant_name_span,
        );
    }

    fn record_enum_pattern_references(
        &mut self,
        enum_name: &str,
        enum_name_span: ByteSpan,
        variant_name: &str,
        variant_name_span: ByteSpan,
    ) {
        self.record_type_reference(enum_name, enum_name_span);

        let Some(owner) = self.resolved.type_symbol_by_name(enum_name) else {
            return;
        };
        if owner.kind != TypeSymbolKind::Enum {
            return;
        }
        let Some(variant) = owner
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
        else {
            return;
        };

        self.record_enum_variant_reference(variant_name_span, owner, variant);
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
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(ty, name_span, &mut free_type_parameters)
        {
            self.record_payload_enum_drop_type_specializations(&ty);
            self.facts.binding_type_exprs.insert(name_span, ty);
        }
        if let Some(kind) = scalar_view_kind(ty) {
            self.facts.binding_scalar_view_kinds.insert(name_span, kind);
        }
        self.record_drop_type_specialization(name_span, ty);
    }

    fn record_expression_type(&mut self, expression_span: ByteSpan, ty: &Type) {
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(ty, expression_span, &mut free_type_parameters)
        {
            self.record_payload_enum_drop_type_specializations(&ty);
            self.facts.expression_type_exprs.insert(expression_span, ty);
        }
    }

    fn record_drop_type_specialization(&mut self, span: ByteSpan, ty: &Type) {
        if let Some(specialization) = self.drop_type_specialization(span, ty) {
            self.facts.drop_type_specializations.push(specialization);
        }
    }

    fn record_payload_enum_drop_type_specializations(&mut self, ty: &TypeExpr) {
        let Some((symbol, substitutions)) =
            payload_enum_symbol_and_substitutions_for_type_expr(ty, self.resolved)
        else {
            return;
        };
        for variant in &symbol.variants {
            let [payload] = variant.payload.as_slice() else {
                continue;
            };
            let payload_ty = substitute_type_expr_parameters(&payload.ty, &substitutions);
            let free_type_parameters =
                free_type_parameters_in_type_expr(&payload_ty, self.resolved);
            if let Some(specialization) = drop_type_specialization_from_self_ty(
                &payload_ty,
                self.resolved,
                free_type_parameters,
            ) {
                self.facts.drop_type_specializations.push(specialization);
            }
        }
    }

    fn drop_type_specialization(
        &self,
        span: ByteSpan,
        ty: &Type,
    ) -> Option<DropTypeSpecialization> {
        let mut free_type_parameters = HashSet::new();
        let self_ty = type_to_type_expr_allowing_parameters(ty, span, &mut free_type_parameters)?;
        drop_type_specialization_from_self_ty(&self_ty, self.resolved, free_type_parameters)
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
        let field_ty = struct_member_type(member, self.resolved, environment);
        self.record_struct_field_reference(
            member.member_span,
            owner,
            field,
            field_ty.as_ref(),
            environment,
        );
        if let Some(field_ty) = field_ty
            && let Some(specialization) =
                self.drop_type_specialization(member.member_span, &field_ty)
        {
            self.facts
                .field_drop_type_specializations
                .insert(member.member_span, specialization);
        }
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

        let field_ty = struct_literal_field_type(literal, field, self.resolved, environment);
        self.record_struct_field_reference(
            field.name_span,
            owner,
            expected_field,
            field_ty.as_ref(),
            environment,
        );
    }

    fn record_struct_field_reference(
        &mut self,
        span: ByteSpan,
        owner: &TypeSymbol,
        field: &crate::resolve::StructFieldSignature,
        concrete_ty: Option<&Type>,
        environment: &TypeEnvironment,
    ) {
        let fallback_ty =
            type_expr_to_type_with_self_type(&field.ty, self.resolved, environment.self_type());
        let field_ty = concrete_ty.unwrap_or(&fallback_ty);
        self.facts.field_targets.insert(span, field.name_span);
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(field_ty, span, &mut free_type_parameters)
        {
            self.facts.field_type_exprs.insert(span, ty);
        }
        if let Some(kind) = scalar_view_kind(field_ty) {
            self.facts.field_scalar_view_kinds.insert(span, kind);
        }
        self.facts.field_hover_labels.insert(
            span,
            format!(
                "field {}.{}: {}",
                type_owner_hover_label(owner, self.resolved),
                field.name,
                type_hover_label(field_ty, self.resolved)
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

    fn record_function_call_reference(
        &mut self,
        call: &CallExpr,
        declaration_span: ByteSpan,
        name: &str,
        kind: &str,
        signature: &FunctionSignature,
    ) {
        let Some(name_span) = call_callee_name_span(call) else {
            return;
        };

        self.facts
            .function_call_targets
            .insert(name_span, declaration_span);
        self.facts.call_hover_labels.insert(
            name_span,
            function_signature_hover_label(kind, name, signature, self.resolved, None),
        );
    }

    fn record_generic_function_call_specialization(
        &mut self,
        call: &crate::ast::CallExpr,
        declaration_span: ByteSpan,
        base_target_name: &str,
        signature: &FunctionSignature,
        expected_return_type: Option<&Type>,
        environment: &TypeEnvironment,
        report_unspecialized: bool,
    ) {
        if signature.generic_parameters.is_empty() {
            return;
        }
        if report_unspecialized {
            self.facts
                .generic_function_call_spans
                .insert(call.span, declaration_span);
        }
        if let Some(specialization) = function_call_specialization(
            call,
            declaration_span,
            base_target_name,
            signature,
            expected_return_type,
            self.resolved,
            environment,
        ) {
            self.facts
                .function_call_specializations
                .insert(call.span, specialization);
        }
    }

    fn record_expected_generic_function_call_specialization(
        &mut self,
        call: &crate::ast::CallExpr,
        expected_return_type: &Type,
        environment: &TypeEnvironment,
    ) {
        if let Some((_owner, resolved_function)) = self.resolved.associated_function_for_call(call)
        {
            self.record_generic_function_call_specialization(
                call,
                resolved_function.name_span,
                &resolved_function.target_name,
                &resolved_function.signature,
                Some(expected_return_type),
                environment,
                true,
            );
            return;
        }

        let Some(symbol) = self.resolved.symbol_for_call(call) else {
            return;
        };
        match &symbol.kind {
            SymbolKind::Function(signature) => self.record_generic_function_call_specialization(
                call,
                symbol.declaration_span,
                &symbol.name,
                signature,
                Some(expected_return_type),
                environment,
                true,
            ),
            SymbolKind::Primitive(signature) => self.record_generic_function_call_specialization(
                call,
                symbol.declaration_span,
                &symbol.name,
                signature,
                Some(expected_return_type),
                environment,
                false,
            ),
            SymbolKind::Type(_) | SymbolKind::Imported(_) => {}
        }
    }
}

fn expected_attempt_type(
    expression: &Expr,
    expected_success: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(expression, resolved, environment) {
        Type::Fallible { error, .. } => Type::Fallible {
            success: Box::new(expected_success.clone()),
            error,
        },
        Type::Optional(_) => Type::Optional(Box::new(expected_success.clone())),
        _ => expected_success.clone(),
    }
}

fn function_call_specialization(
    call: &crate::ast::CallExpr,
    declaration_span: ByteSpan,
    base_target_name: &str,
    signature: &FunctionSignature,
    expected_return_type: Option<&Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<FunctionCallSpecialization> {
    let checked = resolved_call_signature(resolved, call, environment)?;
    let mut substitution_types = infer_generic_substitutions(call, &checked, resolved, environment);
    if let Some(expected_return_type) = expected_return_type {
        let parameters = signature
            .generic_parameters
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        infer_type_expr_substitutions(
            &signature.return_type,
            expected_return_type,
            resolved,
            checked.self_type.as_ref(),
            &parameters,
            &mut substitution_types,
        );
    }
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
    let mut free_type_parameters = HashSet::new();
    let substitutions = substitution_types
        .into_iter()
        .map(|(name, ty)| {
            type_to_type_expr_allowing_parameters(&ty, call.span, &mut free_type_parameters)
                .map(|ty| (name, ty))
        })
        .collect::<Option<HashMap<_, _>>>()?;

    Some(FunctionCallSpecialization {
        declaration_span,
        base_target_name: base_target_name.to_string(),
        generic_parameters: signature.generic_parameters.clone(),
        target_name: format!("{base_target_name}<{}>", type_arguments.join(", ")),
        substitutions,
        free_type_parameters,
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
    let self_type = method_self_type_for_receiver(&receiver_type);
    let mut free_type_parameters = HashSet::new();
    let self_ty = type_to_type_expr_allowing_parameters(
        &self_type,
        member.object.span(),
        &mut free_type_parameters,
    )?;
    let checked = resolved_call_signature(resolved, call, environment)?;
    let substitutions = infer_generic_substitutions(call, &checked, resolved, environment)
        .into_iter()
        .map(|(name, ty)| {
            type_to_type_expr_allowing_parameters(
                &ty,
                member.member_span,
                &mut free_type_parameters,
            )
            .map(|ty| (name, ty))
        })
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
        method_name: method.name.clone(),
        target_name: method_target_name_from_self_ty(&self_ty, &method.name),
        self_ty,
        generic_parameters: method.signature.generic_parameters.clone(),
        substitutions,
        free_type_parameters,
    })
}

fn type_to_type_expr(ty: &Type, span: ByteSpan) -> Option<TypeExpr> {
    type_to_type_expr_inner(ty, span, None)
}

fn type_to_type_expr_allowing_parameters(
    ty: &Type,
    span: ByteSpan,
    free_type_parameters: &mut HashSet<String>,
) -> Option<TypeExpr> {
    type_to_type_expr_inner(ty, span, Some(free_type_parameters))
}

fn type_to_type_expr_inner(
    ty: &Type,
    span: ByteSpan,
    mut free_type_parameters: Option<&mut HashSet<String>>,
) -> Option<TypeExpr> {
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
            element: Box::new(type_to_type_expr_inner(
                element,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
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
                element: Box::new(type_to_type_expr_inner(
                    element,
                    span,
                    free_type_parameters.as_deref_mut(),
                )?),
            })),
        })),
        Type::Array { element, length } => Some(TypeExpr::Array(ArrayType {
            span,
            element: Box::new(type_to_type_expr_inner(
                element,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
            length: ArrayLength {
                span,
                value: length.clone(),
            },
        })),
        Type::Pointer(inner) => Some(TypeExpr::Pointer(PointerType {
            span,
            inner: Box::new(type_to_type_expr_inner(
                inner,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Optional(inner) => Some(TypeExpr::Optional(OptionalType {
            span,
            inner: Box::new(type_to_type_expr_inner(
                inner,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Fallible { success, error } => Some(TypeExpr::Fallible(FallibleType {
            span,
            success: Box::new(type_to_type_expr_inner(
                success,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
            error: Box::new(type_to_type_expr_inner(
                error,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Generic { name, arguments } => Some(TypeExpr::Generic(GenericType {
            span,
            name: name.clone(),
            name_span: span,
            arguments: arguments
                .iter()
                .map(|argument| {
                    type_to_type_expr_inner(argument, span, free_type_parameters.as_deref_mut())
                })
                .collect::<Option<Vec<_>>>()?,
        })),
        Type::Parameter(name) => {
            let free_type_parameters = free_type_parameters?;
            free_type_parameters.insert(name.clone());
            Some(type_reference(name, span))
        }
        Type::None | Type::Unresolved(_) | Type::Unknown => None,
    }
}

fn specialized_target_name(
    base_target_name: &str,
    generic_parameters: &[String],
    substitutions: &HashMap<String, TypeExpr>,
) -> Option<String> {
    let type_arguments = generic_parameters
        .iter()
        .map(|parameter| substitutions.get(parameter).map(type_expr_display_lossy))
        .collect::<Option<Vec<_>>>()?;
    Some(format!("{base_target_name}<{}>", type_arguments.join(", ")))
}

fn method_target_name_from_self_ty(self_ty: &TypeExpr, method_name: &str) -> String {
    format!("{}.{}", type_expr_display_lossy(self_ty), method_name)
}

fn drop_target_name_from_base_and_self_ty(base_target_name: &str, self_ty: &TypeExpr) -> String {
    let Some(base_type_name) = base_target_name.strip_suffix(".drop") else {
        return base_target_name.to_string();
    };
    let TypeExpr::Generic(generic) = self_ty else {
        return base_target_name.to_string();
    };
    let arguments = generic
        .arguments
        .iter()
        .map(type_expr_display_lossy)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base_type_name}<{arguments}>.drop")
}

fn drop_type_specialization_from_self_ty(
    self_ty: &TypeExpr,
    resolved: &ResolveOutput,
    free_type_parameters: HashSet<String>,
) -> Option<DropTypeSpecialization> {
    drop_type_specialization_from_self_ty_inner(
        self_ty,
        resolved,
        free_type_parameters,
        &mut HashSet::new(),
    )
}

fn drop_type_specialization_from_self_ty_inner(
    self_ty: &TypeExpr,
    resolved: &ResolveOutput,
    free_type_parameters: HashSet<String>,
    resolving_names: &mut HashSet<String>,
) -> Option<DropTypeSpecialization> {
    match self_ty {
        TypeExpr::Optional(optional) => {
            return drop_type_specialization_from_self_ty_inner(
                &optional.inner,
                resolved,
                free_type_parameters,
                resolving_names,
            );
        }
        TypeExpr::Fallible(fallible) => {
            return drop_type_specialization_from_self_ty_inner(
                &fallible.success,
                resolved,
                free_type_parameters,
                resolving_names,
            );
        }
        _ => {}
    }

    let (type_name, substitutions) = match self_ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            (generic.name.as_str(), substitutions)
        }
        _ => return None,
    };
    let symbol = resolved.type_symbol_by_reference_name(type_name)?;
    if symbol.kind == crate::resolve::TypeSymbolKind::Alias {
        let target = symbol.alias_target.as_ref()?;
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let specialization = drop_type_specialization_from_self_ty_inner(
            &target,
            resolved,
            free_type_parameters,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return specialization;
    }

    let drop_member = symbol.drop_member.as_ref()?;
    Some(DropTypeSpecialization {
        declaration_span: drop_member.name_span,
        target_name: drop_target_name_from_base_and_self_ty(&drop_member.target_name, self_ty),
        self_ty: self_ty.clone(),
        base_target_name: drop_member.target_name.clone(),
        free_type_parameters,
    })
}

fn payload_enum_symbol_and_substitutions_for_type_expr<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)> {
    payload_enum_symbol_and_substitutions_for_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn payload_enum_symbol_and_substitutions_for_type_expr_inner<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)> {
    match ty {
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            match symbol.kind {
                TypeSymbolKind::Enum if symbol.generic_arity == 0 => Some((symbol, HashMap::new())),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        target,
                        resolved,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions: HashMap<String, TypeExpr> = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Enum => Some((symbol, substitutions)),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        &target,
                        resolved,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn free_type_parameters_in_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> HashSet<String> {
    let mut parameters = HashSet::new();
    collect_free_type_parameters_in_type_expr(ty, resolved, &mut parameters);
    parameters
}

fn collect_free_type_parameters_in_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    parameters: &mut HashSet<String>,
) {
    match ty {
        TypeExpr::Reference(reference) => {
            if resolved
                .type_symbol_by_reference_name(&reference.name)
                .is_none()
                && !builtin_type_name(&reference.name)
            {
                parameters.insert(reference.name.clone());
            }
        }
        TypeExpr::Generic(generic) => {
            for argument in &generic.arguments {
                collect_free_type_parameters_in_type_expr(argument, resolved, parameters);
            }
        }
        TypeExpr::Pointer(pointer) => {
            collect_free_type_parameters_in_type_expr(&pointer.inner, resolved, parameters);
        }
        TypeExpr::Borrow(borrow) => {
            collect_free_type_parameters_in_type_expr(&borrow.inner, resolved, parameters);
        }
        TypeExpr::View(view) => {
            collect_free_type_parameters_in_type_expr(&view.element, resolved, parameters);
        }
        TypeExpr::Array(array) => {
            collect_free_type_parameters_in_type_expr(&array.element, resolved, parameters);
        }
        TypeExpr::Optional(optional) => {
            collect_free_type_parameters_in_type_expr(&optional.inner, resolved, parameters);
        }
        TypeExpr::Fallible(fallible) => {
            collect_free_type_parameters_in_type_expr(&fallible.success, resolved, parameters);
            collect_free_type_parameters_in_type_expr(&fallible.error, resolved, parameters);
        }
    }
}

fn builtin_type_name(name: &str) -> bool {
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
            | "error"
            | "str"
            | "void"
            | "never"
            | "Self"
    )
}

fn type_expr_contains_free_parameters(
    ty: &TypeExpr,
    free_type_parameters: &HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Reference(reference) => free_type_parameters.contains(&reference.name),
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|argument| type_expr_contains_free_parameters(argument, free_type_parameters)),
        TypeExpr::Pointer(pointer) => {
            type_expr_contains_free_parameters(&pointer.inner, free_type_parameters)
        }
        TypeExpr::Borrow(borrow) => {
            type_expr_contains_free_parameters(&borrow.inner, free_type_parameters)
        }
        TypeExpr::View(view) => {
            type_expr_contains_free_parameters(&view.element, free_type_parameters)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_free_parameters(&array.element, free_type_parameters)
        }
        TypeExpr::Optional(optional) => {
            type_expr_contains_free_parameters(&optional.inner, free_type_parameters)
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_free_parameters(&fallible.success, free_type_parameters)
                || type_expr_contains_free_parameters(&fallible.error, free_type_parameters)
        }
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
        Type::View { element, .. } => {
            Some(TypecheckScalarViewKind::Slice(slice_element_kind(element)))
        }
        _ => None,
    }
}

fn slice_element_kind(element: &Type) -> TypecheckSliceElementKind {
    match element {
        Type::I32 => TypecheckSliceElementKind::I32,
        Type::Primitive(name) if name == "u8" => TypecheckSliceElementKind::U8,
        Type::Primitive(name) if name == "usize" => TypecheckSliceElementKind::Usize,
        Type::Primitive(name) if name == "bool" => TypecheckSliceElementKind::Bool,
        Type::Str => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn call_callee_name_span(call: &CallExpr) -> Option<ByteSpan> {
    match call.callee.without_groups() {
        Expr::Identifier(identifier) => Some(identifier.span),
        Expr::Member(member) => Some(member.member_span),
        _ => None,
    }
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
    match method_receiver_kind(ty)? {
        TypecheckMethodReceiverKind::Owned => Some(""),
        TypecheckMethodReceiverKind::ReadonlyBorrow => Some("&"),
        TypecheckMethodReceiverKind::ReadwriteBorrow => Some("&+"),
    }
}

fn method_receiver_kind(ty: &TypeExpr) -> Option<TypecheckMethodReceiverKind> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            Some(TypecheckMethodReceiverKind::Owned)
        }
        TypeExpr::Borrow(borrow) => match borrow.inner.as_ref() {
            TypeExpr::Reference(reference) if reference.name == "Self" => {
                Some(if borrow.is_readwrite {
                    TypecheckMethodReceiverKind::ReadwriteBorrow
                } else {
                    TypecheckMethodReceiverKind::ReadonlyBorrow
                })
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

    #[test]
    fn records_method_receiver_kind_facts() {
        let (ast, resolved) = parse_and_resolve_text(
            r#"struct Box {
    value: i32
}

impl Box {
    method self.take(): i32 {
        return self.value
    }

    method &self.read(): i32 {
        return self.value
    }

    method &+self.write(): void {
        self.value = 2
        return
    }
}

func main(): i32 {
    var box = Box{ value: 1 }
    box.write()
    let copy = Box{ value: 2 }
    return copy.read() + Box{ value: 3 }.take()
}
"#,
        );
        let facts = collect_typecheck_facts(&ast, &resolved);
        let receiver_kinds = facts
            .method_call_spans()
            .filter_map(|span| facts.method_call_receiver_kind(span))
            .collect::<Vec<_>>();

        assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::Owned));
        assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::ReadonlyBorrow));
        assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::ReadwriteBorrow));
    }

    #[test]
    fn records_binding_type_expr_facts_for_generic_parameters() {
        let text = r#"func keep<T>(value: T): T {
    let inferred = value
    return inferred
}
"#;
        let (ast, resolved) = parse_and_resolve_text(text);
        let facts = collect_typecheck_facts(&ast, &resolved);
        let start = text.find("inferred").expect("expected binding name");
        let span = ByteSpan::new(ast.span.source, start, start + "inferred".len());

        let Some(TypeExpr::Reference(reference)) = facts.binding_type_expr(span) else {
            panic!("expected inferred binding type expr for generic parameter");
        };
        assert_eq!(reference.name, "T");
    }

    #[test]
    fn records_expression_type_expr_facts() {
        let text = r#"enum Choice {
    yes
    no
}

func main(choice: Choice): i32 {
    let code = match choice {
        _ { 1 }
    }
    return code
}
"#;
        let (ast, resolved) = parse_and_resolve_text(text);
        let facts = collect_typecheck_facts(&ast, &resolved);
        let function = ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("expected main function");
        let Stmt::Binding(binding) = &function.body.statements[0] else {
            panic!("expected match binding");
        };
        let Expr::Match(match_expression) = binding.initializer.without_groups() else {
            panic!("expected match expression initializer");
        };

        let Some(TypeExpr::Reference(reference)) =
            facts.expression_type_expr(match_expression.expression.span())
        else {
            panic!("expected expression type expr fact");
        };
        assert_eq!(reference.name, "Choice");
    }

    #[test]
    fn records_enum_pattern_variant_reference_facts() {
        let text = r#"enum Choice {
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
        let (ast, resolved) = parse_and_resolve_text(text);
        let facts = collect_typecheck_facts(&ast, &resolved);
        let hit_declaration = identifier_span(&ast, text, "hit(value", "hit");
        let miss_declaration = identifier_span(&ast, text, "miss(value", "miss");

        for start in [
            text.find("hit(_)").expect("expected if-is hit pattern"),
            text.rfind("hit(_)").expect("expected match hit pattern"),
        ] {
            let span = ByteSpan::new(ast.span.source, start, start + "hit".len());
            assert_eq!(facts.enum_variant_target(span), Some(hit_declaration));
        }

        let miss_start = text.rfind("miss(_)").expect("expected match miss pattern");
        let miss_span = ByteSpan::new(ast.span.source, miss_start, miss_start + "miss".len());
        assert_eq!(facts.enum_variant_target(miss_span), Some(miss_declaration));

        let discard_start = text.find("_)").expect("expected discard payload");
        let discard_span = ByteSpan::new(ast.span.source, discard_start, discard_start + 1);
        assert_eq!(facts.enum_variant_target(discard_span), None);
    }

    #[test]
    fn records_concrete_field_type_expr_facts_for_generic_struct_fields() {
        let text = r#"copy struct Box<T> {
    values: [T; 2]
}

func main(): i32 {
    let box = Box<i32>{ values: [1, 2] }
    return box.values[0]
}
"#;
        let (ast, resolved) = parse_and_resolve_text(text);
        let facts = collect_typecheck_facts(&ast, &resolved);
        let literal_start = text.find("values: [1, 2]").expect("expected literal field");
        let literal_span = ByteSpan::new(
            ast.span.source,
            literal_start,
            literal_start + "values".len(),
        );
        let member_start = text.rfind("values[0]").expect("expected member field");
        let member_span =
            ByteSpan::new(ast.span.source, member_start, member_start + "values".len());

        assert_concrete_i32_pair_type_expr(facts.field_type_expr(literal_span));
        assert_concrete_i32_pair_type_expr(facts.field_type_expr(member_span));
    }

    fn assert_concrete_i32_pair_type_expr(ty: Option<&TypeExpr>) {
        let Some(TypeExpr::Array(array)) = ty else {
            panic!("expected concrete fixed array field type expr");
        };
        let TypeExpr::Reference(element) = array.element.as_ref() else {
            panic!("expected fixed array element type");
        };
        assert_eq!(element.name, "i32");
        assert_eq!(array.length.value, "2");
    }

    fn identifier_span(ast: &AstFile, text: &str, needle: &str, identifier: &str) -> ByteSpan {
        let start = text.find(needle).expect("expected identifier");
        ByteSpan::new(ast.span.source, start, start + identifier.len())
    }

    fn resolve_text(text: &str) -> ResolveOutput {
        parse_and_resolve_text(text).1
    }

    fn parse_and_resolve_text(text: &str) -> (AstFile, ResolveOutput) {
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
        let resolved = resolve(&sources, &ast);
        (ast, resolved)
    }
}
