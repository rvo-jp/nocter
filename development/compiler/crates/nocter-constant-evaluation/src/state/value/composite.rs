//! Finite physical children and logical components are different for repeated storage.
use super::{Data, Rule, SPARSE_ENTRY_ITEMS, Value};
use std::collections::BTreeSet;
impl Data {
    pub(super) fn composite(&self) -> bool {
        matches!(
            self,
            Self::Aggregate(_) | Self::Repeated { .. } | Self::Variant { .. }
        )
    }
    pub(in crate::state) fn children(&self) -> impl DoubleEndedIterator<Item = &Value> {
        let (fields, element, replacements): (&[Value], _, _) = match self {
            Self::Aggregate(fields) => (fields, None, None),
            Self::Variant { payload, .. } => (&[], payload.as_ref(), None),
            Self::Repeated {
                element,
                replacements,
                ..
            } => (&[], Some(element), Some(replacements)),
            _ => (&[], None, None),
        };
        fields
            .iter()
            .chain(element)
            .chain(replacements.into_iter().flat_map(|r| r.values()))
    }
    pub(super) fn into_children(self) -> Vec<Value> {
        match self {
            Self::Aggregate(fields) => fields.into_vec(),
            Self::Variant { payload, .. } => payload.into_iter().collect(),
            Self::Repeated {
                element,
                replacements,
                ..
            } => std::iter::once(element)
                .chain(replacements.into_values())
                .collect(),
            _ => Vec::new(),
        }
    }
    pub(super) fn storage_items(&self) -> Result<usize, Rule> {
        match self {
            Self::Text(text) => Ok(text.len()),
            Self::Aggregate(fields) => Ok(fields.len()),
            Self::Variant { payload, .. } => Ok(usize::from(payload.is_some())),
            Self::Repeated { replacements, .. } => Self::repeated_items(replacements.len()),
            _ => Ok(0),
        }
    }
    pub(super) fn repeated_items(count: usize) -> Result<usize, Rule> {
        count
            .checked_mul(SPARSE_ENTRY_ITEMS)
            .and_then(|n| n.checked_add(1))
            .ok_or(Rule::StorageLimit)
    }
    pub(in crate::state) fn component(&self, index: usize) -> Option<&Value> {
        match self {
            Self::Aggregate(fields) => fields.get(index),
            Self::Variant { primary, payload } if index == usize::from(!primary) => {
                payload.as_ref()
            }
            Self::Repeated {
                element,
                length,
                replacements,
            } => {
                let index = u64::try_from(index).ok()?;
                (index < length.get()).then(|| replacements.get(&index).unwrap_or(element))
            }
            _ => None,
        }
    }
    pub(super) fn component_pairs<'a>(
        &'a self,
        other: &'a Self,
        pending: &mut Vec<(&'a Value, &'a Value)>,
    ) -> bool {
        match (self, other) {
            (
                Self::Variant {
                    primary: left,
                    payload: a,
                },
                Self::Variant {
                    primary: right,
                    payload: b,
                },
            ) if left == right => {
                return match (a, b) {
                    (None, None) => true,
                    (Some(a), Some(b)) => {
                        pending.push((a, b));
                        true
                    }
                    _ => false,
                };
            }
            (Self::Aggregate(left), Self::Aggregate(right)) if left.len() == right.len() => {
                pending.extend(left.iter().zip(right));
            }
            (Self::Aggregate(fields), Self::Repeated { length, .. })
                if u64::try_from(fields.len()).ok() == Some(length.get()) =>
            {
                for (index, field) in fields.iter().enumerate() {
                    let Some(other) = other.component(index) else {
                        return false;
                    };
                    pending.push((field, other));
                }
            }
            (Self::Repeated { .. }, Self::Aggregate(_)) => {
                return other.component_pairs(self, pending);
            }
            (
                Self::Repeated {
                    element: left,
                    length,
                    replacements: a,
                },
                Self::Repeated {
                    element: right,
                    length: other_length,
                    replacements: b,
                },
            ) if length == other_length => {
                let keys: BTreeSet<_> = a.keys().chain(b.keys()).copied().collect();
                if u64::try_from(keys.len())
                    .ok()
                    .is_none_or(|count| count < length.get())
                {
                    pending.push((left, right));
                }
                pending.extend(keys.into_iter().map(|index| {
                    (
                        a.get(&index).unwrap_or(left),
                        b.get(&index).unwrap_or(right),
                    )
                }));
            }
            _ => return false,
        }
        true
    }
}
