//! Original explicit input correspondence; logical metadata grants no source assignment or owner.
use super::values::{bundle, id, insert_graph};
use super::*;
use crate::value_access::ir::LocalOrigin;
use crate::value_access::{
    CallKind, ValueSource, ValueSourceFlow, ValueSourceInput, ValueSourceSlot, ValueSources,
};
use nocter_model::{AccessLocalId, ValueInputOperation as Op};

type Nodes = Vec<(Option<TypeId>, ValueSource)>;
fn inputs(program: &Program, code: AccessCodeId) -> ([AccessLocalId; 2], Nodes) {
    let mut arena = ArenaBuilder::new();
    let locals = std::array::from_fn(|_| arena.insert(()));
    let nodes = program
        .codes()
        .get(code)
        .unwrap()
        .parameters()
        .iter()
        .zip(locals)
        .map(|(&parameter, local)| {
            let view = program
                .parameter_contract(parameter.into())
                .unwrap()
                .invocation;
            let data = program
                .function_data(FunctionWitness {
                    origin: Origin::Parameter(parameter.into()),
                    invocation: view,
                })
                .unwrap();
            (
                Some(data),
                ValueSource::Input {
                    local,
                    origin: LocalOrigin::Parameter(parameter),
                },
            )
        })
        .collect();
    (locals, nodes)
}
fn update_graph(program: &Program, code: AccessCodeId, operation: Op) -> ValueSources {
    let (locals, mut nodes) = inputs(program, code);
    nodes.push((
        nodes[0].0,
        ValueSource::Replace {
            source: id(0),
            value: id(1),
            path: ValuePath::default(),
            weak: false,
        },
    ));
    let mut points = ArenaBuilder::new();
    let entry = points.insert(());
    let flow = ValueSourceFlow::test(
        entry,
        BTreeMap::from([(entry, 1)]),
        BTreeMap::from([
            (ValueSourceSlot::Local(locals[0]), id(0)),
            (ValueSourceSlot::Local(locals[1]), id(1)),
        ]),
        BTreeMap::new(),
        BTreeMap::from([((entry, ValueSourceSlot::Local(locals[0])), vec![(0, id(2))])]),
        BTreeSet::from([entry]),
        vec![
            ValueSourceInput {
                source: id(0),
                operation,
            },
            ValueSourceInput {
                source: id(1),
                operation: Op::Take,
            },
        ]
        .into(),
    );
    ValueSources::test(code.into(), nodes, BTreeMap::new(), BTreeSet::new()).with_test_flow(flow)
}
fn caller_graph(
    program: &Program,
    code: AccessCodeId,
    update: AccessCodeId,
    site: AccessExpressionId,
    kind: CallKind,
    weak: bool,
) -> ValueSources {
    let (_, mut nodes) = inputs(program, code);
    nodes.push((
        program.function_data(update.into()),
        ValueSource::Function(update.into()),
    ));
    let offset = usize::from(kind == CallKind::Value);
    nodes.push((
        nodes[0].0,
        ValueSource::CallWrite {
            call: site,
            argument: offset,
            source: id(0),
            path: ValuePath::default(),
            weak,
        },
    ));
    let parameters = program.codes().get(update).unwrap().parameters();
    let arguments = if kind == CallKind::Value {
        vec![2, 0, 1]
    } else {
        vec![0, 1]
    };
    let mut call = bundle(
        2,
        &arguments,
        &[
            (parameters[0].into(), offset),
            (parameters[1].into(), offset + 1),
        ],
    );
    call.kind = kind;
    ValueSources::test(
        code.into(),
        nodes,
        BTreeMap::from([(site, vec![call])]),
        BTreeSet::new(),
    )
}

#[test]
fn explicit_writes_select_completed_inputs_for_direct_and_code_token_calls() {
    let (sources, tree) = input(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc update(edit target: func(): i32, take next: func(): i32): void { drop next }\nfunc caller(edit target: func(): i32, take next: func(): i32): void { drop next }",
    );
    let syntax = bound(&sources, &tree);
    let declarations =
        nocter_declaration_lowering::access_declarations::Query::prepare(syntax).unwrap();
    let context = declarations.bind(syntax).unwrap();
    let program = context.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = code("caller");
    let update = code("update");
    let functions: [FunctionWitness; 2] = [code("first").into(), code("second").into()];
    let parameters = program.codes().get(caller).unwrap().parameters();
    let frame = program
        .bind_code(
            caller.into(),
            [
                (parameters[0].into(), functions[0]),
                (parameters[1].into(), functions[1]),
            ],
        )
        .unwrap();
    let mut expressions = ArenaBuilder::new();
    let site = expressions.insert(());
    for kind in [CallKind::Direct, CallKind::Value] {
        for weak in [false, true] {
            let mut evidence = Evidence::default();
            insert_graph(
                &mut evidence,
                caller_graph(program, caller, update, site, kind, weak),
                &[site],
            );
            insert_graph(&mut evidence, update_graph(program, update, Op::Edit), &[]);
            let expected = if weak {
                Err(InstantiationFailure::Unresolved)
            } else {
                program
                    .concrete_witness(functions[1])
                    .map_err(|_| InstantiationFailure::Unresolved)
            };
            assert_eq!(
                value(program, &evidence, &frame, id(3), &mut budget()),
                expected
            );
            let mut tiny = WorkBudget::new(NonZeroU64::new(1).unwrap());
            assert_eq!(
                value(program, &evidence, &frame, id(3), &mut tiny),
                Err(InstantiationFailure::WorkLimit)
            );
            assert_eq!(
                value(program, &evidence, &frame, id(3), &mut budget()),
                expected
            );
            insert_graph(&mut evidence, update_graph(program, update, Op::Read), &[]);
            assert_eq!(
                value(program, &evidence, &frame, id(3), &mut budget()),
                Err(InstantiationFailure::Unresolved)
            );
        }
    }
}

fn updated_closure(
    closure: ValueClosure,
    checked: &crate::value_access::flow::Query,
) -> ValueSources {
    let body = checked.typed().body();
    let channels = body.inputs().ordered().collect::<Vec<_>>();
    assert_eq!(channels.len(), 3);
    let nodes = channels
        .iter()
        .map(|input| {
            let local = body.locals().get(input.local).unwrap();
            (
                Some(local.data),
                ValueSource::Input {
                    local: input.local,
                    origin: local.origin,
                },
            )
        })
        .collect::<Nodes>();
    let mut nodes = nodes;
    nodes.push((
        nodes[1].0,
        ValueSource::Replace {
            source: id(1),
            value: id(2),
            path: ValuePath::default(),
            weak: false,
        },
    ));
    let mut points = ArenaBuilder::new();
    let entry = points.insert(());
    let flow = ValueSourceFlow::test(
        entry,
        BTreeMap::from([(entry, 1)]),
        channels
            .iter()
            .enumerate()
            .map(|(i, input)| (ValueSourceSlot::Local(input.local), id(i)))
            .collect(),
        BTreeMap::new(),
        BTreeMap::from([(
            (entry, ValueSourceSlot::Local(channels[1].local)),
            vec![(0, id(3))],
        )]),
        BTreeSet::from([entry]),
        channels
            .iter()
            .enumerate()
            .map(|(i, input)| ValueSourceInput {
                source: id(i),
                operation: input.operation,
            })
            .collect(),
    );
    ValueSources::test(
        ValueCode::Closure(closure),
        nodes,
        BTreeMap::new(),
        BTreeSet::new(),
    )
    .with_test_flow(flow)
}

#[test]
fn captured_call_prefixes_use_the_original_closure_input_sources_and_local_ids() {
    let (sources, tree) = input(
        "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc invoke(edit target: func(): i32, take next: func(): i32): void { let update = (edit value: func(): i32, take replacement: func(): i32): void { drop replacement }\nupdate(target, next^) }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(bound(&sources, &tree))
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let invoke = code("invoke");
    let parameters = program.codes().get(invoke).unwrap().parameters();
    let functions: [FunctionWitness; 2] = [code("first").into(), code("second").into()];
    let frame = program
        .bind_code(
            invoke.into(),
            [
                (parameters[0].into(), functions[0]),
                (parameters[1].into(), functions[1]),
            ],
        )
        .unwrap();
    let graph = unit.value_sources(invoke.into()).unwrap().clone();
    let writes = graph
        .nodes()
        .filter_map(|(id, node)| match node.value {
            ValueSource::CallWrite {
                call, argument: 1, ..
            } => Some((id, call)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(written, call)] = writes.as_slice() else {
        panic!("one explicit edit")
    };
    let bodies = unit
        .closures()
        .iter()
        .map(|(&closure, checked)| (closure, checked.as_ref()))
        .collect::<Vec<_>>();
    let [(closure, checked)] = bodies.as_slice() else {
        panic!("one closure")
    };
    let updated = updated_closure(*closure, checked);
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &[*call]);
    insert_graph(&mut evidence, updated, &[]);
    assert_eq!(
        value(program, &evidence, &frame, *written, &mut budget()).unwrap(),
        program.concrete_witness(functions[1]).unwrap()
    );
}
