//! Closed evaluator shape compatibility; source admission and physical layout remain upstream.
mod values;
use super::{Program, Rule, Type};
use std::collections::BTreeSet;
pub(super) use values::encode;

pub(super) fn permits(
    program: &Program,
    source: usize,
    target: usize,
    mut step: impl FnMut() -> Result<(), Rule>,
) -> Result<bool, Rule> {
    if matches!(
        program.types.get(source),
        None | Some(Type::Uninhabited | Type::Unavailable)
    ) || !complete(program, target, &mut step)?
    {
        return Ok(false);
    }
    if matches!(
        program.types.get(target),
        Some(Type::CallableAlternatives(_))
    ) {
        return Ok(callable_targets(program, source, target, &mut step)?.is_some());
    }
    let mut pending = vec![(source, target, false)];
    let mut seen = BTreeSet::new();
    while let Some((source, target, guarded)) = pending.pop() {
        step()?;
        if !seen.insert((source, target, guarded)) {
            continue;
        }
        let (Some(from), Some(to)) = (program.types.get(source), program.types.get(target)) else {
            return Ok(false);
        };
        match (from, to) {
            (Type::Uninhabited, _) if guarded => {}
            (Type::Unit, Type::Unit)
            | (Type::Bool, Type::Bool)
            | (Type::Text, Type::Text)
            | (Type::EmptyArray, Type::EmptyArray) => {}
            (Type::Integer(a), Type::Integer(b)) if a == b => {}
            (Type::NamedFunction(_) | Type::CapturedFunction { .. }, _)
                if from.same_function_payload(to) => {}
            (_, Type::CallableAlternatives(_)) => {
                if callable_targets(program, source, target, &mut step)?.is_none() {
                    return Ok(false);
                }
            }
            (Type::Record(left), Type::Record(right)) if left.len() == right.len() => {
                pending.extend(
                    left.iter()
                        .copied()
                        .zip(right.iter().copied())
                        .map(|(a, b)| (a, b, false)),
                );
            }
            (
                Type::Array {
                    element: a,
                    length: left,
                },
                Type::Array {
                    element: b,
                    length: right,
                },
            ) if left == right => pending.push((*a, *b, false)),
            (
                Type::Variant {
                    primary: a,
                    alternate: left,
                },
                Type::Variant {
                    primary: b,
                    alternate: right,
                },
            ) => {
                pending.push((*a, *b, true));
                match (left, right) {
                    (Some(a), Some(b)) => pending.push((*a, *b, true)),
                    (None, None) => {}
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn complete(
    program: &Program,
    root: usize,
    step: &mut impl FnMut() -> Result<(), Rule>,
) -> Result<bool, Rule> {
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();
    while let Some(data) = pending.pop() {
        step()?;
        if !seen.insert(data) {
            continue;
        }
        match program.types.get(data) {
            None | Some(Type::Unavailable) => return Ok(false),
            Some(Type::Record(fields)) => pending.extend(fields),
            Some(Type::CallableAlternatives(cases)) => pending.extend(cases),
            Some(
                Type::Array { element, .. }
                | Type::CapturedFunction {
                    environment: element,
                    ..
                },
            ) => pending.push(*element),
            Some(Type::Variant { primary, alternate }) => {
                pending.push(*primary);
                pending.extend(alternate);
            }
            _ => {}
        }
    }
    Ok(true)
}

/// Original case order maps to the receiving domain's ordinal. Signatures are not identities.
fn callable_targets(
    program: &Program,
    source: usize,
    target: usize,
    step: &mut impl FnMut() -> Result<(), Rule>,
) -> Result<Option<Box<[usize]>>, Rule> {
    let Some(Type::CallableAlternatives(targets)) = program.types.get(target) else {
        return Ok(None);
    };
    let sources = match program.types.get(source) {
        Some(Type::CallableAlternatives(cases)) => cases.as_ref(),
        Some(Type::NamedFunction(_) | Type::CapturedFunction { .. }) => {
            std::slice::from_ref(&source)
        }
        _ => return Ok(None),
    };
    let mut indices = std::collections::BTreeMap::new();
    for (index, &data) in targets.iter().enumerate() {
        step()?;
        let identity = program
            .types
            .get(data)
            .and_then(Type::function_identity)
            .ok_or(Rule::InvalidPlan)?;
        if indices.insert(identity, index).is_some() {
            return Err(Rule::InvalidPlan);
        }
    }
    let mut selected = Vec::new();
    for &data in sources {
        step()?;
        let identity = program
            .types
            .get(data)
            .and_then(Type::function_identity)
            .ok_or(Rule::InvalidPlan)?;
        let Some(&index) = indices.get(&identity) else {
            return Ok(None);
        };
        selected.push(index);
    }
    Ok(Some(selected.into()))
}

/// Encode one complete callable receipt after the common compatibility check.
fn encode_callable(
    program: &Program,
    source: usize,
    target: usize,
    value: &super::Value,
    storage: &super::budget::Storage,
    mut step: impl FnMut() -> Result<(), Rule>,
) -> Result<super::Value, Rule> {
    if source == target
        || !matches!(
            program.types.get(target),
            Some(Type::CallableAlternatives(_))
        )
    {
        return Ok(value.clone());
    }
    let targets = callable_targets(program, source, target, &mut step)?.ok_or(Rule::InvalidPlan)?;
    let (original, payload) = if matches!(
        program.types.get(source),
        Some(Type::CallableAlternatives(_))
    ) {
        let (case, payload) = value.data().callable().ok_or(Rule::InvalidPlan)?;
        (Some(case), payload)
    } else {
        (None, value)
    };
    let index = usize::try_from(original.unwrap_or(0)).map_err(|_| Rule::InvalidPlan)?;
    let target = u64::try_from(*targets.get(index).ok_or(Rule::InvalidPlan)?)
        .map_err(|_| Rule::InvalidPlan)?;
    if original == Some(target) {
        return Ok(value.clone());
    }
    let tag = storage.primitive(super::Data::Integer(i128::from(target)))?;
    storage.aggregate([tag, payload.clone()].into_iter())
}
