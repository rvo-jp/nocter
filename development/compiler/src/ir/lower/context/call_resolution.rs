use super::*;

impl<'a> LoweringContext<'a> {
    pub(in crate::ir::lower) fn typed_literal_call_target(
        &self,
        expression_span: ByteSpan,
        shape: crate::ast::LiteralShape,
        elements: &[Expr],
    ) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let literal = resolution.resolved.literal_resolution(expression_span)?;
        let result_type = self.expression_type_expr(expression_span)?;
        let key = crate::analysis::literal_specializations::literal_specialization_key(
            shape,
            elements,
            resolution.typed_hir,
            &self.generic_substitutions,
        )?;
        let name = crate::analysis::literal_specializations::literal_target_name(
            &result_type,
            shape,
            &key,
        );
        let target = call_target_for_source(
            literal.literal_declaration_span.source,
            resolution.root_source,
            name.clone(),
        );
        Some((target, name))
    }

    pub(in crate::ir::lower) fn function_name(&self) -> &str {
        &self.function_name
    }

    pub(in crate::ir::lower) fn return_type(&self) -> &Type {
        &self.return_type
    }

    pub(in crate::ir::lower) fn function_return_type(&self) -> &Type {
        &self.function_return_type
    }

    pub(in crate::ir::lower) fn function_return_type_expr(&self) -> Option<&TypeExpr> {
        self.function_return_type_expr.as_ref()
    }

    pub(in crate::ir::lower) fn function_returns_optional(&self) -> bool {
        self.function_returns_optional
    }

    pub(in crate::ir::lower) fn call_return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.function_signatures.return_type(target)
    }

    pub(in crate::ir::lower) fn call_return_type_expr(&self, call: &CallExpr) -> Option<TypeExpr> {
        if let Some(signature) = self
            .call_resolution
            .as_ref()?
            .typed_hir
            .callable_call(call.span)
        {
            return Some(substitute_type_expr_parameters(
                &signature.signature.return_type,
                &self.generic_substitutions,
            ));
        }
        if let Some(return_type) = self.method_call_return_type_expr(call) {
            return Some(return_type);
        }

        let resolution = self.call_resolution.as_ref()?;
        let signature = resolution.resolved.call_signature_for_call(call)?;
        let mut return_type = signature.return_type.clone();
        let mut call_substitutions = HashMap::new();
        if let Some(specialization) = resolution.typed_hir.function_call_specialization(call.span) {
            let specialization =
                specialization.with_context_substitutions(&self.generic_substitutions)?;
            call_substitutions = specialization.substitutions.clone();
            crate::typecheck::extend_associated_type_substitutions_with_resolver(
                &mut call_substitutions,
                resolution.resolved,
                |source| resolution.resolved_sources.get(&source).copied(),
            );
            return_type = substitute_type_expr_parameters(&return_type, &call_substitutions);
        }
        if let Some((owner, _)) = resolution.resolved.associated_function_for_call(call) {
            let self_ty = associated_function_self_type_expr(owner, call.span, &call_substitutions);
            return_type = type_expr_with_self_type(&return_type, &self_ty);
        }
        Some(substitute_type_expr_parameters(
            &return_type,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn call_value_type_expr(&self, call: &CallExpr) -> Option<TypeExpr> {
        let mut ty = self.call_return_type_expr(call)?;
        loop {
            match ty {
                TypeExpr::Fallible(fallible) => ty = *fallible.success,
                TypeExpr::Optional(optional) => ty = *optional.inner,
                _ => return Some(ty),
            }
        }
    }

    pub(in crate::ir::lower) fn call_argument_parameter_type_expr(
        &self,
        call: &CallExpr,
        index: usize,
    ) -> Option<TypeExpr> {
        if let Some(signature) = self
            .call_resolution
            .as_ref()?
            .typed_hir
            .callable_call(call.span)
        {
            return Some(substitute_type_expr_parameters(
                &signature.signature.parameters.get(index)?.ty,
                &self.generic_substitutions,
            ));
        }
        if let Some(ty) = self.method_call_argument_parameter_type_expr(call, index) {
            return Some(ty);
        }

        let resolution = self.call_resolution.as_ref()?;
        let signature = resolution.resolved.call_signature_for_call(call)?;
        let parameter = signature.parameters.get(index)?;
        let mut ty = parameter.ty.clone();
        let mut call_substitutions = HashMap::new();
        if let Some(specialization) = resolution
            .typed_hir
            .function_call_specialization(call.span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            call_substitutions = specialization.substitutions.clone();
            crate::typecheck::extend_associated_type_substitutions_with_resolver(
                &mut call_substitutions,
                resolution.resolved,
                |source| resolution.resolved_sources.get(&source).copied(),
            );
            ty = substitute_type_expr_parameters(&ty, &call_substitutions);
        }
        if let Some((owner, _)) = resolution.resolved.associated_function_for_call(call) {
            let self_ty = associated_function_self_type_expr(owner, call.span, &call_substitutions);
            ty = type_expr_with_self_type(&ty, &self_ty);
        }
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn local_binding_type_expr_for_identifier(
        &self,
        identifier: &IdentifierExpr,
    ) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution
            .resolved
            .local_symbol_for_identifier(identifier)?;
        let ty = resolution.typed_hir.binding_type_expr(symbol.id)?.clone();
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn binding_type_expr(&self, name_span: ByteSpan) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution
            .resolved
            .local_symbol_id_at_name_span(name_span)?;
        let ty = resolution.typed_hir.binding_type_expr(symbol)?.clone();
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn payload_binding_mode(
        &self,
        name_span: ByteSpan,
    ) -> Option<TypecheckPayloadBindingMode> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution
            .resolved
            .local_symbol_id_at_name_span(name_span)?;
        resolution.typed_hir.payload_binding_mode(symbol)
    }

    pub(in crate::ir::lower) fn expression_type_expr(
        &self,
        expression_span: ByteSpan,
    ) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let ty = resolution
            .typed_hir
            .expression_type_expr(expression_span)?
            .clone();
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn interpolation_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckInterpolationPlan> {
        self.call_resolution
            .as_ref()?
            .typed_hir
            .interpolation_plan(expression_span)
            .and_then(|plan| plan.with_context_substitutions(&self.generic_substitutions))
    }

    pub(in crate::ir::lower) fn collection_for_plan(
        &self,
        statement_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckCollectionForPlan> {
        let resolution = self.call_resolution.as_ref()?;
        let plan = resolution
            .typed_hir
            .collection_for_plan(statement_span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        crate::typecheck::specialize_collection_plan(plan, resolution.resolved)
    }

    pub(in crate::ir::lower) fn sequence_spread_plan(
        &self,
        spread_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckSequenceSpreadPlan> {
        let resolution = self.call_resolution.as_ref()?;
        let plan = resolution
            .typed_hir
            .sequence_spread_plan(spread_span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        crate::typecheck::specialize_sequence_spread_plan(plan, resolution.resolved)
    }

    pub(in crate::ir::lower) fn protocol_method_target(
        &self,
        method: &crate::typecheck::TypecheckProtocolMethod,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        let definition = resolution
            .resolved
            .callable_bodies
            .canonical_definition(method.def_id);
        let declaration = resolution
            .resolved
            .semantic_db
            .definition_anchor(definition)?;
        if let Some(target) = self
            .function_names
            .unique_target_for_name(&method.target_name)
        {
            return Some(target.clone());
        }
        if let Some(target_name) = self.function_names.name_for_definition(definition) {
            let target = call_target_for_source(
                declaration.source,
                resolution.root_source,
                target_name.clone(),
            );
            return Some(target);
        }
        let target = call_target_for_source(
            declaration.source,
            resolution.root_source,
            method.target_name.clone(),
        );
        Some(target)
    }

    pub(in crate::ir::lower) fn runtime_callable_target(
        &self,
        callable: &crate::semantics::RuntimeCallable,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        let declaration = resolution
            .resolved
            .semantic_db
            .definition_anchor(callable.definition)?;
        Some(call_target_for_source(
            declaration.source,
            resolution.root_source,
            callable.target_name.clone(),
        ))
    }

    pub(in crate::ir::lower) fn function_call_type_substitution(
        &self,
        call: &CallExpr,
        parameter: &str,
    ) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let specialization = resolution
            .typed_hir
            .function_call_specialization(call.span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        specialization.substitutions.get(parameter).cloned()
    }

    pub(in crate::ir::lower) fn call_parameter_types(
        &self,
        target: &CallTarget,
    ) -> Option<&[Type]> {
        self.function_signatures.parameter_types(target)
    }

    pub(in crate::ir::lower) fn call_parameter_abi_word_count(
        &self,
        target: &CallTarget,
    ) -> Option<usize> {
        self.function_signatures.parameter_abi_word_count(target)
    }

    pub(in crate::ir::lower) fn call_success_return_passing(
        &self,
        target: &CallTarget,
    ) -> Option<ReturnPassing> {
        self.function_signatures.success_return_passing(target)
    }

    pub(in crate::ir::lower) fn direct_call_target_and_name(
        &self,
        call: &CallExpr,
    ) -> Option<(CallTarget, String)> {
        if let Expr::Member(member) = call.callee.as_ref()
            && matches!(
                crate::semantic::OperatorCallableKind::from_lookup_name(&member.member),
                Some(
                    crate::semantic::OperatorCallableKind::ReadonlyIndex
                        | crate::semantic::OperatorCallableKind::ReadwriteIndex
                )
            )
            && let Some(method) = self.index_plan(call.span)?.method
        {
            let target = self.protocol_method_target(&method)?;
            return Some((target, method.target_name));
        }
        if let Expr::Member(member) = call.callee.as_ref()
            && let Some(plan) = self.comparison_plan(member.member_span)
            && let Some(method) = &plan.method
        {
            let target = self.protocol_method_target(method)?;
            return Some((target, method.target_name.clone()));
        }
        if let Some((target, target_name)) = self.function_call_specialization_target_and_name(call)
        {
            return Some((target, target_name));
        }
        if let Some((target, target_name)) = self.callable_call_target_and_name(call) {
            return Some((target, target_name));
        }
        match call.callee.as_ref() {
            Expr::Identifier(identifier) => Some((
                self.call_target(call, &identifier.name),
                identifier.name.clone(),
            )),
            Expr::Member(_) => {
                let resolution = self.call_resolution.as_ref()?;
                if let Some((target, target_name)) = self.method_call_target_and_name(call) {
                    return Some((target, target_name));
                }
                if let Some((_owner, function)) =
                    resolution.resolved.associated_function_for_call(call)
                {
                    let target = call_target_for_source(
                        function.name_span.source,
                        resolution.root_source,
                        function.target_name.clone(),
                    );
                    return Some((target, function.target_name.clone()));
                }
                let symbol = resolution.resolved.symbol_for_call(call)?;
                if !matches!(
                    symbol.kind,
                    SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_)
                ) {
                    return None;
                }
                let definition = resolution
                    .resolved
                    .canonical_callable_definition(symbol.declaration_span)?;
                let declaration = resolution
                    .resolved
                    .semantic_db
                    .definition_anchor(definition)?;
                let target_name = self
                    .function_names
                    .name_for_definition(definition)
                    .unwrap_or(&symbol.name)
                    .clone();
                let target =
                    call_target_for_source(declaration.source, resolution.root_source, target_name);
                Some((target, symbol.name.clone()))
            }
            _ => None,
        }
    }

    pub(in crate::ir::lower) fn error_payload_for_call(
        &self,
        call: &CallExpr,
    ) -> Option<ErrorPayload> {
        let (target, _) = self.direct_call_target_and_name(call)?;
        self.error_payloads.get(&target).cloned()
    }

    pub(in crate::ir::lower) fn call_target(
        &self,
        call: &CallExpr,
        fallback_name: &str,
    ) -> CallTarget {
        let Some(resolution) = &self.call_resolution else {
            return CallTarget::same_file(fallback_name);
        };
        if let Some((target, _target_name)) =
            self.function_call_specialization_target_and_name(call)
        {
            return target;
        }
        if let Some((_owner, function)) = resolution.resolved.associated_function_for_call(call) {
            return call_target_for_source(
                function.name_span.source,
                resolution.root_source,
                function.target_name.clone(),
            );
        }
        if let Some((target, _name)) = self.method_call_target_and_name(call) {
            return target;
        }

        let Some(symbol) = resolution.resolved.symbol_for_call(call) else {
            return CallTarget::same_file(fallback_name);
        };

        match &symbol.kind {
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
                let Some(definition) = resolution
                    .resolved
                    .canonical_callable_definition(symbol.declaration_span)
                else {
                    return CallTarget::same_file(symbol.name.clone());
                };
                let declaration_source = resolution
                    .resolved
                    .semantic_db
                    .definition_anchor(definition)
                    .map_or(symbol.declaration_span.source, |anchor| anchor.source);
                let target_name = self
                    .function_names
                    .name_for_definition(definition)
                    .unwrap_or(&symbol.name)
                    .clone();
                call_target_for_source(declaration_source, resolution.root_source, target_name)
            }
            SymbolKind::Imported(_) => CallTarget::same_file(fallback_name),
        }
    }

    fn function_call_specialization_target_and_name(
        &self,
        call: &CallExpr,
    ) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let specialization = resolution
            .typed_hir
            .function_call_specialization(call.span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        let declaration = resolution
            .resolved
            .semantic_db
            .definition_anchor(specialization.def_id)?;
        let target = call_target_for_source(
            declaration.source,
            resolution.root_source,
            specialization.target_name.clone(),
        );
        Some((target, specialization.target_name.clone()))
    }

    pub(in crate::ir::lower) fn intrinsic_for_call(
        &self,
        call: &CallExpr,
    ) -> Option<crate::intrinsics::IntrinsicId> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution.resolved.symbol_for_call(call)?;
        match &symbol.kind {
            SymbolKind::Primitive(_) => {
                crate::intrinsics::IntrinsicId::from_source_name(&symbol.name)
            }
            SymbolKind::Imported(_)
                if std_os_imported_primitive_name(&symbol.name)
                    || matches!(
                        resolution
                            .resolved
                            .trusted_declarations
                            .role(symbol.declaration_span),
                        Some(
                            crate::semantics::TrustedDeclarationRole::CurrentAllocationContext
                                | crate::semantics::TrustedDeclarationRole::RegionEnter
                                | crate::semantics::TrustedDeclarationRole::RegionRelease
                                | crate::semantics::TrustedDeclarationRole::AllocationAbort
                        )
                    ) =>
            {
                crate::intrinsics::IntrinsicId::from_source_name(&symbol.name)
            }
            SymbolKind::Function(_) | SymbolKind::Type(_) | SymbolKind::Imported(_) => None,
        }
    }

    pub(in crate::ir::lower) fn resolved_calls(&self) -> Option<(SourceId, &'a ResolveOutput)> {
        self.call_resolution
            .as_ref()
            .map(|resolution| (resolution.root_source, resolution.resolved))
    }

    pub(in crate::ir::lower) fn resolved_source(
        &self,
        source: SourceId,
    ) -> Option<&'a ResolveOutput> {
        self.call_resolution
            .as_ref()
            .and_then(|resolution| resolution.resolved_sources.get(&source).copied())
    }

    pub(in crate::ir::lower) fn binding_scalar_view_kind(
        &self,
        name_span: ByteSpan,
    ) -> Option<TypecheckScalarViewKind> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution
            .resolved
            .local_symbol_id_at_name_span(name_span)?;
        resolution.typed_hir.binding_scalar_view_kind(symbol)
    }

    pub(in crate::ir::lower) fn method_call_receiver<'b>(
        &self,
        call: &'b CallExpr,
    ) -> Option<&'b Expr> {
        let resolution = self.call_resolution.as_ref()?;
        if resolution.typed_hir.callable_call(call.span).is_some() {
            return Some(&call.callee);
        }
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        if self.comparison_plan(member.member_span).is_some() {
            return Some(&member.object);
        }
        resolution
            .typed_hir
            .method_call_target(member.member_span)?;
        Some(&member.object)
    }

    pub(in crate::ir::lower) fn method_call_receiver_kind(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckMethodReceiverKind> {
        if let Some(plan) = self.comparison_plan(member_span)
            && plan.method.is_some()
        {
            return Some(crate::typecheck::TypecheckMethodReceiverKind::ReadonlyBorrow);
        }
        self.call_resolution
            .as_ref()?
            .typed_hir
            .method_call_receiver_kind(member_span)
    }

    pub(in crate::ir::lower) fn call_argument_uses_implicit_readonly_borrow(
        &self,
        call: &CallExpr,
        index: usize,
    ) -> bool {
        if index != 0 {
            return false;
        }
        let Expr::Member(member) = call.callee.as_ref() else {
            return false;
        };
        self.comparison_plan(member.member_span)
            .is_some_and(|plan| plan.right_implicit_readonly_borrow)
    }

    pub(in crate::ir::lower) fn comparison_call_reverses_operands(&self, call: &CallExpr) -> bool {
        let Expr::Member(member) = call.callee.as_ref() else {
            return false;
        };
        self.comparison_plan(member.member_span)
            .is_some_and(|plan| plan.reverse_operands)
    }

    pub(in crate::ir::lower) fn coercion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckCoercionPlan> {
        let resolution = self.call_resolution.as_ref()?;
        if let Some(plan) = resolution
            .typed_hir
            .coercion_plan(expression_span)
            .and_then(|plan| plan.with_context_substitutions(&self.generic_substitutions))
        {
            if plan.requirement_span.is_none() {
                return Some(plan);
            }
            let mut candidate_sources = Vec::new();
            collect_type_expr_resolution_sources(&plan.self_ty, &mut candidate_sources);
            collect_type_expr_resolution_sources(&plan.target_ty, &mut candidate_sources);
            candidate_sources.dedup();
            let mut resolvers = candidate_sources
                .into_iter()
                .filter_map(|source| self.resolved_source(source))
                .collect::<Vec<_>>();
            resolvers.push(resolution.resolved);
            return crate::typecheck::specialize_coercion_plan_across_resolvers(plan, resolvers);
        }
        resolution.typed_hir.index_plans().find_map(|plan| {
            let plan = self.index_plan(plan.expression_span)?;
            if plan.object_span != expression_span {
                return None;
            }
            let crate::typecheck::TypecheckConversionKind::BorrowCoercion(coercion) =
                plan.conversion?.kind
            else {
                return None;
            };
            Some(coercion)
        })
    }

    pub(in crate::ir::lower) fn index_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckIndexPlan> {
        let resolution = self.call_resolution.as_ref()?;
        let plan = resolution
            .typed_hir
            .index_plan(expression_span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        if plan.projection != crate::typecheck::TypecheckIndexProjection::Requirement {
            return Some(plan);
        }
        let mut candidate_sources = Vec::new();
        collect_type_expr_resolution_sources(&plan.target_ty, &mut candidate_sources);
        candidate_sources.dedup();
        let mut resolvers = Vec::new();
        for source in candidate_sources {
            let Some(resolved) = self.resolved_source(source) else {
                continue;
            };
            resolvers.push(resolved);
        }
        resolvers.push(resolution.resolved);
        crate::typecheck::specialize_index_plan_across_resolvers(plan, resolvers)
    }

    pub(in crate::ir::lower) fn conversion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckConversionPlan> {
        let resolution = self.call_resolution.as_ref()?;
        if let Some(plan) = resolution
            .typed_hir
            .conversion_plan(expression_span)
            .and_then(|plan| plan.with_context_substitutions(&self.generic_substitutions))
        {
            return Some(plan);
        }
        resolution
            .typed_hir
            .comparison_plans()
            .find_map(|plan| {
                let plan = plan.with_context_substitutions(&self.generic_substitutions)?;
                if plan.left_span == expression_span {
                    plan.left_conversion
                } else if plan.right_span == expression_span {
                    plan.right_conversion
                } else {
                    None
                }
            })
            .or_else(|| {
                resolution.typed_hir.index_plans().find_map(|plan| {
                    let plan = self.index_plan(plan.expression_span)?;
                    if plan.object_span == expression_span {
                        plan.conversion
                    } else {
                        None
                    }
                })
            })
    }

    pub(in crate::ir::lower) fn comparison_plan(
        &self,
        operator_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckComparisonPlan> {
        let resolution = self.call_resolution.as_ref()?;
        let plan = resolution
            .typed_hir
            .comparison_plan(operator_span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        if plan.method.is_some() {
            return Some(plan);
        }

        // A generic callable can be declared in one module and specialized with
        // a type declared in another. Resolve the substituted type in its own
        // source context so its instance surface and any visible coercions are
        // available. The declaration's resolver remains the deterministic
        // fallback for primitive and same-module substitutions.
        let mut candidate_sources = Vec::new();
        collect_type_expr_resolution_sources(&plan.left_ty, &mut candidate_sources);
        collect_type_expr_resolution_sources(&plan.right_ty, &mut candidate_sources);
        candidate_sources.dedup();
        for source in candidate_sources {
            let Some(resolved) = self.resolved_source(source) else {
                continue;
            };
            if let Some(specialized) =
                crate::typecheck::specialize_comparison_plan(plan.clone(), resolved)
            {
                return Some(specialized);
            }
        }
        crate::typecheck::specialize_comparison_plan(plan, resolution.resolved)
    }

    pub(in crate::ir::lower) fn coercion_call_target(
        &self,
        plan: &crate::typecheck::TypecheckCoercionPlan,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        let target_name = super::super::coercion_symbols::coercion_symbol_name(plan);
        let declaration_source = resolution
            .resolved
            .semantic_db
            .definition_span(plan.def_id?)?
            .source;
        Some(
            self.function_names
                .unique_target_for_name(&target_name)
                .cloned()
                .unwrap_or_else(|| {
                    call_target_for_source(declaration_source, resolution.root_source, target_name)
                }),
        )
    }

    fn method_call_target_and_name(&self, call: &CallExpr) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let method_definition = resolution
            .typed_hir
            .method_call_target(member.member_span)?;
        if let Some(specialization) = resolution
            .typed_hir
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            let declaration = resolution
                .resolved
                .semantic_db
                .definition_anchor(specialization.def_id)?;
            let target = self
                .function_names
                .unique_target_for_name(&specialization.target_name)
                .cloned()
                .unwrap_or_else(|| {
                    call_target_for_source(
                        declaration.source,
                        resolution.root_source,
                        specialization.target_name.clone(),
                    )
                });
            return Some((target, specialization.target_name.clone()));
        }
        if resolution
            .typed_hir
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        let definition = resolution
            .resolved
            .callable_bodies
            .canonical_definition(method_definition);
        let declaration = resolution
            .resolved
            .semantic_db
            .definition_anchor(definition)?;
        let target_name = self.function_names.name_for_definition(definition)?.clone();
        let target = call_target_for_source(
            declaration.source,
            resolution.root_source,
            target_name.clone(),
        );
        Some((target, target_name))
    }

    fn callable_call_target_and_name(&self, call: &CallExpr) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let specialization = resolution
            .typed_hir
            .callable_call(call.span)?
            .specialization
            .with_context_substitutions(&self.generic_substitutions)?;
        let target = self
            .function_names
            .unique_target_for_name(&specialization.target_name)
            .cloned()
            .unwrap_or_else(|| {
                call_target_for_source(
                    specialization.callable_ty.span().source,
                    resolution.root_source,
                    specialization.target_name.clone(),
                )
            });
        Some((target, specialization.target_name))
    }

    fn method_call_return_type_expr(&self, call: &CallExpr) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let method_definition = resolution
            .typed_hir
            .method_call_target(member.member_span)?;
        let method = resolution.resolved.method_signature(method_definition)?;
        let mut return_type = method.signature.return_type.clone();
        if let Some(specialization) = resolution
            .typed_hir
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            let mut substitutions = self.generic_substitutions.clone();
            substitutions.extend(specialization.substitutions.clone());
            substitutions.insert("Self".to_string(), specialization.self_ty.clone());
            crate::typecheck::extend_associated_type_substitutions_with_resolver(
                &mut substitutions,
                resolution.resolved,
                |source| resolution.resolved_sources.get(&source).copied(),
            );
            return Some(substitute_type_expr_parameters(
                &return_type,
                &substitutions,
            ));
        }
        if resolution
            .typed_hir
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        if let Some(self_ty) = &method.owner_target_ty {
            return_type = type_expr_with_self_type(&return_type, self_ty);
        }
        Some(substitute_type_expr_parameters(
            &return_type,
            &self.generic_substitutions,
        ))
    }

    fn method_call_argument_parameter_type_expr(
        &self,
        call: &CallExpr,
        index: usize,
    ) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let method_definition = resolution
            .typed_hir
            .method_call_target(member.member_span)?;
        let method = resolution.resolved.method_signature(method_definition)?;
        let parameter = method.signature.parameters.get(index)?;
        let mut ty = parameter.ty.clone();
        if let Some(specialization) = resolution
            .typed_hir
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            let mut substitutions = self.generic_substitutions.clone();
            substitutions.extend(specialization.substitutions.clone());
            substitutions.insert("Self".to_string(), specialization.self_ty.clone());
            crate::typecheck::extend_associated_type_substitutions_with_resolver(
                &mut substitutions,
                resolution.resolved,
                |source| resolution.resolved_sources.get(&source).copied(),
            );
            return Some(substitute_type_expr_parameters(&ty, &substitutions));
        }
        if resolution
            .typed_hir
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        if let Some(self_ty) = &method.owner_target_ty {
            ty = type_expr_with_self_type(&ty, self_ty);
        }
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }
}

fn collect_type_expr_resolution_sources(ty: &TypeExpr, sources: &mut Vec<SourceId>) {
    match ty {
        TypeExpr::Callable(callable) => {
            for parameter in &callable.parameters {
                collect_type_expr_resolution_sources(&parameter.ty, sources);
            }
            collect_type_expr_resolution_sources(&callable.return_type, sources);
        }
        TypeExpr::Closure(closure) => {
            for capture in &closure.captures {
                collect_type_expr_resolution_sources(&capture.ty, sources);
            }
            for parameter in &closure.parameters {
                collect_type_expr_resolution_sources(parameter, sources);
            }
            collect_type_expr_resolution_sources(&closure.return_type, sources);
        }
        TypeExpr::Opaque(opaque) => {
            collect_type_expr_resolution_sources(&opaque.interface, sources);
            for binding in &opaque.associated_bindings {
                collect_type_expr_resolution_sources(&binding.value, sources);
            }
            if let Some(witness) = &opaque.witness {
                collect_type_expr_resolution_sources(witness, sources);
            }
        }
        TypeExpr::Reference(reference) => sources.push(reference.span.source),
        TypeExpr::Generic(generic) => {
            sources.push(generic.name_span.source);
            for argument in &generic.arguments {
                collect_type_expr_resolution_sources(argument, sources);
            }
        }
        TypeExpr::Projection(projection) => {
            collect_type_expr_resolution_sources(&projection.base, sources);
            sources.push(projection.name_span.source);
        }
        TypeExpr::Pointer(pointer) => collect_type_expr_resolution_sources(&pointer.inner, sources),
        TypeExpr::Borrow(borrow) => collect_type_expr_resolution_sources(&borrow.inner, sources),
        TypeExpr::View(view) => collect_type_expr_resolution_sources(&view.element, sources),
        TypeExpr::Array(array) => collect_type_expr_resolution_sources(&array.element, sources),
        TypeExpr::Optional(optional) => {
            collect_type_expr_resolution_sources(&optional.inner, sources)
        }
        TypeExpr::Fallible(fallible) => {
            collect_type_expr_resolution_sources(&fallible.success, sources);
            collect_type_expr_resolution_sources(&fallible.error, sources);
        }
    }
}

fn associated_function_self_type_expr(
    owner: &crate::resolve::TypeSymbol,
    span: ByteSpan,
    substitutions: &HashMap<String, TypeExpr>,
) -> TypeExpr {
    if owner.generic_parameters.is_empty() {
        return TypeExpr::Reference(crate::ast::TypeReference {
            span,
            name: owner.canonical_name.clone(),
        });
    }

    TypeExpr::Generic(crate::ast::GenericType {
        span,
        name: owner.canonical_name.clone(),
        name_span: span,
        arguments: owner
            .generic_parameters
            .iter()
            .map(|parameter| {
                substitutions.get(parameter).cloned().unwrap_or_else(|| {
                    TypeExpr::Reference(crate::ast::TypeReference {
                        span,
                        name: parameter.clone(),
                    })
                })
            })
            .collect(),
    })
}
