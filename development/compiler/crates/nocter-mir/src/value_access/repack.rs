//! Structural complement of one tuple destination; ownership and address freedom are separate.
use super::{Place, Preservation, Preserved, Projection};
use nocter_model::ValueField;
use nocter_runtime_contract::{
    RuntimeDataId, RuntimeEnvironment, RuntimeTypeRepresentation, RuntimeValueFieldRepresentation,
};

pub(super) fn preserved(
    runtime: &RuntimeEnvironment,
    before: RuntimeDataId,
    destination: &Place,
    missing: &[Box<[Projection]>],
) -> Option<Box<[Preserved]>> {
    if destination.projections.is_empty() {
        return None;
    }
    for (index, path) in missing.iter().enumerate() {
        if path.is_empty()
            || path.starts_with(&destination.projections)
            || destination.projections.starts_with(path)
            || index > 0 && (missing[index - 1] >= *path || path.starts_with(&missing[index - 1]))
        {
            return None;
        }
        hole_data(runtime, before, path)?;
    }
    let mut left = before;
    let mut right = destination.root;
    let mut prefix = Vec::new();
    let mut retained = Vec::new();
    for projection in &destination.projections {
        if *projection == Projection::Field(ValueField::Array(0)) {
            left = singleton_element(runtime, left)?;
            right = singleton_element(runtime, right)?;
            prefix.push(*projection);
            continue;
        }
        let Projection::Field(ValueField::Tuple(selected)) = projection else {
            return None;
        };
        let old = tuple_fields(runtime, left)?;
        let new = tuple_fields(runtime, right)?;
        if old.len() != new.len() || *selected >= old.len() {
            return None;
        }
        for (index, (old, new)) in old.iter().zip(new).enumerate() {
            if index == *selected {
                continue;
            }
            if old.ty() != new.ty() {
                return None;
            }
            let mut path = prefix.clone();
            path.push(Projection::Field(ValueField::Tuple(index)));
            let source = Place {
                slot: destination.slot,
                root: before,
                projections: path.into(),
                data: old.ty(),
            };
            let target = Place {
                root: destination.root,
                ..source.clone()
            };
            retained.push((source, target));
        }
        left = old[*selected].ty();
        right = new[*selected].ty();
        prefix.push(*projection);
    }
    if right != destination.data {
        return None;
    }
    initialized(runtime, retained, missing)
}

/// Split only ancestors of actual holes; shared schema branches without holes stay opaque.
fn initialized(
    runtime: &RuntimeEnvironment,
    mut pending: Vec<(Place, Place)>,
    missing: &[Box<[Projection]>],
) -> Option<Box<[Preserved]>> {
    let mut retained = Vec::new();
    while let Some((source, target)) = pending.pop() {
        let index = missing.partition_point(|hole| hole.as_ref() < source.projections.as_ref());
        match missing.get(index) {
            Some(hole) if hole.as_ref() == source.projections.as_ref() => continue,
            Some(hole) if hole.starts_with(&source.projections) => {}
            _ => {
                retained.push(Preserved {
                    source,
                    destination: target,
                    kind: Preservation::Complete,
                });
                continue;
            }
        }
        if runtime
            .types()
            .get(source.data)?
            .array()
            .is_some_and(|(_, length)| length > 1)
        {
            retained.push(Preserved {
                source,
                destination: target,
                kind: Preservation::ArrayRemainder,
            });
            continue;
        }
        let tuple = tuple_fields(runtime, source.data);
        let singleton = singleton_element(runtime, source.data)
            .map(|element| RuntimeValueFieldRepresentation::new(ValueField::Array(0), element));
        if tuple.is_none() && singleton.is_none() {
            return None;
        }
        for field in tuple.into_iter().flatten().copied().chain(singleton) {
            let mut path = source.projections.to_vec();
            path.push(Projection::Field(field.field()));
            pending.push((
                Place {
                    projections: path.clone().into(),
                    data: field.ty(),
                    ..source.clone()
                },
                Place {
                    projections: path.into(),
                    data: field.ty(),
                    ..target.clone()
                },
            ));
        }
    }
    Some(retained.into())
}

fn tuple_fields(
    runtime: &RuntimeEnvironment,
    data: RuntimeDataId,
) -> Option<&[RuntimeValueFieldRepresentation]> {
    let RuntimeTypeRepresentation::ValueAggregate { fields } =
        runtime.type_representations().get(data)?
    else {
        return None;
    };
    fields
        .iter()
        .enumerate()
        .all(|(index, field)| field.field() == ValueField::Tuple(index))
        .then_some(fields)
}

fn singleton_element(runtime: &RuntimeEnvironment, data: RuntimeDataId) -> Option<RuntimeDataId> {
    let (element, length) = runtime.types().get(data)?.array()?;
    (length == 1).then_some(element)
}

/// Structural identity only; the body validator separately checks saved-number slots and bounds.
pub(super) fn hole_data(
    runtime: &RuntimeEnvironment,
    mut data: RuntimeDataId,
    path: &[Projection],
) -> Option<RuntimeDataId> {
    for projection in path {
        data = match projection {
            Projection::Field(field @ (ValueField::Tuple(_) | ValueField::Array(_))) => {
                runtime.value_field(data, *field)?
            }
            Projection::Index(_) => {
                let (element, length) = runtime.types().get(data)?.array()?;
                if length <= 1 {
                    return None;
                }
                element
            }
            _ => return None,
        };
    }
    Some(data)
}
