//! Shared structural producer facts. Equal leaf types never merge distinct component occurrences.
use super::{INVALID, Producers};
use crate::value_access::flow::IntegrityError;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{TypeId, ValueAppliedResult, ValueField, ValueFunctionOrigin, ValueProjection};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Facts(pub(super) usize);
impl Facts {
    const UNKNOWN: Self = Self(1);
}
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Tag {
    #[default]
    NoFacts,
    Unknown,
    Primary,
    Alternate,
}
impl Tag {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::NoFacts, value) | (value, Self::NoFacts) => value,
            (left, right) if left == right => left,
            _ => Self::Unknown,
        }
    }
    pub(super) fn primary(self) -> Option<bool> {
        match self {
            Self::Primary => Some(true),
            Self::Alternate => Some(false),
            Self::NoFacts | Self::Unknown => None,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Node {
    pub(super) producers: Producers,
    children: BTreeMap<ValueField, Facts>,
    pub(super) tag: Tag,
}
/// Analysis-local interning gives structural equality without walking expanded shared values.
/// Arrays have one conservative element summary, never one fact node per numeric element.
pub(super) struct Store {
    nodes: Vec<Node>,
    ids: BTreeMap<Node, Facts>,
}
impl Default for Store {
    fn default() -> Self {
        let unknown = Node {
            tag: Tag::Unknown,
            ..Node::default()
        };
        Self {
            nodes: vec![Node::default(), unknown.clone()],
            ids: BTreeMap::from([(Node::default(), Facts(0)), (unknown, Facts::UNKNOWN)]),
        }
    }
}
fn field<I: Copy>(field: ValueProjection<I>) -> ValueField {
    match field {
        ValueProjection::Array(_) => ValueField::Array(0),
        ValueProjection::Record(id) => ValueField::Record(id),
        ValueProjection::Tuple(index) => ValueField::Tuple(index),
        ValueProjection::Variant(primary) => ValueField::Variant(primary),
    }
}
pub(super) fn select<I: Copy>(
    nodes: &[Node],
    mut facts: Facts,
    fields: &[ValueProjection<I>],
) -> Facts {
    for &selected in fields {
        if facts == Facts::default() {
            return facts;
        }
        facts = nodes[facts.0]
            .children
            .get(&field(selected))
            .copied()
            .unwrap_or(Facts::UNKNOWN);
    }
    facts
}
impl Store {
    pub(super) fn freeze(self) -> std::sync::Arc<[Node]> {
        self.nodes.into()
    }
    fn intern(&mut self, mut node: Node) -> Facts {
        node.children.retain(|_, value| *value != Facts::default());
        if let Some(&id) = self.ids.get(&node) {
            return id;
        }
        let id = Facts(self.nodes.len());
        self.nodes.push(node.clone());
        self.ids.insert(node, id);
        id
    }
    pub(super) fn single(&mut self, witness: FunctionWitness) -> Facts {
        self.intern(Node {
            producers: Producers::from([witness]),
            tag: Tag::Unknown,
            ..Node::default()
        })
    }
    pub(super) fn aggregate(&mut self, children: BTreeMap<ValueField, Facts>) -> Facts {
        self.intern(Node {
            children,
            tag: Tag::Unknown,
            ..Node::default()
        })
    }
    pub(super) const fn unknown() -> Facts {
        Facts::UNKNOWN
    }
    pub(super) fn outcome(&mut self, children: BTreeMap<ValueField, Facts>) -> Facts {
        let tag = if children.contains_key(&ValueField::Variant(true)) {
            Tag::Primary
        } else {
            Tag::Alternate
        };
        self.intern(Node {
            children,
            tag,
            ..Node::default()
        })
    }
    pub(super) fn array(&mut self, element: Facts) -> Facts {
        self.aggregate(BTreeMap::from([(ValueField::Array(0), element)]))
    }
    pub(super) fn producers(&self, facts: Facts) -> Producers {
        self.nodes[facts.0].producers.clone()
    }
    pub(super) fn selected<I: Copy>(&self, facts: Facts, fields: &[ValueProjection<I>]) -> Facts {
        select(&self.nodes, facts, fields)
    }
    pub(super) fn applications(&self, facts: Facts) -> BTreeSet<ValueAppliedResult> {
        let mut pending = vec![facts];
        let mut seen = BTreeSet::new();
        let mut applications = BTreeSet::new();
        while let Some(facts) = pending.pop() {
            if !seen.insert(facts) {
                continue;
            }
            let node = &self.nodes[facts.0];
            applications.extend(
                node.producers
                    .iter()
                    .filter_map(|witness| match witness.origin {
                        ValueFunctionOrigin::Application(result) => Some(result),
                        _ => None,
                    }),
            );
            pending.extend(node.children.values().copied());
        }
        applications
    }
    pub(super) fn join(&mut self, left: Facts, right: Facts) -> Facts {
        let pair = |a, b| if a <= b { (a, b) } else { (b, a) };
        let root = pair(left, right);
        let mut pending = vec![root];
        let mut done = BTreeMap::new();
        while let Some((left, right)) = pending.pop() {
            if done.contains_key(&(left, right)) {
                continue;
            }
            if left == Facts::default() || left == right {
                done.insert((left, right), right);
                continue;
            }
            let a = self.nodes[left.0].clone();
            let b = self.nodes[right.0].clone();
            let children: BTreeMap<_, _> = a
                .children
                .keys()
                .chain(b.children.keys())
                .copied()
                .map(|field| {
                    let a = a.children.get(&field).copied().unwrap_or(Facts::UNKNOWN);
                    let b = b.children.get(&field).copied().unwrap_or(Facts::UNKNOWN);
                    (field, pair(a, b))
                })
                .collect();
            let missing: Vec<_> = children
                .values()
                .copied()
                .filter(|key| !done.contains_key(key))
                .collect();
            if !missing.is_empty() {
                pending.push((left, right));
                pending.extend(missing);
                continue;
            }
            let mut result = a;
            result.tag = result.tag.join(b.tag);
            result.producers.extend(b.producers);
            result.children = children
                .into_iter()
                .map(|(field, key)| (field, done[&key]))
                .collect();
            let id = self.intern(result);
            done.insert((left, right), id);
        }
        done[&root]
    }
    pub(super) fn update<I: Copy>(
        &mut self,
        facts: Facts,
        fields: &[ValueProjection<I>],
        replacement: Facts,
        weak: bool,
    ) -> Facts {
        let mut current = facts;
        let mut parents = Vec::new();
        let mut weak = weak;
        for &selected in fields {
            weak |= matches!(selected, ValueProjection::Array(_));
            let selected = field(selected);
            let node = self.nodes[current.0].clone();
            current =
                node.children
                    .get(&selected)
                    .copied()
                    .unwrap_or(if current == Facts::default() {
                        Facts::default()
                    } else {
                        Facts::UNKNOWN
                    });
            parents.push((node, selected));
        }
        let mut result = if weak {
            self.join(current, replacement)
        } else {
            replacement
        };
        for (mut parent, selected) in parents.into_iter().rev() {
            parent.children.insert(selected, result);
            result = self.intern(parent);
        }
        result
    }
    pub(super) fn received(
        &mut self,
        program: &Program,
        data: TypeId,
        retain_applications: bool,
    ) -> Result<Facts, IntegrityError> {
        let mut pending = vec![data];
        let mut done = BTreeMap::new();
        while let Some(data) = pending.pop() {
            if done.contains_key(&data) {
                continue;
            }
            let schema = program.data_schema(data).ok_or(INVALID)?;
            let children = match schema {
                PayloadSchema::Aggregate(schema) => schema
                    .components()
                    .map(|c| (c.field, c.data))
                    .collect::<Vec<_>>(),
                PayloadSchema::Array(array) if !array.is_empty() => {
                    vec![(ValueField::Array(0), array.element())]
                }
                PayloadSchema::Optional(data) | PayloadSchema::Fallible(data) => {
                    vec![(ValueField::Variant(true), data)]
                }
                _ => Vec::new(),
            };
            let missing: Vec<_> = children
                .iter()
                .filter_map(|(_, child)| (!done.contains_key(child)).then_some(*child))
                .collect();
            if !missing.is_empty() {
                pending.push(data);
                pending.extend(missing);
                continue;
            }
            let id = match schema {
                PayloadSchema::Function(witness)
                    if !matches!(witness.origin, ValueFunctionOrigin::Result(_))
                        && (retain_applications
                            || !matches!(witness.origin, ValueFunctionOrigin::Application(_))) =>
                {
                    self.single(witness)
                }
                _ => self.aggregate(
                    children
                        .into_iter()
                        .map(|(field, child)| (field, done[&child]))
                        .collect(),
                ),
            };
            done.insert(data, id);
        }
        Ok(done[&data])
    }
    /// Match formal component identities to original supplied producer alternatives. These
    /// equations are not exact frame bindings, contextual payloads or correlated call evidence.
    pub(super) fn input_producers(
        &mut self,
        program: &Program,
        data: TypeId,
        actual: Facts,
    ) -> Result<super::Inputs, IntegrityError> {
        let formal = self.received(program, data, true)?;
        let mut pending = vec![(formal, actual, false)];
        let mut seen = BTreeSet::new();
        let mut inputs = super::Inputs::new();
        while let Some((formal, actual, excluded)) = pending.pop() {
            if !seen.insert((formal, actual, excluded)) {
                continue;
            }
            let formal = &self.nodes[formal.0];
            let actual = &self.nodes[actual.0];
            for witness in &formal.producers {
                if let ValueFunctionOrigin::Parameter(
                    parameter @ nocter_model::ValueParameter::Component { .. },
                ) = witness.origin
                {
                    inputs
                        .entry(parameter)
                        .or_default()
                        .join(super::InputProducers::source(
                            actual.producers.clone(),
                            excluded,
                        ));
                }
            }
            pending.extend(formal.children.iter().map(|(field, &formal)| {
                (
                    formal,
                    actual.children.get(field).copied().unwrap_or_default(),
                    excluded
                        || (*field == ValueField::Variant(true)
                            && actual.tag.primary() == Some(false)),
                )
            }));
        }
        Ok(inputs)
    }
    fn adapt_leaf(
        &mut self,
        node: Node,
        invocation: nocter_model::ValueInvocation,
    ) -> Result<Facts, IntegrityError> {
        if !node.children.is_empty() {
            return Err(INVALID);
        }
        let producers = node
            .producers
            .into_iter()
            .map(|witness| {
                if !witness.invocation.can_supply(invocation) {
                    return Err(INVALID);
                }
                Ok(FunctionWitness {
                    invocation,
                    ..witness
                })
            })
            .collect::<Result<Producers, _>>()?;
        let id = self.intern(Node {
            producers,
            tag: Tag::Unknown,
            ..Node::default()
        });
        Ok(id)
    }
    /// Retag actual structural occurrences through a checked boundary, retaining their producers.
    pub(super) fn retag(
        &mut self,
        program: &Program,
        facts: Facts,
        source: TypeId,
        target: TypeId,
    ) -> Result<Facts, IntegrityError> {
        let root = (facts, source, target);
        let mut pending = vec![root];
        let mut done = BTreeMap::new();
        while let Some(key @ (facts, source, target)) = pending.pop() {
            if done.contains_key(&key) {
                continue;
            }
            if source == target || facts == Facts::default() {
                done.insert(key, facts);
                continue;
            }
            let node = self.nodes[facts.0].clone();
            match (program.data_schema(source), program.data_schema(target)) {
                (Some(PayloadSchema::Function(_)), Some(PayloadSchema::Function(view))) => {
                    let id = self.adapt_leaf(node, view.invocation)?;
                    done.insert(key, id);
                }
                (Some(PayloadSchema::Array(a)), Some(PayloadSchema::Array(b)))
                    if a.length() == b.length() =>
                {
                    let element = self.selected(facts, &[ValueField::Array(0)]);
                    let child = (element, a.element(), b.element());
                    if let Some(&element) = done.get(&child) {
                        let id = self.array(element);
                        done.insert(key, id);
                    } else {
                        pending.extend([key, child]);
                    }
                }
                (Some(PayloadSchema::Optional(a)), Some(PayloadSchema::Optional(b)))
                | (Some(PayloadSchema::Fallible(a)), Some(PayloadSchema::Fallible(b))) => {
                    let primary = ValueField::Variant(true);
                    let child = (self.selected(facts, &[primary]), a, b);
                    if let Some(&payload) = done.get(&child) {
                        let mut children = node.children;
                        children.insert(primary, payload);
                        let id = self.intern(Node {
                            children,
                            tag: node.tag,
                            ..Node::default()
                        });
                        done.insert(key, id);
                    } else {
                        pending.extend([key, child]);
                    }
                }
                (Some(PayloadSchema::Aggregate(a)), Some(PayloadSchema::Aggregate(b)))
                    if a.len() == b.len()
                        && matches!(
                            (a.form(), b.form()),
                            (
                                nocter_declarations::value_access::AggregateForm::Tuple(_),
                                nocter_declarations::value_access::AggregateForm::Tuple(_)
                            )
                        ) =>
                {
                    let children: Vec<_> = a
                        .components()
                        .zip(b.components())
                        .map(|(a, b)| (b.field, (self.selected(facts, &[a.field]), a.data, b.data)))
                        .collect();
                    let missing: Vec<_> = children
                        .iter()
                        .filter_map(|(_, child)| (!done.contains_key(child)).then_some(*child))
                        .collect();
                    if missing.is_empty() {
                        let id = self.aggregate(
                            children
                                .into_iter()
                                .map(|(field, child)| (field, done[&child]))
                                .collect(),
                        );
                        done.insert(key, id);
                    } else {
                        pending.push(key);
                        pending.extend(missing);
                    }
                }
                _ => return Err(INVALID),
            }
        }
        Ok(done[&root])
    }
}
