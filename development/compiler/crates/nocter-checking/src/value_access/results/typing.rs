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
        let Some(equalities) =
            deferred_equalities(self.program(), left, right, self.expressions.next_id())
        else {
            return false;
        };
        self.equalities.extend(equalities);
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
        if expected.mode() == Mode::Own
            && !matches!(
                self.program().data_schema(expected.data()),
                Some(PayloadSchema::Function(_))
            )
            && self.defer_equal(Ty::new(actual.data(), expected.mode()), expected)
        {
            return Ok(Some(source));
        }
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

/// Prepare the existing witnessed-result obligations without mutating a typing frame.
pub(in crate::value_access) fn deferred_equalities(
    program: &nocter_declarations::value_access::Program,
    left: Ty,
    right: Ty,
    origin: AccessExpressionId,
) -> Option<Vec<Equality>> {
    if left.mode() != right.mode() {
        return None;
    }
    let mut pending = vec![(left.data(), right.data())];
    let mut seen = std::collections::BTreeSet::new();
    let mut equalities = Vec::new();
    let mut descriptor_only = false;
    while let Some((left, right)) = pending.pop() {
        if left == right || !seen.insert((left, right)) {
            continue;
        }
        match (program.data_schema(left), program.data_schema(right)) {
            (Some(PayloadSchema::Function(a)), Some(PayloadSchema::Function(b)))
                if (is_result(program, left) || is_result(program, right))
                    && a.invocation == b.invocation
                    && program.function_contract(a) == program.function_contract(b) =>
            {
                equalities.push(Equality {
                    left,
                    right,
                    origin,
                });
            }
            (Some(PayloadSchema::Array(a)), Some(PayloadSchema::Array(b)))
                if a.length() == b.length() =>
            {
                if a.is_empty() {
                    if !program.matches_uninhabited_metadata(left, right) {
                        return None;
                    }
                    descriptor_only = true;
                } else {
                    pending.push((a.element(), b.element()));
                }
            }
            (Some(PayloadSchema::Aggregate(a)), Some(PayloadSchema::Aggregate(b))) => {
                use nocter_declarations::value_access::AggregateForm;
                let (AggregateForm::Tuple(a), AggregateForm::Tuple(b)) = (a.form(), b.form())
                else {
                    return None;
                };
                if a.len() != b.len() {
                    return None;
                }
                pending.extend(a.iter().copied().zip(b.iter().copied()));
            }
            _ => return None,
        }
    }
    if equalities.is_empty() && !descriptor_only {
        return None;
    }
    Some(equalities)
}
