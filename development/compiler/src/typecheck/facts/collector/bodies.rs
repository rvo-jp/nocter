use super::*;

impl TypedHirBuilder<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_item_body_facts(&mut self, item: &Item) {
        let generics = match item {
            Item::Function(item) => Some(&item.generics),
            Item::Primitive(item) => Some(&item.generics),
            Item::TypeAlias(item) => Some(&item.generics),
            Item::Struct(item) => Some(&item.generics),
            Item::Enum(item) => Some(&item.generics),
            Item::Interface(item) => Some(&item.generics),
            Item::Instance(item) => Some(&item.generics),
            Item::Destruct(item) => Some(&item.generics),
            Item::Conformance(item) => Some(&item.generics),
            Item::Import(_) | Item::FromImport(_) | Item::Construct(_) | Item::Test(_) => None,
        };
        if let Some(generics) = generics {
            self.with_generic_scope(generics, |collector| {
                collector.collect_item_body_facts_in_scope(item)
            });
        } else {
            self.collect_item_body_facts_in_scope(item);
        }
    }

    fn collect_item_body_facts_in_scope(&mut self, item: &Item) {
        match item {
            Item::Function(function) => {
                let Some(body) = &function.body else {
                    return;
                };
                let mut environment = environment_for_function(function, self.resolved);
                self.record_parameter_bindings(&function.parameters.parameters, &environment);
                let return_type = type_expr_to_type_in_environment(
                    &function.return_type,
                    self.resolved,
                    &environment,
                );
                let return_success_type = return_type.success_type().clone();
                self.collect_block_facts(body, &mut environment, Some(&return_success_type));
            }
            Item::Test(test) => {
                let mut environment = TypeEnvironment::default();
                let return_type = Type::Fallible {
                    success: Box::new(Type::Void),
                    error: Box::new(Type::Error),
                };
                self.collect_block_facts(
                    &test.body,
                    &mut environment,
                    Some(return_type.success_type()),
                );
            }
            Item::Instance(instance) => self.collect_instance_member_body_facts(instance),
            Item::Destruct(destruct) => {
                let mut environment = environment_for_destruct(destruct, self.resolved);
                self.record_parameter_bindings(
                    std::slice::from_ref(&destruct.binding),
                    &environment,
                );
                self.collect_block_facts(&destruct.body, &mut environment, Some(&Type::Void));
            }
            Item::Conformance(conformance) => {
                self.collect_conformance_member_body_facts(conformance)
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    self.with_generic_scope(&method.generics, |collector| {
                        let mut environment =
                            environment_for_interface_method(method, collector.resolved, interface);
                        let receiver = method.receiver.implicit_parameter();
                        collector.record_parameter_bindings(
                            std::slice::from_ref(&receiver),
                            &environment,
                        );
                        collector
                            .record_parameter_bindings(&method.parameters.parameters, &environment);
                        let return_type = type_expr_to_type_in_environment(
                            &method.return_type,
                            collector.resolved,
                            &environment,
                        );
                        collector.collect_block_facts(
                            body,
                            &mut environment,
                            Some(return_type.success_type()),
                        );
                    });
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let Some(body) = &function.body else {
                        continue;
                    };
                    self.with_generic_scope(&function.generics, |collector| {
                        let mut environment =
                            environment_for_function(function, collector.resolved);
                        collector.record_parameter_bindings(
                            &function.parameters.parameters,
                            &environment,
                        );
                        let return_type = type_expr_to_type_in_environment(
                            &function.return_type,
                            collector.resolved,
                            &environment,
                        );
                        collector.collect_block_facts(
                            body,
                            &mut environment,
                            Some(return_type.success_type()),
                        );
                    });
                }
                for (_, literal) in construct.literals() {
                    let Some(body) = &literal.body else {
                        continue;
                    };
                    let mut environment = environment_for_literal(literal, self.resolved);
                    let return_type = type_expr_to_type_in_environment(
                        &literal.return_type,
                        self.resolved,
                        &environment,
                    );
                    self.collect_block_facts(
                        body,
                        &mut environment,
                        Some(return_type.success_type()),
                    );
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }

    fn collect_method_body_facts(
        &mut self,
        owner: &(impl MethodOwnerDecl + ?Sized),
        method: &crate::ast::CallableDecl,
    ) {
        let Some(body) = &method.body else { return };
        self.with_generic_scope(&method.generics, |collector| {
            let mut environment = environment_for_method(method, collector.resolved, owner);
            let receiver = method.receiver.implicit_parameter();
            collector.record_parameter_bindings(std::slice::from_ref(&receiver), &environment);
            collector.record_parameter_bindings(&method.parameters.parameters, &environment);
            let return_type = type_expr_to_type_in_environment(
                &method.return_type,
                collector.resolved,
                &environment,
            );
            let return_success_type = return_type.success_type().clone();
            collector.collect_block_facts(body, &mut environment, Some(&return_success_type));
        });
    }

    pub(in crate::typecheck::facts::collector) fn collect_instance_member_body_facts(
        &mut self,
        instance: &InstanceDecl,
    ) {
        for method in instance.callables() {
            self.collect_method_body_facts(instance, method);
        }
    }

    fn collect_conformance_member_body_facts(&mut self, conformance: &ConformanceDecl) {
        for member in &conformance.members {
            if let ConformanceMember::Method(method) = member {
                self.collect_method_body_facts(conformance, method);
            }
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_block_facts(
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
}
