use super::*;
use crate::value_access::{
    flow::{
        Location, Operation,
        resources::{Disposal, Holder, Step},
    },
    tests::{bound, input},
};
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value;
use std::{fs, path::PathBuf};
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("object")
    };
    &members
        .iter()
        .find(|m| m.name.as_ref() == name)
        .unwrap()
        .value
}
fn string(value: &Value) -> &str {
    let Value::String(s) = value else {
        panic!("string")
    };
    s
}
fn accepted(text: &str) -> Unit {
    let (sources, tree) = input(text);
    SourceUnit::prepare(bound(&sources, &tree))
        .unwrap_or_else(|failure| panic!("{text}: {failure:?}"))
        .into_unit()
}
fn named<'a>(unit: &'a Unit, name: &str) -> &'a flow::Query {
    unit.bodies()
        .find(|(id, _)| {
            unit.program()
                .codes()
                .get(*id)
                .and_then(nocter_declarations::value_access::Code::name)
                .and_then(|symbol| unit.program().symbols().spelling(symbol))
                == Some(name)
        })
        .unwrap()
        .1
}

#[test]
fn explicit_generic_arguments_cannot_disappear_during_ordinary_call_or_type_resolution() {
    let (sources, tree) =
        input("func identity(value: i32): i32 { value }\nfunc run(): i32 { identity<i32>(42) }");
    assert!(!tree.has_errors());
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(error), .. }))
            if error.rule() == crate::value_access::Rule::InvalidCall
    ));

    let (sources, tree) = input("func identity(value: i32<i32>): i32 { value }");
    assert!(!tree.has_errors());
    let Err(SourceFailure::Declarations(declarations::Failure::Syntax(diagnostics))) =
        SourceUnit::prepare(bound(&sources, &tree))
    else {
        panic!("ordinary type resolution must reject a parsed generic application");
    };
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message()
            .contains("explicit generic arguments")
    );
}

#[test]
fn generic_headers_do_not_admit_unspecialized_calls_or_values() {
    use crate::value_access::Rule::UnqualifiedGenericCallable;
    for (text, origin) in [
        (
            "func run(): void { template() }\nfunc template<T>(): void {}",
            "template()",
        ),
        (
            "func run(): void { let _ = template }\nfunc template<T>(): void {}",
            "template",
        ),
    ] {
        let (sources, tree) = input(text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(error),
            ..
        }) = failure
        else {
            panic!("expected authored rejection: {failure:?}");
        };
        assert_eq!(error.rule(), UnqualifiedGenericCallable);
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(error.source().primary().span().range()),
            Some(origin)
        );
    }
}

#[test]
fn generic_body_typing_keeps_the_source_transfers_on_abstract_data() {
    for text in [
        "func transfer<T>(take value: T): T { let result: T = value^\nresult^ }",
        "func transfer<T, const N: usize>(take value: [T; N]): [T; N] { let result: [T; (N)] = value^\nresult^ }",
    ] {
        let (sources, tree) = input(text);
        let syntax = bound(&sources, &tree);
        let headers = declarations::Query::prepare(syntax).unwrap();
        let context = headers.bind(syntax).unwrap();
        let (owner, header) = headers.program().codes().iter().next().unwrap();
        let data = header.result();
        let draft = crate::value_access::checker::type_body(&context, owner, &()).unwrap();
        let moves = draft
            .body()
            .expressions()
            .iter()
            .filter(|(_, value)| matches!(value.kind, crate::value_access::ExpressionKind::Move(_)))
            .map(|(_, value)| value.ty.access().unwrap().data())
            .collect::<Vec<_>>();
        assert_eq!(moves, [data, data]);
        assert!(draft.program().data_schema(data).is_none());
        assert_eq!(draft.program().data_copyable(data), Some(false));
    }
}

#[test]
fn generic_body_typing_rejects_copy_or_movement_without_the_declared_access() {
    use crate::value_access::Rule::{MissingTransfer, TypeMismatch};
    for (text, rule, message) in [
        (
            "func identity<T>(take value: T): T { value }",
            MissingTransfer,
            "transfer",
        ),
        (
            "func identity<T>(value: T): T { value^ }",
            MissingTransfer,
            "owning source",
        ),
        (
            "func identity<T>(take value: T): i32 { value^ }",
            TypeMismatch,
            "T",
        ),
    ] {
        let (sources, tree) = input(text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let SourceFailure::Checking(Failure::Body {
            failure: BodyFailure::Rejected(error),
            ..
        }) = &failure
        else {
            panic!("expected a generic body rejection: {failure:?}");
        };
        assert_eq!(error.rule(), rule, "{failure:?}");
        assert!(
            error.source().message().contains(message),
            "{text}: {error:?}"
        );
    }
}

#[test]
fn generic_body_initialization_rejects_transfers_on_already_moved_paths() {
    for text in [
        "func finish<T>(take value: T): T { let prior = value^\nvalue^ }",
        "func finish<T>(flag: bool, take value: T): T { if flag { let prior = value^ }\nvalue^ }",
        "func finish<T, const N: usize>(take value: [T; N]): [T; N] { let prior = value^\nvalue^ }",
    ] {
        let (sources, tree) = input(text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let SourceFailure::Checking(Failure::Flow { diagnostics, .. }) = &failure else {
            panic!("expected source initialization failure: {failure:?}");
        };
        let error = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code() == "E0378")
            .unwrap();
        let file = sources.get(tree.source()).unwrap();
        assert_eq!(file.text_at(error.primary().span().range()), Some("value^"));
        assert!(
            error
                .notes()
                .iter()
                .any(|note| file.text_at(note.origin().span().range()) == Some("value^"))
        );
    }
}

#[test]
fn generic_body_initialization_respects_replacement_and_unreached_moves() {
    for body in [
        "var slot = value^\nlet prior = slot^\nslot = prior^\nslot^",
        "if false { let prior = value^ }\nvalue^",
    ] {
        let text = format!("func finish<T>(take value: T): T {{ {body} }}");
        let unit = accepted(&text);
        assert!(
            named(&unit, "finish")
                .initialization()
                .violations()
                .is_empty()
        );
    }
}

#[test]
fn generic_flow_keeps_cleanup_obligations_until_substitution() {
    use crate::value_access::flow::resources::Disposal;
    for (text, retired) in [
        ("func finish<T>(take value: T): void {}", true),
        (
            "func finish<T, const N: usize>(take value: [T; N]): void {}",
            true,
        ),
        ("func finish<T>(take value: T): T { value^ }", false),
    ] {
        let unit = accepted(text);
        let checked = named(&unit, "finish");
        let header = unit
            .program()
            .codes()
            .get(checked.typed().source_owner())
            .unwrap();
        let parameter = unit
            .program()
            .parameters()
            .get(header.parameters()[0])
            .unwrap();
        let steps = support::steps(checked);
        let disposal = steps
            .iter()
            .filter_map(|step| match step {
                Step::Dispose { payload, .. } if payload.disposal() == Disposal::GenericData => {
                    Some(payload.data())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if retired {
            assert_eq!(disposal, [parameter.data()]);
        } else {
            assert!(disposal.is_empty());
            assert!(steps.iter().any(|step| matches!(step, Step::Move { destination, payload, .. }
                if destination.holder() == Holder::Return && payload.disposal() == Disposal::GenericData)));
        }
    }
}

#[test]
fn generic_flow_rejects_transfer_that_overlaps_a_still_required_edit() {
    let (sources, tree) = input(
        "func finish<T>(take value: T, take replacement: T): T { var slot = value^\nedit writer = slot\nlet saved = slot^\nwriter = replacement^\nsaved^ }",
    );
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    let SourceFailure::Checking(Failure::Flow { diagnostics, .. }) = &failure else {
        panic!("expected a generic access conflict: {failure:?}");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code() == "E0397")
        .unwrap();
    assert_eq!(
        sources
            .get(tree.source())
            .unwrap()
            .text_at(diagnostic.primary().span().range()),
        Some("slot^")
    );
}

#[test]
fn an_unused_generic_definition_is_checked_in_the_ordinary_unit() {
    let unit = accepted("func retire<T>(take value: T): void {}\nfunc run(): i32 { 42 }");
    let body = named(&unit, "retire");
    let id = body.typed().source_owner();
    assert!(body.access().violations().is_empty());
    assert!(unit.program().code_template(id.into()).is_ok());
    assert_eq!(
        unit.program().bind_code(id.into(), []),
        Err(nocter_declarations::value_access::CodeBindingError::MissingGenericApplication(id)),
    );
}

#[test]
fn closed_required_expressions_inside_generic_definitions_need_no_application() {
    let unit =
        accepted("func finish<T>(take value: T): T { let work: [i32; 2 + 2] = [1; 4]\nvalue^ }");
    assert!(
        named(&unit, "finish")
            .initialization()
            .violations()
            .is_empty()
    );
}

#[test]
fn explicit_generic_arguments_follow_the_selected_callable_schema() {
    for (arguments, code, origin, message) in [
        (
            "Missing, 1 / 0, 2",
            "E0390",
            "<Missing, 1 / 0, 2>",
            "requires 2",
        ),
        ("42, 2", "E0370", "42", "requires a type"),
        ("i32, true", "E0370", "true", "usize"),
        ("i32, 1 / 0", "E0325", "1 / 0", "invalid checked arithmetic"),
        ("i32, local", "E0421", "local", "unavailable"),
        (
            "i32, WIDTH",
            "E0291",
            "<i32, WIDTH>",
            "checked body and specialization",
        ),
        (
            "i32, (WIDTH)",
            "E0291",
            "<i32, (WIDTH)>",
            "checked body and specialization",
        ),
        (
            "WIDTH, WIDTH",
            "E0291",
            "<WIDTH, WIDTH>",
            "checked body and specialization",
        ),
        (
            "i32, width()",
            "E0291",
            "<i32, width()>",
            "checked body and specialization",
        ),
        (
            "i32, FIXED",
            "E0291",
            "<i32, FIXED>",
            "checked body and specialization",
        ),
        (
            "Buffer<i32>, 4",
            "E0291",
            "<i32>",
            "explicit generic arguments",
        ),
    ] {
        let text = format!(
            "struct WIDTH {{}}\nconst WIDTH: usize = 2 + 2\nstatic FIXED: usize = 4\nconst func width(): usize {{ WIDTH }}\nfunc run(): void {{ let local: usize = 2\ntemplate<{arguments}>() }}\nfunc template<T, const N: usize>(): void {{}}"
        );
        let (sources, tree) = input(&text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let diagnostic = match &failure {
            SourceFailure::Checking(Failure::Body {
                failure: BodyFailure::Rejected(error),
                ..
            }) => error.source(),
            SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) => diagnostic.source(),
            _ => panic!(
                "expected an authored generic argument diagnostic for {arguments}: {failure:?}"
            ),
        };
        assert_eq!(diagnostic.code(), code, "{arguments}: {failure:?}");
        assert!(
            diagnostic.message().contains(message),
            "{arguments}: {failure:?}"
        );
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(diagnostic.primary().span().range()),
            Some(origin),
            "{arguments}: {failure:?}"
        );
    }
}

#[test]
fn closed_deferred_drive_handles_pure_recursive_code_without_executing_it() {
    let unit = accepted("func recurse(): i32 { recurse() }\nasync func root(): i32 { recurse() }");
    let root = named(&unit, "root").typed().source_owner();
    let helper = named(&unit, "recurse").typed().source_owner();
    assert!(unit.can_drive_without_suspension(root));
    assert!(!unit.can_drive_without_suspension(helper));
}

#[test]
fn only_the_checked_local_future_view_elides_storage() {
    let unit =
        accepted("async func root(): i32 { 42 }\nfunc caller(): void { let value = root() }");
    let root = named(&unit, "root").typed();
    let caller = named(&unit, "caller").typed();
    let future = unit
        .program()
        .codes()
        .get(root.source_owner())
        .unwrap()
        .result();
    assert!(unit.can_drive_without_suspension(root.source_owner()));
    assert!(unit.local_future_data(caller.code(), future));
    assert!(!unit.local_future_data(root.code(), future));
    assert!(!unit.has_future_tags(caller.code()));
    assert!(unit.data_evidence(future).is_err());
}

#[test]
fn retained_future_reads_reject_updates_after_moving_the_activation() {
    for (binding, place) in [
        ("var watched: i32 = 42", "watched"),
        ("var pair = Pair { left: 42, right: 0 }", "pair.left"),
        (
            "var table = [42, 0]\nlet index = position()",
            "table[index]",
        ),
    ] {
        let construction = format!("observe({place})");
        let update = format!("{place} += 1");
        let text = format!(
            "struct Pair {{ left: i32\nright: i32 }}\n\
             func position(): usize {{ 0 }}\n\
             async func observe(source: i32): i32 {{ source }}\n\
             async func root(): i32 {{ {binding}\n\
             let pending = {construction}\nlet moved = pending^\n{update}\nawait moved^ }}"
        );
        let (sources, tree) = input(&text);
        let Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("expected a retained-access diagnostic for {place}");
        };
        let [diagnostic] = &*diagnostics else {
            panic!("{diagnostics:?}")
        };
        assert_eq!(diagnostic.code(), "E0397");
        let primary = diagnostic.primary().span();
        assert_eq!(
            sources
                .get(primary.source())
                .unwrap()
                .text_at(primary.range()),
            Some(update.as_str())
        );
        assert!(diagnostic.notes().iter().any(|note| {
            let span = note.origin().span();
            sources.get(span.source()).unwrap().text_at(span.range()) == Some(construction.as_str())
        }));
    }
}

#[test]
fn a_stored_future_cannot_extend_a_fresh_read_or_edit_input() {
    for mode in ["", "edit "] {
        let source = format!(
            "func make(): i32 {{ 42 }}\nasync func child({mode}input: i32): i32 {{ input }}\n\
             async func root(): i32 {{ let pending = child(make())\nawait pending^ }}"
        );
        let (sources, tree) = input(&source);
        let Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("expected an expired temporary diagnostic for {mode:?}");
        };
        let [diagnostic] = &*diagnostics else {
            panic!("{diagnostics:?}")
        };
        assert_eq!(diagnostic.code(), "E0398");
        let span = diagnostic.primary().span();
        assert_eq!(
            sources.get(span.source()).unwrap().text_at(span.range()),
            Some("child(make())")
        );
        assert!(diagnostic.notes().iter().any(|note| {
            let span = note.origin().span();
            sources.get(span.source()).unwrap().text_at(span.range()) == Some("make()")
        }));
    }
}

#[test]
fn a_moved_future_keeps_its_edit_exclusive_until_drive_or_cancellation() {
    let pair = "var state = Pair { left: 42, right: 0 }";
    for (binding, place, interruption, code, primary) in [
        (
            pair,
            "state.left",
            "let value = state.left",
            "E0396",
            "state.left",
        ),
        (
            pair,
            "state.left",
            "state.left += 1",
            "E0397",
            "state.left += 1",
        ),
        (pair, "state.left", "drop state", "E0397", "drop state"),
        (
            "var state = [42, 0]\nlet index = first()",
            "state[index]",
            "let value = state[0]",
            "E0396",
            "state[0]",
        ),
    ] {
        let construction = format!("increment({place})");
        let text = format!(
            "struct Pair {{ left: i32\nright: i32 }}\n\
             func first(): usize {{ 0 }}\n\
             async func increment(edit value: i32): i32 {{ value += 1\nvalue }}\n\
             async func root(): i32 {{ {binding}\n\
             let pending = {construction}\nlet moved = pending^\n\
             {interruption}\nawait moved^ }}"
        );
        let (sources, tree) = input(&text);
        let Err(SourceFailure::Checking(Failure::Flow { diagnostics, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("expected an exclusive access diagnostic for {interruption}");
        };
        let [diagnostic] = &*diagnostics else {
            panic!("{diagnostics:?}");
        };
        let span = diagnostic.primary().span();
        assert_eq!(diagnostic.code(), code);
        assert_eq!(
            sources.get(span.source()).unwrap().text_at(span.range()),
            Some(primary)
        );
        assert!(diagnostic.notes().iter().any(|note| {
            let span = note.origin().span();
            sources.get(span.source()).unwrap().text_at(span.range()) == Some(construction.as_str())
        }));
    }
}

#[test]
fn a_fallible_drive_output_does_not_certify_error_cleanup() {
    let (sources, tree) = input(
        "async func child(): i32! { 42 }\n\
         async func root(): i32 { let answer = await child()\ndrop answer\n1 }",
    );
    let Err(SourceFailure::Checking(Failure::Body {
        failure: BodyFailure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(bound(&sources, &tree))
    else {
        panic!("expected a source diagnostic for the unqualified cleanup");
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::Rule::UnqualifiedFuture
    );
    let span = diagnostic.source().primary().span();
    assert_eq!(
        sources.get(span.source()).unwrap().text_at(span.range()),
        Some("drop answer")
    );
}

#[test]
fn deferred_drive_diagnoses_a_formal_invocation_without_effect_evidence() {
    let (sources, tree) = input(
        "func answer(): i32 { 42 }\n\
         func apply(action: func(): i32): i32 { action() }\n\
         async func root(): i32 { apply(answer) }",
    );
    let Err(SourceFailure::Checking(Failure::Body {
        failure: BodyFailure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(bound(&sources, &tree))
    else {
        panic!("expected a source diagnostic for the formal invocation");
    };
    assert_eq!(
        diagnostic.rule(),
        crate::value_access::Rule::UnqualifiedFuture
    );
    let span = diagnostic.source().primary().span();
    assert_eq!(
        sources.get(span.source()).unwrap().text_at(span.range()),
        Some("action()")
    );
}

fn assert_resources(
    unit: &Unit,
    caller: &nocter_declarations::value_access::CodeInstanceKey,
    body: &flow::Query,
) {
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    for (point, block) in body.graph().points().iter() {
        for (action, operation) in block.actions.iter().enumerate() {
            let at = crate::value_access::flow::Location { point, action };
            for transition in body.access().resources().at(at) {
                let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
                unit.project_resource_data(caller, at, &mut work)
                    .and_then(|mut projection| {
                        projection.steps(transition.before_steps())?;
                        projection.after_steps(transition.after_steps())
                    })
                    .unwrap_or_else(|failure| {
                        panic!(
                            "{at:?} {:?} before {:?} after {:?}: {failure:?}",
                            operation.operation,
                            transition.before_steps(),
                            transition.after_steps()
                        )
                    });
            }
        }
    }
}

#[test]
fn corpus_is_validated_by_the_complete_production_boundary() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/value-bindings");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases")
    };
    let mut counts = [0; 6];
    let mut mismatches = Vec::new();
    for case in cases {
        let name = string(field(case, "file"));
        let text = fs::read_to_string(root.join(name)).unwrap();
        let mut sources = nocter_source::SourceMap::new();
        let source = sources
            .add_bytes(nocter_source::SourceName::new(name), text.as_bytes())
            .unwrap();
        let tree = nocter_syntax::parse(
            sources.get(source).unwrap(),
            nocter_syntax::ParseGoal::V2CoreUnit,
        );
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(tree.has_errors(), "{name}");
            counts[0] += 1;
            continue;
        }
        assert!(!tree.has_errors(), "{name}: {:?}", tree.diagnostics());
        if string(field(case, "expectation")) == "header-reject" {
            let Err(declarations::Failure::Headers(diagnostics)) =
                declarations::Query::prepare(bound(&sources, &tree))
            else {
                panic!("{name}: expected a source-backed header rejection");
            };
            assert!(!diagnostics.is_empty(), "{name}");
            assert_eq!(
                diagnostics[0].source().code(),
                string(field(case, "diagnostic")),
                "{name}"
            );
            counts[5] += 1;
            continue;
        }
        let result = SourceUnit::prepare(bound(&sources, &tree)).map_err(|failure| match failure {
            SourceFailure::Declarations(failure) => {
                panic!("{name}: unexpected header rejection: {failure:?}")
            }
            SourceFailure::Modules(failure) => {
                panic!("{name}: unexpected module input failure: {failure:?}")
            }
            SourceFailure::Checking(failure) => failure,
        });
        if string(field(case, "expectation")) == "accept" {
            let source = result.unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let context = source.declarations().bind(bound(&sources, &tree)).unwrap();
            let unit = source.unit();
            assert_eq!(unit.bodies().count(), context.program().codes().len());
            assert!(unit.bind(&context).is_ok());
            counts[1] += 1;
        } else {
            match result {
                Err(Failure::Body {
                    failure: BodyFailure::Rejected(diagnostic),
                    ..
                }) => {
                    assert!(!diagnostic.source().primary().span().range().is_empty());
                    if diagnostic.source().code() != string(field(case, "diagnostic")) {
                        mismatches.push(format!(
                            "{name}: expected {}, received {:?}",
                            string(field(case, "diagnostic")),
                            diagnostic.source()
                        ));
                    }
                    counts[2] += 1;
                }
                Err(Failure::Flow { diagnostics, .. }) => {
                    assert!(!diagnostics.is_empty());
                    let expected = string(field(case, "diagnostic"));
                    assert_eq!(diagnostics[0].code(), expected, "{name}");
                    counts[if expected == "E0378" { 3 } else { 4 }] += 1;
                }
                other => panic!("{name}: expected rejection, got {:?}", other.err()),
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert!(
        counts.iter().all(|&count| count > 0),
        "each verdict family must be exercised: {counts:?}"
    );
}
#[test]
fn unused_bad_definition_rejects_the_entire_unit_and_recursive_valid_bodies_are_closed() {
    let (sources, tree) = input("func good(): void {}\nfunc unused(input: error): error { input }");
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body { .. })
    ));
    let unit = accepted(
        "func first(take input: error): error { second(input^) }\nfunc second(take input: error): error { first(input^) }",
    );
    assert_eq!(unit.bodies().count(), 2);
}
#[test]
fn whole_unit_rebinding_checks_every_body_and_the_exact_authority() {
    let text = "func first(): i32 { 1 }\nfunc second(): i32 { 2 }";
    let (sources, tree) = input(text);
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let c = q.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&c).unwrap();
    let (same, other_tree) = input(&format!("// source moved\n{text}"));
    let same = q.bind(bound(&same, &other_tree)).unwrap();
    assert!(unit.bind(&same).is_ok());
    let fresh = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let fresh = fresh.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        unit.bind(&fresh),
        Err(BodyFailure::ChangedAuthority)
    ));
    let (changed, changed_tree) = input("func first(): i32 { 1 }\nfunc second(): i32 { 3 }");
    let changed = q.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(matches!(unit.bind(&changed), Err(BodyFailure::ChangedBody)));
}
#[test]
fn read_of_a_fresh_noncopy_result_disposes_one_evaluation_temporary() {
    let unit = accepted(
        "func forward(take input: error): error { input^ }\nfunc observe(input: error): void {}\nfunc sample(take input: error): void { observe(forward(input^)) }",
    );
    let q = named(&unit, "sample");
    let mut disposed = Vec::new();
    for (point, block) in q.graph().points().iter() {
        for action in 0..block.actions.len() {
            for t in q.access().resources().at(Location { point, action }) {
                for step in t.steps() {
                    if let Step::Dispose { source, payload } = step
                        && payload.disposal() == Disposal::OwnedError
                    {
                        disposed.push(source.holder());
                    }
                }
            }
        }
    }
    assert_eq!(disposed.len(), 1);
    assert!(matches!(disposed[0], Holder::Temporary { .. }));
}
#[test]
fn cancellation_of_a_reserved_take_prefix_leaves_its_owner_until_scope_cleanup() {
    let unit = accepted(
        "func consume(take value: error,number: i32): void {}\nfunc sample(take input: error,finish: bool): void { consume(input^,if finish { return } else { 0 }) }",
    );
    let q = named(&unit, "sample");
    let local = q.typed().body().locals().iter().next().unwrap().0;
    let mut seen = false;
    for (point, block) in q.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if matches!(event.operation, Operation::CancelCall { prepared: 1, .. }) {
                for t in q.access().resources().at(Location { point, action }) {
                    assert!(t.initialized_before(Holder::Local(local)));
                    assert!(t.initialized_after(Holder::Local(local)));
                    assert!(
                        !t.steps()
                            .iter()
                            .any(|step| matches!(step, Step::Invoke { .. }))
                    );
                    seen = true;
                }
            }
        }
    }
    assert!(seen);
}
#[test]
fn edit_input_replacement_retains_its_initialized_outside_slot() {
    let unit = accepted(
        "func replace(edit target: error,take replacement: error): void { target = replacement^ }",
    );
    let q = named(&unit, "replace");
    let mut seen = false;
    for (point, block) in q.graph().points().iter() {
        for action in 0..block.actions.len() {
            for t in q.access().resources().at(Location { point, action }) {
                seen|=t.steps().iter().any(|step|matches!(step,Step::ReplaceReferent{payload,..} if payload.disposal()==Disposal::OwnedError));
            }
        }
    }
    assert!(seen);
}
#[test]
fn every_call_result_is_a_value_and_conditional_copies_do_not_retain_the_source() {
    accepted(
        "func count(): i32 { 0 }\nfunc sample(flag: bool): i32 { var value = count()\nvalue += 1\nlet answer = if flag { value } else { 2 }\nvalue = 3\nreturn answer + value }",
    );
}
#[test]
fn a_conditional_move_does_not_restore_the_source_on_another_path() {
    let (sources, tree) = input(
        "func invalid(take input: error,flag: bool): error { if flag { let _ = input^ }\nreturn input^ }",
    );
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&header.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Flow { .. })
    ));
}

#[test]
fn literal_control_does_not_invent_normal_loop_exits() {
    for text in [
        "func f(): i32 { loop { if false { break } } }",
        "func f(): i32 { while true { if false { break } } }",
        "func f(): i32 { loop { if true { continue } else { () }\nbreak } }",
        "func f(): i32 { loop { while false { break } } }",
    ] {
        accepted(text);
    }
    for text in [
        "func f(): i32 { loop { if true { break } } }",
        "func f(): i32 { loop { if false { continue } else { break } } }",
        "func f(flag: bool): i32 { loop { if flag { continue }\nbreak } }",
    ] {
        let (sources, tree) = input(text);
        let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })
                    if diagnostic.rule() == crate::value_access::Rule::MissingResult
            ),
            "{text}"
        );
    }
    let (sources, tree) =
        input("func f(): i32 { loop { if false { let bad: i32 = true }\ncontinue } }");
    let q = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        Unit::prepare(&q.bind(bound(&sources, &tree)).unwrap()),
        Err(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })
            if diagnostic.rule() == crate::value_access::Rule::TypeMismatch
    ));
}

mod callable_storage;
mod callbacks;
mod compound_results;
mod constructed_outcomes;
mod construction;
mod creation_recipes;
mod functions;
mod instance_recipes;
mod producer_recipes;
mod records;
mod result_captures;
mod results;
mod selection;
mod structural_results;
mod support;

mod supply;

mod closures;
mod contextual_closures;
mod dependent_results;
mod inferred_closure_results;
mod input_layout;
mod unproduced;

mod transfers;

mod const_conditions;
mod constants;

mod initializers;

mod booleans;

mod comparisons;

mod statics;

mod tuples;

mod array_selection;
mod arrays;
mod header_arrays;
mod integers;
mod staged;

mod array_repetition;

mod indexed_edits;

mod indexed_aliases;

mod indexed_writes;

mod indexed_regions;

mod indexed_bounds;
mod indexed_transfers;

mod const_outcomes;
mod fallible_values;
mod namespaces;
mod optional_values;
mod primitives;
mod text_values;

mod anonymous_inputs;

mod value_sources;
