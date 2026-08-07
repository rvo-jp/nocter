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
            resolution.typecheck_facts,
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
            .typecheck_facts
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
        if let Some(specialization) = resolution
            .typecheck_facts
            .function_call_specialization(call.span)
        {
            let specialization =
                specialization.with_context_substitutions(&self.generic_substitutions)?;
            call_substitutions = specialization.substitutions.clone();
            return_type =
                substitute_type_expr_parameters(&return_type, &specialization.substitutions);
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
            .typecheck_facts
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
            .typecheck_facts
            .function_call_specialization(call.span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            call_substitutions = specialization.substitutions.clone();
            ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
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
        let ty = resolution
            .typecheck_facts
            .binding_type_expr(symbol.name_span)?
            .clone();
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn binding_type_expr(&self, name_span: ByteSpan) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let ty = resolution
            .typecheck_facts
            .binding_type_expr(name_span)?
            .clone();
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
    }

    pub(in crate::ir::lower) fn payload_binding_mode(
        &self,
        name_span: ByteSpan,
    ) -> Option<TypecheckPayloadBindingMode> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .payload_binding_mode(name_span)
    }

    pub(in crate::ir::lower) fn expression_type_expr(
        &self,
        expression_span: ByteSpan,
    ) -> Option<TypeExpr> {
        let resolution = self.call_resolution.as_ref()?;
        let ty = resolution
            .typecheck_facts
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
            .typecheck_facts
            .interpolation_plan(expression_span)
            .cloned()
    }

    pub(in crate::ir::lower) fn collection_for_plan(
        &self,
        statement_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckCollectionForPlan> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .collection_for_plan(statement_span)?
            .with_context_substitutions(&self.generic_substitutions)
    }

    pub(in crate::ir::lower) fn sequence_spread_plan(
        &self,
        spread_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckSequenceSpreadPlan> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .sequence_spread_plan(spread_span)?
            .with_context_substitutions(&self.generic_substitutions)
    }

    pub(in crate::ir::lower) fn iteration_method_target(
        &self,
        method: &crate::typecheck::TypecheckIterationMethod,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        if let Some(target) = self
            .function_names
            .unique_target_for_name(&method.target_name)
        {
            return Some(target.clone());
        }
        Some(call_target_for_source(
            method.declaration_span.source,
            resolution.root_source,
            method.target_name.clone(),
        ))
    }

    pub(in crate::ir::lower) fn runtime_callable_target(
        &self,
        callable: &crate::semantics::RuntimeCallable,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        Some(call_target_for_source(
            callable.declaration.source,
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
            .typecheck_facts
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
                let target = call_target_for_source(
                    symbol.declaration_span.source,
                    resolution.root_source,
                    self.function_names
                        .name_for_declaration(symbol.declaration_span)
                        .unwrap_or(&symbol.name)
                        .clone(),
                );
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
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_)
                if symbol.declaration_span.source != resolution.root_source =>
            {
                let target_name = self
                    .function_names
                    .name_for_declaration(symbol.declaration_span)
                    .unwrap_or(&symbol.name);
                CallTarget::imported(symbol.declaration_span.source, target_name.clone())
            }
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
                CallTarget::same_file(symbol.name.clone())
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
            .typecheck_facts
            .function_call_specialization(call.span)?
            .with_context_substitutions(&self.generic_substitutions)?;
        let target = call_target_for_source(
            specialization.declaration_span.source,
            resolution.root_source,
            specialization.target_name.clone(),
        );
        Some((target, specialization.target_name.clone()))
    }

    pub(in crate::ir::lower) fn primitive_name_for_call(&self, call: &CallExpr) -> Option<&str> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution.resolved.symbol_for_call(call)?;
        match &symbol.kind {
            SymbolKind::Primitive(_) => Some(symbol.name.as_str()),
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
                Some(symbol.name.as_str())
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
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .binding_scalar_view_kind(name_span)
    }

    pub(in crate::ir::lower) fn method_call_receiver<'b>(
        &self,
        call: &'b CallExpr,
    ) -> Option<&'b Expr> {
        let resolution = self.call_resolution.as_ref()?;
        if resolution
            .typecheck_facts
            .callable_call(call.span)
            .is_some()
        {
            return Some(&call.callee);
        }
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        Some(&member.object)
    }

    pub(in crate::ir::lower) fn method_call_receiver_kind(
        &self,
        member_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckMethodReceiverKind> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .method_call_receiver_kind(member_span)
    }

    pub(in crate::ir::lower) fn coercion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckCoercionPlan> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .coercion_plan(expression_span)?
            .with_context_substitutions(&self.generic_substitutions)
    }

    pub(in crate::ir::lower) fn conversion_plan(
        &self,
        expression_span: ByteSpan,
    ) -> Option<crate::typecheck::TypecheckConversionPlan> {
        self.call_resolution
            .as_ref()?
            .typecheck_facts
            .conversion_plan(expression_span)?
            .with_context_substitutions(&self.generic_substitutions)
    }

    pub(in crate::ir::lower) fn coercion_call_target(
        &self,
        plan: &crate::typecheck::TypecheckCoercionPlan,
    ) -> Option<CallTarget> {
        let resolution = self.call_resolution.as_ref()?;
        Some(
            self.function_names
                .unique_target_for_name(&plan.target_name)
                .cloned()
                .unwrap_or_else(|| {
                    call_target_for_source(
                        plan.declaration_span.source,
                        resolution.root_source,
                        plan.target_name.clone(),
                    )
                }),
        )
    }

    fn method_call_target_and_name(&self, call: &CallExpr) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let method_name_span = resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        if let Some(specialization) = resolution
            .typecheck_facts
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            let target = self
                .function_names
                .unique_target_for_name(&specialization.target_name)
                .cloned()
                .unwrap_or_else(|| {
                    call_target_for_source(
                        method_name_span.source,
                        resolution.root_source,
                        specialization.target_name.clone(),
                    )
                });
            return Some((target, specialization.target_name.clone()));
        }
        if resolution
            .typecheck_facts
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        let target_name = self
            .function_names
            .name_for_declaration(method_name_span)?
            .clone();
        let target = call_target_for_source(
            method_name_span.source,
            resolution.root_source,
            target_name.clone(),
        );
        Some((target, target_name))
    }

    fn callable_call_target_and_name(&self, call: &CallExpr) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let specialization = resolution
            .typecheck_facts
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
        let method_name_span = resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        let method = resolution
            .resolved
            .method_signature_by_name_span(method_name_span)?;
        let mut return_type = method.signature.return_type.clone();
        if let Some(specialization) = resolution
            .typecheck_facts
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            return_type = type_expr_with_self_type(&return_type, &specialization.self_ty);
            return_type =
                substitute_type_expr_parameters(&return_type, &specialization.substitutions);
            return Some(substitute_type_expr_parameters(
                &return_type,
                &self.generic_substitutions,
            ));
        }
        if resolution
            .typecheck_facts
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        if let Some(self_ty) = &method.impl_target_ty {
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
        let method_name_span = resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        let method = resolution
            .resolved
            .method_signature_by_name_span(method_name_span)?;
        let parameter = method.signature.parameters.get(index)?;
        let mut ty = parameter.ty.clone();
        if let Some(specialization) = resolution
            .typecheck_facts
            .method_call_specialization(member.member_span)
            .and_then(|specialization| {
                specialization.with_context_substitutions(&self.generic_substitutions)
            })
        {
            ty = type_expr_with_self_type(&ty, &specialization.self_ty);
            ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
            return Some(substitute_type_expr_parameters(
                &ty,
                &self.generic_substitutions,
            ));
        }
        if resolution
            .typecheck_facts
            .generic_method_call_target(member.member_span)
            .is_some()
        {
            return None;
        }
        if let Some(self_ty) = &method.impl_target_ty {
            ty = type_expr_with_self_type(&ty, self_ty);
        }
        Some(substitute_type_expr_parameters(
            &ty,
            &self.generic_substitutions,
        ))
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
