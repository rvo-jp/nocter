//! Private MIR enters the ordinary Machine program, body optimizer and native consumers.
use nocter_runtime_contract::RuntimeDataId;
mod body;
mod cleanup;
mod data;
mod primitives;
mod transport;
use super::ExecutableFailure as Failure;
use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineCallableAbi, MachineFunctionId, MachineLayoutPlan, MachineProgram,
    MachineValueCallableAbi,
};
use nocter_mir::value_access as mir;
use nocter_model::AccessInstanceId;
use nocter_runtime_contract::RuntimeType;
use std::collections::BTreeMap;

struct Context<'a> {
    program: &'a mir::Program,
    layouts: &'a MachineLayoutPlan,
    data: &'a data::Data,
    functions: BTreeMap<AccessInstanceId, MachineFunctionId>,
    abis: BTreeMap<AccessInstanceId, MachineCallableAbi>,
    cleanup: BTreeMap<RuntimeDataId, MachineFunctionId>,
    transports: transport::Prepared,
    primitives: BTreeMap<nocter_runtime_contract::PrimitiveRole, crate::MachinePrimitiveTarget>,
}

fn prepare_layouts(program: &mir::Program) -> Result<MachineLayoutPlan, Failure> {
    let roots = program
        .bodies()
        .values()
        .flat_map(|body| {
            body.slots()
                .iter()
                .flat_map(|(_, slot)| slot.representations.iter().copied())
                .chain(body.signature().ordered().map(|input| input.data))
                .chain(std::iter::once(body.signature().result()))
                .chain(
                    body.geometry()
                        .iter()
                        .flat_map(|pair| [pair.before, pair.after]),
                )
        })
        .chain(
            program
                .runtime()
                .types()
                .iter()
                .filter_map(|(data, shape)| {
                    matches!(shape, RuntimeType::Primitive(_)).then_some(data)
                }),
        );
    let layouts =
        MachineLayoutPlan::from_runtime(program.runtime(), roots).map_err(Failure::Layout)?;
    for (&id, body) in program.bodies() {
        for pair in body.geometry() {
            let valid = match &pair.requirement {
                mir::GeometryRequirement::Complete => {
                    layouts.has_same_geometry(pair.before, pair.after)
                }
                mir::GeometryRequirement::ProjectionOffsets(paths) => {
                    !paths.is_empty()
                        && paths.iter().all(|path| {
                            layouts.has_same_projection_offset(pair.before, pair.after, path)
                        })
                }
            };
            if !valid {
                return Err(Failure::InvalidBody(id));
            }
        }
    }
    Ok(layouts)
}

pub(super) fn program(program: &mir::Program) -> Result<MachineProgram, Failure> {
    let source_root = program
        .bodies()
        .get(&program.root())
        .ok_or(Failure::InvalidRoot)?;
    let result = source_root.signature().result();
    if !source_root.inputs().is_empty() || !body::valid_process_result(program, result) {
        return Err(Failure::InvalidRoot);
    }
    let layouts = prepare_layouts(program)?;
    let functions = program
        .bodies()
        .keys()
        .enumerate()
        .map(|(index, &id)| (id, MachineFunctionId::new(index)))
        .collect();
    let abis = program
        .bodies()
        .iter()
        .map(|(&id, body)| {
            Ok((
                id,
                MachineValueCallableAbi::build(body.signature().clone(), &layouts)
                    .map_err(Failure::Abi)?
                    .transport()
                    .clone(),
            ))
        })
        .collect::<Result<_, Failure>>()?;
    let data = data::Data::build(program, &layouts)?;
    let (primitives, runtime_call_abis) = primitives::prepare(program, &layouts)?;
    let cleanup = cleanup_plan(program)?;
    let transports = transport::prepare(program, &layouts, program.bodies().len() + cleanup.len())?;
    let context = Context {
        data: &data,
        program,
        layouts: &layouts,
        functions,
        abis,
        cleanup,
        transports,
        primitives,
    };
    let mut values = Vec::new();
    for (&id, source) in program.bodies() {
        values.push(body::source(&context, id, source)?);
    }
    for (&data, &id) in &context.cleanup {
        values.push(body::cleanup(&context, data, id)?);
    }
    for (&pair, helper) in &context.transports.helpers {
        values.push(body::transport(&context, pair, helper)?);
    }
    let entry = *context
        .functions
        .get(&program.root())
        .ok_or(Failure::InvalidRoot)?;
    let process = MachineFunctionId::new(values.len());
    values.push(body::process(&context, process, entry, result)?);
    let functions = MachineTable::from_values(values);
    let contexts = crate::MachineContextPlans::build(&functions).map_err(Failure::Context)?;
    Ok(MachineProgram::new(crate::program::MachineProgramParts {
        layouts: layouts.finish(),
        runtime_call_abis,
        imports: crate::import::MachineImportTable::empty(),
        data: data.finish(),
        contexts,
        functions,
        root: crate::MachineProgramRoot::Process {
            root: process,
            entry,
        },
    }))
}

fn effects(action: &mir::Action) -> impl Iterator<Item = &mir::Effect> {
    action
        .alternatives
        .iter()
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .chain(&action.call_bindings)
        .chain(action.operations.iter().filter_map(|operation| {
            if let mir::Operation::Storage(effect) = operation {
                Some(effect)
            } else {
                None
            }
        }))
}

fn cleanup_plan(
    program: &mir::Program,
) -> Result<BTreeMap<RuntimeDataId, MachineFunctionId>, Failure> {
    let mut work = BTreeMap::new();
    for action in program
        .bodies()
        .values()
        .flat_map(|body| body.actions().values())
    {
        for effect in effects(action) {
            match effect {
                mir::Effect::Dispose { source, disposal }
                    if *disposal != mir::Disposal::Trivial =>
                {
                    cleanup::required(program.runtime(), source.data, &mut work)?;
                }
                mir::Effect::SelectedFutureCleanup { cases, .. } => {
                    for input in cases.iter().flat_map(|case| &case.fields) {
                        cleanup::required(program.runtime(), input.source.data, &mut work)?;
                    }
                }
                mir::Effect::DisposeArrayRemainder { source, .. } => {
                    cleanup::required(program.runtime(), source.data, &mut work)?;
                }
                mir::Effect::DisposeArrayElements { source, .. } => {
                    let (element, _) = program
                        .runtime()
                        .types()
                        .get(source.data)
                        .and_then(RuntimeType::array)
                        .ok_or(Failure::InvalidRoot)?;
                    cleanup::required(program.runtime(), element, &mut work)?;
                }
                mir::Effect::Dispose { .. }
                | mir::Effect::VariantTag { .. }
                | mir::Effect::Initialize(_)
                | mir::Effect::Transfer { .. }
                | mir::Effect::SelectedFutureMove { .. }
                | mir::Effect::Repack { .. }
                | mir::Effect::Transport { .. }
                | mir::Effect::Reinterpret { .. }
                | mir::Effect::ConsolidateArrayPrefix { .. }
                | mir::Effect::End(_)
                | mir::Effect::BindAddress { .. } => {}
            }
        }
    }
    Ok(work
        .into_iter()
        .filter(|(_, needed)| *needed)
        .enumerate()
        .map(|(index, (data, _))| (data, MachineFunctionId::new(program.bodies().len() + index)))
        .collect())
}
