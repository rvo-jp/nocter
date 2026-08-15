use super::*;
use crate::semantic::{ExprId, SemanticDb, SemanticSiteId, StmtId};
use crate::typecheck::{CheckedScalarType, PartialSemantic, TypedExpression};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct TypedHir {
    expressions: crate::typecheck::typed_hir::TypedExpressionArena,
    sites: super::site_arena::SemanticSiteArena,
    pub(super) binding_type_labels: HashMap<LocalSymbolId, String>,
    pub(super) binding_type_exprs: HashMap<LocalSymbolId, TypeExpr>,
    pub(super) interpolation_plans: HashMap<ExprId, TypecheckInterpolationPlan>,
    pub(super) comparison_plans: HashMap<ExprId, TypecheckComparisonPlan>,
    pub(super) index_plans: HashMap<ExprId, TypecheckIndexPlan>,
    pub(super) collection_for_plans: HashMap<StmtId, TypecheckCollectionForPlan>,
    pub(super) sequence_spread_plans: HashMap<ExprId, TypecheckSequenceSpreadPlan>,
    pub(super) closure_plans: HashMap<ExprId, TypecheckClosurePlan>,
    pub(super) conversion_plans: HashMap<ExprId, TypecheckConversionPlan>,
    pub(super) binding_readonly: HashMap<LocalSymbolId, bool>,
    pub(super) payload_binding_modes: HashMap<LocalSymbolId, TypecheckPayloadBindingMode>,
    pub(super) type_occurrences: Vec<TypeOccurrenceFact>,
    pub(super) generic_parameter_declarations: Vec<GenericParameterFact>,
    pub(super) field_targets: HashMap<SemanticSiteId, crate::semantic::DefId>,
    pub(super) field_type_exprs: HashMap<SemanticSiteId, TypeExpr>,
    pub(super) field_readonly: HashMap<SemanticSiteId, bool>,
    pub(super) function_call_targets: HashMap<SemanticSiteId, crate::semantic::DefId>,
    pub(super) associated_function_targets: HashMap<SemanticSiteId, crate::semantic::DefId>,
    pub(super) enum_variant_targets: HashMap<SemanticSiteId, crate::semantic::DefId>,
    pub(super) method_call_targets: HashMap<SemanticSiteId, crate::semantic::DefId>,
    pub(super) method_call_receiver_kinds: HashMap<SemanticSiteId, TypecheckMethodReceiverKind>,
    pub(super) method_call_receiver_types: HashMap<SemanticSiteId, crate::semantic::TyId>,
    pub(super) generic_function_call_targets: HashMap<ExprId, crate::semantic::DefId>,
    pub(super) function_call_specializations: HashMap<ExprId, FunctionCallSpecialization>,
    pub(super) method_call_specializations: HashMap<SemanticSiteId, MethodCallSpecialization>,
    pub(super) callable_calls: HashMap<ExprId, CallableCallFact>,
    pub(super) drop_type_specializations: Vec<DropTypeSpecialization>,
}

impl TypedHir {
    pub(crate) fn with_additional_types(
        mut self,
        types: impl IntoIterator<Item = TypeExpr>,
    ) -> Self {
        for ty in types {
            self.expressions.intern_type_tree(ty);
        }
        self
    }

    /// Returns every type carried by a checked runtime plan.  These types are
    /// semantic inputs to MIR even when no authored expression has that exact
    /// type (for example, the synthesized borrow returned by a declared index
    /// operator after generic substitution).
    pub(crate) fn runtime_fact_types(&self) -> Vec<TypeExpr> {
        fn conversion_types(plan: &TypecheckConversionPlan, types: &mut Vec<TypeExpr>) {
            types.push(plan.source_ty.clone());
            types.push(plan.target_ty.clone());
            if let TypecheckConversionKind::BorrowCoercion(coercion) = &plan.kind {
                types.push(coercion.self_ty.clone());
                types.push(coercion.target_ty.clone());
                types.extend(coercion.substitutions.values().cloned());
            }
        }
        fn protocol_method_types(method: &TypecheckProtocolMethod, types: &mut Vec<TypeExpr>) {
            types.push(method.self_ty.clone());
            if method.receiver_mode != crate::ast::MethodReceiverMode::Owned {
                types.push(TypeExpr::Borrow(crate::ast::BorrowType {
                    span: method.self_ty.span(),
                    is_readwrite: method.receiver_mode
                        == crate::ast::MethodReceiverMode::ReadwriteBorrow,
                    inner: Box::new(method.self_ty.clone()),
                }));
            }
        }

        let mut types = self
            .binding_type_exprs
            .values()
            .chain(self.field_type_exprs.values())
            .cloned()
            .collect::<Vec<_>>();
        for fact in self.callable_calls.values() {
            types.extend(
                fact.signature
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone()),
            );
            types.push(fact.signature.return_type.clone());
        }
        for plan in self.interpolation_plans.values() {
            for part in &plan.parts {
                types.push(part.accepted_type.clone());
                protocol_method_types(&part.formatter, &mut types);
            }
        }
        for plan in self.comparison_plans.values() {
            types.extend([plan.left_ty.clone(), plan.right_ty.clone()]);
            if let Some(method) = &plan.method {
                protocol_method_types(method, &mut types);
            }
            for conversion in plan
                .left_conversion
                .iter()
                .chain(plan.right_conversion.iter())
            {
                conversion_types(conversion, &mut types);
            }
        }
        for plan in self.index_plans.values() {
            types.extend([
                plan.target_ty.clone(),
                plan.index_ty.clone(),
                plan.element_ty.clone(),
                TypeExpr::Borrow(crate::ast::BorrowType {
                    span: plan.expression_span,
                    is_readwrite: plan.method.as_ref().is_some_and(|method| {
                        method.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow
                    }),
                    inner: Box::new(plan.element_ty.clone()),
                }),
            ]);
            if let Some(method) = &plan.method {
                protocol_method_types(method, &mut types);
            }
            if let Some(conversion) = &plan.conversion {
                conversion_types(conversion, &mut types);
            }
        }
        for plan in self.collection_for_plans.values() {
            types.extend([
                plan.source_type.clone(),
                plan.iterator_type.clone(),
                plan.item_type.clone(),
            ]);
            protocol_method_types(&plan.step, &mut types);
            if let Some(conversion) = &plan.conversion {
                protocol_method_types(conversion, &mut types);
            }
        }
        for plan in self.sequence_spread_plans.values() {
            types.extend([
                plan.source_type.clone(),
                plan.iterator_type.clone(),
                plan.iterator_item_type.clone(),
                plan.pack_item_type.clone(),
            ]);
            protocol_method_types(&plan.exact_size, &mut types);
            protocol_method_types(&plan.step, &mut types);
            if let Some(conversion) = &plan.conversion {
                protocol_method_types(conversion, &mut types);
            }
        }
        for plan in self.closure_plans.values() {
            types.push(TypeExpr::Closure(plan.ty.clone()));
        }
        for plan in self.conversion_plans.values() {
            conversion_types(plan, &mut types);
        }
        for specialization in self.function_call_specializations.values() {
            types.extend(specialization.substitutions.values().cloned());
        }
        for specialization in self.method_call_specializations.values() {
            types.push(specialization.self_ty.clone());
            types.extend(specialization.substitutions.values().cloned());
        }
        for fact in self.callable_calls.values() {
            types.push(fact.receiver_ty.clone());
            types.push(fact.specialization.callable_ty.clone());
        }
        for specialization in &self.drop_type_specializations {
            types.push(specialization.self_ty.clone());
        }
        types
    }

    pub(crate) fn specialized(&self, substitutions: &HashMap<String, TypeExpr>) -> Self {
        if substitutions.is_empty() {
            return self.clone();
        }
        let (expressions, remap) = self.expressions.specialize(substitutions);
        let mut specialized = self.clone();
        specialized.expressions = expressions;
        specialized.binding_type_exprs = specialized
            .binding_type_exprs
            .into_iter()
            .map(|(symbol, ty)| (symbol, substitute_type_expr_parameters(&ty, substitutions)))
            .collect();
        specialized.field_type_exprs = specialized
            .field_type_exprs
            .into_iter()
            .map(|(span, ty)| (span, substitute_type_expr_parameters(&ty, substitutions)))
            .collect();
        specialized.method_call_receiver_types = specialized
            .method_call_receiver_types
            .into_iter()
            .map(|(span, ty)| (span, remap[ty.index()]))
            .collect();
        specialized.interpolation_plans = specialized
            .interpolation_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.comparison_plans = specialized
            .comparison_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.index_plans = specialized
            .index_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.collection_for_plans = specialized
            .collection_for_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.sequence_spread_plans = specialized
            .sequence_spread_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.conversion_plans = specialized
            .conversion_plans
            .into_iter()
            .filter_map(|(span, plan)| {
                plan.with_context_substitutions(substitutions)
                    .map(|plan| (span, plan))
            })
            .collect();
        specialized.function_call_specializations = specialized
            .function_call_specializations
            .into_iter()
            .filter_map(|(span, fact)| {
                fact.with_context_substitutions(substitutions)
                    .map(|fact| (span, fact))
            })
            .collect();
        specialized.method_call_specializations = specialized
            .method_call_specializations
            .into_iter()
            .filter_map(|(span, fact)| {
                fact.with_context_substitutions(substitutions)
                    .map(|fact| (span, fact))
            })
            .collect();
        specialized.callable_calls = specialized
            .callable_calls
            .into_iter()
            .filter_map(|(span, mut fact)| {
                let specialization = fact
                    .specialization
                    .with_context_substitutions(substitutions)?;
                for parameter in &mut fact.signature.parameters {
                    parameter.ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
                }
                fact.signature.return_type =
                    substitute_type_expr_parameters(&fact.signature.return_type, substitutions);
                fact.receiver_ty =
                    substitute_type_expr_parameters(&fact.receiver_ty, substitutions);
                fact.specialization = specialization;
                Some((span, fact))
            })
            .collect();
        specialized.closure_plans = specialized
            .closure_plans
            .into_iter()
            .map(|(span, mut plan)| {
                let TypeExpr::Closure(ty) =
                    substitute_type_expr_parameters(&TypeExpr::Closure(plan.ty), substitutions)
                else {
                    unreachable!("closure type substitution preserves its outer kind")
                };
                plan.ty = ty;
                (span, plan)
            })
            .collect();
        specialized
    }

    pub(super) fn new(semantic_db: Arc<SemanticDb>, anchor: ByteSpan) -> Self {
        Self {
            expressions: crate::typecheck::typed_hir::TypedExpressionArena::new(
                semantic_db,
                anchor,
            ),
            sites: super::site_arena::SemanticSiteArena::default(),
            binding_type_labels: HashMap::new(),
            binding_type_exprs: HashMap::new(),
            interpolation_plans: HashMap::new(),
            comparison_plans: HashMap::new(),
            index_plans: HashMap::new(),
            collection_for_plans: HashMap::new(),
            sequence_spread_plans: HashMap::new(),
            closure_plans: HashMap::new(),
            conversion_plans: HashMap::new(),
            binding_readonly: HashMap::new(),
            payload_binding_modes: HashMap::new(),
            type_occurrences: Vec::new(),
            generic_parameter_declarations: Vec::new(),
            field_targets: HashMap::new(),
            field_type_exprs: HashMap::new(),
            field_readonly: HashMap::new(),
            function_call_targets: HashMap::new(),
            associated_function_targets: HashMap::new(),
            enum_variant_targets: HashMap::new(),
            method_call_targets: HashMap::new(),
            method_call_receiver_kinds: HashMap::new(),
            method_call_receiver_types: HashMap::new(),
            generic_function_call_targets: HashMap::new(),
            function_call_specializations: HashMap::new(),
            method_call_specializations: HashMap::new(),
            callable_calls: HashMap::new(),
            drop_type_specializations: Vec::new(),
        }
    }

    pub(super) fn record_expression_type(
        &mut self,
        expression_span: ByteSpan,
        ty: Option<TypeExpr>,
        scalar: Option<CheckedScalarType>,
        diverges: bool,
    ) {
        self.expressions
            .record_type(expression_span, ty, scalar, diverges);
    }

    pub(super) fn expression_id(&self, span: ByteSpan) -> Option<ExprId> {
        self.expressions.expression_id_at(span)
    }

    pub(super) fn statement_id(&self, span: ByteSpan) -> Option<StmtId> {
        self.expressions.statement_id_at(span)
    }

    pub(super) fn intern_site(&mut self, span: ByteSpan) -> SemanticSiteId {
        self.sites.intern(span)
    }

    fn site_id(&self, span: ByteSpan) -> Option<SemanticSiteId> {
        self.sites.id(span)
    }

    pub(super) fn record_contextual_expression_type(
        &mut self,
        expression_span: ByteSpan,
        ty: TypeExpr,
        scalar: Option<CheckedScalarType>,
    ) {
        self.expressions
            .record_contextual_type(expression_span, ty, scalar);
    }

    pub(super) fn intern_type_identity(
        &mut self,
        ty: TypeExpr,
        scalar: Option<CheckedScalarType>,
    ) -> crate::semantic::TyId {
        self.expressions.intern_type(ty, scalar)
    }

    pub(crate) fn expression(&self, expression_span: ByteSpan) -> Option<&TypedExpression> {
        let expression = self.expressions.expression_id_at(expression_span)?;
        self.expression_by_id(expression)
    }

    pub(crate) fn expression_by_id(&self, expression: ExprId) -> Option<&TypedExpression> {
        self.expressions.expression(expression)
    }

    pub(crate) fn binding_type_label(&self, symbol: LocalSymbolId) -> Option<&str> {
        self.binding_type_labels.get(&symbol).map(String::as_str)
    }

    pub(crate) fn binding_type_label_entries(
        &self,
    ) -> impl Iterator<Item = (LocalSymbolId, &str)> + '_ {
        self.binding_type_labels
            .iter()
            .map(|(symbol, label)| (*symbol, label.as_str()))
    }

    pub(crate) fn binding_type_expr(&self, symbol: LocalSymbolId) -> Option<&TypeExpr> {
        self.binding_type_exprs.get(&symbol)
    }

    pub(crate) fn binding_type_expr_entries(
        &self,
    ) -> impl Iterator<Item = (LocalSymbolId, &TypeExpr)> + '_ {
        self.binding_type_exprs
            .iter()
            .map(|(symbol, ty)| (*symbol, ty))
    }

    pub(crate) fn expression_type_expr(&self, expression_span: ByteSpan) -> Option<&TypeExpr> {
        let expression = self.expression(expression_span)?;
        let PartialSemantic::Known(ty) = expression.ty else {
            return None;
        };
        self.expressions.type_expr(ty)
    }

    pub(crate) fn type_id(&self, ty: &TypeExpr) -> Option<crate::semantic::TyId> {
        self.expressions.type_id(ty)
    }

    pub(crate) fn type_expr_by_id(&self, ty: crate::semantic::TyId) -> Option<&TypeExpr> {
        self.expressions.type_expr(ty)
    }

    pub(crate) fn scalar_type(&self, ty: crate::semantic::TyId) -> Option<CheckedScalarType> {
        self.expressions.scalar_type(ty)
    }

    pub(crate) fn interpolation_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckInterpolationPlan> {
        self.interpolation_plans
            .get(&self.expression_id(expression_span)?)
    }

    pub(crate) fn comparison_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckComparisonPlan> {
        self.comparison_plans
            .get(&self.expression_id(expression_span)?)
    }

    pub(crate) fn comparison_plans(&self) -> impl Iterator<Item = &TypecheckComparisonPlan> {
        self.comparison_plans.values()
    }

    pub(crate) fn index_plan(&self, expression_span: ByteSpan) -> Option<&TypecheckIndexPlan> {
        self.index_plans.get(&self.expression_id(expression_span)?)
    }

    pub(crate) fn index_plans(&self) -> impl Iterator<Item = &TypecheckIndexPlan> {
        self.index_plans.values()
    }

    pub(crate) fn interpolation_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckInterpolationPlan)> {
        self.interpolation_plans.iter().filter_map(|(id, plan)| {
            self.expressions
                .expression_span(*id)
                .map(|span| (span, plan))
        })
    }

    pub(crate) fn collection_for_plan(
        &self,
        statement_span: ByteSpan,
    ) -> Option<&TypecheckCollectionForPlan> {
        self.collection_for_plans
            .get(&self.statement_id(statement_span)?)
    }

    pub(crate) fn collection_for_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckCollectionForPlan)> {
        self.collection_for_plans.iter().filter_map(|(id, plan)| {
            self.expressions
                .statement_span(*id)
                .map(|span| (span, plan))
        })
    }

    pub(crate) fn sequence_spread_plan(
        &self,
        spread_span: ByteSpan,
    ) -> Option<&TypecheckSequenceSpreadPlan> {
        self.sequence_spread_plans
            .get(&self.expression_id(spread_span)?)
    }

    pub(crate) fn sequence_spread_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckSequenceSpreadPlan)> {
        self.sequence_spread_plans.iter().filter_map(|(id, plan)| {
            self.expressions
                .expression_span(*id)
                .map(|span| (span, plan))
        })
    }

    pub(crate) fn closure_plan(&self, expression_span: ByteSpan) -> Option<&TypecheckClosurePlan> {
        self.closure_plans
            .get(&self.expression_id(expression_span)?)
    }

    pub(crate) fn coercion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckCoercionPlan> {
        let TypecheckConversionKind::BorrowCoercion(plan) = &self
            .conversion_plans
            .get(&self.expression_id(expression_span)?)?
            .kind
        else {
            return None;
        };
        Some(plan)
    }

    pub(crate) fn coercion_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckCoercionPlan)> + '_ {
        self.conversion_plans.iter().filter_map(|(id, plan)| {
            let TypecheckConversionKind::BorrowCoercion(coercion) = &plan.kind else {
                return None;
            };
            self.expressions
                .expression_span(*id)
                .map(|span| (span, coercion))
        })
    }

    pub(crate) fn conversion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<&TypecheckConversionPlan> {
        self.conversion_plans
            .get(&self.expression_id(expression_span)?)
    }

    pub(crate) fn conversion_plans(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &TypecheckConversionPlan)> + '_ {
        self.conversion_plans.iter().filter_map(|(id, plan)| {
            self.expressions
                .expression_span(*id)
                .map(|span| (span, plan))
        })
    }

    pub(crate) fn binding_is_readonly(&self, symbol: LocalSymbolId) -> Option<bool> {
        self.binding_readonly.get(&symbol).copied()
    }

    pub(crate) fn payload_binding_mode(
        &self,
        symbol: LocalSymbolId,
    ) -> Option<TypecheckPayloadBindingMode> {
        self.payload_binding_modes.get(&symbol).copied()
    }

    pub(crate) fn type_occurrences(&self) -> impl Iterator<Item = &TypeOccurrenceFact> + '_ {
        self.type_occurrences.iter()
    }

    pub(crate) fn generic_parameter_declarations(
        &self,
    ) -> impl Iterator<Item = &GenericParameterFact> + '_ {
        self.generic_parameter_declarations.iter()
    }

    pub(crate) fn generic_parameter(
        &self,
        definition: crate::semantic::DefId,
    ) -> Option<&GenericParameterFact> {
        self.generic_parameter_declarations
            .iter()
            .find(|parameter| parameter.definition == definition)
    }

    pub(crate) fn method_call_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.method_call_targets
            .keys()
            .filter_map(|id| self.sites.span(*id))
    }

    pub(crate) fn field_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.field_targets
            .keys()
            .filter_map(|id| self.sites.span(*id))
    }

    pub(crate) fn field_is_readonly(&self, span: ByteSpan) -> Option<bool> {
        self.field_readonly.get(&self.site_id(span)?).copied()
    }

    pub(crate) fn field_target(&self, member_span: ByteSpan) -> Option<crate::semantic::DefId> {
        self.field_targets.get(&self.site_id(member_span)?).copied()
    }

    pub(crate) fn field_type_expr(&self, field_span: ByteSpan) -> Option<&TypeExpr> {
        self.field_type_exprs.get(&self.site_id(field_span)?)
    }

    pub(crate) fn associated_function_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.associated_function_targets
            .keys()
            .filter_map(|id| self.sites.span(*id))
    }

    pub(crate) fn function_call_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.function_call_targets
            .keys()
            .filter_map(|id| self.sites.span(*id))
    }

    pub(crate) fn enum_variant_target_spans(&self) -> impl Iterator<Item = ByteSpan> + '_ {
        self.enum_variant_targets
            .keys()
            .filter_map(|id| self.sites.span(*id))
    }

    pub(crate) fn function_call_target(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::semantic::DefId> {
        self.function_call_targets
            .get(&self.site_id(member_span)?)
            .copied()
    }

    pub(crate) fn method_call_target(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::semantic::DefId> {
        self.method_call_targets
            .get(&self.site_id(member_span)?)
            .copied()
    }

    pub(crate) fn method_call_receiver_kind(
        &self,
        member_span: ByteSpan,
    ) -> Option<TypecheckMethodReceiverKind> {
        self.method_call_receiver_kinds
            .get(&self.site_id(member_span)?)
            .copied()
    }

    pub(crate) fn method_call_receiver_type(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::semantic::TyId> {
        self.method_call_receiver_types
            .get(&self.site_id(member_span)?)
            .copied()
    }

    pub(crate) fn generic_function_call_target(
        &self,
        call_span: ByteSpan,
    ) -> Option<crate::semantic::DefId> {
        self.generic_function_call_targets
            .get(&self.expression_id(call_span)?)
            .copied()
    }

    pub(crate) fn function_call_specialization(
        &self,
        call_span: ByteSpan,
    ) -> Option<&FunctionCallSpecialization> {
        self.function_call_specializations
            .get(&self.expression_id(call_span)?)
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
            .filter_map(|(id, specialization)| {
                self.expressions
                    .expression_span(*id)
                    .map(|span| (span, specialization))
            })
    }

    pub(crate) fn method_call_specialization(
        &self,
        member_span: ByteSpan,
    ) -> Option<&MethodCallSpecialization> {
        self.method_call_specializations
            .get(&self.site_id(member_span)?)
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
            .filter_map(|(id, specialization)| {
                self.sites.span(*id).map(|span| (span, specialization))
            })
    }

    pub(crate) fn callable_call(&self, call_span: ByteSpan) -> Option<&CallableCallFact> {
        self.callable_calls.get(&self.expression_id(call_span)?)
    }

    pub(crate) fn callable_call_entries(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &CallableCallFact)> + '_ {
        self.callable_calls.iter().filter_map(|(id, fact)| {
            self.expressions
                .expression_span(*id)
                .map(|span| (span, fact))
        })
    }

    pub(crate) fn drop_type_specializations(
        &self,
    ) -> impl Iterator<Item = &DropTypeSpecialization> + '_ {
        self.drop_type_specializations.iter()
    }

    pub(crate) fn associated_function_target(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::semantic::DefId> {
        self.associated_function_targets
            .get(&self.site_id(member_span)?)
            .copied()
    }

    pub(crate) fn enum_variant_target(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::semantic::DefId> {
        self.enum_variant_targets
            .get(&self.site_id(member_span)?)
            .copied()
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
    pub(crate) string_type_definition: crate::semantic::DefId,
    pub(crate) constructor: crate::semantics::RuntimeCallable,
    pub(crate) format_interface_definition: crate::semantic::DefId,
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
            string_type_definition: self.string_type_definition,
            constructor: self.constructor.clone(),
            format_interface_definition: self.format_interface_definition,
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
    pub(crate) def_id: crate::semantic::DefId,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) self_ty: TypeExpr,
    pub(crate) receiver_mode: crate::ast::MethodReceiverMode,
    pub(super) method_name: String,
    pub(super) free_type_parameters: HashSet<String>,
}

impl TypecheckProtocolMethod {
    pub(crate) fn new(
        def_id: crate::semantic::DefId,
        declaration_span: ByteSpan,
        target_name: String,
        self_ty: TypeExpr,
        receiver_mode: crate::ast::MethodReceiverMode,
        method_name: String,
        free_type_parameters: HashSet<String>,
    ) -> Self {
        Self {
            def_id,
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
            def_id: self.def_id,
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
            def_id: self.def_id,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub(crate) target: Option<crate::semantic::DefId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenericParameterFact {
    pub(crate) definition: crate::semantic::DefId,
    pub(crate) name: String,
    pub(crate) span: ByteSpan,
    pub(crate) is_copy: bool,
    pub(crate) bounds: Vec<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionCallSpecialization {
    pub(crate) def_id: crate::semantic::DefId,
    pub(crate) declaration_span: ByteSpan,
    pub(super) base_target_name: String,
    pub(super) generic_parameters: Vec<String>,
    pub(crate) target_name: String,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(super) free_type_parameters: HashSet<String>,
}

impl FunctionCallSpecialization {
    pub(crate) fn ordered_type_arguments(&self) -> Option<Vec<&TypeExpr>> {
        self.generic_parameters
            .iter()
            .map(|parameter| self.substitutions.get(parameter))
            .collect()
    }

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
            def_id: self.def_id,
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
    pub(crate) def_id: crate::semantic::DefId,
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
    pub(crate) def_id: crate::semantic::DefId,
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
            def_id: self.def_id,
            declaration_span: self.declaration_span,
            target_name: drop_target_name_from_base_and_self_ty(&self.base_target_name, &self_ty),
            self_ty,
            base_target_name: self.base_target_name.clone(),
            free_type_parameters: HashSet::new(),
        })
    }
}

impl MethodCallSpecialization {
    pub(crate) fn ordered_type_arguments(&self) -> Option<Vec<&TypeExpr>> {
        self.generic_parameters
            .iter()
            .map(|parameter| self.substitutions.get(parameter))
            .collect()
    }

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
            def_id: self.def_id,
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
