use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    pub(super) binding_type_labels: HashMap<ByteSpan, String>,
    pub(super) binding_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) expression_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) binding_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    pub(super) binding_readonly: HashMap<ByteSpan, bool>,
    pub(super) declaration_hover_labels: HashMap<ByteSpan, String>,
    pub(super) call_hover_labels: HashMap<ByteSpan, String>,
    pub(super) field_hover_labels: HashMap<ByteSpan, String>,
    pub(super) enum_variant_hover_labels: HashMap<ByteSpan, String>,
    pub(super) type_references: Vec<TypeReferenceFact>,
    pub(super) field_targets: HashMap<ByteSpan, ByteSpan>,
    pub(super) field_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) field_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    pub(super) field_readonly: HashMap<ByteSpan, bool>,
    pub(super) function_call_targets: HashMap<ByteSpan, ByteSpan>,
    pub(super) associated_function_targets: HashMap<ByteSpan, ByteSpan>,
    pub(super) enum_variant_targets: HashMap<ByteSpan, ByteSpan>,
    pub(super) method_call_targets: HashMap<ByteSpan, ByteSpan>,
    pub(super) method_call_receiver_kinds: HashMap<ByteSpan, TypecheckMethodReceiverKind>,
    pub(super) generic_function_call_spans: HashMap<ByteSpan, ByteSpan>,
    pub(super) function_call_specializations: HashMap<ByteSpan, FunctionCallSpecialization>,
    pub(super) generic_method_call_spans: HashMap<ByteSpan, ByteSpan>,
    pub(super) method_call_specializations: HashMap<ByteSpan, MethodCallSpecialization>,
    pub(super) drop_type_specializations: Vec<DropTypeSpecialization>,
    pub(super) field_drop_type_specializations: HashMap<ByteSpan, DropTypeSpecialization>,
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
    pub(super) base_target_name: String,
    pub(super) generic_parameters: Vec<String>,
    pub(crate) target_name: String,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(super) free_type_parameters: HashSet<String>,
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
    pub(super) method_name: String,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(super) generic_parameters: Vec<String>,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(super) free_type_parameters: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DropTypeSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(super) base_target_name: String,
    pub(super) free_type_parameters: HashSet<String>,
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
