use super::*;

impl TypedHirBuilder<'_> {
    pub(in crate::typecheck::facts::collector) fn record_generic_function_call_specialization(
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
        let expression_id = self
            .facts
            .expression_id(call.span)
            .expect("generic call must have an expression identity");
        if report_unspecialized {
            let definition = self
                .resolved
                .semantic_db
                .definition_at(declaration_span)
                .expect("resolved generic function must have a semantic definition");
            self.facts
                .generic_function_call_targets
                .insert(expression_id, definition);
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
                .insert(expression_id, specialization);
        }
    }

    pub(in crate::typecheck::facts::collector) fn record_expected_generic_function_call_specialization(
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
