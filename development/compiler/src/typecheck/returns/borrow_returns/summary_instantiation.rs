use super::*;

/// Rebinds body-local input identities to the matching public callable contract.
///
/// Source-backed bodies retain physical spans for diagnostics, but summaries cross the callable
/// boundary and therefore use the receiver, parameter, and literal-capture identities authored in
/// the module root contract.
pub(in crate::typecheck::returns) fn canonicalize_provenance_summary_inputs(
    summary: ValueProvenance,
    resolved: &ResolveOutput,
) -> ValueProvenance {
    match summary {
        ValueProvenance::Independent => ValueProvenance::Independent,
        ValueProvenance::Origins(origins) => ValueProvenance::Origins(
            origins
                .into_iter()
                .map(|origin| canonicalize_storage_origin_input(origin, resolved))
                .collect(),
        ),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => ValueProvenance::Aggregate {
            fallback: fallback
                .map(|value| Box::new(canonicalize_provenance_summary_inputs(*value, resolved))),
            fields: fields
                .into_iter()
                .map(|(name, value)| {
                    (
                        name,
                        canonicalize_provenance_summary_inputs(value, resolved),
                    )
                })
                .collect(),
            elements: elements
                .into_iter()
                .map(|(index, value)| {
                    (
                        index,
                        canonicalize_provenance_summary_inputs(value, resolved),
                    )
                })
                .collect(),
        },
        ValueProvenance::Fallible { success, error } => ValueProvenance::Fallible {
            success: success
                .map(|value| Box::new(canonicalize_provenance_summary_inputs(*value, resolved))),
            error: error
                .map(|value| Box::new(canonicalize_provenance_summary_inputs(*value, resolved))),
        },
    }
}

fn canonicalize_storage_origin_input(
    origin: StorageOrigin,
    resolved: &ResolveOutput,
) -> StorageOrigin {
    match origin {
        StorageOrigin::Input(input) => StorageOrigin::Input(InputId::declared_at(
            resolved.canonical_callable_input_identity(input.declaration_span()),
        )),
        StorageOrigin::InputWithCurrentFallback(input) => {
            StorageOrigin::InputWithCurrentFallback(InputId::declared_at(
                resolved.canonical_callable_input_identity(input.declaration_span()),
            ))
        }
        StorageOrigin::Allocated(origin) => StorageOrigin::Allocated(Box::new(
            canonicalize_storage_origin_input(*origin, resolved),
        )),
        origin => origin,
    }
}

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
    if !has_selected_allocation_domain(&mapped) {
        return map_origin(&StorageOrigin::CurrentAllocationContext);
    }
    Some(preserve_current_fallback(mapped))
}

fn has_selected_allocation_domain(provenance: &ValueProvenance) -> bool {
    match provenance {
        ValueProvenance::Independent => false,
        ValueProvenance::Origins(origins) => origins.iter().any(|origin| {
            matches!(
                origin,
                StorageOrigin::Allocated(_)
                    | StorageOrigin::Input(_)
                    | StorageOrigin::InputWithCurrentFallback(_)
            )
        }),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            fallback
                .as_deref()
                .is_some_and(has_selected_allocation_domain)
                || fields.values().any(has_selected_allocation_domain)
                || elements.values().any(has_selected_allocation_domain)
        }
        ValueProvenance::Fallible { success, error } => {
            success
                .as_deref()
                .is_some_and(has_selected_allocation_domain)
                || error.as_deref().is_some_and(has_selected_allocation_domain)
        }
    }
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
