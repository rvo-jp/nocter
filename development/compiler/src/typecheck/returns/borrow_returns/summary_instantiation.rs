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
                merge_provenance(&mut provenance, map_origin(origin));
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
