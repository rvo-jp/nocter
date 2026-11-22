//! Concrete lifecycle schedules and initialization bits. No payload layout, ABI or native admission.
pub mod flags;
mod payloads;
use super::{Environment, EnvironmentExit, Instance, InstanceProgram, Payload};
use nocter_checking::value_access::accepted::{FutureCaptureCases, FutureInputCleanup};
use nocter_checking::value_access::flow::{
    Location, Operation,
    resources::{
        ActivationInput, Condition, Holder, Payload as CheckedPayload, Step as ResourceStep,
    },
};
use nocter_declarations::value_access::FunctionInput;
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
    Resource(Resource),
    ScopedReplacement(ScopedReplacement),
    /// Move the represented inputs and selector between two whole future carriers.
    MoveLocalFuture {
        source: Holder,
        destination: Holder,
        selector: bool,
        captures: FutureCaptureCases<FunctionInput>,
    },
    /// Dispose the selected local activation's owned inputs without invoking its body.
    CancelLocalFuture {
        source: Holder,
        captures: FutureCaptureCases<FutureInputCleanup>,
    },
    /// Install prepared inputs in the newly committed local activation.
    CaptureFutureInputs {
        destination: Holder,
        inputs: Box<[ActivationInput<CheckedPayload>]>,
    },
    /// End the adapter-owned carrier and dispose its selected payload exactly once.
    /// Normal return only, after the callee protects its answer and before caller receipt.
    DisposeEnvironment {
        argument: usize,
        part: EnvironmentPart,
        carrier: Payload,
        payload: Payload,
    },
}
/// Original data projection and the selected owning destination's entry-state holes.
/// This is frozen evidence from the existing ledger, not another ownership analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    data: nocter_checking::value_access::ResourceDataStep,
    destination_missing: Option<Arc<[nocter_checking::value_access::flow::resources::Missing]>>,
}
impl Resource {
    #[must_use]
    pub const fn data(&self) -> &nocter_checking::value_access::ResourceDataStep {
        &self.data
    }
    #[must_use]
    pub fn operation(&self) -> &ResourceStep<Payload> {
        self.data.operation()
    }
    #[must_use]
    pub fn destination_missing(
        &self,
    ) -> Option<&[nocter_checking::value_access::flow::resources::Missing]> {
        self.destination_missing.as_deref()
    }
}
impl Step {
    /// The original resource operation, independently of its retained representation metadata.
    #[must_use]
    pub fn resource(&self) -> Option<&ResourceStep<Payload>> {
        match self {
            Self::Resource(step) => Some(step.operation()),
            Self::ScopedReplacement(replacement) => Some(replacement.operation()),
            Self::MoveLocalFuture { .. }
            | Self::CancelLocalFuture { .. }
            | Self::CaptureFutureInputs { .. }
            | Self::DisposeEnvironment { .. } => None,
        }
    }
}
/// A matched replacement operation with separate original reference-root contracts.
/// These roots do not certify enclosing allocations or stable field geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedReplacement {
    operation: Box<nocter_checking::value_access::ResourceDataStep>,
    before: nocter_declarations::value_access::DataInstanceKey,
    after: nocter_declarations::value_access::DataInstanceKey,
    carriers: nocter_checking::value_access::ReferentCarrierContracts,
    carrier_missing: BTreeMap<
        nocter_checking::value_access::flow::resources::Holder,
        Arc<[nocter_checking::value_access::flow::resources::Missing]>,
    >,
}
impl ScopedReplacement {
    /// Original owning carrier exclusions equal at all three transition boundaries.
    /// None proves no stable carrier; an empty slice identifies a complete owning carrier.
    #[must_use]
    pub fn carrier_missing(
        &self,
        holder: nocter_checking::value_access::flow::resources::Holder,
    ) -> Option<&[nocter_checking::value_access::flow::resources::Missing]> {
        self.carrier_missing.get(&holder).map(AsRef::as_ref)
    }
    #[must_use]
    pub const fn carriers(&self) -> &nocter_checking::value_access::ReferentCarrierContracts {
        &self.carriers
    }
    #[must_use]
    pub fn operation(&self) -> &ResourceStep<Payload> {
        self.operation.operation()
    }
    #[must_use]
    pub const fn data(&self) -> &nocter_checking::value_access::ResourceDataStep {
        &self.operation
    }
    #[must_use]
    pub const fn root_before(&self) -> &nocter_declarations::value_access::DataInstanceKey {
        &self.before
    }
    #[must_use]
    pub const fn root_after(&self) -> &nocter_declarations::value_access::DataInstanceKey {
        &self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternative {
    before: Box<[Step]>,
    after: Box<[Step]>,
    flags: flags::Recipe,
}
impl Alternative {
    #[must_use]
    pub fn before(&self) -> &[Step] {
        &self.before
    }
    #[must_use]
    pub fn after(&self) -> &[Step] {
        &self.after
    }
    #[must_use]
    pub const fn flags(&self) -> &flags::Recipe {
        &self.flags
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
    flags: flags::Layout,
}
impl Body {
    #[must_use]
    pub const fn actions(&self) -> &BTreeMap<Location, Action> {
        &self.actions
    }
    #[must_use]
    pub const fn flags(&self) -> &flags::Layout {
        &self.flags
    }
}

/// Checking snapshots survive only until their physical bit recipes have been frozen.
#[derive(Eq, PartialEq)]
struct Draft {
    condition: Condition,
    operation_state: Condition,
    final_state: Condition,
    before: Box<[Step]>,
    after: Box<[Step]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidInstanceProgram,
    Payload {
        instance: AccessInstanceId,
        at: Location,
        failure: nocter_checking::value_access::InstantiationFailure,
    },
    /// Ownership bits alone do not distinguish different selected schedules at this boundary.
    /// A later realization must retain more checked control evidence instead of guessing.
    UnresolvedResourceChoice {
        instance: AccessInstanceId,
        at: Location,
    },
}

/// Lifecycle schedules and activation-local initialization recipes for one exact instance program.
/// This neither selects payload layouts nor executes the program.
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
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            nocter_constant_evaluation::CompileTimeEvaluationLimits::default().steps(),
        );
        Self::prepare_with_budget(instances, &mut work)
    }
    /// Specialize the original transition payloads under one deterministic metadata-work quota.
    /// # Errors
    /// Rejects ambiguous/missing value contexts and work exhaustion without publishing a draft.
    pub fn prepare_with_budget(
        instances: Arc<InstanceProgram>,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<Self, Failure> {
        let bodies = lower(&instances, work)?;
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
    /// Actual contextual payload roots used by the frozen schedules, without a new source grant.
    /// Includes adapter-owned environments as well as ordinary resource obligations.
    pub fn payload_instances(
        &self,
    ) -> impl Iterator<Item = nocter_declarations::value_access::DataInstanceKey> {
        let mut keys = std::collections::BTreeSet::new();
        for body in self.bodies.values() {
            for action in body.actions.values() {
                for alternative in &action.alternatives {
                    for step in alternative.before.iter().chain(alternative.after.iter()) {
                        let resource = match step {
                            Step::Resource(step) => {
                                keys.extend(step.data().roots().cloned());
                                Some(step.operation())
                            }
                            Step::ScopedReplacement(replacement) => {
                                keys.extend(replacement.data().roots().cloned());
                                keys.insert(replacement.before.clone());
                                keys.insert(replacement.after.clone());
                                keys.extend(
                                    replacement
                                        .carriers
                                        .before()
                                        .iter()
                                        .chain(replacement.carriers.after().into_iter().flatten())
                                        .map(|carrier| carrier.instance().clone()),
                                );
                                Some(replacement.operation())
                            }
                            Step::DisposeEnvironment {
                                carrier, payload, ..
                            } => {
                                keys.insert(carrier.instance().clone());
                                keys.insert(payload.instance().clone());
                                None
                            }
                            Step::MoveLocalFuture { .. }
                            | Step::CancelLocalFuture { .. }
                            | Step::CaptureFutureInputs { .. } => None,
                        };
                        if let Some(resource) = resource {
                            let result = resource.try_map_payloads::<std::convert::Infallible, _>(
                                |payload| {
                                    keys.insert(payload.instance().clone());
                                    Ok(())
                                },
                            );
                            match result {
                                Ok(_) => {}
                                Err(never) => match never {},
                            }
                        }
                    }
                }
            }
        }
        keys.into_iter()
    }
}

const INVALID: Failure = Failure::InvalidInstanceProgram;

fn lower(
    program: &InstanceProgram,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<BTreeMap<AccessInstanceId, Body>, Failure> {
    let mut bodies = BTreeMap::new();
    for (id, instance) in program.instances().iter() {
        let checked = program.body(id).ok_or(INVALID)?;
        let mut actions = BTreeMap::new();
        for (point, block) in checked.graph().points().iter() {
            if !instance.control().reachable(point) {
                continue;
            }
            for (action, event) in block.actions.iter().enumerate() {
                let at = Location { point, action };
                let completion = completion(instance, &event.operation)?;
                let mut alternatives = BTreeMap::new();
                for transition in checked.access().resources().at(at) {
                    let condition = transition.condition();
                    let mut projection =
                        payloads::Projection::new(instance, id, at, transition, work)?;
                    let before = projection.steps(transition.before_steps())?;
                    let mut after = completion.clone().into_iter().collect::<Vec<_>>();
                    after.extend(projection.after_steps(transition.after_steps())?);
                    let candidate = Draft {
                        condition: condition.clone(),
                        operation_state: transition.operation_condition().clone(),
                        final_state: transition.after_condition(),
                        before: before.into(),
                        after: after.into(),
                    };
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
                actions.insert(at, alternatives.into_values().collect());
            }
        }
        let entry = checked
            .access()
            .resources()
            .entry_condition()
            .ok_or(INVALID)?;
        bodies.insert(id, flags::lower(actions, entry)?);
    }
    Ok(bodies)
}

fn completion(instance: &Instance, operation: &Operation) -> Result<Option<Step>, Failure> {
    let Operation::CommitCall(call) = operation else {
        return Ok(None);
    };
    let selected = instance.calls().get(call).ok_or(INVALID)?;
    let (argument, part, exit) = match selected.environment() {
        Environment::None | Environment::Selected { .. } => return Ok(None),
        Environment::CodeToken { argument, exit, .. } => {
            (argument, EnvironmentPart::CodeToken, exit)
        }
        Environment::Capture { argument, exit, .. } => (argument, EnvironmentPart::Capture, exit),
    };
    Ok(match exit {
        EnvironmentExit::Retained | EnvironmentExit::BodyOwned => None,
        EnvironmentExit::DisposeBeforeResult { carrier, payload } => {
            Some(Step::DisposeEnvironment {
                argument,
                part,
                carrier,
                payload,
            })
        }
    })
}
