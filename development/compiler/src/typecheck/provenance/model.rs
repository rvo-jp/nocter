use crate::source::ByteSpan;
use crate::typecheck::model::Type;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::typecheck) struct CallableId(ByteSpan);

impl CallableId {
    pub(in crate::typecheck) const fn declared_at(span: ByteSpan) -> Self {
        Self(span)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::typecheck) struct InputId(ByteSpan);

impl InputId {
    pub(in crate::typecheck) const fn declared_at(span: ByteSpan) -> Self {
        Self(span)
    }

    pub(in crate::typecheck) const fn declaration_span(self) -> ByteSpan {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::typecheck) struct RegionId(ByteSpan);

impl RegionId {
    pub(in crate::typecheck) const fn declared_at(span: ByteSpan) -> Self {
        Self(span)
    }

    pub(in crate::typecheck) const fn declaration_span(self) -> ByteSpan {
        self.0
    }
}

impl PartialOrd for InputId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InputId {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.0;
        let right = other.0;
        (left.source.raw(), left.start, left.end).cmp(&(right.source.raw(), right.start, right.end))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck) enum StorageOrigin {
    Static,
    Input(InputId),
    Scope {
        binding: ByteSpan,
        description: String,
    },
    Region {
        region: RegionId,
        description: String,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck) enum ValueProvenance {
    Independent,
    Origins(Vec<StorageOrigin>),
    Aggregate {
        fallback: Option<Box<ValueProvenance>>,
        fields: BTreeMap<String, ValueProvenance>,
        elements: BTreeMap<usize, ValueProvenance>,
    },
    Fallible {
        success: Option<Box<ValueProvenance>>,
        error: Option<Box<ValueProvenance>>,
    },
}

impl ValueProvenance {
    pub(in crate::typecheck) fn has_storage_dependency(&self) -> bool {
        match self {
            Self::Independent => false,
            Self::Origins(_) => true,
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => {
                fallback
                    .as_deref()
                    .is_some_and(Self::has_storage_dependency)
                    || fields.values().any(Self::has_storage_dependency)
                    || elements.values().any(Self::has_storage_dependency)
            }
            Self::Fallible { success, error } => {
                success.as_deref().is_some_and(Self::has_storage_dependency)
                    || error.as_deref().is_some_and(Self::has_storage_dependency)
            }
        }
    }

    pub(in crate::typecheck) fn static_storage() -> Self {
        Self::Origins(vec![StorageOrigin::Static])
    }

    pub(in crate::typecheck) fn input(input: InputId) -> Self {
        Self::Origins(vec![StorageOrigin::Input(input)])
    }

    pub(in crate::typecheck) fn scope(binding: ByteSpan, description: String) -> Self {
        Self::Origins(vec![StorageOrigin::Scope {
            binding,
            description,
        }])
    }

    pub(in crate::typecheck) fn region(region: RegionId, description: String) -> Self {
        Self::Origins(vec![StorageOrigin::Region {
            region,
            description,
        }])
    }

    pub(in crate::typecheck) fn unknown() -> Self {
        Self::Origins(vec![StorageOrigin::Unknown])
    }

    pub(in crate::typecheck) fn escaping_source(&self) -> Option<&str> {
        match self {
            Self::Origins(origins) => origins.iter().find_map(|origin| match origin {
                StorageOrigin::Scope { description, .. } => Some(description.as_str()),
                StorageOrigin::Region { description, .. } => Some(description.as_str()),
                StorageOrigin::Unknown => Some("unknown storage"),
                StorageOrigin::Static | StorageOrigin::Input(_) => None,
            }),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => fallback
                .as_deref()
                .and_then(ValueProvenance::escaping_source)
                .or_else(|| fields.values().find_map(ValueProvenance::escaping_source))
                .or_else(|| elements.values().find_map(ValueProvenance::escaping_source)),
            Self::Fallible { success, error } => success
                .as_deref()
                .and_then(ValueProvenance::escaping_source)
                .or_else(|| error.as_deref().and_then(ValueProvenance::escaping_source)),
            Self::Independent => None,
        }
    }

    pub(in crate::typecheck) fn input_origins(&self) -> Vec<InputId> {
        let mut inputs = Vec::new();
        self.collect_input_origins(&mut inputs);
        inputs
    }

    pub(in crate::typecheck) fn first_region_origin(
        &self,
        matching: impl Fn(RegionId) -> bool + Copy,
    ) -> Option<(RegionId, &str)> {
        match self {
            Self::Origins(origins) => origins.iter().find_map(|origin| match origin {
                StorageOrigin::Region {
                    region,
                    description,
                } if matching(*region) => Some((*region, description.as_str())),
                _ => None,
            }),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => fallback
                .as_deref()
                .and_then(|provenance| provenance.first_region_origin(matching))
                .or_else(|| {
                    fields
                        .values()
                        .find_map(|provenance| provenance.first_region_origin(matching))
                })
                .or_else(|| {
                    elements
                        .values()
                        .find_map(|provenance| provenance.first_region_origin(matching))
                }),
            Self::Fallible { success, error } => success
                .as_deref()
                .and_then(|provenance| provenance.first_region_origin(matching))
                .or_else(|| {
                    error
                        .as_deref()
                        .and_then(|provenance| provenance.first_region_origin(matching))
                }),
            Self::Independent => None,
        }
    }

    fn collect_input_origins(&self, inputs: &mut Vec<InputId>) {
        match self {
            Self::Origins(origins) => {
                for origin in origins {
                    if let StorageOrigin::Input(input) = origin
                        && !inputs.contains(input)
                    {
                        inputs.push(*input);
                    }
                }
            }
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => {
                if let Some(fallback) = fallback {
                    fallback.collect_input_origins(inputs);
                }
                for provenance in fields.values() {
                    provenance.collect_input_origins(inputs);
                }
                for provenance in elements.values() {
                    provenance.collect_input_origins(inputs);
                }
            }
            Self::Fallible { success, error } => {
                if let Some(success) = success {
                    success.collect_input_origins(inputs);
                }
                if let Some(error) = error {
                    error.collect_input_origins(inputs);
                }
            }
            Self::Independent => {}
        }
    }

    pub(in crate::typecheck) fn success_provenance(&self) -> Option<ValueProvenance> {
        match self {
            Self::Fallible { success, .. } => success.as_deref().cloned(),
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck) fn fallible_error_provenance(&self) -> Option<ValueProvenance> {
        match self {
            Self::Fallible { error, .. } => error.as_deref().cloned(),
            _ => None,
        }
    }

    pub(in crate::typecheck) fn field_provenance(&self, field: &str) -> Option<ValueProvenance> {
        match self {
            Self::Aggregate {
                fallback, fields, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                merge_provenance(&mut provenance, fields.get(field).cloned());
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck) fn element_provenance(
        &self,
        index: Option<usize>,
    ) -> Option<ValueProvenance> {
        match self {
            Self::Aggregate {
                fallback, elements, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                if let Some(index) = index {
                    merge_provenance(&mut provenance, elements.get(&index).cloned());
                } else {
                    for element_provenance in elements.values() {
                        merge_provenance(&mut provenance, Some(element_provenance.clone()));
                    }
                }
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    pub(in crate::typecheck) fn merge(&mut self, other: &ValueProvenance) {
        match (&mut *self, other) {
            (Self::Independent, other) => *self = other.clone(),
            (_, Self::Independent) => {}
            (Self::Origins(origins), Self::Origins(other_origins)) => {
                for origin in other_origins {
                    if !origins.contains(origin) {
                        origins.push(origin.clone());
                    }
                }
            }
            (
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
                Self::Aggregate {
                    fallback: other_fallback,
                    fields: other_fields,
                    elements: other_elements,
                },
            ) => {
                merge_boxed_provenance(fallback, other_fallback.as_deref().cloned());
                for (field, other_field_provenance) in other_fields {
                    fields
                        .entry(field.clone())
                        .and_modify(|field_provenance| {
                            field_provenance.merge(other_field_provenance)
                        })
                        .or_insert_with(|| other_field_provenance.clone());
                }
                for (index, other_element_provenance) in other_elements {
                    elements
                        .entry(*index)
                        .and_modify(|element_provenance| {
                            element_provenance.merge(other_element_provenance)
                        })
                        .or_insert_with(|| other_element_provenance.clone());
                }
            }
            (
                Self::Fallible { success, error },
                Self::Fallible {
                    success: other_success,
                    error: other_error,
                },
            ) => {
                merge_boxed_provenance(success, other_success.as_deref().cloned());
                merge_boxed_provenance(error, other_error.as_deref().cloned());
            }
            (Self::Fallible { success, .. }, other) => {
                merge_boxed_provenance(success, Some(other.clone()));
            }
            (existing, Self::Fallible { success, error }) => {
                let mut merged_success = success.as_deref().cloned();
                merge_provenance(&mut merged_success, Some(existing.clone()));
                *existing = Self::Fallible {
                    success: merged_success.map(Box::new),
                    error: error.clone(),
                };
            }
            (Self::Aggregate { fallback, .. }, other) => {
                merge_boxed_provenance(fallback, Some(other.clone()));
            }
            (
                existing,
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
            ) => {
                let mut merged_fallback = fallback.as_deref().cloned();
                merge_provenance(&mut merged_fallback, Some(existing.clone()));
                *existing = Self::Aggregate {
                    fallback: merged_fallback.map(Box::new),
                    fields: fields.clone(),
                    elements: elements.clone(),
                };
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::typecheck) struct LexicalRegionTree {
    parents: HashMap<RegionId, Option<RegionId>>,
}

impl LexicalRegionTree {
    pub(in crate::typecheck) fn define(&mut self, region: RegionId, parent: Option<RegionId>) {
        self.parents.insert(region, parent);
    }

    pub(in crate::typecheck) fn is_same_or_nested_within(
        &self,
        candidate: RegionId,
        ancestor: RegionId,
    ) -> bool {
        let mut current = Some(candidate);
        while let Some(region) = current {
            if region == ancestor {
                return true;
            }
            current = self.parents.get(&region).copied().flatten();
        }
        false
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::typecheck) struct ProvenanceEnvironment {
    bindings: HashMap<ByteSpan, ValueProvenance>,
    known_bindings: HashSet<ByteSpan>,
}

impl ProvenanceEnvironment {
    pub(in crate::typecheck) fn get(&self, binding: ByteSpan) -> Option<&ValueProvenance> {
        self.bindings.get(&binding)
    }

    pub(in crate::typecheck) fn define_binding(
        &mut self,
        binding: ByteSpan,
        contains_storage: bool,
        provenance: Option<ValueProvenance>,
    ) {
        self.known_bindings.insert(binding);
        if contains_storage {
            if let Some(provenance) = provenance {
                self.bindings.insert(binding, provenance);
            } else {
                self.bindings.remove(&binding);
            }
        } else {
            self.bindings.remove(&binding);
        }
    }

    pub(in crate::typecheck) fn join_reachable(&mut self, states: &[ProvenanceEnvironment]) {
        let mut joined = HashMap::new();
        let mut known = self.known_bindings.clone();
        for state in states {
            known.extend(state.known_bindings.iter().copied());
            for (binding, provenance) in &state.bindings {
                joined
                    .entry(*binding)
                    .and_modify(|existing: &mut ValueProvenance| existing.merge(provenance))
                    .or_insert_with(|| provenance.clone());
            }
        }
        self.bindings = joined;
        self.known_bindings = known;
    }

    pub(in crate::typecheck) fn update_existing_from(&mut self, state: &ProvenanceEnvironment) {
        for binding in self.known_bindings.clone() {
            if let Some(next) = state.bindings.get(&binding) {
                self.bindings.insert(binding, next.clone());
            } else {
                self.bindings.remove(&binding);
            }
        }
    }

    pub(in crate::typecheck) fn first_existing_binding_with_region<'a>(
        &self,
        state: &'a ProvenanceEnvironment,
        region: RegionId,
    ) -> Option<(ByteSpan, &'a str)> {
        self.known_bindings.iter().find_map(|binding| {
            state
                .bindings
                .get(binding)?
                .first_region_origin(|candidate| candidate == region)
                .map(|(_, description)| (*binding, description))
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::typecheck) struct CallableProvenanceSummary {
    result: Option<ValueProvenance>,
    needs_current_allocation_context: bool,
}

impl CallableProvenanceSummary {
    pub(in crate::typecheck) fn result(&self) -> Option<&ValueProvenance> {
        self.result.as_ref()
    }

    pub(in crate::typecheck) fn needs_current_allocation_context(&self) -> bool {
        self.needs_current_allocation_context
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::typecheck) struct CallableProvenanceSummaries {
    entries: HashMap<CallableId, CallableProvenanceSummary>,
}

impl CallableProvenanceSummaries {
    pub(in crate::typecheck) fn insert_result(
        &mut self,
        callable: CallableId,
        result: ValueProvenance,
    ) {
        self.entries.entry(callable).or_default().result = Some(result);
    }

    pub(in crate::typecheck) fn get(
        &self,
        callable: CallableId,
    ) -> Option<&CallableProvenanceSummary> {
        self.entries.get(&callable)
    }

    pub(in crate::typecheck) fn result(&self, callable: CallableId) -> Option<&ValueProvenance> {
        self.get(callable)
            .and_then(CallableProvenanceSummary::result)
    }

    pub(in crate::typecheck) fn set_needs_current_allocation_context(
        &mut self,
        callable: CallableId,
    ) {
        self.entries
            .entry(callable)
            .or_default()
            .needs_current_allocation_context = true;
    }

    pub(in crate::typecheck) fn needs_current_allocation_context(
        &self,
        callable: CallableId,
    ) -> bool {
        self.get(callable)
            .is_some_and(CallableProvenanceSummary::needs_current_allocation_context)
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::typecheck) struct ProvenanceFlow {
    value: Option<ValueProvenance>,
    fallible_error: Option<ValueProvenance>,
}

impl ProvenanceFlow {
    pub(in crate::typecheck) fn merge_value(&mut self, provenance: Option<ValueProvenance>) {
        merge_provenance(&mut self.value, provenance);
    }

    pub(in crate::typecheck) fn merge_fallible_error(
        &mut self,
        provenance: Option<ValueProvenance>,
    ) {
        merge_provenance(&mut self.fallible_error, provenance);
    }

    pub(in crate::typecheck) fn into_return_provenance(
        self,
        return_type: &Type,
    ) -> Option<ValueProvenance> {
        if matches!(return_type, Type::Fallible { .. }) {
            return fallible_provenance(self.value, self.fallible_error);
        }
        self.value
    }
}

pub(in crate::typecheck) fn fallible_provenance(
    success: Option<ValueProvenance>,
    error: Option<ValueProvenance>,
) -> Option<ValueProvenance> {
    if success.is_none() && error.is_none() {
        return None;
    }
    Some(ValueProvenance::Fallible {
        success: success.map(Box::new),
        error: error.map(Box::new),
    })
}

pub(in crate::typecheck) fn merge_provenance(
    provenance: &mut Option<ValueProvenance>,
    next: Option<ValueProvenance>,
) {
    let Some(next) = next else {
        return;
    };
    if let Some(existing) = provenance {
        existing.merge(&next);
    } else {
        *provenance = Some(next);
    }
}

fn merge_boxed_provenance(
    provenance: &mut Option<Box<ValueProvenance>>,
    next: Option<ValueProvenance>,
) {
    let mut unboxed = provenance.take().map(|provenance| *provenance);
    merge_provenance(&mut unboxed, next);
    *provenance = unboxed.map(Box::new);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn span(start: usize) -> ByteSpan {
        ByteSpan::new(SourceId::new(0), start, start + 1)
    }

    #[test]
    fn joins_distinct_input_identities_without_using_names() {
        let first = InputId::declared_at(span(1));
        let second = InputId::declared_at(span(2));
        let mut provenance = ValueProvenance::input(first);
        provenance.merge(&ValueProvenance::input(second));

        assert_eq!(
            provenance,
            ValueProvenance::Origins(vec![
                StorageOrigin::Input(first),
                StorageOrigin::Input(second)
            ])
        );
    }

    #[test]
    fn preserves_static_and_shorter_origins_at_a_join() {
        let mut provenance = ValueProvenance::static_storage();
        provenance.merge(&ValueProvenance::scope(
            span(3),
            "local binding `value`".into(),
        ));

        assert_eq!(provenance.escaping_source(), Some("local binding `value`"));
        assert!(matches!(
            provenance,
            ValueProvenance::Origins(ref origins) if origins.len() == 2
        ));
    }

    #[test]
    fn callable_summary_keeps_result_and_effect_as_separate_facts() {
        let callable = CallableId::declared_at(span(4));
        let mut summaries = CallableProvenanceSummaries::default();
        summaries.insert_result(callable, ValueProvenance::static_storage());

        let summary = summaries.get(callable).expect("summary");
        assert_eq!(summary.result(), Some(&ValueProvenance::static_storage()));
        assert!(!summary.needs_current_allocation_context);
    }

    #[test]
    fn callable_result_reports_input_origins_through_aggregates() {
        let first = InputId::declared_at(span(5));
        let second = InputId::declared_at(span(6));
        let provenance = ValueProvenance::Aggregate {
            fallback: Some(Box::new(ValueProvenance::input(first))),
            fields: BTreeMap::from([("value".into(), ValueProvenance::input(second))]),
            elements: BTreeMap::new(),
        };

        assert_eq!(provenance.input_origins(), vec![first, second]);
    }

    #[test]
    fn region_tree_finds_transitive_children_by_declaration_identity() {
        let outer = RegionId::declared_at(span(10));
        let inner = RegionId::declared_at(span(11));
        let sibling = RegionId::declared_at(span(12));
        let mut tree = LexicalRegionTree::default();
        tree.define(outer, None);
        tree.define(inner, Some(outer));
        tree.define(sibling, None);

        assert!(tree.is_same_or_nested_within(inner, outer));
        assert!(!tree.is_same_or_nested_within(sibling, outer));
    }

    #[test]
    fn finds_region_origin_inside_aggregate_projection() {
        let region = RegionId::declared_at(span(20));
        let provenance = ValueProvenance::Aggregate {
            fallback: None,
            fields: BTreeMap::from([(
                "text".into(),
                ValueProvenance::region(region, "region `temp`".into()),
            )]),
            elements: BTreeMap::new(),
        };

        assert_eq!(
            provenance.first_region_origin(|candidate| candidate == region),
            Some((region, "region `temp`"))
        );
    }
}
