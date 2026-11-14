//! Demand-driven realization of frozen checked bodies, separate from call-boundary linking.
use super::{Builder, Types};
use crate::value_access::flow;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
use nocter_constant_evaluation::state as plan;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{AccessCodeId, ValueInputOperation};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Instances {
    ids: BTreeMap<CodeInstanceKey, plan::CodeInstance>,
    keys: BTreeMap<plan::CodeInstance, CodeInstanceKey>,
    pending: Vec<CodeInstanceKey>,
}
impl Instances {
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
    values: F,
    instances: Instances,
    types: Types,
    published_types: usize,
    statics: BTreeSet<nocter_model::ValueStatic>,
    work: &'work mut plan::WorkBudget,
}
impl<'a, 'work, F: Fn(AccessCodeId) -> Option<plan::Value>> Source<'a, 'work, F> {
    pub(in crate::value_access::evaluation) fn prepare(
        checked: &'a crate::value_access::accepted::Checked,
        root: nocter_model::ValueCodeRoot,
        values: F,
        work: &'work mut plan::WorkBudget,
    ) -> super::Result<(plan::Program, Self)> {
        let mut source = Self {
            checked,
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
    fn contracts(&mut self, extension: &mut plan::ProgramExtension) -> super::Result<()> {
        let program = &self.checked.program;
        for key in self.instances.pending.drain(..) {
            let id = *self.instances.ids.get(&key).ok_or(INVALID)?;
            let query = self.checked.code(key.code()).ok_or(INVALID)?;
            let body = query.typed().body();
            let inputs = body
                .inputs()
                .ordered()
                .map(|input| {
                    let data = self
                        .checked
                        .instantiate_input_data(&key, input.local, self.work)?;
                    Ok(plan::InputContract {
                        data: self.types.get_instance(program, data)?,
                        operation: input.operation,
                    })
                })
                .collect::<super::Result<_>>()?;
            let contract = plan::CallContract {
                inputs,
                result: self.types.get_instance(
                    program,
                    self.checked.instantiate_result_contract(&key, self.work)?,
                )?,
                initializer: initializer(program, key.code())?,
                environment_backing: body
                    .inputs()
                    .environment()
                    .is_some_and(|input| input.operation != ValueInputOperation::Take),
            };
            if extension.contracts.insert(id, contract).is_some() {
                return Err(INVALID.into());
            }
        }
        extension.types = self.types.appended(self.published_types);
        self.published_types = self.types.len();
        Ok(())
    }
    fn realize(&mut self, id: plan::CodeInstance) -> super::Result<plan::ProgramExtension> {
        let key = self.instances.keys.get(&id).ok_or(INVALID)?.clone();
        let query = self.checked.code(key.code()).ok_or(INVALID)?;
        let program = &self.checked.program;
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
        }
        .build()?;
        let mut extension = plan::ProgramExtension::default();
        for &(_, id) in &body.statics {
            if !self.statics.insert(id) {
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
        Ok(extension)
    }
}
impl<F: Fn(AccessCodeId) -> Option<plan::Value>> plan::PlanSource for Source<'_, '_, F> {
    fn load(
        &mut self,
        code: plan::CodeInstance,
    ) -> std::result::Result<plan::ProgramExtension, plan::Rule> {
        self.work.step().map_err(|_| plan::Rule::PlanWorkLimit)?;
        self.realize(code).map_err(|failure| match failure {
            super::Failure::WorkLimit => plan::Rule::PlanWorkLimit,
            super::Failure::Integrity => plan::Rule::InvalidPlan,
        })
    }
}
