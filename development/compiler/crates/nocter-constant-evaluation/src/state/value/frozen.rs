//! Freeze and reconstruct finite physical DAGs without expanding repeated cardinalities.
use super::{Data, Value};
use nocter_model::{ConstantValue, FrozenData, FrozenNode};
use std::collections::BTreeMap;
impl Value {
    /// Freeze one result into a finite publication DAG, preserving shared components.
    /// # Panics
    /// Panics if an internal value violates the postorder construction invariant.
    #[must_use]
    pub fn freeze(&self) -> FrozenData {
        let mut pending = vec![(self, false)];
        let mut ids = BTreeMap::new();
        let mut nodes = Vec::new();
        while let Some((value, expanded)) = pending.pop() {
            if ids.contains_key(&value.identity()) {
                continue;
            }
            if !expanded && value.data().composite() {
                pending.push((value, true));
                pending.extend(value.data().children().rev().map(|field| (field, false)));
                continue;
            }
            let node = match value.data() {
                Data::Unit => FrozenNode::Unit,
                Data::Bool(value) => FrozenNode::Scalar(ConstantValue::Bool(*value)),
                Data::Integer(value) => FrozenNode::Scalar(ConstantValue::Integer(*value)),
                Data::Variant { primary, payload } => FrozenNode::Variant {
                    primary: *primary,
                    payload: payload.as_ref().map(|value| ids[&value.identity()]),
                },
                Data::Aggregate(fields) => FrozenNode::Aggregate(
                    fields.iter().map(|field| ids[&field.identity()]).collect(),
                ),
                Data::Repeated {
                    element,
                    length,
                    replacements,
                } => FrozenNode::Repeated {
                    element: ids[&element.identity()],
                    length: *length,
                    replacements: replacements
                        .iter()
                        .map(|(&index, value)| (index, ids[&value.identity()]))
                        .collect(),
                },
            };
            ids.insert(value.identity(), nodes.len());
            nodes.push(node);
        }
        FrozenData::new(nodes).expect("postorder frozen data")
    }
    /// Reconstruct the selected immutable plan datum without visiting unrelated graph nodes.
    /// Shared components remain shared; no source executes during reconstruction.
    #[must_use]
    pub fn from_frozen(data: &FrozenData) -> Option<Self> {
        let mut values: BTreeMap<usize, Self> = BTreeMap::new();
        let mut pending = vec![(data.root(), false)];
        while let Some((id, expanded)) = pending.pop() {
            if values.contains_key(&id) {
                continue;
            }
            let node = data.nodes().get(id)?;
            if !expanded
                && matches!(
                    node,
                    FrozenNode::Aggregate(_)
                        | FrozenNode::Repeated { .. }
                        | FrozenNode::Variant { .. }
                )
            {
                pending.push((id, true));
                pending.extend(children(node).rev().map(|field| (field, false)));
                continue;
            }
            let value = match node {
                FrozenNode::Unit => Self::unit(),
                FrozenNode::Variant { primary, payload } => Self::variant(
                    *primary,
                    match payload {
                        Some(id) => Some(values.get(id)?.clone()),
                        None => None,
                    },
                ),
                FrozenNode::Scalar(ConstantValue::Bool(value)) => Self::boolean(*value),
                FrozenNode::Scalar(ConstantValue::Integer(value)) => Self::integer(*value),
                FrozenNode::Aggregate(fields) => Self::aggregate(
                    fields
                        .iter()
                        .map(|id| values.get(id).cloned())
                        .collect::<Option<Box<[_]>>>()?,
                ),
                FrozenNode::Repeated {
                    element,
                    length,
                    replacements,
                } => Self::plain(Data::Repeated {
                    element: values.get(element)?.clone(),
                    length: *length,
                    replacements: replacements
                        .iter()
                        .map(|&(index, value)| Some((index, values.get(&value)?.clone())))
                        .collect::<Option<BTreeMap<_, _>>>()?,
                }),
                FrozenNode::Scalar(_) => return None,
            };
            values.insert(id, value);
        }
        values.remove(&data.root())
    }
}
fn children(node: &FrozenNode) -> impl DoubleEndedIterator<Item = usize> + '_ {
    let (fields, element, replacements): (&[usize], _, _) = match node {
        FrozenNode::Aggregate(fields) => (fields, None, None),
        FrozenNode::Variant { payload, .. } => (&[], *payload, None),
        FrozenNode::Repeated {
            element,
            replacements,
            ..
        } => (&[], Some(*element), Some(replacements)),
        _ => (&[], None, None),
    };
    fields.iter().copied().chain(element).chain(
        replacements
            .into_iter()
            .flat_map(|r| r.iter().map(|&(_, value)| value)),
    )
}
