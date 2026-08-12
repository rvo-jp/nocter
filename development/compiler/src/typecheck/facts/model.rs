use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct TypecheckFacts {
    pub(super) binding_type_labels: HashMap<ByteSpan, String>,
    pub(super) binding_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) expression_type_exprs: HashMap<ByteSpan, TypeExpr>,
    pub(super) interpolation_plans: HashMap<ByteSpan, TypecheckInterpolationPlan>,
    pub(super) comparison_plans: HashMap<ByteSpan, TypecheckComparisonPlan>,
    pub(super) index_plans: HashMap<ByteSpan, TypecheckIndexPlan>,
    pub(super) collection_for_plans: HashMap<ByteSpan, TypecheckCollectionForPlan>,
    pub(super) sequence_spread_plans: HashMap<ByteSpan, TypecheckSequenceSpreadPlan>,
    pub(super) closure_plans: HashMap<ByteSpan, TypecheckClosurePlan>,
    pub(super) conversion_plans: HashMap<ByteSpan, TypecheckConversionPlan>,
    pub(super) binding_scalar_view_kinds: HashMap<ByteSpan, TypecheckScalarViewKind>,
    pub(super) binding_readonly: HashMap<ByteSpan, bool>,
    pub(super) payload_binding_modes: HashMap<ByteSpan, TypecheckPayloadBindingMode>,
    pub(super) type_occurrences: Vec<TypeOccurrenceFact>,
    pub(super) generic_parameter_declarations: Vec<GenericParameterFact>,
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
    pub(super) callable_calls: HashMap<ByteSpan, CallableCallFact>,
    pub(super) drop_type_specializations: Vec<DropTypeSpecialization>,
    pub(super) field_drop_type_specializations: HashMap<ByteSpan, DropTypeSpecialization>,
}

impl TypecheckFacts {
    pub(crate) fn binding_type_label(&self, name_span: ByteSpan) -> Option<&str> {
        self.binding_type_labels.get(&name_span).map(String::as_str)
    }

    pub(crate) fn binding_type_label_entries(&self) -> impl Iterator<Item = (ByteSpan, &str)> + '_ {
        self.binding_type_labels
            .iter()
            .map(|(span, label)| (*span, label.as_str()))
    }

    pub(crate) fn binding_type_expr(&self, name_span: ByteSpan) -> Option<&TypeExpr> {
        self.binding_type_exprs.get(&name_span)
    }

    pub(crate) fn binding_type_expr_entries(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypeExpr)> + '_ {
        self.binding_type_exprs.iter().map(|(span, ty)| (*span, ty))
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

    pub(crate) fn comparison_plan(
        &self,
        operator_span: ByteSpan,
    ) -> Option<&TypecheckComparisonPlan> {
        self.comparison_plans.get(&operator_span)
    }

    pub(crate) fn comparison_plans(&self) -> impl Iterator<Item = &TypecheckComparisonPlan> {
        self.comparison_plans.values()
    }

    pub(crate) fn index_plan(&self, expression_span: ByteSpan) -> Option<&TypecheckIndexPlan> {
        self.index_plans.get(&expression_span)
    }

    pub(crate) fn index_plans(&self) -> impl Iterator<Item = &TypecheckIndexPlan> {
        self.index_plans.values()
    }

    pub(crate) fn interpolation_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckInterpolationPlan)> {
        self.interpolation_plans
            .iter()
            .map(|(span, plan)| (*span, plan))
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

    pub(crate) fn coercion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckCoercionPlan> {
        let TypecheckConversionKind::BorrowCoercion(plan) =
            &self.conversion_plans.get(&expression_span)?.kind
        else {
            return None;
        };
        Some(plan)
    }

    pub(crate) fn coercion_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckCoercionPlan)> + '_ {
        self.conversion_plans.iter().filter_map(|(span, plan)| {
            let TypecheckConversionKind::BorrowCoercion(coercion) = &plan.kind else {
                return None;
            };
            Some((*span, coercion))
        })
    }

    pub(crate) fn conversion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckConversionPlan> {
        self.conversion_plans.get(&expression_span)
    }

    pub(crate) fn conversion_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckConversionPlan)> + '_ {
        self.conversion_plans
            .iter()
            .map(|(span, plan)| (*span, plan))
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

    pub(crate) fn type_occurrences(&self) -> impl Iterator<Item = &TypeOccurrenceFact> + '_ {
        self.type_occurrences.iter()
    }

    pub(crate) fn generic_parameter_declarations(
        &self,
    ) -> impl Iterator<Item = &GenericParameterFact> + '_ {
        self.generic_parameter_declarations.iter()
    }

    pub(crate) fn generic_parameter(&self, span: ByteSpan) -> Option<&GenericParameterFact> {
        self.generic_parameter_declarations
            .iter()
            .find(|parameter| parameter.span == span)
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

    pub(crate) fn callable_call(&self, call_span: ByteSpan) -> Option<&CallableCallFact> {
        self.callable_calls.get(&call_span)
    }

    pub(crate) fn callable_call_entries(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &CallableCallFact)> + '_ {
        self.callable_calls.iter().map(|(span, fact)| (*span, fact))
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
    pub(crate) format_interface_declaration: ByteSpan,
    pub(crate) parts: Vec<TypecheckInterpolationPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckInterpolationPart {
    pub(crate) span: ByteSpan,
    pub(crate) expression_span: Option<ByteSpan>,
    pub(crate) accepted_type: TypeExpr,
    pub(crate) formatter: TypecheckProtocolMethod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckComparisonPlan {
    pub(crate) kind: crate::ast::ComparisonOperatorKind,
    pub(crate) operator_span: ByteSpan,
    pub(crate) call_span: ByteSpan,
    pub(crate) left_span: ByteSpan,
    pub(crate) right_span: ByteSpan,
    pub(crate) left_ty: TypeExpr,
    pub(crate) right_ty: TypeExpr,
    pub(crate) method: Option<TypecheckProtocolMethod>,
    pub(crate) right_implicit_readonly_borrow: bool,
    pub(crate) left_conversion: Option<TypecheckConversionPlan>,
    pub(crate) right_conversion: Option<TypecheckConversionPlan>,
    pub(crate) reverse_operands: bool,
    pub(crate) invert_result: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckIndexAccess {
    Readonly,
    Readwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckIndexProjection {
    Array,
    Slice,
    Str,
    Requirement,
    Declared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckIndexPlan {
    pub(crate) expression_span: ByteSpan,
    pub(crate) object_span: ByteSpan,
    pub(crate) index_span: ByteSpan,
    pub(crate) target_ty: TypeExpr,
    pub(crate) index_ty: TypeExpr,
    pub(crate) element_ty: TypeExpr,
    pub(crate) access: TypecheckIndexAccess,
    pub(crate) projection: TypecheckIndexProjection,
    pub(crate) requirement_span: Option<ByteSpan>,
    pub(crate) method: Option<TypecheckProtocolMethod>,
    pub(crate) conversion: Option<TypecheckConversionPlan>,
}

impl TypecheckIndexPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            expression_span: self.expression_span,
            object_span: self.object_span,
            index_span: self.index_span,
            target_ty: substitute_type_expr_parameters(&self.target_ty, context_substitutions),
            index_ty: substitute_type_expr_parameters(&self.index_ty, context_substitutions),
            element_ty: substitute_type_expr_parameters(&self.element_ty, context_substitutions),
            access: self.access,
            projection: self.projection,
            requirement_span: self.requirement_span,
            method: match &self.method {
                Some(method) => Some(method.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            conversion: match &self.conversion {
                Some(plan) => Some(plan.with_context_substitutions(context_substitutions)?),
                None => None,
            },
        })
    }
}

impl TypecheckComparisonPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            kind: self.kind,
            operator_span: self.operator_span,
            call_span: self.call_span,
            left_span: self.left_span,
            right_span: self.right_span,
            left_ty: substitute_type_expr_parameters(&self.left_ty, context_substitutions),
            right_ty: substitute_type_expr_parameters(&self.right_ty, context_substitutions),
            method: match &self.method {
                Some(method) => Some(method.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            right_implicit_readonly_borrow: self.right_implicit_readonly_borrow,
            left_conversion: match &self.left_conversion {
                Some(plan) => Some(plan.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            right_conversion: match &self.right_conversion {
                Some(plan) => Some(plan.with_context_substitutions(context_substitutions)?),
                None => None,
            },
            reverse_operands: self.reverse_operands,
            invert_result: self.invert_result,
        })
    }
}

impl TypecheckInterpolationPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            string_type_declaration: self.string_type_declaration,
            constructor: self.constructor.clone(),
            format_interface_declaration: self.format_interface_declaration,
            parts: self
                .parts
                .iter()
                .map(|part| {
                    Some(TypecheckInterpolationPart {
                        span: part.span,
                        expression_span: part.expression_span,
                        accepted_type: substitute_type_expr_parameters(
                            &part.accepted_type,
                            context_substitutions,
                        ),
                        formatter: part
                            .formatter
                            .with_context_substitutions(context_substitutions)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypecheckCollectionForSourceMode {
    Direct,
    ReadonlyConversion,
    ReadwriteConversion,
    OwnedConversion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckProtocolMethod {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) receiver_mode: crate::ast::MethodReceiverMode,
    pub(super) method_name: String,
    pub(super) free_type_parameters: HashSet<String>,
}

impl TypecheckProtocolMethod {
    pub(crate) fn new(
        declaration_span: ByteSpan,
        target_name: String,
        self_ty: TypeExpr,
        receiver_mode: crate::ast::MethodReceiverMode,
        method_name: String,
        free_type_parameters: HashSet<String>,
    ) -> Self {
        Self {
            declaration_span,
            target_name,
            self_ty,
            receiver_mode,
            method_name,
            free_type_parameters,
        }
    }

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
    pub(crate) conversion: Option<TypecheckProtocolMethod>,
    pub(crate) step: TypecheckProtocolMethod,
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
    pub(crate) operator_span: ByteSpan,
    pub(crate) source_span: ByteSpan,
    pub(crate) mode: TypecheckSequenceSpreadMode,
    pub(crate) source_mode: TypecheckCollectionForSourceMode,
    pub(crate) source_type: TypeExpr,
    pub(crate) iterator_type: TypeExpr,
    pub(crate) iterator_item_type: TypeExpr,
    pub(crate) pack_item_type: TypeExpr,
    pub(crate) conversion: Option<TypecheckProtocolMethod>,
    pub(crate) exact_size: TypecheckProtocolMethod,
    pub(crate) step: TypecheckProtocolMethod,
}

impl TypecheckSequenceSpreadPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        Some(Self {
            spread_span: self.spread_span,
            operator_span: self.operator_span,
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
    Integer(IntegerType),
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
pub(crate) struct TypecheckCoercionPlan {
    pub(crate) def_id: Option<crate::semantic::DefId>,
    pub(crate) focus_span: ByteSpan,
    pub(crate) receiver_mode: crate::ast::MethodReceiverMode,
    pub(crate) source_is_readwrite: bool,
    pub(crate) self_ty: TypeExpr,
    pub(crate) target_ty: TypeExpr,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(crate) has_explicit_result_provenance: bool,
    pub(crate) requirement_span: Option<ByteSpan>,
    pub(super) free_type_parameters: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypecheckConversionPlan {
    pub(crate) expression_span: ByteSpan,
    pub(crate) source_span: ByteSpan,
    pub(crate) operator_span: Option<ByteSpan>,
    pub(crate) source_ty: TypeExpr,
    pub(crate) target_ty: TypeExpr,
    pub(crate) kind: TypecheckConversionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypecheckConversionKind {
    LosslessInteger,
    CapabilityWeakening,
    BorrowCoercion(TypecheckCoercionPlan),
}

impl TypecheckConversionPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let kind = match &self.kind {
            TypecheckConversionKind::LosslessInteger => TypecheckConversionKind::LosslessInteger,
            TypecheckConversionKind::CapabilityWeakening => {
                TypecheckConversionKind::CapabilityWeakening
            }
            TypecheckConversionKind::BorrowCoercion(plan) => {
                TypecheckConversionKind::BorrowCoercion(
                    plan.with_context_substitutions(context_substitutions)?,
                )
            }
        };
        Some(Self {
            expression_span: self.expression_span,
            source_span: self.source_span,
            operator_span: self.operator_span,
            source_ty: substitute_type_expr_parameters(&self.source_ty, context_substitutions),
            target_ty: substitute_type_expr_parameters(&self.target_ty, context_substitutions),
            kind,
        })
    }
}

impl TypecheckCoercionPlan {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let self_ty = substitute_type_expr_parameters(&self.self_ty, context_substitutions);
        let target_ty = substitute_type_expr_parameters(&self.target_ty, context_substitutions);
        let substitutions = self
            .substitutions
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    substitute_type_expr_parameters(ty, context_substitutions),
                )
            })
            .collect::<HashMap<_, _>>();
        if type_expr_contains_free_parameters(&self_ty, &self.free_type_parameters)
            || type_expr_contains_free_parameters(&target_ty, &self.free_type_parameters)
            || substitutions
                .values()
                .any(|ty| type_expr_contains_free_parameters(ty, &self.free_type_parameters))
        {
            return None;
        }
        Some(Self {
            def_id: self.def_id,
            focus_span: self.focus_span,
            receiver_mode: self.receiver_mode,
            source_is_readwrite: self.source_is_readwrite,
            self_ty,
            target_ty,
            substitutions,
            has_explicit_result_provenance: self.has_explicit_result_provenance,
            requirement_span: self.requirement_span,
            free_type_parameters: HashSet::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeOccurrenceFact {
    pub(crate) focus_span: ByteSpan,
    pub(crate) contextual_type: TypeExpr,
    pub(crate) target: Option<TypeOccurrenceTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeOccurrenceTarget {
    Declaration(ByteSpan),
    Member(ByteSpan),
    GenericParameter(ByteSpan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenericParameterFact {
    pub(crate) name: String,
    pub(crate) span: ByteSpan,
    pub(crate) is_copy: bool,
    pub(crate) bounds: Vec<TypeExpr>,
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
