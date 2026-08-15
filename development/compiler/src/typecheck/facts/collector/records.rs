use super::*;

impl TypedHirBuilder<'_> {
    pub(in crate::typecheck::facts::collector) fn record_index_plan(
        &mut self,
        expression: &crate::ast::IndexExpr,
        access: crate::typecheck::indexing::IndexAccess,
        environment: &TypeEnvironment,
    ) {
        let Ok(selected) = crate::typecheck::indexing::select_index_expression(
            expression,
            access,
            self.resolved,
            environment,
        ) else {
            return;
        };
        let mut free_type_parameters = std::collections::HashSet::new();
        let Some(target_ty) = crate::typecheck::facts::type_to_type_expr_allowing_parameters(
            &selected.target_type,
            expression.object.span(),
            &mut free_type_parameters,
        ) else {
            return;
        };
        let Some(index_ty) = crate::typecheck::facts::type_to_type_expr_allowing_parameters(
            &selected.index_type,
            expression.index.span(),
            &mut free_type_parameters,
        ) else {
            return;
        };
        let Some(element_ty) = crate::typecheck::facts::type_to_type_expr_allowing_parameters(
            &selected.element_type,
            expression.span,
            &mut free_type_parameters,
        ) else {
            return;
        };
        let conversion = selected.coercion.and_then(|coercion| {
            crate::typecheck::facts::typecheck_conversion_plan(
                expression.object.span(),
                expression.object.span(),
                None,
                crate::typecheck::conversions::selected_receiver_coercion(
                    &selected.target_type,
                    coercion,
                ),
            )
        });
        if let Some(conversion) = &conversion {
            self.facts
                .conversion_plans
                .insert(expression.object.span(), conversion.clone());
            self.intern_conversion_plan_types(conversion);
        }
        self.intern_compiler_type_tree(&target_ty);
        self.intern_compiler_type_tree(&index_ty);
        self.intern_compiler_type_tree(&element_ty);
        self.facts.index_plans.insert(
            expression.span,
            TypecheckIndexPlan {
                expression_span: expression.span,
                object_span: expression.object.span(),
                index_span: expression.index.span(),
                target_ty,
                index_ty,
                element_ty,
                access: match selected.access {
                    crate::typecheck::indexing::IndexAccess::Readonly => {
                        TypecheckIndexAccess::Readonly
                    }
                    crate::typecheck::indexing::IndexAccess::Readwrite => {
                        TypecheckIndexAccess::Readwrite
                    }
                },
                projection: match selected.projection {
                    crate::typecheck::indexing::IndexProjection::Array => {
                        TypecheckIndexProjection::Array
                    }
                    crate::typecheck::indexing::IndexProjection::Slice => {
                        TypecheckIndexProjection::Slice
                    }
                    crate::typecheck::indexing::IndexProjection::Str => {
                        TypecheckIndexProjection::Str
                    }
                    crate::typecheck::indexing::IndexProjection::Requirement => {
                        TypecheckIndexProjection::Requirement
                    }
                    crate::typecheck::indexing::IndexProjection::Declared => {
                        TypecheckIndexProjection::Declared
                    }
                },
                requirement_span: selected.requirement_span,
                method: selected.method,
                conversion,
            },
        );
    }

    pub(in crate::typecheck::facts::collector) fn record_conversion_plan(
        &mut self,
        expression_span: ByteSpan,
        source_span: ByteSpan,
        operator_span: Option<ByteSpan>,
        selected: crate::typecheck::conversions::SelectedConversion,
    ) {
        if let Some(plan) = super::super::typecheck_conversion_plan(
            expression_span,
            source_span,
            operator_span,
            selected,
        ) {
            self.intern_conversion_plan_types(&plan);
            self.facts.conversion_plans.insert(expression_span, plan);
        }
    }

    fn intern_conversion_plan_types(&mut self, conversion: &TypecheckConversionPlan) {
        self.intern_compiler_type_tree(&conversion.source_ty);
        self.intern_compiler_type_tree(&conversion.target_ty);
        let TypecheckConversionKind::BorrowCoercion(plan) = &conversion.kind else {
            return;
        };
        self.intern_compiler_type_tree(&plan.self_ty);
        self.intern_compiler_type_tree(&plan.target_ty);
        self.intern_compiler_type_tree(&crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
            span: conversion.expression_span,
            is_readwrite: plan.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow,
            inner: Box::new(plan.self_ty.clone()),
        }));
    }

    pub(in crate::typecheck::facts::collector) fn record_interpolation_plan(
        &mut self,
        expression: &crate::ast::InterpolatedStringExpr,
        environment: &TypeEnvironment,
    ) {
        let Some(runtime) = self.resolved.trusted_declarations.interpolation_runtime() else {
            return;
        };
        let mut string_parameters = std::collections::HashSet::new();
        let Some(string_type) = crate::typecheck::facts::type_to_type_expr_allowing_parameters(
            &crate::typecheck::strings::interpolated_string_type(self.resolved),
            expression.span,
            &mut string_parameters,
        ) else {
            return;
        };
        self.intern_compiler_type_tree(&string_type);
        self.intern_compiler_type_tree(&crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
            span: expression.span,
            is_readwrite: true,
            inner: Box::new(string_type),
        }));
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
            self.intern_compiler_type_tree(&accepted_type);
            self.intern_compiler_type_tree(&formatter.self_ty);
            if formatter.receiver_mode != crate::ast::MethodReceiverMode::Owned {
                self.intern_compiler_type_tree(&crate::ast::TypeExpr::Borrow(
                    crate::ast::BorrowType {
                        span,
                        is_readwrite: formatter.receiver_mode
                            == crate::ast::MethodReceiverMode::ReadwriteBorrow,
                        inner: Box::new(formatter.self_ty.clone()),
                    },
                ));
            }
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
                string_type_definition: runtime.string_type_definition,
                constructor: runtime.constructor.clone(),
                format_interface_definition: runtime.format_interface_definition,
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
        let target = self.generic_parameter_declaration(name).or_else(|| {
            self.resolved
                .symbols
                .symbol_by_name(name)
                .filter(|symbol| matches!(symbol.kind, SymbolKind::Type(_)))
                .map(|symbol| symbol.def_id)
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
                    .and_then(|definition| self.facts.generic_parameter(definition))
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
                                .and_then(|associated| {
                                    self.resolved
                                        .semantic_db
                                        .definition_at(associated.name_span)
                                })
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
            .and_then(|(_, associated)| {
                self.resolved
                    .semantic_db
                    .definition_at(associated.name_span)
            })
        };
        self.facts.type_occurrences.push(TypeOccurrenceFact {
            focus_span: projection.name_span,
            contextual_type: TypeExpr::Projection(projection.clone()),
            target,
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
            .and_then(|associated| {
                self.resolved
                    .semantic_db
                    .definition_at(associated.name_span)
            });
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
        let symbol = self.local_symbol(payload.span);
        self.facts.payload_binding_modes.insert(symbol, mode);
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
            let symbol = self.local_symbol(name_span);
            self.facts
                .binding_readonly
                .insert(symbol, !environment.is_mutable_binding(name));
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_binding(
        &mut self,
        name_span: ByteSpan,
        ty: &Type,
        is_mutable: bool,
    ) {
        self.record_binding_type(name_span, ty);
        let symbol = self.local_symbol(name_span);
        self.facts.binding_readonly.insert(symbol, !is_mutable);
    }

    pub(in crate::typecheck::facts::collector) fn record_binding_type(
        &mut self,
        name_span: ByteSpan,
        ty: &Type,
    ) {
        let symbol = self.local_symbol(name_span);
        if !ty.is_unknown_or_unresolved() {
            self.facts
                .binding_type_labels
                .insert(symbol, type_hover_label(ty, self.resolved));
        }
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(ty, name_span, &mut free_type_parameters)
        {
            self.record_payload_enum_drop_type_specializations(&ty);
            self.facts.binding_type_exprs.insert(symbol, ty);
        }
        self.record_drop_type_specialization(name_span, ty);
    }

    fn local_symbol(&self, occurrence_span: ByteSpan) -> LocalSymbolId {
        self.resolved
            .local_symbol_id_at_span(occurrence_span)
            .expect("resolver omitted local binding identity")
    }

    pub(in crate::typecheck::facts::collector) fn record_expression_type(
        &mut self,
        expression_span: ByteSpan,
        ty: &Type,
    ) {
        let scalar = checked_scalar_type(ty);
        let diverges = ty == &Type::Never;
        let mut free_type_parameters = HashSet::new();
        let ty =
            type_to_type_expr_allowing_parameters(ty, expression_span, &mut free_type_parameters);
        if let Some(ty) = &ty {
            self.record_payload_enum_drop_type_specializations(ty);
        }
        self.facts
            .record_expression_type(expression_span, ty, scalar, diverges);
    }

    pub(in crate::typecheck::facts::collector) fn record_contextual_expression_type(
        &mut self,
        expression_span: ByteSpan,
        ty: &Type,
    ) {
        let scalar = checked_scalar_type(ty);
        let mut free_type_parameters = HashSet::new();
        let Some(ty) =
            type_to_type_expr_allowing_parameters(ty, expression_span, &mut free_type_parameters)
        else {
            return;
        };
        self.facts
            .record_contextual_expression_type(expression_span, ty, scalar);
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
        let definition = self
            .resolved
            .semantic_db
            .definition_at(field.name_span)
            .expect("resolved field must have a semantic definition");
        self.facts.field_targets.insert(span, definition);
        let mut free_type_parameters = HashSet::new();
        if let Some(ty) =
            type_to_type_expr_allowing_parameters(field_ty, span, &mut free_type_parameters)
        {
            self.facts.field_type_exprs.insert(span, ty);
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_enum_variant_reference(
        &mut self,
        span: ByteSpan,
        variant: &crate::resolve::EnumVariantSignature,
    ) {
        let definition = self
            .resolved
            .semantic_db
            .definition_at(variant.name_span)
            .expect("resolved variant must have a semantic definition");
        self.facts.enum_variant_targets.insert(span, definition);
    }

    pub(in crate::typecheck::facts::collector) fn record_function_call_reference(
        &mut self,
        call: &CallExpr,
        declaration_span: ByteSpan,
    ) {
        let Some(name_span) = call_callee_name_span(call) else {
            return;
        };

        let definition = self
            .resolved
            .semantic_db
            .definition_at(declaration_span)
            .expect("resolved function must have a semantic definition");
        self.facts
            .function_call_targets
            .insert(name_span, definition);
    }
}
