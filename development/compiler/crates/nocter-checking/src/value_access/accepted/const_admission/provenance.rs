//! Follow the ordinary argument binding that supplied a failed conditional witness.
use crate::value_access::{
    ExpressionKind, accepted::Failure, constant, flow, results, source_origin,
};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, ValueFunctionOrigin, ValueParameter};
use nocter_source_index::SourceOrigin;
use std::collections::BTreeMap;
const INVALID: Failure = Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);

pub(super) struct Parent {
    pub(super) key: CodeInstanceKey,
    pub(super) call: AccessExpressionId,
    pub(super) origin: SourceOrigin,
}
impl Parent {
    fn argument(
        &self,
        catalog: &constant::Catalog,
        verified: &results::Verified<'_, '_>,
        callee: &CodeInstanceKey,
        parameter: ValueParameter,
    ) -> Result<(SourceOrigin, Option<ValueParameter>), Failure> {
        let query = catalog.get(self.key.code()).ok_or(INVALID)?.body();
        let callable = self.key.code().source_owner();
        let body_failure = |failure| Failure::Body { callable, failure };
        let bound = query.bind_verified(verified).map_err(body_failure)?;
        let ExpressionKind::Call {
            kind, arguments, ..
        } = &query
            .typed()
            .body()
            .expressions()
            .get(self.call)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        let ordinal = query
            .typed()
            .program()
            .callable_input_parameters(callee.code())
            .map_err(|_| INVALID)?
            .into_iter()
            .find_map(|(ordinal, formal)| (formal == parameter).then_some(ordinal))
            .ok_or(INVALID)?;
        let argument = arguments
            .get(ordinal + usize::from(*kind == crate::value_access::CallKind::Value))
            .ok_or(INVALID)?;
        let witness = match &argument.binding {
            Some(binding) if binding.parameter == parameter => binding.witness,
            Some(_) => return Err(INVALID),
            None => {
                let data = query
                    .typed()
                    .body()
                    .expressions()
                    .get(argument.value)
                    .and_then(|expression| expression.ty.access())
                    .ok_or(INVALID)?
                    .data();
                let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                    query.typed().program().data_schema(data)
                else {
                    return Err(INVALID);
                };
                witness
            }
        };
        let origin = bound
            .typed()
            .expression_origin(argument.value)
            .ok_or(INVALID)?;
        let origin = source_origin(bound.typed().syntax(), origin).map_err(body_failure)?;
        let dependency = match witness.origin {
            ValueFunctionOrigin::Parameter(parameter)
                if self.key.witnesses().contains_key(&parameter) =>
            {
                Some(parameter)
            }
            ValueFunctionOrigin::Declaration(_)
            | ValueFunctionOrigin::Closure(_)
            | ValueFunctionOrigin::Contract(_)
            | ValueFunctionOrigin::Result(_)
            | ValueFunctionOrigin::Application(_) => None,
            ValueFunctionOrigin::Parameter(_) => return Err(INVALID),
        };
        Ok((origin, dependency))
    }
}
pub(super) fn annotate(
    catalog: &constant::Catalog,
    verified: &results::Verified<'_, '_>,
    parents: &BTreeMap<CodeInstanceKey, Option<Parent>>,
    key: &CodeInstanceKey,
    violation: constant::Violation,
    diagnostic: &mut constant::Diagnostic,
) -> Result<(), Failure> {
    let query = catalog.get(key.code()).ok_or(INVALID)?.body();
    let mut dependency = violation
        .dependency(query, key)
        .map_err(Failure::Integrity)?;
    let mut path = Vec::new();
    let mut current = key;
    while let Some(Some(parent)) = parents.get(current) {
        path.push(parent.origin);
        if let Some(parameter) = dependency {
            let (origin, outer) = parent.argument(catalog, verified, current, parameter)?;
            diagnostic.note_witness(origin);
            dependency = outer;
        }
        current = &parent.key;
    }
    for origin in path.into_iter().rev() {
        diagnostic.note_call(origin);
    }
    Ok(())
}
