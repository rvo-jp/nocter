//! Immutable value handles; evaluator mutations reserve a new node before copying shared data.
use super::{
    Rule,
    budget::{Lease, Resident, Storage},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Data {
    Unit,
    Bool(bool),
    Integer(i128),
    Record(Box<[Value]>),
}
struct Node {
    data: Data,
    allocation: Option<Lease>,
}
/// Immutable plan data or a result that retains its allocation lease after return/memoization.
/// Constructors create plan data; execution imports it into the active budget before use.
/// Cloning shares data rather than recursively copying it.
#[derive(Clone)]
pub struct Value {
    node: Option<Arc<Node>>,
}
impl Value {
    fn plain(data: Data) -> Self {
        Self {
            node: Some(Arc::new(Node {
                data,
                allocation: None,
            })),
        }
    }
    #[must_use]
    pub fn unit() -> Self {
        Self::plain(Data::Unit)
    }
    #[must_use]
    pub fn boolean(value: bool) -> Self {
        Self::plain(Data::Bool(value))
    }
    #[must_use]
    pub fn integer(value: impl Into<i128>) -> Self {
        Self::plain(Data::Integer(value.into()))
    }
    #[must_use]
    pub fn record(fields: impl Into<Box<[Self]>>) -> Self {
        Self::plain(Data::Record(fields.into()))
    }
    #[must_use]
    pub fn data(&self) -> &Data {
        &self.inner().data
    }
    fn inner(&self) -> &Arc<Node> {
        self.node.as_ref().expect("live value handle")
    }
    pub(super) fn identity(&self) -> usize {
        Arc::as_ptr(self.inner()) as usize
    }
    pub(super) fn belongs_to(&self, storage: &Storage) -> bool {
        self.inner()
            .allocation
            .as_ref()
            .is_some_and(|lease| lease.belongs_to(storage))
    }
    pub(super) fn make_mut(&mut self, storage: &Storage) -> Result<&mut Data, Rule> {
        if !self.belongs_to(storage) {
            return Err(Rule::InvalidPlan);
        }
        if Arc::get_mut(self.node.as_mut().expect("live value")).is_none() {
            let count = match self.data() {
                Data::Record(fields) => fields.len(),
                _ => 0,
            };
            let allocation = storage.items(count)?;
            let data = self.data().clone();
            self.node = Some(Arc::new(Node {
                data,
                allocation: Some(allocation),
            }));
        }
        Ok(&mut Arc::get_mut(self.node.as_mut().expect("live value"))
            .expect("unique value")
            .data)
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = vec![(self, other)];
        let mut seen = BTreeSet::new();
        while let Some((left, right)) = pending.pop() {
            if left.identity() == right.identity()
                || !seen.insert((left.identity(), right.identity()))
            {
                continue;
            }
            match (left.data(), right.data()) {
                (Data::Unit, Data::Unit) => {}
                (Data::Bool(a), Data::Bool(b)) if a == b => {}
                (Data::Integer(a), Data::Integer(b)) if a == b => {}
                (Data::Record(a), Data::Record(b)) if a.len() == b.len() => {
                    pending.extend(a.iter().zip(b));
                }
                _ => return false,
            }
        }
        true
    }
}
impl Eq for Value {}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data().fmt(f)
    }
}
impl Drop for Value {
    fn drop(&mut self) {
        let Some(node) = self.node.take() else {
            return;
        };
        let Some(Node {
            data: Data::Record(fields),
            ..
        }) = Arc::into_inner(node)
        else {
            return;
        };
        let mut pending = fields.into_vec();
        while let Some(mut value) = pending.pop() {
            let Some(node) = value.node.take() else {
                continue;
            };
            if let Some(Node {
                data: Data::Record(fields),
                ..
            }) = Arc::into_inner(node)
            {
                pending.extend(fields);
            }
        }
    }
}
impl Storage {
    pub(super) fn primitive(&self, data: Data) -> Result<Value, Rule> {
        if matches!(data, Data::Record(_)) {
            return Err(Rule::InvalidPlan);
        }
        let allocation = self.items(0)?;
        Ok(Value {
            node: Some(Arc::new(Node {
                data,
                allocation: Some(allocation),
            })),
        })
    }
    pub(super) fn record(
        &self,
        fields: impl ExactSizeIterator<Item = Value>,
    ) -> Result<Value, Rule> {
        let allocation = self.items(fields.len())?;
        let fields = fields.collect::<Box<[_]>>();
        if fields.iter().any(|value| !value.belongs_to(self)) {
            return Err(Rule::InvalidPlan);
        }
        Ok(Value {
            node: Some(Arc::new(Node {
                data: Data::Record(fields),
                allocation: Some(allocation),
            })),
        })
    }
    pub(super) fn import(&self, value: &Value) -> Result<Value, Rule> {
        if value.belongs_to(self) {
            return Ok(value.clone());
        }
        if !matches!(value.data(), Data::Record(_)) {
            return self.primitive(value.data().clone());
        }
        let mut pending = vec![self.entry((value.clone(), false))?];
        let mut completed: BTreeMap<usize, Resident<Value>> = BTreeMap::new();
        while let Some(task) = pending.pop() {
            let (source, expanded) = &task.value;
            let identity = source.identity();
            if completed.contains_key(&identity) {
                continue;
            }
            if !source.belongs_to(self)
                && !*expanded
                && let Data::Record(fields) = source.data()
            {
                pending.push(self.entry((source.clone(), true))?);
                for field in fields.iter().rev() {
                    pending.push(self.entry((field.clone(), false))?);
                }
                continue;
            }
            let value = if source.belongs_to(self) {
                source.clone()
            } else {
                match source.data() {
                    Data::Record(fields) => self.record(fields.iter().map(|field| {
                        completed
                            .get(&field.identity())
                            .expect("postorder value")
                            .value
                            .clone()
                    }))?,
                    data => self.primitive(data.clone())?,
                }
            };
            completed.insert(identity, self.entry(value)?);
        }
        completed
            .get(&value.identity())
            .map(|entry| entry.value.clone())
            .ok_or(Rule::InvalidPlan)
    }
}

impl Value {
    /// Freeze one result into a finite publication DAG, preserving shared components.
    /// # Panics
    /// Panics if an internal value violates the postorder construction invariant.
    #[must_use]
    pub fn freeze(&self) -> nocter_model::FrozenData {
        use nocter_model::{ConstantValue, FrozenData, FrozenNode};
        let mut pending = vec![(self, false)];
        let mut ids = BTreeMap::new();
        let mut nodes = Vec::new();
        while let Some((value, expanded)) = pending.pop() {
            if ids.contains_key(&value.identity()) {
                continue;
            }
            if !expanded && let Data::Record(fields) = value.data() {
                pending.push((value, true));
                pending.extend(fields.iter().rev().map(|field| (field, false)));
                continue;
            }
            let node = match value.data() {
                Data::Unit => FrozenNode::Unit,
                Data::Bool(value) => FrozenNode::Scalar(ConstantValue::Bool(*value)),
                Data::Integer(value) => FrozenNode::Scalar(ConstantValue::Integer(*value)),
                Data::Record(fields) => FrozenNode::Aggregate(
                    fields.iter().map(|field| ids[&field.identity()]).collect(),
                ),
            };
            ids.insert(value.identity(), nodes.len());
            nodes.push(node);
        }
        FrozenData::new(nodes).expect("postorder frozen data")
    }
    /// Reconstruct the selected immutable plan datum without visiting unrelated graph nodes.
    /// Shared records remain shared; no source executes during reconstruction.
    #[must_use]
    pub fn from_frozen(data: &nocter_model::FrozenData) -> Option<Self> {
        use nocter_model::{ConstantValue, FrozenNode};
        let mut values: BTreeMap<usize, Self> = BTreeMap::new();
        let mut pending = vec![(data.root(), false)];
        while let Some((id, expanded)) = pending.pop() {
            if values.contains_key(&id) {
                continue;
            }
            let node = data.nodes().get(id)?;
            if !expanded && let FrozenNode::Aggregate(fields) = node {
                pending.push((id, true));
                pending.extend(fields.iter().rev().map(|&field| (field, false)));
                continue;
            }
            let value = match node {
                FrozenNode::Unit => Self::unit(),
                FrozenNode::Scalar(ConstantValue::Bool(value)) => Self::boolean(*value),
                FrozenNode::Scalar(ConstantValue::Integer(value)) => Self::integer(*value),
                FrozenNode::Aggregate(fields) => Self::record(
                    fields
                        .iter()
                        .map(|id| values.get(id).cloned())
                        .collect::<Option<Box<[_]>>>()?,
                ),
                FrozenNode::Scalar(_) => return None,
            };
            values.insert(id, value);
        }
        values.remove(&data.root())
    }
}
