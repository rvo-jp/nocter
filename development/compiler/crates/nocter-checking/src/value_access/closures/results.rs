//! Close an anonymous result from its own checked body; later uses cannot select its contract.
use crate::value_access::{Failure, Rule, Site, checker::Checker, ir::Statement};
use nocter_declarations::value_access::PayloadSchema;
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{AccessBlockId, AccessExpressionId, TypeId};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    pub(super) fn close_closure_result(
        &mut self,
        node: NodeId,
        root: AccessBlockId,
    ) -> Result<TypeId, Failure> {
        if let Some(result) = self.result {
            return Ok(result);
        }
        let returns = self.statements.iter().filter_map(|(_, statement)| {
            let Statement::Return(value) = statement else {
                return None;
            };
            self.ty(*value)
                .access()
                .map(|access| (*value, access.data()))
        });
        let block = self.blocks.get(root).ok_or(Failure::InconsistentSyntax)?;
        let completion = block
            .ty
            .normal()
            .map(|access| -> Result<_, Failure> {
                Ok((
                    block.result.ok_or(Failure::InconsistentSyntax)?,
                    access.data(),
                ))
            })
            .transpose()?;
        let mut candidates = returns.chain(completion);
        let Some((first, result)) = candidates.next() else {
            return self.reject_message(
                Rule::UnknownType,
                node,
                "this closure supplies no result data to infer; write a result annotation".into(),
                Vec::new(),
            );
        };
        for (value, actual) in candidates {
            if actual != result {
                let origin = self.result_candidate_origin(value)?;
                let related = DiagnosticNote::new(
                    format!("another result supplies {}", self.data_label(result)),
                    crate::value_access::source_origin(
                        self.syntax(),
                        self.result_candidate_origin(first)?,
                    )?,
                );
                return self.reject_message(Rule::TypeMismatch, origin,
                    format!("closure result inference needs one data type, but returns include {} and {}; write a result annotation to supply context", self.data_label(result), self.data_label(actual)), vec![related]);
            }
        }
        if matches!(
            self.program().data_schema(result),
            Some(PayloadSchema::Function(_))
        ) {
            return self.reject_message(Rule::UnknownType, node,
                "this anonymous result needs callable witness publication beyond the current closure result model".into(), Vec::new());
        }
        self.result = Some(result);
        Ok(result)
    }
    fn result_candidate_origin(
        &self,
        value: AccessExpressionId,
    ) -> Result<nocter_syntax::SyntaxOrigin, Failure> {
        let Some(&Site::Body(locator)) = self.origins.expressions.get(&value) else {
            return Err(Failure::InconsistentSyntax);
        };
        self.projection
            .resolve(locator)
            .ok_or(Failure::InconsistentSyntax)
    }
}
