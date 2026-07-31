use super::*;

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
