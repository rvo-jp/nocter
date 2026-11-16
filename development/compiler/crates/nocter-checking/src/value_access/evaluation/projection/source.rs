//! Demand-driven realization of frozen checked bodies, separate from call-boundary linking.
use super::{Builder, Types};
use crate::value_access::flow;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
use nocter_constant_evaluation::state as plan;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::AccessCodeId;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Instances {
    ids: BTreeMap<CodeInstanceKey, plan::CodeInstance>,
    keys: BTreeMap<plan::CodeInstance, CodeInstanceKey>,
    pending: Vec<CodeInstanceKey>,
    boundaries: BTreeMap<CodeInstanceKey, crate::value_access::results::CallBoundary>,
}
impl Instances {
    pub(super) fn boundary(
        &mut self,
        checked: &crate::value_access::accepted::Checked,
        frame: &CodeInstanceKey,
        work: &mut plan::WorkBudget,
    ) -> super::Result<&crate::value_access::results::CallBoundary> {
        use std::collections::btree_map::Entry;
        Ok(match self.boundaries.entry(frame.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(checked.instantiate_call_boundary(frame, work)?),
        })
    }
    pub(super) fn intern(&mut self, key: CodeInstanceKey) -> Result<plan::CodeInstance> {
        if let Some(&id) = self.ids.get(&key) {
            return Ok(id);
        }
        let id = plan::CodeInstance {
            source: key.code(),
            instance: u32::try_from(self.ids.len()).map_err(|_| INVALID)?,
        };
        self.keys.insert(id, key.clone());
        self.pending.push(key.clone());
        self.ids.insert(key, id);
        Ok(id)
    }
}

pub(super) fn initializer(program: &Program, code: nocter_model::ValueCode) -> Result<bool> {
    Ok(match code {
        nocter_model::ValueCode::Declaration(id) => program
            .codes()
            .get(id)
            .ok_or(INVALID)?
            .kind()
            .is_initializer(),
        nocter_model::ValueCode::Expression(_) => true,
        nocter_model::ValueCode::Closure(_) => false,
    })
}
/// Retains the exact checked products; loading code can only project frozen operations.
pub(in crate::value_access::evaluation) struct Source<'a, 'work, F> {
    checked: &'a crate::value_access::accepted::Checked,
    conditions: crate::value_access::accepted::ConstConditions,
    values: F,
    instances: Instances,
    types: Types,
    published_types: usize,
    statics: BTreeSet<nocter_model::ValueStatic>,
    work: &'work mut plan::WorkBudget,
}
impl<'a, 'work, F: Fn(AccessCodeId) -> Option<plan::Value>> Source<'a, 'work, F> {
    pub(super) fn prepare(
        checked: &'a crate::value_access::accepted::Checked,
        root: nocter_model::ValueCodeRoot,
        conditions: crate::value_access::accepted::ConstConditions,
        values: F,
        work: &'work mut plan::WorkBudget,
    ) -> super::Result<(plan::Program, Self)> {
        let mut source = Self {
            checked,
            conditions,
            values,
            instances: Instances::default(),
            types: Types::default(),
            published_types: 0,
            statics: BTreeSet::new(),
            work,
        };
        source.instances.intern(
            checked
                .program
                .bind_code(root.code(), [])
                .map_err(|_| INVALID)?,
        )?;
        let mut initial = plan::ProgramExtension::default();
        source.contracts(&mut initial)?;
        let program = plan::Program::linked(
            initial.types,
            initial.contracts,
            initial.plans,
            initial.statics,
        )
        .map_err(|_| INVALID)?;
        Ok((program, source))
    }
    pub(in crate::value_access::evaluation) fn condition_failure(
        &self,
        instance: plan::CodeInstance,
        site: plan::Site,
    ) -> Option<(CodeInstanceKey, crate::value_access::constant::Violation)> {
        let frame = self.instances.keys.get(&instance)?;
        if frame.code() != site.code {
            return None;
        }
        let condition = self.conditions.get(frame)?.iter().find(|condition| {
            condition.cause.point == site.point && condition.cause.action == site.action
        })?;
        Some((frame.clone(), condition.violation))
    }
    fn contracts(&mut self, extension: &mut plan::ProgramExtension) -> super::Result<()> {
        let program = &self.checked.program;
        let mut contracts = BTreeMap::new();
        for key in self.instances.pending.clone() {
            let id = *self.instances.ids.get(&key).ok_or(INVALID)?;
            let boundary = self.instances.boundary(self.checked, &key, self.work)?;
            let contract = plan::CallContract {
                inputs: boundary
                    .inputs
                    .iter()
                    .map(|input| {
                        Ok(plan::InputContract {
                            data: self.types.get_instance(program, input.data.clone())?,
                            operation: input.operation,
                        })
                    })
                    .collect::<super::Result<_>>()?,
                result: self.types.get_instance(program, boundary.result.clone())?,
                initializer: initializer(program, key.code())?,
                environment_backing: boundary.environment_backing,
            };
            if extension.contracts.contains_key(&id) || contracts.insert(id, contract).is_some() {
                return Err(INVALID.into());
            }
        }
        extension.contracts.extend(contracts);
        extension.types = self.types.appended(self.published_types);
        self.published_types = self.types.len();
        self.instances.pending.clear();
        Ok(())
    }
    fn realize(&mut self, id: plan::CodeInstance) -> super::Result<plan::ProgramExtension> {
        let key = self.instances.keys.get(&id).ok_or(INVALID)?.clone();
        let query = self.checked.code(key.code()).ok_or(INVALID)?;
        let program = &self.checked.program;
        let prepared = self.checked.instantiate_prepared_data(&key, self.work)?;
        let values = self.checked.instantiate_value_data_map(&key, self.work)?;
        let control = self.checked.instantiate_control(&key, self.work)?;
        let conditions = self.conditions.get(&key).into_iter().flatten().map(|condition| {
            let action = query.graph().points().get(condition.cause.point)
                .and_then(|point| point.actions.get(condition.cause.action)).ok_or(INVALID)?;
            if !matches!(action.operation, flow::Operation::CommitCall(expression) if expression == condition.expression) {
                return Err(super::Failure::Integrity);
            }
            Ok(plan::InputCondition {
                input: condition.input,
                fields: condition.fields.clone(),
                cause: plan::Site { code: key.code(), point: condition.cause.point, action: condition.cause.action },
            })
        }).collect::<super::Result<_>>()?;
        let body = Builder {
            checked: self.checked,
            work: self.work,
            query,
            program,
            instance: &key,
            instances: &mut self.instances,
            types: &mut self.types,
            slots: Vec::new(),
            keys: BTreeMap::new(),
            invocations: BTreeMap::new(),
            prepared,
            values,
            resources: BTreeMap::new(),
            control,
            conditions,
        }
        .build()?;
        let mut extension = plan::ProgramExtension::default();
        for &(_, id) in &body.statics {
            if self.statics.contains(&id) {
                continue;
            }
            let data = program.static_declaration(id).ok_or(INVALID)?.result();
            let value = (self.values)(id.initializer).ok_or(INVALID)?;
            extension.statics.insert(
                id,
                plan::Static {
                    data: self.types.get(program, data)?,
                    value,
                },
            );
        }
        extension.plans.insert(id, body);
        self.contracts(&mut extension)?;
        self.statics.extend(extension.statics.keys().copied());
        Ok(extension)
    }
}
impl<F: Fn(AccessCodeId) -> Option<plan::Value>> plan::PlanSource for Source<'_, '_, F> {
    fn load(
        &mut self,
        code: plan::CodeInstance,
    ) -> std::result::Result<plan::ProgramExtension, plan::Rule> {
        self.work.step().map_err(|_| plan::Rule::PlanWorkLimit)?;
        let key = self
            .instances
            .keys
            .get(&code)
            .ok_or(plan::Rule::InvalidPlan)?;
        if self.checked.code(key.code()).is_none() {
            return Err(plan::Rule::UnavailablePlan);
        }
        self.realize(code).map_err(|failure| match failure {
            super::Failure::WorkLimit => plan::Rule::PlanWorkLimit,
            super::Failure::Integrity => plan::Rule::InvalidPlan,
            super::Failure::UnresolvedContext => plan::Rule::UnresolvedContext,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    #[test]
    fn failed_static_lookup_does_not_mark_an_unpublished_value_as_linked() {
        use plan::PlanSource as _;
        use std::cell::Cell;
        let checked =
            super::super::tests::checked("static BASE: i32 = 42\nconst func run(): i32 { BASE }");
        let root = checked
            .program
            .resolve_code(checked.program.symbols().get("run").unwrap())
            .unwrap();
        let ready = Cell::new(false);
        let lookups = Cell::new(0);
        let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
        let (mut program, mut source) = Source::prepare(
            &checked,
            root.into(),
            BTreeMap::new(),
            |_| {
                lookups.set(lookups.get() + 1);
                ready.get().then(|| plan::Value::integer(42))
            },
            &mut work,
        )
        .unwrap();
        assert!(matches!(
            source.load(root.into()),
            Err(plan::Rule::InvalidPlan)
        ));
        assert!(source.statics.is_empty());
        ready.set(true);
        let retry = source.load(root.into()).unwrap();
        assert_eq!(lookups.get(), 2);
        assert_eq!(retry.statics.len(), 1);
        assert_eq!(source.statics.len(), 1);
        program.extend(retry).unwrap();
    }

    #[test]
    fn failed_boundary_batch_retains_pending_contracts_for_a_complete_retry() {
        let checked = super::super::tests::checked(
            "const func root(): void {}
const func first(): void {}
const func second(value: i32): i32 { value }",
        );
        let code = |name| {
            checked
                .program
                .resolve_code(checked.program.symbols().get(name).unwrap())
                .unwrap()
        };
        let frame = |name| checked.program.bind_code(code(name).into(), []).unwrap();
        let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
        let (mut program, mut source) = Source::prepare(
            &checked,
            code("root").into(),
            BTreeMap::new(),
            |_| None,
            &mut work,
        )
        .unwrap();
        let first = frame("first");
        source.instances.intern(first.clone()).unwrap();
        source.instances.intern(frame("second")).unwrap();
        source
            .instances
            .boundary(&checked, &first, source.work)
            .unwrap();
        let pending = source.instances.pending.clone();
        let published_types = source.published_types;
        *source.work = plan::WorkBudget::new(NonZeroU64::new(1).unwrap());
        let mut failed = plan::ProgramExtension::default();
        assert!(matches!(
            source.contracts(&mut failed),
            Err(super::super::Failure::WorkLimit)
        ));
        assert!(failed.contracts.is_empty());
        assert_eq!(source.instances.pending, pending);
        assert_eq!(source.published_types, published_types);
        *source.work = plan::WorkBudget::new(plan::Limits::default().steps());
        let mut retry = plan::ProgramExtension::default();
        source.contracts(&mut retry).unwrap();
        assert_eq!(retry.contracts.len(), 2);
        assert!(source.instances.pending.is_empty());
        program.extend(retry).unwrap();
    }
}
