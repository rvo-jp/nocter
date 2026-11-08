//! Referent/permission analysis. Opaque carried payload and executable cleanup remain separate.
mod effects;
mod liveness;
mod projection;
mod state;
#[cfg(test)]
mod tests;

use super::{Graph, Initialization, IntegrityError, Location, Terminator};
use crate::value_access::Query as TypedQuery;
pub use liveness::{Carrier, Liveness};
use nocter_model::{AccessLocalId, AccessParameterId, ValueAccessMode};
pub use projection::AccessDiagnostic;
use state::World;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Root {
    Owned(AccessLocalId),
    Input(AccessParameterId),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Creation {
    Parameter(AccessLocalId),
    Action(Location),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequiredUse {
    Action(Location),
    StagedResult(Location),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedAccess {
    pub created: Creation,
    pub permission: ValueAccessMode,
    pub required: Option<RequiredUse>,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessRule {
    ConflictingRead,
    ConflictingUpdate,
    ConflictingTransfer,
    ReservedModification,
    ConflictingArguments,
    InvalidReturnRoot,
    ExpiredRoot,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Violation {
    pub location: Location,
    pub rule: AccessRule,
    pub root: Root,
    pub retained: Option<RetainedAccess>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    BlockedByInitialization,
    Analyzed,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReturnOrigin {
    NoReferent,
    Input(AccessParameterId),
}

/// Referent/access evidence, not an accepted program or payload/destructor proof.
#[derive(Clone, Debug)]
pub struct Analysis {
    status: Status,
    liveness: Liveness,
    violations: Box<[Violation]>,
    returns: Box<[ReturnOrigin]>,
}
impl Analysis {
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
    pub const fn returns(&self) -> &[ReturnOrigin] {
        &self.returns
    }
}

pub(super) fn analyze(
    typed: &TypedQuery,
    graph: &Graph,
    initialization: &Initialization,
) -> Result<Analysis, IntegrityError> {
    let liveness = liveness::analyze(typed, graph, initialization)?;
    if !initialization.violations().is_empty() {
        return Ok(Analysis {
            status: Status::BlockedByInitialization,
            liveness,
            violations: Box::new([]),
            returns: Box::new([]),
        });
    }
    let mut pending = BTreeSet::from([(graph.entry(), World::entry(typed))]);
    let mut entries: BTreeMap<_, BTreeSet<World>> = BTreeMap::new();
    let mut violations = BTreeSet::new();
    let mut returns = BTreeSet::new();
    while let Some((point, mut world)) = pending.pop_first() {
        world.trim(liveness.entry(point))?;
        if !entries.entry(point).or_default().insert(world.clone()) {
            continue;
        }
        let block = graph.points().get(point).ok_or(INVALID)?;
        let mut worlds = vec![world];
        for (action, event) in block.actions.iter().enumerate() {
            let location = Location { point, action };
            let mut next = Vec::new();
            for mut world in worlds {
                world.trim(liveness.before(location))?;
                match effects::apply(typed, &mut world, &event.operation, location, &liveness) {
                    Ok(mut alternatives) => {
                        for alternative in &mut alternatives {
                            alternative.trim(liveness.after(location))?;
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
                let staged = world.staged.as_ref().ok_or(INVALID)?;
                returns.insert(match staged.value.access {
                    None => ReturnOrigin::NoReferent,
                    Some(key) => match world.nodes.get(&key).ok_or(INVALID)?.root {
                        Root::Input(parameter) => ReturnOrigin::Input(parameter),
                        Root::Owned(_) => return Err(INVALID),
                    },
                });
            } else {
                for successor in block.terminator.successors() {
                    if initialization.reachable(successor) {
                        pending.insert((successor, world.clone()));
                    }
                }
            }
        }
    }
    Ok(Analysis {
        status: Status::Analyzed,
        liveness,
        violations: violations.into_iter().collect(),
        returns: returns.into_iter().collect(),
    })
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
