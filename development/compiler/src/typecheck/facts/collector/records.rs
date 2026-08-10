use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn record_conversion_plan(
        &mut self,
        expression_span: ByteSpan,
        source_span: ByteSpan,
        operator_span: Option<ByteSpan>,
        selected: crate::typecheck::conversions::SelectedConversion,
    ) {
        let mut free_type_parameters = HashSet::new();
        let Some(source_ty) = type_to_type_expr_allowing_parameters(
            &selected.source_type,
            expression_span,
            &mut free_type_parameters,
        ) else {
            return;
        };
        let Some(target_ty) = type_to_type_expr_allowing_parameters(
            &selected.target_type,
            expression_span,
            &mut free_type_parameters,
        ) else {
            return;
        };
        let kind = match selected.kind {
            crate::typecheck::conversions::SelectedConversionKind::Exact => return,
            crate::typecheck::conversions::SelectedConversionKind::LosslessInteger => {
                TypecheckConversionKind::LosslessInteger
            }
            crate::typecheck::conversions::SelectedConversionKind::CapabilityWeakening => {
                TypecheckConversionKind::CapabilityWeakening
            }
            crate::typecheck::conversions::SelectedConversionKind::BorrowCoercion(coercion) => {
                let Some(self_ty) = type_to_type_expr_allowing_parameters(
                    &coercion.source_type,
                    expression_span,
                    &mut free_type_parameters,
                ) else {
                    return;
                };
                let substitutions = coercion
                    .substitutions
                    .iter()
                    .filter_map(|(name, ty)| {
                        type_to_type_expr_allowing_parameters(
                            ty,
                            expression_span,
                            &mut free_type_parameters,
                        )
                        .map(|ty| (name.clone(), ty))
                    })
                    .collect::<HashMap<_, _>>();
                if substitutions.len() != coercion.substitutions.len() {
                    return;
                }
                TypecheckConversionKind::BorrowCoercion(TypecheckCoercionPlan {
                    declaration_span: coercion.declaration_span,
                    focus_span: coercion.focus_span,
                    receiver_mode: coercion.receiver_mode,
                    source_is_readwrite: coercion.source_is_readwrite,
                    target_name: format!(
                        "{}.__nocter$coerce${}",
                        canonical_type_expr(&self_ty),
                        coercion.focus_span.start
                    ),
                    self_ty,
                    target_ty: target_ty.clone(),
                    substitutions,
                    has_explicit_result_provenance: coercion.has_explicit_result_provenance,
                    free_type_parameters: free_type_parameters.clone(),
                })
            }
        };
        self.facts.conversion_plans.insert(
            expression_span,
            TypecheckConversionPlan {
                expression_span,
                source_span,
                operator_span,
                source_ty,
                target_ty,
                kind,
            },
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_interpolation_plan(
        &mut self,
        expression: &crate::ast::InterpolatedStringExpr,
        environment: &TypeEnvironment,
    ) {
        let Some(runtime) = self.resolved.trusted_declarations.interpolation_runtime() else {
            return;
        };
        let mut parts = Vec::with_capacity(expression.parts.len());
        for part in &expression.parts {
            let (span, expression_span, ty) = match part {
                crate::ast::InterpolatedStringPart::Text(text) => {
                    (text.span, None, crate::typecheck::model::Type::Str)
                }
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    let ty = expression_type(&part.expression, self.resolved, environment);
                    (part.span, Some(part.expression_span), ty)
                }
            };
            let Some(formatter) = crate::typecheck::strings::interpolation_format_method(
                &ty,
                expression_span.unwrap_or(span),
                self.resolved,
                environment,
            ) else {
                return;
            };
            let mut free_type_parameters = std::collections::HashSet::new();
            let Some(accepted_type) =
                crate::typecheck::facts::type_to_type_expr_allowing_parameters(
                    &ty,
                    span,
                    &mut free_type_parameters,
                )
            else {
                return;
            };
            parts.push(TypecheckInterpolationPart {
                span,
                expression_span,
                accepted_type,
                formatter,
            });
        }
        self.facts.interpolation_plans.insert(
            expression.span,
            TypecheckInterpolationPlan {
                string_type_declaration: runtime.string_type_declaration,
                constructor: runtime.constructor.clone(),
                format_interface_declaration: runtime.format_interface_declaration,
                parts,
            },
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_type_reference(
        &mut self,
        name: &str,
        span: ByteSpan,
        contextual_type: TypeExpr,
    ) {
        let target = self
            .generic_parameter_declaration(name)
            .map(TypeOccurrenceTarget::GenericParameter)
            .or_else(|| {
                self.resolved
                    .symbols
                    .symbol_by_name(name)
                    .filter(|symbol| matches!(symbol.kind, SymbolKind::Type(_)))
                    .map(|symbol| TypeOccurrenceTarget::Declaration(symbol.declaration_span))
            });

        self.facts.type_occurrences.push(TypeOccurrenceFact {
            focus_span: span,
            contextual_type,
            target,
        });
    }

    pub(in crate::typecheck::facts::collector) fn record_associated_type_reference(
        &mut self,
        projection: &crate::ast::ProjectedType,
    ) {
        let target = if let TypeExpr::Reference(reference) = projection.base.as_ref() {
            if reference.name == "Self" {
                self.associated_type_declaration(&projection.name)
            } else {
                self.generic_parameter_declaration(&reference.name)
                    .and_then(|span| self.facts.generic_parameter(span))
                    .and_then(|parameter| {
                        let mut candidates = parameter.bounds.iter().filter_map(|bound| {
                            let name = match bound {
                                TypeExpr::Reference(reference) => &reference.name,
                                TypeExpr::Generic(generic) => &generic.name,
                                _ => return None,
                            };
                            self.resolved
                                .type_symbol_by_reference_name(name)?
                                .associated_types
                                .iter()
                                .find(|associated| associated.name == projection.name)
                                .map(|associated| associated.name_span)
                        });
                        let candidate = candidates.next()?;
                        candidates.next().is_none().then_some(candidate)
                    })
            }
        } else {
            let base =
                super::super::super::type_expr::type_expr_to_type(&projection.base, self.resolved);
            super::super::super::associated_types::concrete_projection_contract(
                &base,
                &projection.name,
                self.resolved,
            )
            .map(|(_, associated)| associated.name_span)
        };
        self.facts.type_occurrences.push(TypeOccurrenceFact {
            focus_span: projection.name_span,
            contextual_type: TypeExpr::Projection(projection.clone()),
            target: target.map(TypeOccurrenceTarget::Member),
        });
    }

    pub(in crate::typecheck::facts::collector) fn record_opaque_associated_type_reference(
        &mut self,
        interface: &TypeExpr,
        binding: &crate::ast::OpaqueAssociatedTypeBinding,
    ) {
        let interface_name = match interface {
            TypeExpr::Reference(reference) => &reference.name,
            TypeExpr::Generic(generic) => &generic.name,
            _ => return,
        };
        let target = self
            .resolved
            .type_symbol_by_reference_name(interface_name)
            .and_then(|symbol| {
                symbol
                    .associated_types
                    .iter()
                    .find(|associated| associated.name == binding.name)
            })
            .map(|associated| TypeOccurrenceTarget::Member(associated.name_span));
        self.facts.type_occurrences.push(TypeOccurrenceFact {
            focus_span: binding.name_span,
            contextual_type: binding.value.clone(),
            target,
        });
    }

    pub(in crate::typecheck::facts::collector) fn record_if_is_pattern_references(
        &mut self,
        statement: &IfIsStmt,
    ) {
        self.record_enum_pattern_references(
            &statement.enum_name,
            statement.enum_name_span,
            &statement.variant_name,
            statement.variant_name_span,
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_switch_arm_pattern_references(
        &mut self,
        arm: &SwitchArm,
    ) {
        self.record_enum_pattern_references(
            &arm.enum_name,
            arm.enum_name_span,
            &arm.variant_name,
            arm.variant_name_span,
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_enum_pattern_references(
        &mut self,
        enum_name: &str,
        enum_name_span: ByteSpan,
        variant_name: &str,
        variant_name_span: ByteSpan,
    ) {
        self.record_type_reference(
            enum_name,
            enum_name_span,
            TypeExpr::Reference(TypeReference {
                span: enum_name_span,
                name: enum_name.to_string(),
            }),
        );

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

        self.record_enum_variant_reference(variant_name_span, variant);
    }

    pub(in crate::typecheck::facts::collector) fn record_parameter_bindings(
        &mut self,
        parameters: &[Parameter],
        environment: &TypeEnvironment,
    ) {
        for parameter in parameters {
            self.record_environment_binding(parameter.name_span, &parameter.name, environment);
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_payload_binding(
        &mut self,
        payload: &SwitchPayloadBinding,
        environment: &TypeEnvironment,
    ) {
        self.record_environment_binding(payload.span, &payload.name, environment);
        let Some(ty) = environment.get(&payload.name) else {
            return;
        };
        if ty.is_unknown_or_unresolved() {
            return;
        }
        let mode = if non_copy_owned_type_kind(ty, self.resolved).is_some() {
            TypecheckPayloadBindingMode::Move
        } else {
            TypecheckPayloadBindingMode::Copy
        };
        self.facts.payload_binding_modes.insert(payload.span, mode);
    }

    pub(in crate::typecheck::facts::collector) fn record_environment_binding(
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

    pub(in crate::typecheck::facts::collector) fn record_environment_binding_readonly(
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

    pub(in crate::typecheck::facts::collector) fn record_binding(
        &mut self,
        name_span: ByteSpan,
        ty: &Type,
        is_mutable: bool,
    ) {
        self.record_binding_type(name_span, ty);
        self.facts.binding_readonly.insert(name_span, !is_mutable);
    }

    pub(in crate::typecheck::facts::collector) fn record_binding_type(
        &mut self,
        name_span: ByteSpan,
        ty: &Type,
    ) {
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

    pub(in crate::typecheck::facts::collector) fn record_expression_type(
        &mut self,
        expression_span: ByteSpan,
        ty: &Type,
    ) {
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(ty, expression_span, &mut free_type_parameters)
        {
            self.record_payload_enum_drop_type_specializations(&ty);
            self.facts.expression_type_exprs.insert(expression_span, ty);
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_drop_type_specialization(
        &mut self,
        span: ByteSpan,
        ty: &Type,
    ) {
        if let Some(specialization) = self.drop_type_specialization(span, ty) {
            self.facts.drop_type_specializations.push(specialization);
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_payload_enum_drop_type_specializations(
        &mut self,
        ty: &TypeExpr,
    ) {
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

    pub(in crate::typecheck::facts::collector) fn drop_type_specialization(
        &self,
        span: ByteSpan,
        ty: &Type,
    ) -> Option<DropTypeSpecialization> {
        let mut free_type_parameters = HashSet::new();
        let self_ty = type_to_type_expr_allowing_parameters(ty, span, &mut free_type_parameters)?;
        drop_type_specialization_from_self_ty(&self_ty, self.resolved, free_type_parameters)
    }

    pub(in crate::typecheck::facts::collector) fn record_struct_field_member_reference(
        &mut self,
        member: &MemberExpr,
        environment: &TypeEnvironment,
    ) {
        let Some((_, field)) = resolved_struct_field_for_member(member, self.resolved, environment)
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

    pub(in crate::typecheck::facts::collector) fn record_struct_literal_field_reference(
        &mut self,
        literal: &StructLiteralExpr,
        field: &StructLiteralField,
        environment: &TypeEnvironment,
    ) {
        let Some((_, expected_field)) =
            resolved_struct_field_for_literal_field(literal, field, self.resolved, environment)
        else {
            return;
        };

        let field_ty = struct_literal_field_type(literal, field, self.resolved, environment);
        self.record_struct_field_reference(
            field.name_span,
            expected_field,
            field_ty.as_ref(),
            environment,
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_struct_field_reference(
        &mut self,
        span: ByteSpan,
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
    }

    pub(in crate::typecheck::facts::collector) fn record_enum_variant_reference(
        &mut self,
        span: ByteSpan,
        variant: &crate::resolve::EnumVariantSignature,
    ) {
        self.facts
            .enum_variant_targets
            .insert(span, variant.name_span);
    }

    pub(in crate::typecheck::facts::collector) fn record_function_call_reference(
        &mut self,
        call: &CallExpr,
        declaration_span: ByteSpan,
    ) {
        let Some(name_span) = call_callee_name_span(call) else {
            return;
        };

        self.facts
            .function_call_targets
            .insert(name_span, declaration_span);
    }
}
