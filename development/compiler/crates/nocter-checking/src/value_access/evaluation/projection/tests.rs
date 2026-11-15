//! Actual closed source keeps its concretely projected resource contracts through publication.
use super::*;
use nocter_constant_evaluation::state::PlanSource as _;
use nocter_test_support::ValueSourceFixture;

pub(super) fn checked(text: &str) -> crate::value_access::accepted::Checked {
    checked_without(text, &[])
}
fn checked_without(text: &str, omitted: &[&str]) -> crate::value_access::accepted::Checked {
    let source = ValueSourceFixture::new(text);
    let declarations =
        nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax()).unwrap();
    let context = declarations.bind(source.syntax()).unwrap();
    let omitted = omitted
        .iter()
        .map(|name| {
            context
                .program()
                .resolve_code(context.program().symbols().get(name).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let queries = context
        .program()
        .codes()
        .iter()
        .filter(|(id, _)| !omitted.contains(id))
        .map(|(id, _)| {
            (
                id.into(),
                crate::value_access::Query::prepare(&context, id).unwrap(),
            )
        })
        .collect();
    crate::value_access::accepted::check_queries(&context, queries).unwrap()
}
fn copy_extension(extension: &plan::ProgramExtension) -> plan::ProgramExtension {
    plan::ProgramExtension {
        types: extension.types.clone(),
        contracts: extension.contracts.clone(),
        plans: extension.plans.clone(),
        statics: extension.statics.clone(),
    }
}

#[test]
fn linking_an_unavailable_body_retains_its_boundary_without_realizing_code() {
    let checked = checked_without(
        "func runtime(value: i32): i32 { value + 7 }
func run(): i32 { let callback = runtime
callback(35) }",
        &["runtime"],
    );
    let code = |name| {
        checked
            .program
            .resolve_code(checked.program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = code("run");
    let target = code("runtime");
    assert!(checked.code(target.into()).is_none());
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let (mut program, mut source) =
        Source::prepare(&checked, root.into(), |_| None, &mut work).unwrap();
    let extension = source.load(root.into()).unwrap();
    assert_eq!(extension.plans.len(), 1);
    let (&linked, contract) = extension
        .contracts
        .iter()
        .find(|(key, _)| key.source == target.into())
        .unwrap();
    assert_eq!(contract.inputs.len(), 1);
    assert!(!extension.plans.contains_key(&linked));
    program.extend(extension).unwrap();
    assert!(matches!(
        source.load(linked),
        Err(plan::Rule::UnavailablePlan)
    ));
}

#[test]
fn boundary_cache_rejects_partial_work_and_reuses_complete_metadata_without_work() {
    let checked = checked("const func root(value: i32): i32 { value }");
    let code = checked
        .program
        .resolve_code(checked.program.symbols().get("root").unwrap())
        .unwrap();
    let frame = checked.program.bind_code(code.into(), []).unwrap();
    let mut instances = Instances::default();
    let mut limited = plan::WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    assert!(matches!(
        instances.boundary(&checked, &frame, &mut limited),
        Err(Failure::WorkLimit)
    ));
    let expected = instances
        .boundary(
            &checked,
            &frame,
            &mut plan::WorkBudget::new(plan::Limits::default().steps()),
        )
        .unwrap()
        .result
        .clone();
    limited.step().unwrap_err();
    let reused = instances.boundary(&checked, &frame, &mut limited).unwrap();
    assert_eq!(reused.result, expected);
}

#[test]
fn concrete_resource_contracts_survive_projection_and_invalid_constraints_publish_nothing() {
    let checked = checked(
        "const func first(): i32 { 41 }\nconst func run(): i32 { let amount = 1\nlet callback = (amount;): i32 { first() + amount }\nlet next = callback\nnext() }",
    );
    let root = checked
        .program
        .resolve_code(checked.program.symbols().get("run").unwrap())
        .unwrap();
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let (mut program, mut source) =
        Source::prepare(&checked, root.into(), |_| None, &mut work).unwrap();
    let id = plan::CodeInstance::from(root);
    let extension = source.load(id).unwrap();
    let body = &extension.plans[&id];
    let effects = body
        .points
        .iter()
        .flat_map(|(_, point)| point.actions.iter())
        .flat_map(|action| action.alternatives.iter())
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .collect::<Vec<_>>();
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, plan::Effect::MoveRoot { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, plan::Effect::Context { .. }))
    );
    assert!(!effects.iter().any(|effect| matches!(effect, plan::Effect::Move { destination, .. } if destination.fields.is_empty())));
    let mut invalid = copy_extension(&extension);
    let body = invalid.plans.get_mut(&id).unwrap();
    let mut changed = false;
    body.points = body.points.map(|_, point| {
        let mut point = point.clone();
        for action in &mut point.actions {
            for alternative in &mut action.alternatives {
                for effect in alternative.before.iter_mut().chain(&mut alternative.after) {
                    if let plan::Effect::Context { data, .. } = effect {
                        *data = usize::MAX;
                        changed = true;
                    }
                }
            }
        }
        point
    });
    assert!(changed);
    assert!(matches!(
        program.extend(invalid),
        Err(plan::Rule::InvalidPlan)
    ));
    program.extend(extension).unwrap();
}

#[test]
fn owner_registration_retains_one_slot_and_never_relabels_its_initial_context() {
    let checked = checked(
        "const func first(): i32 { 41 }\nconst func second(): i32 { 42 }\nconst func run(): i32 { var value = first\nread view = value\nview() }",
    );
    let program = &checked.program;
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let frame = program.bind_code(code("run").into(), []).unwrap();
    let query = checked.code(frame.code()).unwrap();
    let local = |mode| {
        query
            .typed()
            .body()
            .locals()
            .iter()
            .find(|(_, local)| local.kind.access_mode() == mode)
            .unwrap()
            .0
    };
    let owner = local(ValueAccessMode::Own);
    let alias = local(ValueAccessMode::Read);
    let expression = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Function(_)))
        .unwrap()
        .0;
    let mut types = Types::default();
    let [first, second] = ["first", "second"].map(|name| {
        let key = program
            .instantiate_closed_data(program.function_data(code(name).into()).unwrap())
            .unwrap();
        types.get_instance(program, key).unwrap()
    });
    assert_ne!(first, second);
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let prepared = checked
        .instantiate_prepared_data(&frame, &mut work)
        .unwrap();
    let values = checked
        .instantiate_value_data_map(&frame, &mut work)
        .unwrap();
    let mut instances = source::Instances::default();
    let mut builder = Builder {
        control: checked.instantiate_control(&frame, &mut work).unwrap(),
        checked: &checked,
        work: &mut work,
        query,
        program,
        instance: &frame,
        instances: &mut instances,
        types: &mut types,
        slots: Vec::new(),
        keys: BTreeMap::new(),
        invocations: BTreeMap::new(),
        prepared,
        values,
        resources: BTreeMap::new(),
    };
    let key = Key::Resource(Holder::Local(owner));
    let slot = builder.slot_data(key, first, plan::Storage::Owned).unwrap();
    // Registration is metadata only: observing another possible representation neither creates
    // a second owner nor changes the entry type. A real receipt/phase constraint must justify use.
    assert_eq!(
        builder
            .slot_data(key, second, plan::Storage::Owned)
            .unwrap(),
        slot
    );
    assert_eq!(builder.slots.len(), 1);
    assert_eq!(builder.slots[slot].data, first);
    assert!(
        builder
            .slot_data(key, second, plan::Storage::Alias)
            .is_err()
    );
    let key = Key::Resource(Holder::Local(alias));
    let slot = builder.slot_data(key, first, plan::Storage::Alias).unwrap();
    assert_eq!(
        builder
            .slot_data(key, second, plan::Storage::Alias)
            .unwrap(),
        slot
    );
    assert_eq!(builder.slots[slot].data, first);
    for (key, storage) in [
        (
            Key::Resource(Holder::Expression(expression)),
            plan::Storage::Owned,
        ),
        (Key::Operand(expression, 0), plan::Storage::Auxiliary),
    ] {
        builder.slot_data(key, first, storage).unwrap();
        assert!(builder.slot_data(key, second, storage).is_err());
    }
}

#[test]
fn published_callable_input_contexts_match_the_original_body_entry_slots() {
    let checked = checked(
        "const func first(): i32 { 42 }\nconst func apply(take callback: func(): i32): i32 { callback() }\nconst func run(): i32 { apply(first) }",
    );
    let program = &checked.program;
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = code("run");
    let apply = code("apply");
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let (mut program, mut source) =
        Source::prepare(&checked, root.into(), |_| None, &mut work).unwrap();
    let extension = source.load(root.into()).unwrap();
    let (&callee, contract) = extension
        .contracts
        .iter()
        .find(|(id, _)| id.source == apply.into())
        .unwrap();
    assert_eq!(contract.inputs.len(), 1);
    let expected = contract.inputs[0];
    program.extend(extension).unwrap();
    let extension = source.load(callee).unwrap();
    let body = &extension.plans[&callee];
    assert_eq!(body.inputs.len(), 1);
    let (slot, operation) = body.inputs[0];
    assert_eq!(body.slots[slot].data, expected.data);
    assert_eq!(operation, expected.operation);
    program.extend(extension).unwrap();
}

#[test]
fn partial_writes_constrain_complete_carriers_at_both_source_boundaries() {
    let checked = checked(
        "const func run(): i32 { var pair = (0, 40)\nedit picked = pair.1\npicked = 42\npair.1 }",
    );
    let root = checked
        .program
        .resolve_code(checked.program.symbols().get("run").unwrap())
        .unwrap();
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let (mut program, mut source) =
        Source::prepare(&checked, root.into(), |_| None, &mut work).unwrap();
    let id = plan::CodeInstance::from(root);
    let extension = source.load(id).unwrap();
    let body = &extension.plans[&id];
    let pair = body
        .points
        .iter()
        .flat_map(|(_, point)| point.actions.iter())
        .flat_map(|action| action.alternatives.iter())
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .find_map(|effect| match effect {
            plan::Effect::Assemble {
                destination,
                fields,
            } if fields.len() == 2 => Some(body.slots[*destination].data),
            _ => None,
        })
        .unwrap();
    for entry in [true, false] {
        let mut invalid = copy_extension(&extension);
        let body = invalid.plans.get_mut(&id).unwrap();
        let slots = body.slots.clone();
        let mut changed = 0;
        body.points = body.points.map(|_, point| {
            let mut point = point.clone();
            for action in &mut point.actions {
                for alternative in &mut action.alternatives {
                    let Some(leaf) = alternative.before.iter().chain(&alternative.after).find_map(|effect| {
                        if let plan::Effect::Replace { destination, .. } = effect { Some(slots[destination.slot].data) } else { None }
                    }) else { continue; };
                    assert_ne!(leaf, pair);
                    assert!(alternative.before.iter().any(|effect| matches!(effect, plan::Effect::Context { data, .. } if *data == pair)));
                    assert!(alternative.after.iter().any(|effect| matches!(effect, plan::Effect::Context { data, .. } if *data == pair)));
                    let effects = if entry { &mut alternative.before } else { &mut alternative.after };
                    for effect in effects {
                        if let plan::Effect::Context { data, .. } = effect && *data == pair {
                            *data = leaf;
                            changed += 1;
                        }
                    }
                }
            }
            point
        });
        assert!(changed > 0);
        assert!(matches!(
            program.extend(invalid),
            Err(plan::Rule::InvalidPlan)
        ));
    }
    program.extend(extension).unwrap();
}
