//! Structural complement of one aggregate destination; ownership and address freedom are separate.
use super::{Place, Preservation, Preserved, Projection, RemainderExclusion};
use nocter_model::ValueField;
use nocter_runtime_contract::{
    RuntimeDataId, RuntimeEnvironment, RuntimeTypeRepresentation, RuntimeValueFieldRepresentation,
};

pub(super) fn preserved(
    runtime: &RuntimeEnvironment,
    before: RuntimeDataId,
    destination: &Place,
    missing: &[RemainderExclusion],
) -> Option<Box<[Preserved]>> {
    if destination.projections.is_empty() {
        return None;
    }
    let whole_holes = missing
        .iter()
        .filter_map(|hole| match hole {
            RemainderExclusion::Value(place) => Some(place.projections.as_ref()),
            RemainderExclusion::ArrayPrefix { .. } => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for (index, exclusion) in missing.iter().enumerate() {
        let hole = exclusion.place();
        let path = &hole.projections;
        if path.is_empty()
            || hole.slot != destination.slot
            || hole.root != before
            || path.starts_with(&destination.projections)
            || destination.projections.starts_with(path)
            || index > 0 && missing[index - 1] >= *exclusion
            || hole_data(runtime, before, path)? != hole.data
        {
            return None;
        }
        if exclusion.endpoint().is_some() {
            let (_, length) = runtime.types().get(hole.data)?.array()?;
            if length == 0 {
                return None;
            }
        }
        // A missing whole value cannot contain another exclusion. Prefixes may contain
        // later point holes, which remain independent original resource facts.
        let end = path.len() + usize::from(exclusion.endpoint().is_some());
        if (0..end).any(|length| whole_holes.contains(&path[..length])) {
            return None;
        }
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
        let Projection::Field(selected @ (ValueField::Tuple(_) | ValueField::Record(_))) =
            projection
        else {
            return None;
        };
        let old = aggregate_fields(runtime, left)?;
        let new = aggregate_fields(runtime, right)?;
        let selected = old.iter().position(|field| field.field() == *selected)?;
        if old.len() != new.len() {
            return None;
        }
        for (index, (old, new)) in old.iter().zip(new).enumerate() {
            if old.field() != new.field() {
                return None;
            }
            if index == selected {
                continue;
            }
            if old.ty() != new.ty() {
                return None;
            }
            let mut path = prefix.clone();
            path.push(Projection::Field(old.field()));
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
        left = old[selected].ty();
        right = new[selected].ty();
        prefix.push(*projection);
    }
    if right != destination.data {
        return None;
    }
    let mut paths = missing
        .iter()
        .map(|hole| (hole.place().projections.as_ref(), hole.endpoint().is_some()))
        .collect::<Vec<_>>();
    paths.sort_unstable();
    if paths.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    initialized(runtime, retained, &paths)
}

/// Split only ancestors of actual holes; shared schema branches without holes stay opaque.
fn initialized(
    runtime: &RuntimeEnvironment,
    mut pending: Vec<(Place, Place)>,
    missing: &[(&[Projection], bool)],
) -> Option<Box<[Preserved]>> {
    let mut retained = Vec::new();
    while let Some((source, target)) = pending.pop() {
        let index = missing.partition_point(|(path, _)| *path < source.projections.as_ref());
        let prefix = match missing.get(index) {
            Some((path, false)) if *path == source.projections.as_ref() => continue,
            Some((path, prefix)) if path.starts_with(&source.projections) => {
                *prefix && *path == source.projections.as_ref()
            }
            _ => {
                retained.push(Preserved {
                    source,
                    destination: target,
                    kind: Preservation::Complete,
                });
                continue;
            }
        };
        if runtime
            .types()
            .get(source.data)?
            .array()
            .is_some_and(|(_, length)| length > 1 || prefix)
        {
            retained.push(Preserved {
                source,
                destination: target,
                kind: Preservation::ArrayRemainder,
            });
            continue;
        }
        let aggregate = aggregate_fields(runtime, source.data);
        let singleton = singleton_element(runtime, source.data)
            .map(|element| RuntimeValueFieldRepresentation::new(ValueField::Array(0), element));
        if aggregate.is_none() && singleton.is_none() {
            return None;
        }
        for field in aggregate.into_iter().flatten().copied().chain(singleton) {
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

fn aggregate_fields(
    runtime: &RuntimeEnvironment,
    data: RuntimeDataId,
) -> Option<&[RuntimeValueFieldRepresentation]> {
    let RuntimeTypeRepresentation::ValueAggregate { fields } =
        runtime.type_representations().get(data)?
    else {
        return None;
    };
    Some(fields)
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
            Projection::Field(
                field @ (ValueField::Record(_) | ValueField::Tuple(_) | ValueField::Array(_)),
            ) => runtime.value_field(data, *field)?,
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
