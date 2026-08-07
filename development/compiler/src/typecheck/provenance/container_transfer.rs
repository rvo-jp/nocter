//! Provenance normalization for ownership transferred out of a container.

use super::{StorageOrigin, ValueProvenance};
use std::collections::{BTreeMap, HashSet};

impl ValueProvenance {
    /// Removes only lexical scopes that describe the container variables also
    /// represented by an input origin. Origins carried by the transferred
    /// element itself remain intact and are substituted at the caller.
    pub(in crate::typecheck) fn without_input_container_scopes(self) -> Self {
        let input_bindings = self
            .input_origins()
            .into_iter()
            .map(|input| input.declaration_span())
            .collect::<HashSet<_>>();
        remove_container_scopes(self, &input_bindings)
    }
}

fn remove_container_scopes(
    provenance: ValueProvenance,
    input_bindings: &HashSet<crate::source::ByteSpan>,
) -> ValueProvenance {
    match provenance {
        ValueProvenance::Independent => ValueProvenance::Independent,
        ValueProvenance::Origins(origins) => {
            let origins = origins
                .into_iter()
                .filter(|origin| {
                    !matches!(
                        origin,
                        StorageOrigin::Scope { binding, .. } if input_bindings.contains(binding)
                    )
                })
                .collect::<Vec<_>>();
            if origins.is_empty() {
                ValueProvenance::Independent
            } else {
                ValueProvenance::Origins(origins)
            }
        }
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => ValueProvenance::Aggregate {
            fallback: fallback
                .map(|value| remove_container_scopes(*value, input_bindings))
                .map(Box::new),
            fields: fields
                .into_iter()
                .map(|(name, value)| (name, remove_container_scopes(value, input_bindings)))
                .collect::<BTreeMap<_, _>>(),
            elements: elements
                .into_iter()
                .map(|(index, value)| (index, remove_container_scopes(value, input_bindings)))
                .collect::<BTreeMap<_, _>>(),
        },
        ValueProvenance::Fallible { success, error } => ValueProvenance::Fallible {
            success: success
                .map(|value| remove_container_scopes(*value, input_bindings))
                .map(Box::new),
            error: error
                .map(|value| remove_container_scopes(*value, input_bindings))
                .map(Box::new),
        },
    }
}
