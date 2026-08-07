use super::{StorageOrigin, ValueProvenance};
use std::collections::BTreeMap;

/// Marks storage retained by a returned value as newly allocated while keeping
/// its allocation domain available to ordinary lifetime and region checks.
///
/// This is separate from the execution-time current-context requirement. A
/// callable may allocate scratch storage without returning an allocated value,
/// and an explicit allocator may produce a result without using the ambient
/// context.
impl ValueProvenance {
    /// Marks the successful returned value as allocated while leaving a
    /// fallible error branch independent from the success storage effect.
    pub(in crate::typecheck) fn returned_allocation(self) -> Self {
        match self {
            Self::Fallible { success, error } => Self::Fallible {
                success: Some(Box::new(
                    success
                        .map(|value| *value)
                        .unwrap_or(Self::Independent)
                        .allocated(),
                )),
                error,
            },
            value => value.allocated(),
        }
    }

    pub(in crate::typecheck) fn allocated(self) -> Self {
        match self {
            Self::Independent => {
                Self::Origins(vec![StorageOrigin::allocated(StorageOrigin::Static)])
            }
            Self::Origins(origins) => {
                Self::Origins(origins.into_iter().map(StorageOrigin::allocated).collect())
            }
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => Self::Aggregate {
                fallback: fallback.map(|value| Box::new(value.allocated())),
                fields: fields
                    .into_iter()
                    .map(|(name, value)| (name, value.allocated()))
                    .collect(),
                elements: elements
                    .into_iter()
                    .map(|(index, value)| (index, value.allocated()))
                    .collect(),
            },
            Self::Fallible { success, error } => Self::Fallible {
                success: success.map(|value| Box::new(value.allocated())),
                error: error.map(|value| Box::new(value.allocated())),
            },
        }
    }

    pub(in crate::typecheck) fn contains_result_allocation(&self) -> bool {
        match self {
            Self::Independent => false,
            Self::Origins(origins) => origins.iter().any(StorageOrigin::is_allocated),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => {
                fallback
                    .as_deref()
                    .is_some_and(Self::contains_result_allocation)
                    || fields.values().any(Self::contains_result_allocation)
                    || elements.values().any(Self::contains_result_allocation)
            }
            Self::Fallible { success, error } => {
                success
                    .as_deref()
                    .is_some_and(Self::contains_result_allocation)
                    || error
                        .as_deref()
                        .is_some_and(Self::contains_result_allocation)
            }
        }
    }

    pub(in crate::typecheck) fn without_result_allocation(self) -> Self {
        match self {
            Self::Independent => Self::Independent,
            Self::Origins(origins) => Self::Origins(
                origins
                    .into_iter()
                    .map(StorageOrigin::into_allocation_domain)
                    .collect(),
            ),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => Self::Aggregate {
                fallback: fallback.map(|value| Box::new(value.without_result_allocation())),
                fields: fields
                    .into_iter()
                    .map(|(name, value)| (name, value.without_result_allocation()))
                    .collect(),
                elements: elements
                    .into_iter()
                    .map(|(index, value)| (index, value.without_result_allocation()))
                    .collect(),
            },
            Self::Fallible { success, error } => Self::Fallible {
                success: success.map(|value| Box::new(value.without_result_allocation())),
                error: error.map(|value| Box::new(value.without_result_allocation())),
            },
        }
    }

    pub(in crate::typecheck) fn retain_only_result_allocations(self) -> Self {
        match self {
            Self::Independent => Self::Independent,
            Self::Origins(origins) => {
                let origins = origins
                    .into_iter()
                    .filter(StorageOrigin::is_allocated)
                    .collect::<Vec<_>>();
                if origins.is_empty() {
                    Self::Independent
                } else {
                    Self::Origins(origins)
                }
            }
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => {
                let fallback = fallback
                    .map(|value| value.retain_only_result_allocations())
                    .filter(Self::contains_result_allocation)
                    .map(Box::new);
                let fields = fields
                    .into_iter()
                    .map(|(name, value)| (name, value.retain_only_result_allocations()))
                    .filter(|(_, value)| value.contains_result_allocation())
                    .collect::<BTreeMap<_, _>>();
                let elements = elements
                    .into_iter()
                    .map(|(index, value)| (index, value.retain_only_result_allocations()))
                    .filter(|(_, value)| value.contains_result_allocation())
                    .collect::<BTreeMap<_, _>>();
                if fallback.is_none() && fields.is_empty() && elements.is_empty() {
                    Self::Independent
                } else {
                    Self::Aggregate {
                        fallback,
                        fields,
                        elements,
                    }
                }
            }
            Self::Fallible { success, error } => {
                let success = success
                    .map(|value| value.retain_only_result_allocations())
                    .filter(Self::contains_result_allocation)
                    .map(Box::new);
                let error = error
                    .map(|value| value.retain_only_result_allocations())
                    .filter(Self::contains_result_allocation)
                    .map(Box::new);
                if success.is_none() && error.is_none() {
                    Self::Independent
                } else {
                    Self::Fallible { success, error }
                }
            }
        }
    }
}

impl StorageOrigin {
    fn allocated(origin: Self) -> Self {
        match origin {
            Self::Allocated(_) => origin,
            _ => Self::Allocated(Box::new(origin)),
        }
    }

    pub(in crate::typecheck) fn is_allocated(&self) -> bool {
        matches!(self, Self::Allocated(_))
    }

    pub(in crate::typecheck) fn allocation_domain(&self) -> &Self {
        match self {
            Self::Allocated(origin) => origin.allocation_domain(),
            _ => self,
        }
    }

    fn into_allocation_domain(self) -> Self {
        match self {
            Self::Allocated(origin) => origin.into_allocation_domain(),
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{ByteSpan, SourceId};
    use crate::typecheck::provenance::InputId;
    use std::collections::BTreeMap;

    fn span(start: usize) -> ByteSpan {
        ByteSpan::new(SourceId::new(0), start, start + 1)
    }

    #[test]
    fn allocation_marker_preserves_aggregate_shape_and_domains() {
        let input = InputId::declared_at(span(1));
        let provenance = ValueProvenance::Aggregate {
            fallback: None,
            fields: BTreeMap::from([
                ("buffer".into(), ValueProvenance::input(input)),
                ("tag".into(), ValueProvenance::Independent),
            ]),
            elements: BTreeMap::new(),
        }
        .allocated();

        assert!(provenance.contains_result_allocation());
        assert_eq!(
            provenance.field_provenance("buffer"),
            Some(ValueProvenance::Origins(vec![StorageOrigin::Allocated(
                Box::new(StorageOrigin::Input(input))
            )]))
        );
        assert_eq!(
            provenance.field_provenance("tag"),
            Some(ValueProvenance::Origins(vec![StorageOrigin::Allocated(
                Box::new(StorageOrigin::Static)
            )]))
        );
    }

    #[test]
    fn allocation_marker_is_idempotent() {
        let provenance = ValueProvenance::current_allocation_context()
            .allocated()
            .allocated();
        assert_eq!(
            provenance,
            ValueProvenance::Origins(vec![StorageOrigin::Allocated(Box::new(
                StorageOrigin::CurrentAllocationContext
            ))])
        );
    }

    #[test]
    fn removing_allocation_retains_the_lifetime_domain() {
        let input = InputId::declared_at(span(2));
        assert_eq!(
            ValueProvenance::input(input)
                .allocated()
                .without_result_allocation(),
            ValueProvenance::input(input)
        );
    }

    #[test]
    fn retaining_allocations_discards_unallocated_peer_origins() {
        let input = InputId::declared_at(span(3));
        let mut provenance = ValueProvenance::input(input);
        provenance.merge(&ValueProvenance::current_allocation_context().allocated());

        assert_eq!(
            provenance.retain_only_result_allocations(),
            ValueProvenance::Origins(vec![StorageOrigin::Allocated(Box::new(
                StorageOrigin::CurrentAllocationContext
            ))])
        );
    }
}
