//! Lexical callable leaves belong to one explicit input, never its environment prefix.
use super::Program;
use crate::value_access::{FunctionInput, PayloadSchema};
use nocter_model::{TypeId, ValueClosure, ValueFunctionOrigin, ValueInput, ValueParameter};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn owner(parameter: ValueParameter) -> Option<ValueClosure> {
    match parameter {
        ValueParameter::Closure { owner, .. }
        | ValueParameter::Component {
            input: ValueInput::Closure { owner, .. },
            ..
        } => Some(owner),
        _ => None,
    }
}

pub(super) fn validate(program: &Program, closure: ValueClosure, inputs: &[FunctionInput]) -> bool {
    let mut found = BTreeSet::new();
    let mut shapes = BTreeMap::new();
    for (position, input) in inputs.iter().enumerate() {
        if summarize(program, input.data, &mut shapes).is_none() {
            return false;
        }
        let direct = matches!(
            program.data_schema(input.data),
            Some(PayloadSchema::Function(_))
        );
        let mut pending = vec![input.data];
        let mut ordinal = Some(0usize);
        while let Some(data) = pending.pop() {
            let shape = shapes[&data];
            if !direct && !shape.formal {
                ordinal = ordinal.and_then(|ordinal| ordinal.checked_add(shape.leaves?));
                continue;
            }
            match program.data_schema(data) {
                Some(PayloadSchema::Function(witness)) => {
                    let Some(index) = ordinal else {
                        return false;
                    };
                    let parameter = if direct {
                        ValueParameter::Closure {
                            owner: closure,
                            position,
                        }
                    } else {
                        ValueParameter::Component {
                            input: ValueInput::Closure {
                                owner: closure,
                                position,
                            },
                            ordinal: index,
                        }
                    };
                    if witness.origin != ValueFunctionOrigin::Parameter(parameter)
                        || program
                            .parameter_contract(parameter)
                            .is_none_or(|contract| contract.invocation != witness.invocation)
                        || !found.insert(parameter)
                    {
                        return false;
                    }
                    ordinal = index.checked_add(1);
                }
                Some(_) => pending.extend(children(program, data).unwrap().into_iter().rev()),
                None => return false,
            }
        }
    }
    let direct = |position| ValueParameter::Closure {
        owner: closure,
        position,
    };
    let component = |position, ordinal| ValueParameter::Component {
        input: ValueInput::Closure {
            owner: closure,
            position,
        },
        ordinal,
    };
    program
        .closure_parameters
        .range(direct(0)..=direct(usize::MAX))
        .chain(
            program
                .closure_parameters
                .range(component(0, 0)..=component(usize::MAX, usize::MAX)),
        )
        .all(|(parameter, _)| found.contains(parameter))
}

#[derive(Clone, Copy)]
struct Shape {
    // None denotes a logical leaf count larger than the identity domain. An exact-code DAG
    // still needs no formals; a later formal cannot claim an unrepresentable ordinal.
    leaves: Option<usize>,
    formal: bool,
}

fn children(program: &Program, data: TypeId) -> Option<Vec<TypeId>> {
    // Header records cannot contain lexical input formals. Identity alone suffices here;
    // a zero array must not require its unstored nominal element's payload header.
    if program.nominal_identity(data).is_some() {
        return Some(vec![]);
    }
    Some(match program.data_schema(data)? {
        PayloadSchema::Aggregate(schema) => schema.components().map(|field| field.data).collect(),
        PayloadSchema::Array(array) => vec![array.element()],
        PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload) => vec![payload],
        _ => vec![],
    })
}

/// Summarize each shared schema once, without expanding tuple paths or array cardinality.
fn summarize(program: &Program, root: TypeId, shapes: &mut BTreeMap<TypeId, Shape>) -> Option<()> {
    let mut pending = vec![(root, false)];
    while let Some((data, expanded)) = pending.pop() {
        if shapes.contains_key(&data) {
            continue;
        }
        let fields = children(program, data)?;
        if !expanded && !fields.is_empty() {
            pending.push((data, true));
            pending.extend(fields.into_iter().rev().map(|field| (field, false)));
            continue;
        }
        let mut shape = Shape {
            leaves: Some(0),
            formal: false,
        };
        if let Some(PayloadSchema::Function(witness)) = program.data_schema(data) {
            shape.leaves = Some(1);
            shape.formal = matches!(witness.origin, ValueFunctionOrigin::Parameter(_));
        }
        for field in fields {
            let child = shapes[&field];
            shape.formal |= child.formal;
            shape.leaves = shape
                .leaves
                .and_then(|count| count.checked_add(child.leaves?));
        }
        shapes.insert(data, shape);
    }
    Some(())
}
