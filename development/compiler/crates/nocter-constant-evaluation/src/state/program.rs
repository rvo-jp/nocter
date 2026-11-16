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
    pub(super) types: Vec<Type>,
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
            types: Into::<Box<[Type]>>::into(types).into_vec(),
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
        let contracts = extension.contracts.keys().copied().collect::<Vec<_>>();
        let plans = extension.plans.keys().copied().collect::<Vec<_>>();
        let statics = extension.statics.keys().copied().collect::<Vec<_>>();
        if contracts.iter().any(|key| self.contracts.contains_key(key))
            || plans.iter().any(|key| self.plans.contains_key(key))
            || statics.iter().any(|key| self.statics.contains_key(key))
        {
            return Err(Rule::InvalidPlan);
        }
        let first_type = self.types.len();
        self.types.extend(extension.types);
        self.contracts.extend(extension.contracts);
        self.plans.extend(
            extension
                .plans
                .into_iter()
                .map(|(key, plan)| (key, Arc::new(plan))),
        );
        self.statics.extend(extension.statics);
        let result = validation::validate_added(
            self,
            first_type,
            contracts.iter().copied(),
            plans.iter().copied(),
            statics.iter().copied(),
        );
        if result.is_err() {
            // Preflight excluded every collision, so rollback removes only this batch.
            self.types.truncate(first_type);
            for key in contracts {
                self.contracts.remove(&key);
            }
            for key in plans {
                self.plans.remove(&key);
            }
            for key in statics {
                self.statics.remove(&key);
            }
        }
        result
    }
}

/// A deferred plan failure with an optional original operation in the requested body.
/// The request itself retains the complete code specialization; no plan need exist yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanFailure {
    pub rule: Rule,
    pub site: Option<super::Site>,
}
impl From<Rule> for PlanFailure {
    fn from(rule: Rule) -> Self {
        Self { rule, site: None }
    }
}
/// Supply only already selected immutable code/metadata, never semantic lookup or body checking.
/// The executor validates and atomically installs every batch before entering its requested body.
pub trait PlanSource {
    /// # Errors
    /// Reports preparation limits or invalid/unavailable matched code.
    fn load(&mut self, code: CodeInstance) -> Result<ProgramExtension, PlanFailure>;
}
