//! Concrete logical ownership schedules over the checked CFG. No layout, ABI or native admission.
use super::{Environment, EnvironmentExit, Instance, InstanceProgram};
use nocter_checking::value_access::{
    accepted::DataEvidence,
    flow::{
        Location, Operation,
        resources::{Condition, Payload, Step as ResourceStep},
    },
};
use nocter_model::AccessInstanceId;
use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::Arc,
};

/// The adapter retains this part of its received carrier while the body borrows it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentPart {
    CodeToken,
    Capture,
}

/// Resource operations retain their selected identity and exact concrete payload obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    Resource(ResourceStep),
    /// End the adapter-owned carrier and dispose its selected payload exactly once.
    /// Normal return only, after the callee protects its answer and before caller receipt.
    DisposeEnvironment {
        argument: usize,
        part: EnvironmentPart,
        payload: Payload,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternative {
    condition: Condition,
    before: Box<[Step]>,
    after: Box<[Step]>,
}
impl Alternative {
    #[must_use]
    pub const fn condition(&self) -> &Condition {
        &self.condition
    }
    #[must_use]
    pub fn before(&self) -> &[Step] {
        &self.before
    }
    #[must_use]
    pub fn after(&self) -> &[Step] {
        &self.after
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    alternatives: Box<[Alternative]>,
}
impl Action {
    #[must_use]
    pub fn alternatives(&self) -> &[Alternative] {
        &self.alternatives
    }
}

/// Locations refer to the same instance's checked graph, not a separately reconstructed CFG.
#[derive(Clone, Debug)]
pub struct Body {
    actions: BTreeMap<Location, Action>,
}
impl Body {
    #[must_use]
    pub const fn actions(&self) -> &BTreeMap<Location, Action> {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidInstanceProgram,
    /// Ownership bits alone do not distinguish different selected schedules at this boundary.
    /// A later realization must retain more checked control evidence instead of guessing.
    UnresolvedResourceChoice {
        instance: AccessInstanceId,
        at: Location,
    },
}

/// A complete logical lifecycle schedule for one exact closed instance program.
/// This neither admits a machine representation nor executes the program.
#[derive(Clone, Debug)]
pub struct Program {
    instances: Arc<InstanceProgram>,
    bodies: BTreeMap<AccessInstanceId, Body>,
}
impl Program {
    /// Freeze concrete, guarded before/after operations without repeating source decisions.
    /// # Errors
    /// Rejects missing checked evidence or ownership guards that do not distinguish schedules.
    /// No partial program is published.
    pub fn prepare(instances: Arc<InstanceProgram>) -> Result<Self, Failure> {
        let bodies = lower(&instances)?;
        Ok(Self { instances, bodies })
    }
    #[must_use]
    pub fn instance_program(&self) -> &InstanceProgram {
        &self.instances
    }
    #[must_use]
    pub const fn bodies(&self) -> &BTreeMap<AccessInstanceId, Body> {
        &self.bodies
    }
}

const INVALID: Failure = Failure::InvalidInstanceProgram;

fn lower(program: &InstanceProgram) -> Result<BTreeMap<AccessInstanceId, Body>, Failure> {
    let mut bodies = BTreeMap::new();
    for (id, instance) in program.instances().iter() {
        let checked = program.body(id).ok_or(INVALID)?;
        let mut actions = BTreeMap::new();
        for (point, block) in checked.graph().points().iter() {
            if !checked.initialization().reachable(point) {
                continue;
            }
            for (action, event) in block.actions.iter().enumerate() {
                let at = Location { point, action };
                let completion = completion(instance, &event.operation)?;
                let mut alternatives = BTreeMap::new();
                for transition in checked.access().resources().at(at) {
                    let condition = transition.condition();
                    let before = project(instance, transition.before_steps())?;
                    let mut after = completion.clone().into_iter().collect::<Vec<_>>();
                    after.extend(project(instance, transition.after_steps())?);
                    let candidate = (before.into_boxed_slice(), after.into_boxed_slice());
                    match alternatives.entry(condition) {
                        Entry::Occupied(previous) if previous.get() != &candidate => {
                            return Err(Failure::UnresolvedResourceChoice { instance: id, at });
                        }
                        Entry::Occupied(_) => {}
                        Entry::Vacant(entry) => {
                            entry.insert(candidate);
                        }
                    }
                }
                if alternatives.is_empty() {
                    return Err(INVALID);
                }
                actions.insert(
                    at,
                    Action {
                        alternatives: alternatives
                            .into_iter()
                            .map(|(condition, (before, after))| Alternative {
                                condition,
                                before,
                                after,
                            })
                            .collect(),
                    },
                );
            }
        }
        bodies.insert(id, Body { actions });
    }
    Ok(bodies)
}

fn project(instance: &Instance, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
    steps
        .iter()
        .map(|step| {
            step.try_map_payloads(|source| {
                instance
                    .data()
                    .get(&source.data())
                    .and_then(DataEvidence::payload)
                    .ok_or(INVALID)
            })
            .map(Step::Resource)
        })
        .collect()
}

fn completion(instance: &Instance, operation: &Operation) -> Result<Option<Step>, Failure> {
    let Operation::CommitCall(call) = operation else {
        return Ok(None);
    };
    let selected = instance.calls().get(call).ok_or(INVALID)?;
    let (argument, part, exit) = match selected.environment() {
        Environment::None => return Ok(None),
        Environment::CodeToken { argument, exit, .. } => {
            (argument, EnvironmentPart::CodeToken, exit)
        }
        Environment::Capture { argument, exit, .. } => (argument, EnvironmentPart::Capture, exit),
    };
    Ok(match exit {
        EnvironmentExit::Retained | EnvironmentExit::BodyOwned => None,
        EnvironmentExit::DisposeBeforeResult(payload) => Some(Step::DisposeEnvironment {
            argument,
            part,
            payload,
        }),
    })
}
