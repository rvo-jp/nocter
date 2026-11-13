//! Close an anonymous result from its own checked body; later uses cannot select its contract.
use crate::value_access::{Failure, Rule, Site, checker::Checker, ir::Statement};
use crate::value_access::{annotations::Annotation, ir::Boundary};
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{AccessBlockId, AccessExpressionId, TypeId, ValueAccess, ValueAccessMode};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    pub(in crate::value_access) fn supplied_result(
        &mut self,
        node: NodeId,
        value: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        match &self.result {
            Some(Annotation::Data(_)) => Ok(value),
            Some(Annotation::Function(pattern)) => {
                let data = self.function_pattern_data(node, value, pattern, false)?;
                self.boundary(node, value, Boundary::Value(data))
            }
            None => self.ty(value).access().map_or(Ok(value), |access| {
                self.boundary(node, value, Boundary::Value(access.data()))
            }),
        }
    }
    pub(super) fn close_closure_result(
        &mut self,
        node: NodeId,
        root: AccessBlockId,
    ) -> Result<TypeId, Failure> {
        if let Some(Annotation::Data(result)) = &self.result {
            return Ok(*result);
        }
        self.supply_closure_completion(root)?;
        let returns = self.statements.iter().filter_map(|(_, statement)| {
            let Statement::Return(value) = statement else {
                return None;
            };
            let ty = self.ty(*value);
            ty.access().map(|_| (*value, ty))
        });
        let block = self.blocks.get(root).ok_or(Failure::InconsistentSyntax)?;
        let completion = block
            .ty
            .normal()
            .map(|_| -> Result<_, Failure> {
                Ok((block.result.ok_or(Failure::InconsistentSyntax)?, block.ty))
            })
            .transpose()?;
        let candidates: Vec<_> = returns.chain(completion).collect();
        let Some(&(first, first_type)) = candidates
            .iter()
            .find(|(_, ty)| ty.may_complete())
            .or_else(|| candidates.first())
        else {
            return self.reject_message(
                Rule::UnknownType,
                node,
                if matches!(self.result, Some(Annotation::Function(_))) {
                    "this callable result has no established witness; a signature alone cannot select returned code or environment"
                } else {
                    "this closure supplies no result data to infer; write a result annotation"
                }.into(),
                Vec::new(),
            );
        };
        let result = first_type
            .access()
            .ok_or(Failure::InconsistentSyntax)?
            .data();
        let mut equalities = Vec::new();
        for (value, actual) in candidates {
            let actual = actual.access().ok_or(Failure::InconsistentSyntax)?.data();
            if actual != result {
                if let Some(prepared) = crate::value_access::results::deferred_equalities(
                    self.program(),
                    ValueAccess::new(result, ValueAccessMode::Own),
                    ValueAccess::new(actual, ValueAccessMode::Own),
                    value,
                ) {
                    equalities.extend(prepared);
                    continue;
                }
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
        self.pending_results |= !equalities.is_empty();
        self.equalities.extend(equalities);
        self.result = Some(Annotation::Data(result));
        Ok(result)
    }
    fn supply_closure_completion(&mut self, root: AccessBlockId) -> Result<(), Failure> {
        if matches!(self.result, Some(Annotation::Function(_))) {
            let block = self.blocks.get(root).ok_or(Failure::InconsistentSyntax)?;
            if block.ty.normal().is_some() {
                let value = block.result.ok_or(Failure::InconsistentSyntax)?;
                let nocter_syntax::SyntaxOrigin::Node(origin) =
                    self.result_candidate_origin(value)?
                else {
                    return Err(Failure::InconsistentSyntax);
                };
                let value = self.supplied_result(origin, value)?;
                let ty = self.ty(value);
                let block = self
                    .blocks
                    .get_mut(root)
                    .ok_or(Failure::InconsistentSyntax)?;
                block.result = Some(value);
                block.ty = ty;
            }
        }
        Ok(())
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
