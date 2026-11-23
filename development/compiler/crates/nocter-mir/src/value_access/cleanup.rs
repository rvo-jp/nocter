//! Close executable bodies through ordinary calls and the selected resource schedules.
use super::{Body, Destructor, Disposal, Effect, Failure};
use nocter_model::AccessInstanceId;
use nocter_runtime_contract::{
    RuntimeDataId, RuntimeEnvironment, RuntimePrimitive, RuntimeType, RuntimeTypeRepresentation,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Closed {
    pub(super) bodies: BTreeSet<AccessInstanceId>,
    pub(super) cleanup: BTreeSet<RuntimeDataId>,
}

pub(super) fn close(
    root: AccessInstanceId,
    bodies: &BTreeMap<AccessInstanceId, Body>,
    runtime: &RuntimeEnvironment,
    destructors: &BTreeMap<RuntimeDataId, Option<Destructor>>,
) -> Result<Closed, Failure> {
    let mut pending = BTreeSet::from([root]);
    let mut reached = BTreeSet::new();
    let mut cleanup = BTreeMap::new();
    while let Some(id) = pending.pop_first() {
        if !reached.insert(id) {
            continue;
        }
        let body = bodies.get(&id).ok_or(Failure::InvalidBody(id))?;
        for action in body.actions().values() {
            for operation in &action.operations {
                operation.visit_called_bodies(|target| {
                    pending.insert(target);
                });
            }
            for effect in action.effects() {
                let mut require = |data| {
                    required(runtime, destructors, data, &mut cleanup, |target| {
                        pending.insert(target);
                    })
                    .map_err(|InvalidSchema| Failure::InvalidBody(id))
                };
                match effect {
                    Effect::Dispose { source, disposal } if *disposal != Disposal::Trivial => {
                        require(source.data)?;
                    }
                    Effect::SelectedFutureCleanup { cases, .. } => {
                        for input in cases.iter().flat_map(|case| &case.fields) {
                            require(input.source.data)?;
                        }
                    }
                    Effect::DisposeArrayRemainder { source, .. } => {
                        require(source.data)?;
                    }
                    Effect::DisposeArrayElements { source, .. } => {
                        let (element, _) = runtime
                            .types()
                            .get(source.data)
                            .and_then(RuntimeType::array)
                            .ok_or(Failure::InvalidBody(id))?;
                        require(element)?;
                    }
                    Effect::Dispose { .. }
                    | Effect::VariantTag { .. }
                    | Effect::ConstructEnum { .. }
                    | Effect::Initialize(_)
                    | Effect::Transfer { .. }
                    | Effect::SetFutureSelector { .. }
                    | Effect::SelectedFutureMove { .. }
                    | Effect::Repack { .. }
                    | Effect::Transport { .. }
                    | Effect::Reinterpret { .. }
                    | Effect::ConsolidateArrayPrefix { .. }
                    | Effect::End(_)
                    | Effect::BindAddress { .. } => {}
                }
            }
        }
    }
    Ok(Closed {
        bodies: reached,
        cleanup: cleanup
            .into_iter()
            .filter_map(|(data, needed)| needed.then_some(data))
            .collect(),
    })
}

#[derive(Debug)]
struct InvalidSchema;

pub(super) fn compatible_receivers(
    runtime: &RuntimeEnvironment,
    entry: RuntimeDataId,
    completed: RuntimeDataId,
) -> bool {
    let shapes = runtime.type_representations();
    matches!(
        (shapes.get(entry), shapes.get(completed)),
        (
            Some(RuntimeTypeRepresentation::ValueAggregate { .. }),
            Some(RuntimeTypeRepresentation::ValueAggregate { .. }),
        ) | (
            Some(RuntimeTypeRepresentation::ValueEnum { .. }),
            Some(RuntimeTypeRepresentation::ValueEnum { .. }),
        )
    )
}

fn required(
    runtime: &RuntimeEnvironment,
    destructors: &BTreeMap<RuntimeDataId, Option<Destructor>>,
    data: RuntimeDataId,
    cache: &mut BTreeMap<RuntimeDataId, bool>,
    mut on_destructor: impl FnMut(AccessInstanceId),
) -> Result<bool, InvalidSchema> {
    let mut pending = vec![(data, false)];
    let mut active = BTreeSet::new();
    while let Some((data, expanded)) = pending.pop() {
        if cache.contains_key(&data) {
            continue;
        }
        let authored = destructors
            .get(&data)
            .map(|plan| plan.ok_or(InvalidSchema))
            .transpose()?;
        let completed = authored.map_or(data, |plan| plan.completed);
        if authored.is_some() && !compatible_receivers(runtime, data, completed) {
            return Err(InvalidSchema);
        }
        let children = match runtime.types().get(completed) {
            Some(RuntimeType::Primitive(RuntimePrimitive::Error)) => {
                cache.insert(data, true);
                continue;
            }
            Some(
                RuntimeType::Primitive(_)
                | RuntimeType::Unit
                | RuntimeType::EmptyArray { .. }
                | RuntimeType::StaticCallable { environment: None },
            ) => {
                cache.insert(data, false);
                continue;
            }
            Some(RuntimeType::StaticCallable {
                environment: Some(environment),
            }) => vec![*environment],
            Some(RuntimeType::CallableAlternatives(cases)) => cases.to_vec(),
            Some(kind @ (RuntimeType::Optional(payload) | RuntimeType::Fallible(payload))) => {
                // Exclusion has no stored value; an inhabited primary retains its own helper.
                let mut children = Vec::new();
                if !matches!(
                    runtime.types().get(*payload),
                    Some(RuntimeType::Uninhabited)
                ) {
                    children.push(*payload);
                }
                if matches!(kind, RuntimeType::Fallible(_)) {
                    children.push(
                        runtime
                            .types()
                            .primitive(RuntimePrimitive::Error)
                            .ok_or(InvalidSchema)?,
                    );
                }
                children
            }
            Some(RuntimeType::Aggregate) => match runtime.type_representations().get(completed) {
                Some(RuntimeTypeRepresentation::ScalarEnum { .. }) => Vec::new(),
                Some(RuntimeTypeRepresentation::ValueAggregate { fields }) => {
                    fields.iter().map(|field| field.ty()).collect()
                }
                Some(RuntimeTypeRepresentation::ValueEnum { variants }) => variants
                    .iter()
                    .flat_map(|variant| variant.fields().iter().map(|&(_, ty)| ty))
                    .collect(),
                _ => return Err(InvalidSchema),
            },
            Some(RuntimeType::FixedArray { element, length }) => {
                if *length == 0 {
                    Vec::new()
                } else {
                    vec![*element]
                }
            }
            _ => return Err(InvalidSchema),
        };
        if expanded {
            if let Some(plan) = authored {
                on_destructor(plan.body);
            }
            let needed =
                authored.is_some() || children.iter().any(|child| cache.get(child) == Some(&true));
            cache.insert(data, needed);
            active.remove(&data);
        } else {
            if !active.insert(data) {
                return Err(InvalidSchema);
            }
            pending.push((data, true));
            pending.extend(children.into_iter().rev().map(|child| (child, false)));
        }
    }
    cache.get(&data).copied().ok_or(InvalidSchema)
}

#[cfg(test)]
mod tests;
