use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    pub(super) binding_type_labels: HashMap<ByteSpan, String>,
    pub(super) binding_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) expression_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) interpolation_plans: HashMap<ByteSpan, TypecheckInterpolationPlan>,
    pub(super) collection_for_plans: HashMap<ByteSpan, TypecheckCollectionForPlan>,
    pub(super) sequence_spread_plans: HashMap<ByteSpan, TypecheckSequenceSpreadPlan>,
    pub(super) closure_plans: HashMap<ByteSpan, TypecheckClosurePlan>,
    pub(super) binding_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    pub(super) binding_readonly: HashMap<ByteSpan, bool>,
    pub(super) payload_binding_modes: HashMap<ByteSpan, TypecheckPayloadBindingMode>,
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

    pub(crate) fn interpolation_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckInterpolationPlan> {
        self.interpolation_plans.get(&expression_span)
    }

    pub(crate) fn collection_for_plan(
        &self,
        statement_span: ByteSpan,
    ) -> Option<&TypecheckCollectionForPlan> {
        self.collection_for_plans.get(&statement_span)
    }

    pub(crate) fn collection_for_plans(
        &self,
    ) -> impl Iterator<Item = (&ByteSpan, &TypecheckCollectionForPlan)> {
        self.collection_for_plans.iter()
    }

    pub(crate) fn sequence_spread_plan(
        &self,
        spread_span: ByteSpan,
    ) -> Option<&TypecheckSequenceSpreadPlan> {
        self.sequence_spread_plans.get(&spread_span)
    }

    pub(crate) fn sequence_spread_plans(
        &self,
    ) -> impl Iterator<Item = (&ByteSpan, &TypecheckSequenceSpreadPlan)> {
        self.sequence_spread_plans.iter()
    }

    pub(crate) fn closure_plan(&self, expression_span: ByteSpan) -> Option<&TypecheckClosurePlan> {
        self.closure_plans.get(&expression_span)
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

    pub(crate) fn payload_binding_mode(
        &self,
        name_span: ByteSpan,
    ) -> Option<TypecheckPayloadBindingMode> {
        self.payload_binding_modes.get(&name_span).copied()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckClosurePlan {
    pub(crate) expression_span: ByteSpan,
    pub(crate) ty: crate::ast::ClosureTypeExpr,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckInterpolationPlan {
    pub(crate) string_type_declaration: ByteSpan,
    pub(crate) constructor: crate::semantics::RuntimeCallable,
    pub(crate) parts: Vec<TypecheckInterpolationPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckInterpolationPart {
    pub(crate) span: ByteSpan,
    pub(crate) expression_span: Option<ByteSpan>,
    pub(crate) input: crate::semantics::InterpolationInputKind,
    pub(crate) formatter: crate::semantics::RuntimeCallable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckCollectionForSourceMode {
    Direct,
    ReadonlyConversion,
    OwnedConversion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckIterationMethod {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) receiver_mode: crate::ast::MethodReceiverMode,
    pub(super) method_name: String,
    pub(super) free_type_parameters: HashSet<String>,
}

impl TypecheckIterationMethod {
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
            target_name: method_target_name_from_self_ty(&self_ty, &self.method_name),
            self_ty,
            receiver_mode: self.receiver_mode,
            method_name: self.method_name.clone(),
            free_type_parameters: HashSet::new(),
        })
    }

    pub(crate) fn as_method_call_specialization(
        &self,
        generic_parameters: Vec<String>,
        substitutions: HashMap<String, TypeExpr>,
    ) -> MethodCallSpecialization {
        MethodCallSpecialization {
            declaration_span: self.declaration_span,
            method_name: self.method_name.clone(),
            target_name: self.target_name.clone(),
            self_ty: self.self_ty.clone(),
            generic_parameters,
            substitutions,
            free_type_parameters: self.free_type_parameters.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckCollectionForPlan {
    pub(crate) binding_span: ByteSpan,
    pub(crate) source_span: ByteSpan,
    pub(crate) source_mode: TypecheckCollectionForSourceMode,
    pub(crate) source_type: TypeExpr,
    pub(crate) iterator_type: TypeExpr,
    pub(crate) item_type: TypeExpr,
    pub(crate) conversion: Option<TypecheckIterationMethod>,
    pub(crate) step: TypecheckIterationMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckSequenceSpreadMode {
    Copy,
    Readonly,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckSequenceSpreadPlan {
    pub(crate) spread_span: ByteSpan,
    pub(crate) source_span: ByteSpan,
    pub(crate) mode: TypecheckSequenceSpreadMode,
    pub(crate) source_mode: TypecheckCollectionForSourceMode,
    pub(crate) source_type: TypeExpr,
    pub(crate) iterator_type: TypeExpr,
    pub(crate) iterator_item_type: TypeExpr,
    pub(crate) pack_item_type: TypeExpr,
    pub(crate) conversion: Option<TypecheckIterationMethod>,
    pub(crate) exact_size: TypecheckIterationMethod,
    pub(crate) step: TypecheckIterationMethod,
}

impl TypecheckSequenceSpreadPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            spread_span: self.spread_span,
            source_span: self.source_span,
            mode: self.mode,
            source_mode: self.source_mode,
            source_type: substitute_type_expr_parameters(&self.source_type, context_substitutions),
            iterator_type: substitute_type_expr_parameters(
                &self.iterator_type,
                context_substitutions,
            ),
            iterator_item_type: substitute_type_expr_parameters(
                &self.iterator_item_type,
                context_substitutions,
            ),
            pack_item_type: substitute_type_expr_parameters(
                &self.pack_item_type,
                context_substitutions,
            ),
            conversion: match &self.conversion {
                Some(method) => Some(method.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            exact_size: self
                .exact_size
                .with_context_substitutions(context_substitutions)?,
            step: self
                .step
                .with_context_substitutions(context_substitutions)?,
        })
    }
}

impl TypecheckCollectionForPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            binding_span: self.binding_span,
            source_span: self.source_span,
            source_mode: self.source_mode,
            source_type: substitute_type_expr_parameters(&self.source_type, context_substitutions),
            iterator_type: substitute_type_expr_parameters(
                &self.iterator_type,
                context_substitutions,
            ),
            item_type: substitute_type_expr_parameters(&self.item_type, context_substitutions),
            conversion: match &self.conversion {
                Some(method) => Some(method.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            step: self
                .step
                .with_context_substitutions(context_substitutions)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckPayloadBindingMode {
    Copy,
    Move,
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
    pub(crate) method_name: String,
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
