//! Referent permissions and logical ownership over the same finite alternative worlds.
//! Full-language dependencies and executable target realization remain outside this fragment.
mod effects;
mod liveness;
mod projection;
mod state;
#[cfg(test)]
mod tests;

use super::{Graph, Initialization, IntegrityError, Location, Terminator, resources::Holder};
use crate::value_access::Query as TypedQuery;
pub use liveness::{Carrier, Liveness};
use nocter_model::{AccessFlowPointId, AccessLocalId, FieldPath, PathRelation, ValueAccessMode};
pub use projection::AccessDiagnostic;
use state::World;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Root {
    Static(nocter_model::ValueStatic),
    Owned(AccessLocalId),
    Input(AccessLocalId),
    Expression(nocter_model::AccessExpressionId),
    Temporary {
        call: nocter_model::AccessExpressionId,
        index: usize,
    },
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// An original logical address region. Unknown coordinates are not equality, bounds or
/// byte-offset evidence; an observed region alone grants no access or relocation permission.
pub struct Region {
    pub root: Root,
    pub fields: AccessPath,
}
pub use crate::value_access::{AccessPath, IndexFact};
impl Root {
    fn region(self) -> Region {
        Region {
            root: self,
            fields: AccessPath::default(),
        }
    }
}
impl Region {
    fn project(&mut self, fields: &FieldPath) {
        self.fields = self
            .fields
            .concat(&crate::value_access::paths::known(fields));
    }
    fn inside_payload(&self, carrier: &Self) -> bool {
        self.root == carrier.root
            && [true, false].into_iter().any(|primary| {
                let mut payload = carrier.clone();
                payload.project(&FieldPath::new([nocter_model::ValueField::Variant(
                    primary,
                )]));
                matches!(
                    crate::value_access::paths::relation(&payload.fields, &self.fields),
                    PathRelation::Equal | PathRelation::Ancestor
                )
            })
    }
    fn overlaps(&self, other: &Self) -> bool {
        self.root == other.root
            && crate::value_access::paths::relation(&self.fields, &other.fields)
                != PathRelation::Disjoint
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Creation {
    Entry(AccessLocalId),
    Action(Location),
}
/// Retaining a selected place protects its identity without activating a read/edit grant.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionKind {
    Place,
    /// A retained active tag: payload access remains delegated to the parent authority.
    Tag,
    Access(ValueAccessMode),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedAccess {
    pub created: Creation,
    pub kind: RetentionKind,
    pub required: Option<Location>,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessRule {
    ConflictingRead,
    ConflictingUpdate,
    ConflictingTransfer,
    ReservedModification,
    ConflictingArguments,
    ExpiredRoot,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Violation {
    pub location: Location,
    pub rule: AccessRule,
    pub root: Root,
    pub fields: AccessPath,
    pub retained: Option<RetainedAccess>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    BlockedByInitialization,
    Analyzed,
}
/// Referent and concrete first-fragment resource evidence; whole-unit closure is separate.
#[derive(Clone, Debug)]
pub struct Analysis {
    status: Status,
    liveness: Liveness,
    violations: Box<[Violation]>,
    resources: super::resources::Report,
    retained_regions: BTreeMap<Location, RetainedRegions>,
}
#[derive(Clone, Debug, Default)]
pub struct RetainedRegions {
    before: BTreeSet<Region>,
    after: BTreeSet<Region>,
}
impl RetainedRegions {
    #[must_use]
    pub fn before(&self) -> impl ExactSizeIterator<Item = &Region> {
        self.before.iter()
    }
    #[must_use]
    pub fn after(&self) -> impl ExactSizeIterator<Item = &Region> {
        self.after.iter()
    }
}
impl Analysis {
    /// No retained address or reservation names this root on either side of this action.
    /// This grants neither initialized storage nor permission to relocate a partial payload.
    #[must_use]
    pub fn root_unretained(&self, at: Location, root: Root) -> bool {
        self.retained_regions.get(&at).is_some_and(|regions| {
            !regions
                .before
                .iter()
                .chain(&regions.after)
                .any(|region| region.root == root)
        })
    }
    /// No address/reservation for this root survives normal completion of the original action.
    /// A consumer must separately discharge any address used by the operation itself.
    #[must_use]
    pub fn root_unretained_after(&self, at: Location, root: Root) -> bool {
        self.retained_regions
            .get(&at)
            .is_some_and(|regions| !regions.after.iter().any(|region| region.root == root))
    }
    /// Union of original node/reservation regions on each side of an analyzed action.
    /// Missing evidence proves nothing. These paths do not establish stable physical geometry.
    #[must_use]
    pub fn retained_regions(&self, at: Location) -> Option<&RetainedRegions> {
        self.retained_regions.get(&at)
    }
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }
    #[must_use]
    pub const fn liveness(&self) -> &Liveness {
        &self.liveness
    }
    #[must_use]
    pub const fn violations(&self) -> &[Violation] {
        &self.violations
    }
    #[must_use]
    pub const fn resources(&self) -> &super::resources::Report {
        &self.resources
    }
}

pub(super) fn analyze(
    typed: &TypedQuery,
    graph: &Graph,
    initialization: &Initialization,
    numeric: &super::numeric::Analysis,
) -> Result<Analysis, IntegrityError> {
    let liveness = liveness::analyze(typed, graph, initialization)?;
    if !initialization.violations().is_empty() {
        return Ok(Analysis {
            status: Status::BlockedByInitialization,
            liveness,
            violations: Box::new([]),
            resources: super::resources::Report::default(),
            retained_regions: BTreeMap::new(),
        });
    }
    let entry = World::entry(typed)?;
    let mut resources = super::resources::Report::new(entry.resources.condition());
    let mut pending = BTreeSet::from([(graph.entry(), entry)]);
    let mut entries: BTreeMap<_, BTreeSet<World>> = BTreeMap::new();
    let mut violations = BTreeSet::new();
    let mut retained_regions: BTreeMap<Location, RetainedRegions> = BTreeMap::new();
    while let Some((point, mut world)) = pending.pop_first() {
        world.trim(liveness.entry(point))?;
        if !entries.entry(point).or_default().insert(world.clone()) {
            continue;
        }
        let block = graph.points().get(point).ok_or(INVALID)?;
        let mut cursor = numeric.cursor(typed, point);
        let mut worlds = vec![world];
        for (action, event) in block.actions.iter().enumerate() {
            let location = Location { point, action };
            let facts = cursor.next(&event.operation)?;
            let mut next = Vec::new();
            for mut world in worlds {
                world.trim(liveness.before(location))?;
                let retained = world.retained_regions();
                let before = world.resources.clone();
                match effects::apply(
                    typed,
                    &mut world,
                    &event.operation,
                    location,
                    &liveness,
                    numeric,
                    facts.as_ref(),
                ) {
                    Ok(mut alternatives) => {
                        for (alternative, plan) in &mut alternatives {
                            alternative.trim(liveness.after(location))?;
                            let regions = retained_regions.entry(location).or_default();
                            regions.before.extend(retained.iter().cloned());
                            regions.after.extend(alternative.retained_regions());
                            resources.record(location, &before, plan, &alternative.resources);
                            next.push(alternative.clone());
                        }
                    }
                    Err(effects::Failure::Violation(violation)) => {
                        violations.insert(violation);
                    }
                    Err(effects::Failure::Integrity) => return Err(INVALID),
                }
            }
            worlds = next;
        }
        for world in worlds {
            if block.terminator == Terminator::Return {
                let returned = if let Some(evidence) = &typed.result_evidence {
                    world
                        .resources
                        .returned_with(typed.result_data(), |actual, expected| {
                            evidence.matches_payload_metadata(typed.program(), actual, expected)
                        })
                } else {
                    world
                        .resources
                        .returned(typed.program(), typed.result_data())
                };
                if !returned {
                    return Err(INVALID);
                }
            } else {
                queue_successors(
                    typed,
                    initialization,
                    &block.terminator,
                    &world,
                    &mut pending,
                )?;
            }
        }
    }
    Ok(Analysis {
        status: Status::Analyzed,
        liveness,
        violations: violations.into_iter().collect(),
        resources,
        retained_regions,
    })
}

fn queue_successors(
    typed: &TypedQuery,
    initialization: &Initialization,
    terminator: &Terminator,
    world: &World,
    pending: &mut BTreeSet<(AccessFlowPointId, World)>,
) -> Result<(), IntegrityError> {
    if let Terminator::EnumSwitch {
        source,
        cases,
        fallback,
    } = terminator
    {
        let carrier = Holder::Expression(*source);
        for &(tag, successor) in cases.iter() {
            if initialization.reachable(successor) {
                let mut selected = world.clone();
                selected
                    .resources
                    .refine_enum_case(typed.program(), carrier, tag)?;
                pending.insert((successor, selected));
            }
        }
        if initialization.reachable(*fallback) {
            pending.insert((*fallback, world.clone()));
        }
    } else {
        pending.extend(
            terminator
                .successors()
                .into_iter()
                .filter(|&next| initialization.reachable(next))
                .map(|next| (next, world.clone())),
        );
    }
    Ok(())
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
