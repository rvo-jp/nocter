//! Private MIR enters the ordinary Machine program, body optimizer and native consumers.
mod body;
mod data;
mod primitives;
use super::ExecutableFailure as Failure;
use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineCallableAbi, MachineFunctionId, MachineLayoutPlan, MachineProgram,
    MachineValueCallableAbi,
};
use nocter_mir::value_access as mir;
use nocter_model::{AccessInstanceId, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeRepresentation};
use std::collections::{BTreeMap, BTreeSet};

struct Context<'a> {
    program: &'a mir::Program,
    layouts: &'a MachineLayoutPlan,
    data: &'a data::Data,
    functions: BTreeMap<AccessInstanceId, MachineFunctionId>,
    abis: BTreeMap<AccessInstanceId, MachineCallableAbi>,
    cleanup: BTreeMap<TypeId, MachineFunctionId>,
    primitives: BTreeMap<nocter_runtime_contract::PrimitiveRole, crate::MachinePrimitiveTarget>,
}

pub(super) fn program(program: &mir::Program) -> Result<MachineProgram, Failure> {
    let source_root = program
        .bodies()
        .get(&program.root())
        .ok_or(Failure::InvalidRoot)?;
    if !source_root.inputs().is_empty()
        || !matches!(
            program
                .runtime()
                .types()
                .get(source_root.signature().result()),
            Some(RuntimeType::Unit | RuntimeType::Primitive(RuntimePrimitive::Signed(32)))
        )
    {
        return Err(Failure::InvalidRoot);
    }
    let roots = program
        .bodies()
        .values()
        .flat_map(|body| {
            body.slots()
                .iter()
                .flat_map(|(_, slot)| slot.representations.iter().copied())
                .chain(body.signature().ordered().map(|input| input.data))
                .chain(std::iter::once(body.signature().result()))
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
    let context = Context {
        data: &data,
        program,
        layouts: &layouts,
        functions,
        abis,
        cleanup: cleanup_plan(program)?,
        primitives,
    };
    let mut values = Vec::new();
    for (&id, source) in program.bodies() {
        values.push(body::source(&context, id, source)?);
    }
    for (&data, &id) in &context.cleanup {
        values.push(body::cleanup(&context, data, id)?);
    }
    let entry = *context
        .functions
        .get(&program.root())
        .ok_or(Failure::InvalidRoot)?;
    let process = MachineFunctionId::new(values.len());
    values.push(body::process(
        &context,
        process,
        entry,
        source_root.signature().result(),
    )?);
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

fn cleanup_plan(program: &mir::Program) -> Result<BTreeMap<TypeId, MachineFunctionId>, Failure> {
    let mut work = BTreeMap::new();
    for action in program
        .bodies()
        .values()
        .flat_map(|body| body.actions().values())
    {
        let phases = action
            .alternatives
            .iter()
            .flat_map(|alternative| alternative.before.iter().chain(&alternative.after));
        let operations = action
            .operations
            .iter()
            .filter_map(|operation| match operation {
                mir::Operation::Storage(effect) => Some(effect),
                _ => None,
            });
        for effect in phases.chain(&action.call_bindings).chain(operations) {
            match effect {
                mir::Effect::Dispose { source, disposal }
                    if *disposal != mir::Disposal::Trivial =>
                {
                    needs_cleanup(program, source.data, &mut work)?;
                }
                mir::Effect::DisposeArrayRemainder { source, .. } => {
                    needs_cleanup(program, source.data, &mut work)?;
                }
                mir::Effect::DisposeArrayElements { source, .. } => {
                    let (element, _) = program
                        .runtime()
                        .types()
                        .get(source.data)
                        .and_then(RuntimeType::array)
                        .ok_or(Failure::InvalidRoot)?;
                    needs_cleanup(program, element, &mut work)?;
                }
                mir::Effect::Dispose { .. }
                | mir::Effect::VariantTag { .. }
                | mir::Effect::Initialize(_)
                | mir::Effect::Transfer { .. }
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

fn needs_cleanup(
    program: &mir::Program,
    data: TypeId,
    cache: &mut BTreeMap<TypeId, bool>,
) -> Result<bool, Failure> {
    let mut pending = vec![(data, false)];
    let mut active = BTreeSet::new();
    while let Some((data, expanded)) = pending.pop() {
        if cache.contains_key(&data) {
            continue;
        }
        let children = match program.runtime().types().get(data) {
            Some(RuntimeType::Primitive(RuntimePrimitive::Error) | RuntimeType::Fallible(_)) => {
                cache.insert(data, true);
                continue;
            }
            Some(
                RuntimeType::Primitive(_)
                | RuntimeType::Unit
                | RuntimeType::EmptyArray { .. }
                | RuntimeType::StaticCallable { environment: None },
            ) => {
                cache.insert(data, false);
                continue;
            }
            Some(RuntimeType::StaticCallable {
                environment: Some(environment),
            }) => vec![*environment],
            Some(RuntimeType::Optional(payload)) => vec![*payload],
            Some(RuntimeType::Aggregate) => {
                let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
                    program.runtime().type_representations().get(data)
                else {
                    return Err(Failure::InvalidRoot);
                };
                fields.iter().map(|field| field.ty()).collect()
            }
            Some(RuntimeType::FixedArray { element, length }) => {
                if *length == 0 {
                    Vec::new()
                } else {
                    vec![*element]
                }
            }
            _ => return Err(Failure::InvalidRoot),
        };
        if expanded {
            let needed = children.iter().any(|child| cache.get(child) == Some(&true));
            cache.insert(data, needed);
            active.remove(&data);
        } else {
            if !active.insert(data) {
                return Err(Failure::InvalidRoot);
            }
            pending.push((data, true));
            pending.extend(children.into_iter().rev().map(|child| (child, false)));
        }
    }
    cache.get(&data).copied().ok_or(Failure::InvalidRoot)
}
