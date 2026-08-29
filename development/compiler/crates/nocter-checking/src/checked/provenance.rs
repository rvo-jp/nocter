use std::collections::{BTreeMap, BTreeSet};

use nocter_declarations::{CallableProvenance, ProvenanceOrigin};
use nocter_model::{
    Arena, BodyId, BodyNodeId, BodyScopeId, CallableId, CaptureId, ClosureId, FieldId,
    LocalBindingId, ParameterId, ParameterOrigin, ResultProvenance, VariantId,
};

/// One storage authority carried by a checked value.
///
/// Caller-visible inputs and compiler-owned ambient allocation remain distinct. The remaining
/// variants identify storage that cannot cross its corresponding semantic boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProvenanceSource {
    Callable(ProvenanceOrigin),
    CurrentAllocation,
    Local(LocalBindingId),
    OwnedParameter(ParameterId),
    Region(LocalBindingId),
    /// Storage owned by an ordinary expression temporary until statement end.
    StatementTemporary(BodyNodeId),
    /// Storage owned by a retained expression temporary until its lexical scope ends.
    ScopedTemporary {
        value: BodyNodeId,
        scope: BodyScopeId,
    },
    ClosureParameter {
        closure: ClosureId,
        origin: ParameterOrigin,
    },
    ClosureCaptureValue {
        closure: ClosureId,
        capture: CaptureId,
    },
    ClosureEnvironment(ClosureId),
    Unknown,
}

/// A semantic projection within a value that can carry storage independently of its siblings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProvenanceProjection {
    Field(FieldId),
    VariantPayload {
        variant: VariantId,
        parameter: ParameterId,
    },
    Element,
    OutcomeValue,
    OutcomeFailure,
    ClosureCaptureValue(CaptureId),
    ClosureCaptureStorage(CaptureId),
}

/// Field-sensitive storage provenance for one checked value.
///
/// Sources on the root apply to every projection. Child entries retain independently constructed
/// aggregate components so selecting one field does not acquire unrelated sibling origins.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ValueProvenance {
    sources: BTreeSet<ProvenanceSource>,
    projections: BTreeMap<ProvenanceProjection, ValueProvenance>,
}

impl ValueProvenance {
    #[must_use]
    pub fn independent() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_source(source: ProvenanceSource) -> Self {
        Self {
            sources: BTreeSet::from([source]),
            projections: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn from_projection(projection: ProvenanceProjection, value: ValueProvenance) -> Self {
        Self {
            sources: BTreeSet::new(),
            projections: BTreeMap::from([(projection, value)]),
        }
    }

    #[must_use]
    pub fn direct_sources(&self) -> impl ExactSizeIterator<Item = ProvenanceSource> + '_ {
        self.sources.iter().copied()
    }

    #[must_use]
    pub const fn projections(&self) -> &BTreeMap<ProvenanceProjection, ValueProvenance> {
        &self.projections
    }

    #[must_use]
    pub fn all_sources(&self) -> BTreeSet<ProvenanceSource> {
        let mut sources = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            sources.extend(value.sources.iter().copied());
            pending.extend(value.projections.values());
        }
        sources
    }

    /// Erases field-sensitive shape while retaining every storage authority carried by a value.
    ///
    /// Callable provenance contracts name input origins, not a structural mapping between input
    /// and result projections. Values crossing such a boundary must therefore become root-wide:
    /// retaining an input projection tree would invent a stronger contract and would give loops
    /// an unbounded provenance domain when a result is fed back into a later call.
    #[must_use]
    pub(crate) fn flattened(&self) -> Self {
        Self {
            sources: self.all_sources(),
            projections: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn projected(&self, projection: ProvenanceProjection) -> Self {
        let mut result = Self {
            sources: self.sources.clone(),
            projections: BTreeMap::new(),
        };
        if let Some(child) = self.projections.get(&projection) {
            result.union_with(child);
        }
        result
    }

    pub(crate) fn union_with(&mut self, other: &Self) -> bool {
        let old_sources = self.sources.len();
        self.sources.extend(other.sources.iter().copied());
        let mut changed = self.sources.len() != old_sources;
        for (projection, child) in &other.projections {
            if let Some(existing) = self.projections.get_mut(projection) {
                changed |= existing.union_with(child);
            } else {
                self.projections.insert(*projection, child.clone());
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn insert_projection(
        &mut self,
        projection: ProvenanceProjection,
        value: ValueProvenance,
    ) {
        self.projections
            .entry(projection)
            .and_modify(|existing| {
                existing.union_with(&value);
            })
            .or_insert(value);
    }

    pub(crate) fn replace_projection(
        &mut self,
        path: &[ProvenanceProjection],
        value: ValueProvenance,
    ) {
        let Some((first, remaining)) = path.split_first() else {
            *self = value;
            return;
        };
        self.projections
            .entry(*first)
            .or_default()
            .replace_projection(remaining, value);
    }

    pub(crate) fn remove_projection(&mut self, path: &[ProvenanceProjection]) {
        let Some((first, remaining)) = path.split_first() else {
            *self = Self::independent();
            return;
        };
        if remaining.is_empty() {
            self.projections.remove(first);
        } else if let Some(child) = self.projections.get_mut(first) {
            child.remove_projection(remaining);
        }
    }
}

/// Whether a callable result can retain the compiler-owned current allocation context.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AmbientStorageDependence {
    #[default]
    Independent,
    Current,
    Unknown,
}

/// The effective checked result-storage contract of one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableProvenance {
    origins: CallableProvenance,
    ambient: AmbientStorageDependence,
}

impl CheckedCallableProvenance {
    pub(crate) const fn new(
        origins: CallableProvenance,
        ambient: AmbientStorageDependence,
    ) -> Self {
        Self { origins, ambient }
    }

    #[must_use]
    pub const fn origins(&self) -> &[ProvenanceOrigin] {
        self.origins.origins()
    }

    #[must_use]
    pub const fn ambient(&self) -> AmbientStorageDependence {
        self.ambient
    }
}

/// Program-wide callable result-provenance authority.
#[derive(Clone, Debug)]
pub struct CallableProvenanceTable {
    entries: Arena<CallableId, CheckedCallableProvenance>,
}

/// The effective result-storage contract inferred for one generated closure body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedClosureProvenance {
    parameters: ResultProvenance,
    captures: Box<[CaptureId]>,
    environment: bool,
    ambient: AmbientStorageDependence,
}

impl CheckedClosureProvenance {
    pub(crate) fn new(
        parameters: ResultProvenance,
        captures: impl Into<Box<[CaptureId]>>,
        environment: bool,
        ambient: AmbientStorageDependence,
    ) -> Self {
        Self {
            parameters,
            captures: captures.into(),
            environment,
            ambient,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &ResultProvenance {
        &self.parameters
    }

    #[must_use]
    pub const fn captures(&self) -> &[CaptureId] {
        &self.captures
    }

    #[must_use]
    pub const fn retains_environment(&self) -> bool {
        self.environment
    }

    #[must_use]
    pub const fn ambient(&self) -> AmbientStorageDependence {
        self.ambient
    }
}

/// Program-wide generated-closure result-provenance authority.
#[derive(Clone, Debug)]
pub struct ClosureProvenanceTable {
    entries: Arena<ClosureId, CheckedClosureProvenance>,
}

impl ClosureProvenanceTable {
    pub(crate) const fn new(entries: Arena<ClosureId, CheckedClosureProvenance>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn get(&self, closure: ClosureId) -> Option<&CheckedClosureProvenance> {
        self.entries.get(closure)
    }

    #[must_use]
    pub const fn entries(&self) -> &Arena<ClosureId, CheckedClosureProvenance> {
        &self.entries
    }
}

impl CallableProvenanceTable {
    pub(crate) const fn new(entries: Arena<CallableId, CheckedCallableProvenance>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn get(&self, callable: CallableId) -> Option<&CheckedCallableProvenance> {
        self.entries.get(callable)
    }

    #[must_use]
    pub const fn entries(&self) -> &Arena<CallableId, CheckedCallableProvenance> {
        &self.entries
    }
}

/// Provenance retained for every node and every normally returned value of one body.
#[derive(Clone, Debug)]
pub struct CheckedBodyProvenance {
    nodes: Arena<BodyNodeId, ValueProvenance>,
    returned: ValueProvenance,
}

impl CheckedBodyProvenance {
    pub(crate) const fn new(
        nodes: Arena<BodyNodeId, ValueProvenance>,
        returned: ValueProvenance,
    ) -> Self {
        Self { nodes, returned }
    }

    #[must_use]
    pub const fn nodes(&self) -> &Arena<BodyNodeId, ValueProvenance> {
        &self.nodes
    }

    #[must_use]
    pub const fn returned(&self) -> &ValueProvenance {
        &self.returned
    }
}

/// Dense body-provenance authority paired with the callable contract table.
#[derive(Clone, Debug)]
pub struct ProvenanceTable {
    callables: CallableProvenanceTable,
    closures: ClosureProvenanceTable,
    bodies: Arena<BodyId, CheckedBodyProvenance>,
}

impl ProvenanceTable {
    pub(crate) const fn new(
        callables: CallableProvenanceTable,
        closures: ClosureProvenanceTable,
        bodies: Arena<BodyId, CheckedBodyProvenance>,
    ) -> Self {
        Self {
            callables,
            closures,
            bodies,
        }
    }

    #[must_use]
    pub const fn callables(&self) -> &CallableProvenanceTable {
        &self.callables
    }

    #[must_use]
    pub const fn closures(&self) -> &ClosureProvenanceTable {
        &self.closures
    }

    #[must_use]
    pub fn body(&self, body: BodyId) -> Option<&CheckedBodyProvenance> {
        self.bodies.get(body)
    }

    #[must_use]
    pub const fn bodies(&self) -> &Arena<BodyId, CheckedBodyProvenance> {
        &self.bodies
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, FieldId, LocalBindingId};

    use super::{ProvenanceProjection, ProvenanceSource, ValueProvenance};

    #[test]
    fn projection_does_not_acquire_sibling_origins() {
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let left = fields.insert(());
        let right = fields.insert(());
        let _ = fields.finish();
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let first = locals.insert(());
        let second = locals.insert(());
        let _ = locals.finish();
        let mut aggregate = ValueProvenance::independent();
        aggregate.insert_projection(
            ProvenanceProjection::Field(left),
            ValueProvenance::from_source(ProvenanceSource::Local(first)),
        );
        aggregate.insert_projection(
            ProvenanceProjection::Field(right),
            ValueProvenance::from_source(ProvenanceSource::Local(second)),
        );

        assert_eq!(
            aggregate
                .projected(ProvenanceProjection::Field(left))
                .all_sources(),
            std::collections::BTreeSet::from([ProvenanceSource::Local(first)])
        );
    }

    #[test]
    fn root_origins_apply_to_every_projection() {
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = fields.insert(());
        let _ = fields.finish();
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let local = locals.insert(());
        let _ = locals.finish();
        let root = ValueProvenance::from_source(ProvenanceSource::Local(local));

        assert_eq!(
            root.projected(ProvenanceProjection::Field(field))
                .all_sources(),
            std::collections::BTreeSet::from([ProvenanceSource::Local(local)])
        );
    }

    #[test]
    fn callable_boundary_flattening_erases_projection_shape() {
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = fields.insert(());
        let _ = fields.finish();
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let local = locals.insert(());
        let _ = locals.finish();
        let projected = ValueProvenance::from_projection(
            ProvenanceProjection::Field(field),
            ValueProvenance::from_source(ProvenanceSource::Local(local)),
        );

        let flattened = projected.flattened();

        assert!(flattened.projections().is_empty());
        assert_eq!(
            flattened.direct_sources().collect::<Vec<_>>(),
            vec![ProvenanceSource::Local(local)]
        );
    }
}
