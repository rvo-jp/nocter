//! Validated call boundaries are linked independently from immutable executable bodies.
use super::{CodeInstance, Plan, Rule, Static, Type, validation};
use nocter_model::ValueInputOperation;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputContract {
    pub data: usize,
    pub operation: ValueInputOperation,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallContract {
    pub inputs: Box<[InputContract]>,
    pub result: usize,
    pub initializer: bool,
    pub environment_backing: bool,
}
impl CallContract {
    /// Derive a boundary from a plan; ordinary validation still checks its local slots/control.
    /// # Errors
    /// Rejects an input referring outside the plan's slot table.
    pub fn from_plan(plan: &Plan) -> Result<Self, Rule> {
        Ok(Self {
            inputs: plan
                .inputs
                .iter()
                .map(|&(slot, operation)| {
                    Ok(InputContract {
                        data: plan.slots.get(slot).ok_or(Rule::InvalidPlan)?.data,
                        operation,
                    })
                })
                .collect::<Result<_, Rule>>()?,
            result: plan.result,
            initializer: plan.initializer,
            environment_backing: plan.environment_backing.is_some(),
        })
    }
}
/// An append-only batch. Type indices in this batch address the complete resulting table.
#[derive(Default)]
pub struct ProgramExtension {
    pub types: Vec<Type>,
    pub contracts: BTreeMap<CodeInstance, CallContract>,
    pub plans: BTreeMap<CodeInstance, Plan>,
    pub statics: BTreeMap<nocter_model::ValueStatic, Static>,
}
/// Validated immutable call boundaries, realized code bodies and established global data.
#[derive(Clone, Debug)]
pub struct Program {
    pub(super) types: Box<[Type]>,
    pub(super) statics: BTreeMap<nocter_model::ValueStatic, Static>,
    pub(super) contracts: BTreeMap<CodeInstance, CallContract>,
    pub(super) plans: BTreeMap<CodeInstance, Arc<Plan>>,
}
impl Program {
    /// Construct a fully realized closed program.
    /// # Errors
    /// Rejects invalid types, slots, control, call contracts or resource references.
    pub fn new(
        types: impl Into<Box<[Type]>>,
        plans: BTreeMap<CodeInstance, Plan>,
        statics: BTreeMap<nocter_model::ValueStatic, Static>,
    ) -> Result<Self, Rule> {
        let contracts = plans
            .iter()
            .map(|(&code, plan)| Ok((code, CallContract::from_plan(plan)?)))
            .collect::<Result<_, Rule>>()?;
        Self::linked(types, contracts, plans, statics)
    }
    /// Link every call boundary without requiring every executable body yet.
    /// A contract supplies no body or const capability. Any supplied body must match it exactly.
    /// # Errors
    /// Rejects invalid or unlinked boundaries and mismatched/invalid realized bodies.
    pub fn linked(
        types: impl Into<Box<[Type]>>,
        contracts: BTreeMap<CodeInstance, CallContract>,
        plans: BTreeMap<CodeInstance, Plan>,
        statics: BTreeMap<nocter_model::ValueStatic, Static>,
    ) -> Result<Self, Rule> {
        let program = Self {
            types: types.into(),
            contracts,
            statics,
            plans: plans
                .into_iter()
                .map(|(code, plan)| (code, Arc::new(plan)))
                .collect(),
        };
        validation::validate(&program)?;
        Ok(program)
    }
    /// Atomically add code and metadata; existing identities, types and bodies cannot change.
    /// # Errors
    /// Rejects duplicate identities, invalid references and any body/contract mismatch.
    pub fn extend(&mut self, extension: ProgramExtension) -> Result<(), Rule> {
        let mut candidate = self.clone();
        let mut types = candidate.types.into_vec();
        types.extend(extension.types);
        candidate.types = types.into();
        for (key, contract) in extension.contracts {
            if candidate.contracts.insert(key, contract).is_some() {
                return Err(Rule::InvalidPlan);
            }
        }
        for (key, plan) in extension.plans {
            if candidate.plans.insert(key, Arc::new(plan)).is_some() {
                return Err(Rule::InvalidPlan);
            }
        }
        for (key, value) in extension.statics {
            if candidate.statics.insert(key, value).is_some() {
                return Err(Rule::InvalidPlan);
            }
        }
        validation::validate(&candidate)?;
        *self = candidate;
        Ok(())
    }
}

/// Supply only already selected immutable code/metadata, never semantic lookup or body checking.
/// The executor validates and atomically installs every batch before entering its requested body.
pub trait PlanSource {
    /// # Errors
    /// Reports preparation limits or invalid/unavailable matched code.
    fn load(&mut self, code: CodeInstance) -> Result<ProgramExtension, Rule>;
}
