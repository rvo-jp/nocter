//! Convert only represented values, preserving shared DAGs and sparse repeated storage.
use super::{Program, Rule, Type};
use crate::state::{
    Data, Value,
    budget::{Resident, Storage},
};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Request {
    value: Value,
    source: usize,
    target: usize,
}
impl Request {
    fn key(&self) -> (usize, usize, usize) {
        (self.value.identity(), self.source, self.target)
    }
}

/// Compatibility has already been checked. No source permission or ownership is inferred here.
pub(in crate::state) fn encode(
    program: &Program,
    source: usize,
    target: usize,
    value: &Value,
    storage: &Storage,
    mut step: impl FnMut() -> Result<(), Rule>,
) -> Result<Value, Rule> {
    if source == target {
        return Ok(value.clone());
    }
    let root = Request {
        value: value.clone(),
        source,
        target,
    };
    let mut pending = vec![storage.entry((root.clone(), false))?];
    let mut completed = BTreeMap::<_, Resident<Value>>::new();
    while let Some(task) = pending.pop() {
        step()?;
        let (request, expanded) = &task.value;
        let key = request.key();
        if completed.contains_key(&key) {
            continue;
        }
        if request.source == request.target {
            completed.insert(key, storage.entry(request.value.clone())?);
            continue;
        }
        if matches!(
            program.types.get(request.target),
            Some(Type::CallableAlternatives(_))
        ) {
            let value = super::encode_callable(
                program,
                request.source,
                request.target,
                &request.value,
                storage,
                &mut step,
            )?;
            completed.insert(key, storage.entry(value)?);
            continue;
        }
        let children = children(program, request)?;
        if !expanded {
            pending.push(storage.entry((request.clone(), true))?);
            children.visit(|child| {
                pending.push(storage.entry((child, false))?);
                Ok(())
            })?;
            continue;
        }
        let mut changed = false;
        children.visit(|child| {
            let value = &completed.get(&child.key()).ok_or(Rule::InvalidPlan)?.value;
            changed |= value.identity() != child.value.identity();
            Ok(())
        })?;
        let value = if changed {
            rebuild(request, children, &completed, storage)?
        } else {
            request.value.clone()
        };
        completed.insert(key, storage.entry(value)?);
    }
    completed
        .get(&root.key())
        .map(|entry| entry.value.clone())
        .ok_or(Rule::InvalidPlan)
}

#[derive(Clone, Copy)]
enum Children<'a> {
    None,
    One {
        value: &'a Value,
        source: usize,
        target: usize,
    },
    Record {
        values: &'a [Value],
        source: &'a [usize],
        target: &'a [usize],
    },
    Elements {
        value: &'a Data,
        source: usize,
        target: usize,
    },
}
impl Children<'_> {
    fn visit(&self, mut visit: impl FnMut(Request) -> Result<(), Rule>) -> Result<(), Rule> {
        let request = |value: &Value, source, target| Request {
            value: value.clone(),
            source,
            target,
        };
        match self {
            Self::None => {}
            Self::One {
                value,
                source,
                target,
            } => visit(request(value, *source, *target))?,
            Self::Record {
                values,
                source,
                target,
            } => {
                for ((value, &source), &target) in values.iter().zip(*source).zip(*target) {
                    visit(request(value, source, target))?;
                }
            }
            Self::Elements {
                value,
                source,
                target,
            } => {
                for value in value.children() {
                    visit(request(value, *source, *target))?;
                }
            }
        }
        Ok(())
    }
    fn pair(&self, index: usize) -> Result<(usize, usize), Rule> {
        match self {
            Self::Record { source, target, .. } => Ok((
                *source.get(index).ok_or(Rule::InvalidPlan)?,
                *target.get(index).ok_or(Rule::InvalidPlan)?,
            )),
            Self::One { source, target, .. } | Self::Elements { source, target, .. } => {
                Ok((*source, *target))
            }
            Self::None => Err(Rule::InvalidPlan),
        }
    }
}
fn children<'a>(program: &'a Program, request: &'a Request) -> Result<Children<'a>, Rule> {
    match (
        program.types.get(request.source),
        program.types.get(request.target),
        request.value.data(),
    ) {
        (Some(Type::Record(source)), Some(Type::Record(target)), Data::Aggregate(values))
            if source.len() == target.len() && values.len() == source.len() =>
        {
            Ok(Children::Record {
                values,
                source,
                target,
            })
        }
        (
            Some(Type::Array {
                element: source,
                length: a,
            }),
            Some(Type::Array {
                element: target,
                length: b,
            }),
            value,
        ) if a == b && matches!(value, Data::Aggregate(_) | Data::Repeated { .. }) => {
            Ok(Children::Elements {
                value,
                source: *source,
                target: *target,
            })
        }
        (
            Some(source @ Type::Variant { .. }),
            Some(target @ Type::Variant { .. }),
            Data::Variant { primary, payload },
        ) => {
            let Some(value) = payload else {
                return Ok(Children::None);
            };
            let branch = usize::from(!primary);
            Ok(Children::One {
                value,
                source: source.component(branch).ok_or(Rule::InvalidPlan)?,
                target: target.component(branch).ok_or(Rule::InvalidPlan)?,
            })
        }
        (Some(_), Some(_), _) => Ok(Children::None),
        _ => Err(Rule::InvalidPlan),
    }
}
fn rebuild(
    request: &Request,
    children: Children<'_>,
    completed: &BTreeMap<(usize, usize, usize), Resident<Value>>,
    storage: &Storage,
) -> Result<Value, Rule> {
    let converted = |value: &Value, index| {
        let (source, target) = children.pair(index)?;
        completed
            .get(&(value.identity(), source, target))
            .map(|entry| entry.value.clone())
            .ok_or(Rule::InvalidPlan)
    };
    match request.value.data() {
        Data::Aggregate(values) => {
            // Resolve before allocation; every entry is already retained in the metered memo.
            for (index, value) in values.iter().enumerate() {
                converted(value, index)?;
            }
            storage.aggregate(
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| converted(value, index).expect("resolved child")),
            )
        }
        Data::Variant {
            primary,
            payload: Some(payload),
        } => storage.variant(*primary, Some(converted(payload, 0)?)),
        Data::Repeated {
            element,
            length,
            replacements,
        } => {
            let element = converted(element, 0)?;
            for value in replacements.values() {
                converted(value, 0)?;
            }
            storage.repeated(
                element,
                *length,
                replacements.iter().map(|(&index, value)| {
                    (index, converted(value, 0).expect("resolved replacement"))
                }),
            )
        }
        _ => Err(Rule::InvalidPlan),
    }
}
