//! Immutable value handles; evaluator mutations reserve a new node before copying shared data.
use super::{
    Rule,
    budget::{Lease, Storage},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU64,
    sync::Arc,
};
mod composite;
mod frozen;
mod storage;
const SPARSE_ENTRY_ITEMS: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Data {
    Unit,
    Bool(bool),
    Integer(i128),
    /// Ordered physical storage; the validated slot type owns its record/tuple/array identity.
    Aggregate(Box<[Value]>),
    /// One default element and sparse replacements, independent of logical cardinality.
    Repeated {
        element: Value,
        length: NonZeroU64,
        replacements: BTreeMap<u64, Value>,
    },
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
    pub fn aggregate(fields: impl Into<Box<[Self]>>) -> Self {
        Self::plain(Data::Aggregate(fields.into()))
    }
    #[must_use]
    pub fn repeated(element: Self, length: NonZeroU64) -> Self {
        Self::plain(Data::Repeated {
            element,
            length,
            replacements: BTreeMap::new(),
        })
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
    fn unique_node(&mut self, storage: &Storage) -> Result<&mut Node, Rule> {
        if !self.belongs_to(storage) {
            return Err(Rule::InvalidPlan);
        }
        if Arc::get_mut(self.node.as_mut().expect("live value")).is_none() {
            let count = self.data().storage_items()?;
            let allocation = storage.items(count)?;
            let data = self.data().clone();
            self.node = Some(Arc::new(Node {
                data,
                allocation: Some(allocation),
            }));
        }
        Ok(Arc::get_mut(self.node.as_mut().expect("live value")).expect("unique value"))
    }
    pub(super) fn component_mut(
        &mut self,
        storage: &Storage,
        index: usize,
    ) -> Result<&mut Self, Rule> {
        let node = self.unique_node(storage)?;
        match &mut node.data {
            Data::Aggregate(fields) => fields.get_mut(index).ok_or(Rule::InvalidPlan),
            Data::Repeated {
                element,
                length,
                replacements,
            } => {
                let index = u64::try_from(index).map_err(|_| Rule::InvalidPlan)?;
                if index >= length.get() {
                    return Err(Rule::InvalidPlan);
                }
                match replacements.entry(index) {
                    std::collections::btree_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        node.allocation
                            .as_mut()
                            .ok_or(Rule::InvalidPlan)?
                            .grow(storage, SPARSE_ENTRY_ITEMS)?;
                        Ok(entry.insert(element.clone()))
                    }
                }
            }
            _ => Err(Rule::InvalidPlan),
        }
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
                _ if left.data().component_pairs(right.data(), &mut pending) => {}
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
        let Some(node) = self.node.take().and_then(Arc::into_inner) else {
            return;
        };
        let mut pending = node.data.into_children();
        while let Some(mut value) = pending.pop() {
            if let Some(node) = value.node.take().and_then(Arc::into_inner) {
                pending.extend(node.data.into_children());
            }
        }
    }
}
