//! Match receiving result families to the exact declaration-owned schema, without value evidence.
use super::{AggregateForm, CodeBindingError, INVALID, PayloadSchema, Program};
use nocter_model::{TypeId, ValueFunctionOrigin as Origin};
use std::collections::BTreeSet;

pub(super) fn constraints(
    program: &Program,
    declared: TypeId,
    target: TypeId,
) -> Result<BTreeSet<TypeId>, CodeBindingError> {
    let mut pending = vec![(declared, target)];
    let mut seen = BTreeSet::new();
    let mut results = BTreeSet::new();
    while let Some((declared, target)) = pending.pop() {
        if !seen.insert((declared, target)) {
            continue;
        }
        match (program.data_schema(declared), program.data_schema(target)) {
            (Some(PayloadSchema::Function(expected)), Some(PayloadSchema::Function(wanted))) => {
                if !expected.invocation.can_supply(wanted.invocation) {
                    return Err(INVALID);
                }
                let mut contract = program.function_contract(expected).ok_or(INVALID)?;
                contract.invocation = wanted.invocation;
                let template = program.contract_template(wanted).ok_or(INVALID)?;
                if program.function_contract(template).as_ref() != Some(&contract) {
                    return Err(INVALID);
                }
                if matches!(wanted.origin, Origin::Result(_) | Origin::Application(_)) {
                    let Origin::Result(result) = expected.origin else {
                        return Err(INVALID);
                    };
                    if template.origin != Origin::Result(result) {
                        return Err(INVALID);
                    }
                    results.insert(target);
                }
            }
            (Some(PayloadSchema::Builtin(a)), Some(PayloadSchema::Builtin(b))) if a == b => {}
            (Some(PayloadSchema::Optional(a)), Some(PayloadSchema::Optional(b)))
            | (Some(PayloadSchema::Fallible(a)), Some(PayloadSchema::Fallible(b))) => {
                pending.push((a, b));
            }
            (Some(PayloadSchema::Array(a)), Some(PayloadSchema::Array(b)))
                if a.length() == b.length() =>
            {
                if a.is_empty() {
                    if !program.matches_uninhabited_metadata(declared, target) {
                        return Err(INVALID);
                    }
                } else {
                    pending.push((a.element(), b.element()));
                }
            }
            (Some(PayloadSchema::Aggregate(a)), Some(PayloadSchema::Aggregate(b)))
                if a.len() == b.len() =>
            {
                match (a.form(), b.form()) {
                    (AggregateForm::Tuple(_), AggregateForm::Tuple(_)) => {}
                    (AggregateForm::Record(a), AggregateForm::Record(b))
                        if a.data() == b.data() => {}
                    _ => return Err(INVALID),
                }
                pending.extend(
                    a.components()
                        .zip(b.components())
                        .map(|(a, b)| (a.data, b.data)),
                );
            }
            _ => return Err(INVALID),
        }
    }
    Ok(results)
}
