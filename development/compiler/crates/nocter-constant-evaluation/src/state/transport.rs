//! Closed evaluator shape compatibility; source admission and physical layout remain upstream.
use super::{Program, Rule, Type};
use std::collections::BTreeSet;

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
                if super::validation::same_function_payload(from, to) => {}
            (Type::CallableAlternatives(left), Type::CallableAlternatives(right))
                if left == right => {}
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
