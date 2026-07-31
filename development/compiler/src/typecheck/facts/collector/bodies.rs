use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_item_body_facts(&mut self, item: &Item) {
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

    pub(in crate::typecheck::facts::collector) fn collect_impl_member_body_facts(
        &mut self,
        impl_: &ImplDecl,
    ) {
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
