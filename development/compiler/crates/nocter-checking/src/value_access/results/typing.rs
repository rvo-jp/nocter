//! Result signatures constrain environments; named witness identity closes separately.
use super::{Equality, is_result};
use crate::value_access::{Failure, checker::Checker, ir::Boundary};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessExpressionId, TypeId, ValueAccess as Ty, ValueAccessMode as Mode};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    pub(in crate::value_access) fn result_boundary(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        data: TypeId,
    ) -> Result<Option<AccessExpressionId>, Failure> {
        if !is_result(self.program(), data) {
            return Ok(None);
        }
        let Some(PayloadSchema::Function(witness)) = self.program().data_schema(data) else {
            return Err(Failure::InconsistentSyntax);
        };
        let constraint = self
            .program()
            .function_contract(witness)
            .ok_or(Failure::InconsistentSyntax)?;
        let supplied = self.function_constraint_data(node, source, &constraint, false)?;
        self.boundary(node, source, Boundary::Value(supplied))
            .map(Some)
    }
    pub(in crate::value_access) fn defer_equal(&mut self, left: Ty, right: Ty) -> bool {
        if left.mode() != right.mode()
            || !(is_result(self.program(), left.data()) || is_result(self.program(), right.data()))
        {
            return false;
        }
        let (Some(PayloadSchema::Function(a)), Some(PayloadSchema::Function(b))) = (
            self.program().data_schema(left.data()),
            self.program().data_schema(right.data()),
        ) else {
            return false;
        };
        if a.invocation != b.invocation
            || self.program().function_contract(a) != self.program().function_contract(b)
        {
            return false;
        }
        self.equalities.push(Equality {
            left: left.data(),
            right: right.data(),
            origin: self.expressions.next_id(),
        });
        self.pending_results = true;
        true
    }
    pub(in crate::value_access) fn result_assignment(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        expected: Ty,
    ) -> Result<Option<AccessExpressionId>, Failure> {
        let actual = self
            .ty(source)
            .access()
            .ok_or(Failure::InconsistentSyntax)?;
        if expected.mode() != Mode::Own
            || !(is_result(self.program(), expected.data())
                || is_result(self.program(), actual.data()))
        {
            return Ok(None);
        }
        let Some(PayloadSchema::Function(witness)) = self.program().data_schema(expected.data())
        else {
            return Ok(None);
        };
        let Some(PayloadSchema::Function(_)) = self.program().data_schema(actual.data()) else {
            return Ok(None);
        };
        let constraint = self
            .program()
            .function_contract(witness)
            .ok_or(Failure::InconsistentSyntax)?;
        let supplied = self.function_constraint_data(node, source, &constraint, false)?;
        let value = if supplied == actual.data() {
            source
        } else {
            self.boundary(node, source, Boundary::Value(supplied))?
        };
        if !self.defer_equal(Ty::new(supplied, expected.mode()), expected) {
            return Ok(None);
        }
        Ok(Some(value))
    }
}
