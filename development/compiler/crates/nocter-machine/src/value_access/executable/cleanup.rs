//! Cleanup dependencies follow stored schemas, independently of source-body discovery.
use super::Failure;
use nocter_runtime_contract::{
    RuntimeDataId, RuntimeEnvironment, RuntimePrimitive, RuntimeType, RuntimeTypeRepresentation,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn required(
    runtime: &RuntimeEnvironment,
    data: RuntimeDataId,
    cache: &mut BTreeMap<RuntimeDataId, bool>,
) -> Result<bool, Failure> {
    let mut pending = vec![(data, false)];
    let mut active = BTreeSet::new();
    while let Some((data, expanded)) = pending.pop() {
        if cache.contains_key(&data) {
            continue;
        }
        let children = match runtime.types().get(data) {
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
                            .ok_or(Failure::InvalidRoot)?,
                    );
                }
                children
            }
            Some(RuntimeType::Aggregate) => {
                let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
                    runtime.type_representations().get(data)
                else {
                    return Err(Failure::InvalidRoot);
                };
                fields.iter().map(|field| field.ty()).collect()
            }
            Some(RuntimeType::FixedArray { element, length }) => {
                if *length == 0 {
                    Vec::new()
                } else {
                    vec![*element]
                }
            }
            _ => return Err(Failure::InvalidRoot),
        };
        if expanded {
            let needed = children.iter().any(|child| cache.get(child) == Some(&true));
            cache.insert(data, needed);
            active.remove(&data);
        } else {
            if !active.insert(data) {
                return Err(Failure::InvalidRoot);
            }
            pending.push((data, true));
            pending.extend(children.into_iter().rev().map(|child| (child, false)));
        }
    }
    cache.get(&data).copied().ok_or(Failure::InvalidRoot)
}

#[cfg(test)]
mod tests;
