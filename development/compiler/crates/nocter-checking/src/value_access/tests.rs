mod annotations;
mod header_value_evidence;
mod module_sources;
mod source_scopes;
use super::*;
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_model::{BuiltinType, ValueAccessMode as Mode, ValueBindingKind};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{BoundSyntax, ParseGoal, SyntaxTree, parse};

pub(super) fn input(text: &str) -> (SourceMap, SyntaxTree) {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("body.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
    (sources, tree)
}
pub(super) fn bound<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> BoundSyntax<'a> {
    BoundSyntax::new(sources.get(tree.source()).unwrap(), tree).unwrap()
}
pub(super) fn queries(text: &str) -> Result<Vec<Query>, Failure> {
    let (sources, tree) = input(text);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = header.bind(bound(&sources, &tree)).unwrap();
    context
        .program()
        .codes()
        .iter()
        .map(|(id, _)| Query::prepare(&context, id))
        .collect()
}
pub(super) fn named<'a>(queries: &'a [Query], name: &str) -> &'a Query {
    queries
        .iter()
        .find(|q| {
            q.program()
                .codes()
                .get(q.source_owner())
                .unwrap()
                .name()
                .and_then(|symbol| q.program().symbols().spelling(symbol))
                == Some(name)
        })
        .unwrap()
}
fn reject(text: &str, rule: Rule) {
    let Err(Failure::Rejected(diagnostic)) = queries(text) else {
        panic!("expected {rule:?}: {text}")
    };
    assert_eq!(diagnostic.rule(), rule, "{:?}", diagnostic.source());
    assert!(!diagnostic.source().primary().span().range().is_empty());
}

#[test]
fn immediate_body_cannot_await_or_manufacture_a_future_from_an_annotation() {
    reject("func run(): i32 { await 42 }", Rule::AwaitOutsideAsync);
    reject(
        "func run(): i32 { let pending: future i32 = 42\n42 }",
        Rule::TypeMismatch,
    );
}

#[test]
fn deferred_body_checks_its_output_and_keeps_failure_inside_the_future() {
    use nocter_declarations::value_access::PayloadSchema;

    let checked = queries(
        "async func succeed(): i32! { return 42 }\n\
         async func fail(take cause: error): i32! { throw cause^ }",
    )
    .unwrap();
    for query in &checked {
        let program = query.program();
        let result = query.result_data();
        assert_eq!(
            program.data_schema(result),
            Some(PayloadSchema::Fallible(
                program.data_type(BuiltinType::I32).unwrap()
            ))
        );
        let invocation = program.codes().get(query.source_owner()).unwrap().result();
        assert_eq!(
            program.data_schema(invocation),
            Some(PayloadSchema::Future(result))
        );
    }
    reject("async func invalid(): i32 { true }", Rule::TypeMismatch);
}

#[test]
fn ordinary_closures_do_not_inherit_await_from_a_deferred_body() {
    reject(
        "async func run(): void { let nested = () { await 42 } }",
        Rule::AwaitOutsideAsync,
    );
    reject(
        "async func run(): i32 { await 42 }",
        Rule::InvalidAwaitOperand,
    );
    reject(
        "async func child(): i32 { 42 }\nasync func run(): i32 { let pending = child()\nawait pending }",
        Rule::InvalidAwaitOperand,
    );
}

#[test]
fn await_consumes_one_owned_future_layer_before_outcome_propagation() {
    use nocter_declarations::value_access::PayloadSchema;

    let typed = queries(
        "async func child(): i32! { 42 }\n\
         async func fresh(): i32! { await child()? }\n\
         async func moved(): i32! { let pending: future i32! = child()\nawait pending^? }",
    )
    .unwrap();
    for name in ["fresh", "moved"] {
        let body = named(&typed, name);
        let (awaited, operand) = body
            .body()
            .expressions()
            .iter()
            .find_map(|(_, item)| match item.kind {
                ExpressionKind::Await { future } => Some((item.ty, future)),
                _ => None,
            })
            .unwrap();
        let output = body.result_data();
        assert!(matches!(
            body.program().data_schema(output),
            Some(PayloadSchema::Fallible(_))
        ));
        assert_eq!(
            awaited.normal(),
            Some(nocter_model::ValueAccess::new(output, Mode::Own))
        );
        let source = body
            .body()
            .expressions()
            .get(operand)
            .unwrap()
            .ty
            .normal()
            .unwrap();
        assert_eq!(source.mode(), Mode::Own);
        assert_eq!(
            body.program().data_schema(source.data()),
            Some(PayloadSchema::Future(output))
        );
    }
    reject(
        "async func child(): i32 { 42 }\nfunc run(): i32 { let pending: future i64 = child()\n42 }",
        Rule::TypeMismatch,
    );
}

#[test]
fn await_drives_the_transferred_activation_before_receiving_its_output() {
    use flow::resources::{Disposal, Holder, Step};

    let typed = queries(
        "async func first(): i32 { 12 }\n\
         async func second(): i32 { 30 }\n\
         async func run(flag: bool): i32 {\n\
            let pending = if flag { first() } else { second() }\n\
            await pending^\n\
         }",
    )
    .unwrap();
    let query = named(&typed, "run");
    let flow = flow::Query::prepare(Arc::new(query.clone())).unwrap();
    let mut producers = Vec::new();
    let mut drives = 0;
    for (point, block) in flow.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            drives += usize::from(matches!(
                event.operation,
                flow::Operation::DriveFuture { .. }
            ));
            for transition in flow
                .access()
                .resources()
                .at(flow::Location { point, action })
            {
                assert!(!transition.steps().iter().any(|step| matches!(
                    step, Step::Dispose { payload, .. }
                        if matches!(payload.disposal(), Disposal::UnstartedFuture(_))
                )));
                let flow::Operation::DriveFuture { future, result } = event.operation else {
                    continue;
                };
                let source_holder = Holder::Expression(future);
                let output_holder = Holder::Expression(result);
                assert!(transition.initialized_before(source_holder));
                assert!(!transition.initialized_before(output_holder));
                assert!(
                    !transition
                        .operation_condition()
                        .owned()
                        .any(|(holder, _)| { holder == source_holder || holder == output_holder })
                );
                let [Step::DriveFuture { source, payload }] = transition.before_steps() else {
                    panic!("{:?}", transition.before_steps());
                };
                assert_eq!(source, &source_holder.place());
                let Disposal::UnstartedFuture(producer) = payload.disposal() else {
                    panic!("the owned activation lost its producer");
                };
                producers.push(producer);
                let [
                    Step::Receive {
                        result: received,
                        payload,
                    },
                ] = transition.after_steps()
                else {
                    panic!("{:?}", transition.after_steps());
                };
                assert_eq!(*received, result);
                assert_eq!(payload.data(), query.result_data());
                assert!(!transition.initialized_after(source_holder));
                assert!(transition.initialized_after(output_holder));
            }
        }
    }
    assert_eq!(drives, 1);
    let mut expected = ["first", "second"].map(|name| named(&typed, name).source_owner());
    expected.sort_unstable();
    producers.sort_unstable();
    assert_eq!(producers, expected);
}

#[test]
fn activation_construction_receives_ordered_take_snapshots_before_a_later_drive() {
    use flow::resources::{Disposal, Holder, Step};
    use nocter_model::ValueFunctionOrigin;

    let typed = queries(
        "func change(edit original: i32): i32 { original = 0\n1 }\n\
         async func producer(take first: i32, take second: i32): i32 { first + second }\n\
         async func run(): i32 {\n\
             var original = 41\n\
             let pending = producer(original, change(original))\n\
             await pending^\n\
         }",
    )
    .unwrap();
    let producer = named(&typed, "producer").source_owner();
    let change = named(&typed, "change").source_owner();
    let query = named(&typed, "run");
    let flow = flow::Query::prepare(Arc::new(query.clone())).unwrap();
    assert_eq!(flow.access().status(), flow::access::Status::Analyzed);
    let mut order = Vec::new();
    for (point, block) in flow.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            match event.operation {
                flow::Operation::PrepareArgument { call, argument: 0 }
                    if matches!(&query.body().expressions().get(call).unwrap().kind,
                        ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(id), .. }
                        if *id == producer) =>
                {
                    order.push("first input snapshot");
                }
                flow::Operation::CommitCall(call)
                    if matches!(&query.body().expressions().get(call).unwrap().kind,
                        ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(id), .. }
                        if *id == change) =>
                {
                    order.push("second input effect");
                }
                flow::Operation::ConstructFuture(call) => {
                    order.push("activation");
                    let mut transitions = flow
                        .access()
                        .resources()
                        .at(flow::Location { point, action });
                    let transition = transitions.next().unwrap();
                    assert!(transitions.next().is_none());
                    let [
                        Step::ConstructActivation {
                            destination,
                            payload,
                            inputs,
                        },
                    ] = transition.after_steps()
                    else {
                        panic!("{:?}", transition.after_steps());
                    };
                    assert_eq!(*destination, Holder::Expression(call));
                    assert_eq!(payload.disposal(), Disposal::UnstartedFuture(producer));
                    assert_eq!(inputs.len(), 2);
                    for (index, (source, payload)) in inputs.iter().enumerate() {
                        let holder = Holder::Prepared { call, index };
                        assert_eq!(source, &holder.place());
                        assert_eq!(payload.disposal(), Disposal::Trivial);
                        assert!(transition.initialized_before(holder));
                        assert!(!transition.initialized_after(holder));
                    }
                }
                flow::Operation::DriveFuture { .. } => order.push("drive"),
                _ => {}
            }
        }
    }
    assert_eq!(
        order,
        [
            "first input snapshot",
            "second input effect",
            "activation",
            "drive"
        ]
    );
}

#[test]
fn interrupted_deferred_preparation_has_a_selected_producer_without_any_invocation() {
    use nocter_declarations::{CallableExecution, value_access::PayloadSchema};

    let (sources, tree) = input(
        "async func later(take input: i32): i32 { input }\n\
         func run(): i32 {\n\
         later(if true { return 19 } else { return 23 })\n\
         42\n}",
    );
    let syntax = bound(&sources, &tree);
    let declarations = declarations::Query::prepare(syntax).unwrap();
    let context = declarations.bind(syntax).unwrap();
    let program = context.program();
    let code = |name| {
        program
            .resolve_code_in(
                nocter_declarations::value_access::ScopeId::ROOT,
                program.symbols().get(name).unwrap(),
            )
            .unwrap()
    };
    let body = Query::prepare(&context, code("run")).unwrap();
    let (call, target) = body
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            let ExpressionKind::Call { target, .. } = expression.kind else {
                return None;
            };
            Some((expression.ty, target))
        })
        .unwrap();
    assert_eq!(
        target,
        nocter_model::ValueFunctionOrigin::Declaration(code("later"))
    );
    let output = program.data_type(BuiltinType::I32).unwrap();
    assert_eq!(
        program.function_execution(target),
        Some(CallableExecution::Deferred { output })
    );
    assert_eq!(
        program.data_schema(call.access().unwrap().data()),
        Some(PayloadSchema::Future(output))
    );
    assert!(!call.may_complete());
    let graph = flow::Query::prepare(Arc::new(body)).unwrap();
    assert!(
        graph
            .graph()
            .points()
            .iter()
            .flat_map(|(_, p)| &p.actions)
            .all(|action| !matches!(action.operation, flow::Operation::CommitCall(_)))
    );
}

#[test]
fn unstarted_futures_keep_their_producer_through_transfer_and_cancel_once() {
    use flow::resources::{Disposal, Payload, Step};

    let typed = queries(
        "async func first(): i32 { 1 }\n\
         async func second(): i32 { 2 }\n\
         func run(): void {\n\
         let a = first()\n\
         let b = second()\n\
         let moved = a^\n\
         drop b\n\
         drop moved\n}",
    )
    .unwrap();
    let first = named(&typed, "first").source_owner();
    let second = named(&typed, "second").source_owner();
    let query = named(&typed, "run");
    let graph = flow::Query::prepare(Arc::new(query.clone())).unwrap();
    let mut created = Vec::new();
    let mut cancelled = Vec::new();
    for (point, block) in graph.graph().points().iter() {
        for action in 0..block.actions.len() {
            for transition in graph
                .access()
                .resources()
                .at(flow::Location { point, action })
            {
                for step in transition.steps() {
                    match step {
                        Step::ConstructActivation {
                            payload, inputs, ..
                        } => {
                            assert!(inputs.is_empty());
                            created.push(*payload);
                        }
                        Step::Dispose { payload, .. }
                            if matches!(payload.disposal(), Disposal::UnstartedFuture(_)) =>
                        {
                            cancelled.push(*payload);
                        }
                        Step::Invoke { .. } => {
                            panic!("a future must not invoke its body at construction")
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert_eq!(created.len(), 2);
    assert_eq!(created[0].data(), created[1].data());
    assert_eq!(
        created
            .iter()
            .copied()
            .map(Payload::disposal)
            .collect::<Vec<_>>(),
        [
            Disposal::UnstartedFuture(first),
            Disposal::UnstartedFuture(second)
        ]
    );
    assert_eq!(cancelled, [created[1], created[0]]);
}

#[test]
fn future_result_in_a_callable_contract_requires_evidence_at_invocation() {
    queries("func retain(action: func(): future i32): void {}").unwrap();
    reject(
        "func invoke(action: func(): future i32): void { let pending = action() }",
        Rule::UnqualifiedFuture,
    );
}

#[test]
fn future_in_local_record_representation_is_not_admitted_by_absence() {
    reject(
        "struct Wrapper { pending: (future i32)? }\n\
         func run(): void { let value = Wrapper { pending: none } }",
        Rule::UnqualifiedFuture,
    );
}
#[test]
fn local_kinds_are_independent_of_data_and_value_supply() {
    let all=queries("func f(edit input: i32): i32 {\nlet fixed = input\nvar mutable = fixed\nread observed = mutable\nedit changing = input\nchanging += observed\nreturn fixed\n}").unwrap();
    let f = named(&all, "f");
    let kinds: Vec<_> = f
        .body()
        .locals()
        .iter()
        .map(|(_, local)| local.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            ValueBindingKind::EditAlias,
            ValueBindingKind::FixedValue,
            ValueBindingKind::MutableValue,
            ValueBindingKind::ReadAlias,
            ValueBindingKind::EditAlias
        ]
    );
    assert!(
        f.body()
            .locals()
            .iter()
            .all(|(_, local)| f.program().data_kind(local.data) == Some(BuiltinType::I32))
    );
}
#[test]
fn value_boundaries_distinguish_explicit_transfer_from_unmarked_copy() {
    let all = queries("func f(take input: error): error { let result = input^\nreturn result^ }")
        .unwrap();
    let f = named(&all, "f");
    assert_eq!(
        f.body()
            .expressions()
            .iter()
            .filter(|(_, e)| matches!(e.kind, ExpressionKind::Move(_)))
            .count(),
        2
    );
    let all = queries("func f(input: i32): i32 { let result = input\nreturn result }").unwrap();
    assert!(
        named(&all, "f")
            .body()
            .expressions()
            .iter()
            .any(|(_, e)| matches!(
                e.kind,
                ExpressionKind::Boundary {
                    action: BoundaryAction::CopyPayload,
                    ..
                }
            ))
    );
}
#[test]
fn taking_copy_input_does_not_reserve_a_transfer_of_the_original() {
    let all = queries(
        "func use_value(take input: i32): void {}\nfunc f(input: i32): void { use_value(input) }",
    )
    .unwrap();
    let f = named(&all, "f");
    let actions: Vec<_> = f
        .body()
        .expressions()
        .iter()
        .filter_map(|(_, e)| {
            if let ExpressionKind::Call { arguments, .. } = &e.kind {
                Some(arguments[0].action.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(matches!(actions.as_slice(), [ArgumentAction::CopyPlace(_)]));
}
#[test]
fn body_use_never_strengthens_a_read_input_or_fixed_local() {
    reject(
        "func f(input: error): error { return input }",
        Rule::MissingTransfer,
    );
    reject("func f(input: i32): void { input = 1 }", Rule::NotWritable);
    reject(
        "func f(): void { let fixed = 1\nfixed = 2 }",
        Rule::NotWritable,
    );
    reject(
        "func f(): void { let fixed = 1\nedit alias = fixed }",
        Rule::InvalidUpdate,
    );
    reject("func f(): void { read alias = 1 }", Rule::TypeMismatch);
    reject(
        "func f(input: error): void { drop input }",
        Rule::InvalidDrop,
    );
}
#[test]
fn branches_construct_values_and_terminating_edges_supply_no_answer() {
    assert!(queries("func f(take a: error, take b: error, flag: bool): error { let result = if flag { a^ } else { b^ }\nreturn result^ }").is_ok());
    assert!(queries("func f(flag: bool): i32 { return if flag { return 4 } else { 3 } }").is_ok());
    reject("func f(): i32 { if true { () } }", Rule::TypeMismatch);
    reject("func f(): i32 {}", Rule::MissingResult);
    assert!(queries("func f(): i32 { loop {} }").is_ok());
}
#[test]
fn numeric_checks_and_local_diagnostics_are_preserved() {
    assert!(queries("func f(): i32 { return -(2147483648) }").is_ok());
    reject("func f(): i32 { return 2147483648 }", Rule::IntegerRange);
    reject("func f(): i32 { return unknown }", Rule::UnknownName);
    reject("func f(): void { break }", Rule::InvalidLoopControl);
    reject("func f(): void { let _: i32 = 1 }", Rule::AnnotatedDiscard);
    reject("func f(): void { var _ = 1 }", Rule::AnnotatedDiscard);
    reject(
        "func f(input: i32): bool { return input }",
        Rule::TypeMismatch,
    );
    reject(
        "func f(a: error,b: error): bool { return a == b }",
        Rule::TypeMismatch,
    );
    let all = queries("func f(): void { let fixed = 1 }").unwrap();
    assert!(
        named(&all, "f")
            .body()
            .blocks()
            .get(named(&all, "f").body().root())
            .unwrap()
            .ty
            .normal()
            .is_some_and(|access| access.mode() == Mode::Own)
    );
}
#[test]
fn typed_body_rebinding_checks_authority_and_own_body() {
    let (sources, tree) = input("func f(input: i32): i32 { return input }");
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = header.bind(bound(&sources, &tree)).unwrap();
    let id = context.program().codes().iter().next().unwrap().0;
    let query = Query::prepare(&context, id).unwrap();
    assert!(query.bind(&context).is_ok());
    let (other, other_tree) = input("// moved\nfunc f(input: i32): i32 { return input }");
    let rebound = header.bind(bound(&other, &other_tree)).unwrap();
    assert!(query.bind(&rebound).is_ok());
    let rebuilt = declarations::Query::prepare(bound(&other, &other_tree)).unwrap();
    let rebuilt = rebuilt.bind(bound(&other, &other_tree)).unwrap();
    assert!(matches!(
        query.bind(&rebuilt),
        Err(Failure::ChangedAuthority)
    ));
    let (changed, changed_tree) = input("func f(input: i32): i32 { return input + 1 }");
    let changed = header.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(matches!(query.bind(&changed), Err(Failure::ChangedBody)));
}

#[test]
fn local_array_annotations_request_their_paired_length_code_before_normalization() {
    for annotation in [
        "[i32; count()]",
        "(bool, [i32; 0])",
        "func(edit [i32; 2]): void",
    ] {
        let text = format!("func f(): void {{ let value: {annotation} = missing }}");
        let Err(Failure::RequiredExpression(required)) = queries(&text) else {
            panic!("length normalization needs a completed value: {text}");
        };
        assert!(matches!(
            required.code(),
            nocter_model::ValueCodeRoot::Expression(_)
        ));
    }
}
