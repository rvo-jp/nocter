use super::*;

/// Replaces declaration-relative storage origins with call-site origins while
/// retaining the result's aggregate and fallible shape.
pub(in crate::typecheck::returns) fn instantiate_provenance_summary(
    summary: &ValueProvenance,
    map_origin: &mut impl FnMut(&StorageOrigin) -> Option<ValueProvenance>,
) -> Option<ValueProvenance> {
    match summary {
        ValueProvenance::Independent => Some(ValueProvenance::Independent),
        ValueProvenance::Origins(origins) => {
            let mut provenance = None;
            for origin in origins {
                let mapped = match origin {
                    StorageOrigin::Allocated(domain) => {
                        instantiate_origin(domain, map_origin).map(ValueProvenance::allocated)
                    }
                    _ => instantiate_origin(origin, map_origin),
                };
                merge_provenance(&mut provenance, mapped);
            }
            provenance
        }
        ValueProvenance::Fallible { success, error } => fallible_provenance(
            success
                .as_deref()
                .and_then(|value| instantiate_provenance_summary(value, map_origin)),
            error
                .as_deref()
                .and_then(|value| instantiate_provenance_summary(value, map_origin)),
        ),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let mapped_fallback = fallback
                .as_deref()
                .and_then(|value| instantiate_provenance_summary(value, map_origin));
            let mapped_fields = fields
                .iter()
                .filter_map(|(name, value)| {
                    instantiate_provenance_summary(value, map_origin)
                        .map(|mapped| (name.clone(), mapped))
                })
                .collect::<BTreeMap<_, _>>();
            let mapped_elements = elements
                .iter()
                .filter_map(|(index, value)| {
                    instantiate_provenance_summary(value, map_origin).map(|mapped| (*index, mapped))
                })
                .collect::<BTreeMap<_, _>>();
            if mapped_fallback.is_none() && mapped_fields.is_empty() && mapped_elements.is_empty() {
                None
            } else {
                Some(ValueProvenance::Aggregate {
                    fallback: mapped_fallback.map(Box::new),
                    fields: mapped_fields,
                    elements: mapped_elements,
                })
            }
        }
    }
}

fn instantiate_origin(
    origin: &StorageOrigin,
    map_origin: &mut impl FnMut(&StorageOrigin) -> Option<ValueProvenance>,
) -> Option<ValueProvenance> {
    let StorageOrigin::InputWithCurrentFallback(source) = origin else {
        return map_origin(origin);
    };
    let mapped = map_origin(&StorageOrigin::Input(*source))?;
    if !mapped.has_storage_dependency() {
        return map_origin(&StorageOrigin::CurrentAllocationContext);
    }
    Some(preserve_current_fallback(mapped))
}

fn preserve_current_fallback(provenance: ValueProvenance) -> ValueProvenance {
    match provenance {
        ValueProvenance::Origins(origins) => ValueProvenance::Origins(
            origins
                .into_iter()
                .map(|origin| match origin {
                    StorageOrigin::Input(input) => StorageOrigin::InputWithCurrentFallback(input),
                    origin => origin,
                })
                .collect(),
        ),
        provenance => provenance,
    }
}
