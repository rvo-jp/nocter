//! Whole physical source input uses the same staged header/body/value engine as a single source.
use super::source_scopes::Fixture;
use super::*;
use accepted::{ModuleSourceUnit, SourceFailure};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::PayloadSchema;

mod primitives;
mod reuse;

#[test]
fn authored_enum_payload_types_reconcile_required_counts_and_opaque_copy() {
    let root = "see ./body.nct\nconst WIDTH: usize\nenum Packet { empty\ndata(values: [i32; WIDTH]) }\ncopy enum Secret";
    let body =
        "see ./index.nct\nconst WIDTH: usize = 3\ncopy enum Secret { hidden(value: (i32, bool)) }";
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, root),
            ("/app/body.nct", Implementation, body),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let unit = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = unit.unit().program();
    let select = |name| {
        program
            .enumeration(
                program
                    .resolve_nominal(program.symbols().get(name).unwrap())
                    .unwrap(),
            )
            .unwrap()
    };
    assert!(select("Secret").copyable());
    let packet = select("Packet");
    assert!(packet.has_payload() && !packet.copyable());
    let data = program.variants().get(packet.variants()[1]).unwrap();
    let field = program.fields().get(data.fields()[0]).unwrap();
    let Some(PayloadSchema::Array(array)) = program.data_schema(field.data()) else {
        panic!("the variant field retains its exact array type");
    };
    assert_eq!(array.length(), 3);
    let stale = nocter_model::FrozenData::new(vec![nocter_model::FrozenNode::Scalar(
        nocter_model::ConstantValue::Integer(4),
    )])
    .unwrap();
    assert!(
        unit.declarations()
            .current()
            .unwrap()
            .verify_header_values(|_| Some(&stale))
            .is_err()
    );
}

#[test]
fn enum_payload_visibility_follows_the_original_contract() {
    for (header, completion, accepted) in [
        ("enum Packet { data(value: Private) }", "", false),
        ("enum Packet", "enum Packet { data(value: Private) }", true),
    ] {
        let root = format!("see ./body.nct\n{header}");
        let body = format!("see ./index.nct\nstruct Private {{ value: i32 }}\n{completion}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if accepted {
            assert!(result.is_ok(), "{result:?}");
        } else {
            let Err(SourceFailure::Declarations(declarations::Failure::Visibility(error))) = result
            else {
                panic!("expected hidden payload exposure diagnostic: {result:?}");
            };
            assert_eq!(
                error.rule(),
                declarations::VisibilityRule::SignatureExposure
            );
            let range = error.source().primary().span().range();
            assert_eq!(range.start().get() as usize, root.rfind("Private").unwrap());
        }
    }
}

#[test]
fn enum_payload_header_errors_point_to_the_original_field() {
    for (source, expected, notes) in [
        (
            "enum Packet { data(value: i32, value: bool) }",
            "value: bool",
            1,
        ),
        ("enum Packet { next(value: Packet) }", "Packet) }", 1),
        ("copy enum Packet { report(value: error) }", "error) }", 0),
    ] {
        let fixture = Fixture::new(&[("/app/index.nct", Root, source)], &[]);
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(errors))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("expected invalid payload field: {source}");
        };
        let [error] = errors.as_slice() else {
            panic!("one source-positioned field error");
        };
        assert_eq!(
            error.source().primary().span().range().start().get() as usize,
            source.rfind(expected).unwrap(),
            "{source}: {error:?}",
        );
        assert_eq!(error.source().notes().len(), notes);
    }
}

#[test]
fn enum_construction_reports_a_missing_or_extra_payload_at_the_variant() {
    for (expression, rule) in [
        ("Packet.data", Rule::InvalidConstruction),
        ("Packet.data()", Rule::InvalidConstruction),
        ("Packet.data(1, 2)", Rule::InvalidConstruction),
        ("Packet.empty()", Rule::InvalidCall),
    ] {
        let root = "see ./body.nct\nenum Packet { empty\ndata(value: i32) }\nfunc run(): void";
        let body = format!("see ./index.nct\nfunc run(): void {{ let value = {expression} }}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let Err(SourceFailure::Checking(accepted::Failure::Body {
            failure: Failure::Rejected(error),
            ..
        })) = ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("expected exact constructor arity: {expression}");
        };
        assert_eq!(error.rule(), rule);
        assert_eq!(error.source().primary().source(), fixture.tree(1).source());
    }
}

#[test]
fn authored_enum_construction_freezes_the_selected_typed_values() {
    use nocter_model::{ConstantValue, FrozenNode};
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                concat!(
                    "see ./body.nct\n",
                    "enum Packet { empty\ndata(number: i32, flag: bool)\nfailed(reason: error) }\n",
                    "const DATA: Packet\nconst EMPTY: Packet\nfunc run(): i32"
                ),
            ),
            (
                "/app/body.nct",
                Implementation,
                concat!(
                    "see ./index.nct\n",
                    "const func make(value: i32): Packet { Packet.data(value, true) }\n",
                    "const DATA: Packet = make(42)\nconst EMPTY: Packet = Packet.empty\n",
                    "func run(): i32 { if DATA is Packet.empty { 0 } else { 42 } }"
                ),
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let input = fixture.input();
    let source = ModuleSourceUnit::prepare(&input).unwrap();
    let unit = source.unit();
    let value = |name| {
        let program = unit.program();
        unit.constant_value(
            program
                .resolve_constant(program.symbols().get(name).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let selected = value("DATA");
    let FrozenNode::Enum { tag: 1, fields } = selected.node() else {
        panic!("expected Packet.data")
    };
    let [number, flag] = fields.as_ref() else {
        panic!("exact active components")
    };
    assert_eq!(
        selected.nodes()[*number],
        FrozenNode::Scalar(ConstantValue::Integer(42))
    );
    assert_eq!(
        selected.nodes()[*flag],
        FrozenNode::Scalar(ConstantValue::Bool(true))
    );
    assert_eq!(
        value("EMPTY").node(),
        &FrozenNode::Enum {
            tag: 0,
            fields: Box::new([])
        }
    );
}

#[test]
fn required_enum_values_preserve_the_selected_tag_in_compound_constants() {
    use nocter_model::FrozenNode;
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nenum Status { idle\nactive\nstopped }\nconst ANSWER: (Status, Status)",
            ),
            (
                "/app/body.nct",
                Implementation,
                concat!(
                    "see ./index.nct\n",
                    "const func choose(input: Status): Status {\n",
                    "    var selected = input\n",
                    "    if edit selected is Status.idle {\n",
                    "        selected = Status.active\n",
                    "    } else if take selected is Status.active {\n",
                    "        selected = Status.stopped\n",
                    "    }\n",
                    "    selected\n",
                    "}\n",
                    "const ANSWER: (Status, Status) = (choose(Status.idle), choose(Status.active))",
                ),
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let unit = source.unit();
    let program = unit.program();
    let constant = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    let value = unit.constant_value(constant).unwrap();
    let FrozenNode::Aggregate(fields) = value.node() else {
        panic!("compound enum constant");
    };
    let tags = fields
        .iter()
        .map(|&id| match &value.nodes()[id] {
            FrozenNode::Enum { tag, fields } if fields.is_empty() => *tag,
            _ => panic!("expected a scalar enum case"),
        })
        .collect::<Vec<_>>();
    assert_eq!(tags, [1, 2]);
    verify_enum_carrier_retirement(unit);
}

fn verify_enum_carrier_retirement(unit: &accepted::Unit) {
    use crate::value_access::{
        ExpressionKind,
        flow::{
            Location, Operation,
            resources::{Holder, Step},
        },
    };
    use nocter_model::ValueInputOperation;
    let (query, condition, carrier) = unit
        .bodies()
        .find_map(|(_, query)| {
            query
                .typed()
                .body()
                .expressions()
                .iter()
                .find_map(|(id, expression)| {
                    if let ExpressionKind::EnumCondition {
                        source,
                        operation: ValueInputOperation::Take,
                        ..
                    } = expression.kind
                    {
                        Some((query, id, Holder::Expression(source)))
                    } else {
                        None
                    }
                })
        })
        .expect("one checked take condition");
    let location = query
        .graph()
        .points()
        .iter()
        .find_map(|(point, block)| {
            block
                .actions
                .iter()
                .position(|action| action.operation == Operation::Evaluate(condition))
                .map(|action| Location { point, action })
        })
        .expect("the condition evaluates on a reachable path");
    let retirement =
        |step: &Step| matches!(step, Step::Dispose { source, .. } if source.holder() == carrier);
    let mut transitions = query.access().resources().at(location).peekable();
    assert!(transitions.peek().is_some());
    for transition in transitions {
        assert!(!transition.before_steps().iter().any(retirement));
        assert_eq!(
            transition
                .after_steps()
                .iter()
                .filter(|step| retirement(step))
                .count(),
            1
        );
    }
}

#[test]
fn scalar_enum_match_requires_structural_coverage_even_for_a_known_input() {
    let root = "see ./body.nct\nenum Status { idle\nactive }\nenum Other { idle }\ntype State = Status\nfunc run(): i32";
    for (arms, message) in [
        ("Status.active { 1 }", "cover the complete visible enum"),
        (
            "Status.active { 1 }\nState.active { 2 }\n_ { 0 }",
            "this variant already has a match arm",
        ),
        (
            "_ { 0 }\nStatus.active { 1 }",
            "the discard arm must be last",
        ),
        (
            "Status.active { 1 }\nStatus.idle { 0 }\n_ { 2 }",
            "all variants are already covered",
        ),
        (
            "Status.active { 1 }\nOther.idle { 0 }",
            "must name a variant of the same enum",
        ),
    ] {
        let body =
            format!("see ./index.nct\nfunc run(): i32 {{ match Status.active {{\n{arms}\n}} }}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let Err(SourceFailure::Checking(accepted::Failure::Body {
            failure: Failure::Rejected(diagnostic),
            ..
        })) = ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("expected coverage rejection: {body}");
        };
        assert!(
            diagnostic.source().message().contains(message),
            "{diagnostic:?}"
        );
    }
}

fn counts(body_increment: u64) -> Fixture {
    let body_count =
        format!("const BASE: usize = 39\nconst func width(): usize {{ BASE + {body_increment} }}");
    Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./a.nct\nsee ./root_count.nct\nfunc run(): [i32; width()]",
            ),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./body_count.nct\nfunc run(): [i32; width()] { [0; 42] }",
            ),
            (
                "/app/root_count.nct",
                Implementation,
                "const BASE: usize = 40\nconst func width(): usize { BASE + 2 }",
            ),
            ("/app/body_count.nct", Implementation, &body_count),
        ],
        &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, 3)],
    )
}

#[test]
fn module_source_coordinator_executes_each_header_supplier_before_accepting_ordinary_bodies() {
    let fixture = counts(3);
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = source.unit().program();
    let run = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    let result = program.codes().get(run).unwrap().result();
    assert!(
        matches!(program.data_schema(result), Some(PayloadSchema::Array(array)) if array.length() == 42)
    );
    let context = source.declarations().current().unwrap();
    source.unit().bind(&context).unwrap();
    assert_eq!(
        context.for_code(run).unwrap().syntax().tree().source(),
        fixture.tree(1).source()
    );
    assert!(source.unit().program().headers_complete());
}

#[test]
fn reversed_comparison_commits_and_cleans_error_temporaries_in_written_order() {
    comparison_cleanup(">", &[1, 0]);
    comparison_cleanup(">=", &[1, 0, 0, 1]);
}

fn comparison_cleanup(operator: &str, preparation: &[usize]) {
    use crate::value_access::flow::{
        Location, Operation,
        resources::{Disposal, Holder, Step},
    };

    let implementation = "see ./index.nct\n\
        struct Ticket { rank: i32\ncause: error }\n\
        impl Ticket {\n\
            operator (self < other: Self): bool { self.rank < other.rank }\n\
            operator (self == other: Self): bool { self.rank == other.rank }\n\
        }\n\
        func run(take a: error, take b: error): bool {\n\
            let left = Ticket { rank: 2, cause: a^ }\n\
            let right = Ticket { rank: 1, cause: b^ }\n\
            left^ COMPARISON right^\n\
        }"
    .replace("COMPARISON", operator);
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nfunc run(take a: error, take b: error): bool",
            ),
            ("/app/body.nct", Implementation, &implementation),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let unit = source.unit();
    let program = unit.program();
    let body = unit
        .body(
            program
                .resolve(program.symbols().get("run").unwrap())
                .unwrap(),
        )
        .unwrap();
    let mut prepared = Vec::new();
    let mut committed = Vec::new();
    let mut normal_cleanup = false;
    for (point, block) in body.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if let Operation::PrepareArgument { argument, .. } = event.operation {
                prepared.push(argument);
            }
            for transition in body.access().resources().at(Location { point, action }) {
                let mut disposed = Vec::new();
                for step in transition.steps() {
                    match step {
                        Step::Move {
                            destination,
                            payload,
                            ..
                        } if let Holder::Temporary { index, .. } = destination.holder() => {
                            assert!(matches!(event.operation, Operation::CommitCall(_)));
                            assert_eq!(payload.disposal(), Disposal::Aggregate);
                            committed.push(index);
                        }
                        Step::Dispose { source, payload }
                            if let Holder::Temporary { index, .. } = source.holder() =>
                        {
                            assert!(matches!(event.operation, Operation::EndEvaluation(_)));
                            assert_eq!(payload.disposal(), Disposal::Aggregate);
                            disposed.push(index);
                        }
                        _ => {}
                    }
                }
                if !disposed.is_empty() {
                    assert_eq!(disposed, [0, 1]);
                    normal_cleanup = true;
                }
            }
        }
    }
    // The written left operand becomes formal other, while cleanup releases right then left.
    assert_eq!(prepared, preparation);
    assert_eq!(committed, [1, 0]);
    assert!(normal_cleanup);
}

#[test]
fn module_source_coordinator_rejects_real_count_disagreement_at_both_physical_headers() {
    let fixture = counts(4);
    let Err(SourceFailure::Modules(declarations::ModuleSourcesFailure::Contract(error))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("actual header disagreement")
    };
    assert_eq!(
        error.rule(),
        nocter_declaration_lowering::DeclarationContractRule::MismatchedBody
    );
    assert_eq!(error.source().primary().source(), fixture.tree(1).source());
    assert_eq!(
        error.source().notes()[0].origin().source(),
        fixture.tree(0).source()
    );
}

#[test]
fn module_source_coordinator_checks_unused_sources_and_projects_their_actual_rejections() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nfunc run(): i32 { 42 }",
            ),
            (
                "/app/unused.nct",
                Implementation,
                "func unused(): i32 { missing }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Checking(accepted::Failure::Body {
        failure: Failure::Rejected(error),
        ..
    })) = ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("unused body")
    };
    assert_eq!(error.rule(), Rule::UnknownName);
    assert_eq!(error.source().primary().source(), fixture.tree(2).source());
}

#[test]
fn module_source_coordinator_keeps_header_cycle_diagnostics_on_complete_representation_fields() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Node"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Node { next: Node }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Headers(errors))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("record header cycle")
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].source().primary().source(),
        fixture.tree(1).source()
    );
    assert!(matches!(
        errors[0].violation().rule,
        nocter_declarations::value_access::DeclarationRule::RecursivePayload { .. }
    ));
}

#[test]
fn module_source_coordinator_accepts_empty_library_sources_and_rejects_unselected_primitives() {
    let empty = Fixture::new(&[("/app/index.nct", Root, "")], &[]);
    let source = ModuleSourceUnit::prepare(&empty.input()).unwrap();
    assert!(source.unit().program().codes().is_empty());
    let primitive = Fixture::new(
        &[
            ("/app/index.nct", Root, ""),
            (
                "/app/body.nct",
                Implementation,
                "primitive func operation(): i32",
            ),
        ],
        &[],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Primitive(error))) =
        ModuleSourceUnit::prepare(&primitive.input())
    else {
        panic!("unselected primitive")
    };
    assert_eq!(error.rule(), declarations::PrimitiveRule::Unauthorized);
    assert_eq!(
        error.source().primary().source(),
        primitive.tree(1).source()
    );
}

#[test]
fn module_visibility_sealing_checks_signature_names_without_exposing_hidden_representation_fields()
{
    for (root, body, accepted) in [
        (
            "struct Outer\nfunc inspect(value: Outer): void",
            "struct Outer { hidden: Private }\nstruct Private {}\nfunc inspect(value: Outer): void {}",
            true,
        ),
        (
            "struct Outer { shown: Private }",
            "struct Outer { shown: Private }\nstruct Private {}",
            false,
        ),
        (
            "func inspect(value: Private): void",
            "struct Private {}\nfunc inspect(value: Private): void {}",
            false,
        ),
        (
            "func inspect(value: [Private; 0]): void",
            "struct Private {}\nfunc inspect(value: [Private; 0]): void {}",
            false,
        ),
        (
            "func inspect(action: func(Private): void): void",
            "struct Private {}\nfunc inspect(action: func(Private): void): void {}",
            false,
        ),
        (
            "#visibility: \"/\"\nstruct Restricted\nfunc inspect(value: Restricted): void",
            "struct Restricted {}\nfunc inspect(value: Restricted): void {}",
            false,
        ),
        (
            "#visibility: \"/\"\nstruct Restricted\n#visibility: \"./\"\nfunc inspect(value: Restricted): void",
            "struct Restricted {}\nfunc inspect(value: Restricted): void {}",
            true,
        ),
    ] {
        let root = format!("see ./body.nct\n{root}");
        let body = format!("see ./index.nct\n{body}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if accepted {
            assert!(result.is_ok(), "{root}: {result:?}");
        } else {
            let Err(SourceFailure::Declarations(declarations::Failure::Visibility(error))) = result
            else {
                panic!("signature exposure: {root}: {result:?}")
            };
            assert_eq!(
                error.rule(),
                declarations::VisibilityRule::SignatureExposure
            );
            assert_eq!(error.source().primary().source(), fixture.tree(0).source());
            assert_eq!(error.source().notes().len(), 1);
        }
    }
}

#[test]
fn selected_type_imports_and_root_reexports_share_the_original_nominal_identity() {
    use nocter_compile_input::ModuleIdentity;
    use nocter_declarations::value_access::Namespace;
    use nocter_model::PackageIdentity;
    let fixture = Fixture::new(&[
        ("/app/index.nct", Root, "use ./wire.Item as PublicItem\nsee ./body.nct\nfunc inspect(value: PublicItem): void"),
        ("/app/body.nct", Implementation, "use ./wire.Item as PublicItem\nsee ./index.nct\nfunc inspect(value: PublicItem): void {}"),
        ("/app/wire/index.nct", Root, "use ../data.Data as Item"),
        ("/app/data/index.nct", Root, "see ./body.nct\nstruct Data"),
        ("/app/data/body.nct", Implementation, "see ./index.nct\nstruct Data { value: i32 }"),
    ], &[(0, 0, 1), (1, 0, 0), (3, 0, 4), (4, 0, 3)])
    .with_modules(&[(&[], &[0, 1]), (&["wire"], &[2]), (&["data"], &[3, 4])])
    .with_uses(&[(0, 0, &["wire"]), (1, 0, &["wire"]), (2, 0, &["data"])]);
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = source.declarations().program();
    let identity = |path: &[&str]| {
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied())
    };
    let root = identity(&[]);
    let lookup = |path: &[&str], name| {
        source
            .declarations()
            .lookup_export(
                &identity(path),
                &root,
                Namespace::Type,
                program.symbols().get(name).unwrap(),
            )
            .unwrap()
    };
    assert_eq!(lookup(&[], "PublicItem"), lookup(&["wire"], "Item"));
    assert_eq!(lookup(&[], "PublicItem"), lookup(&["data"], "Data"));
    let retained = source.declarations().query().unwrap();
    for (path, name) in [
        (&[][..], "PublicItem"),
        (&["wire"][..], "Item"),
        (&["data"][..], "Data"),
    ] {
        assert_eq!(
            retained.lookup_export(
                &identity(path),
                &root,
                Namespace::Type,
                retained.program().symbols().get(name).unwrap()
            ),
            Some(lookup(path, name))
        );
    }
    let current = source.declarations().current().unwrap();
    assert!(source.unit().bind(&current).is_ok());
}

#[test]
fn selected_type_imports_diagnose_missing_private_non_type_and_widening_names_at_the_import() {
    use nocter_declaration_lowering::{ImportRule, NamespaceRule};
    for (import, restriction, expected_import, expected_namespace) in [
        (
            "use ./data.Absent",
            "",
            Some(ImportRule::MissingImportedName),
            None,
        ),
        (
            "use ./data.Hidden",
            "",
            Some(ImportRule::MissingImportedName),
            None,
        ),
        (
            "use ./data.compute",
            "",
            Some(ImportRule::NonTypeSelection),
            None,
        ),
        (
            "use ./data.Data",
            "#visibility: \"./\"\n",
            Some(ImportRule::InaccessibleImportedName),
            None,
        ),
        (
            "use ./data.Data",
            "#visibility: \"/\"\n",
            Some(ImportRule::WideningReexport),
            None,
        ),
        (
            "#visibility: \"/\"\nuse ./data.Data",
            "#visibility: \"/\"\n",
            None,
            None,
        ),
        (
            "use ./data.{Data as Item, Data as Item}",
            "",
            None,
            Some(NamespaceRule::NameCollision),
        ),
        (
            "use ./data.Data as i32",
            "",
            None,
            Some(NamespaceRule::ReservedName),
        ),
    ] {
        let root = format!("see ./body.nct\n{restriction}struct Data\nfunc compute(): i32");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, import),
                ("/app/data/index.nct", Root, &root),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data {}\nstruct Hidden {}\nfunc compute(): i32 { 42 }",
                ),
            ],
            &[(1, 0, 2), (2, 0, 1)],
        )
        .with_modules(&[(&[], &[0]), (&["data"], &[1, 2])])
        .with_uses(&[(0, 0, &["data"])]);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        let diagnostic = match (result, expected_import, expected_namespace) {
            (Ok(_), None, None) => continue,
            (
                Err(SourceFailure::Declarations(declarations::Failure::Import(diagnostic))),
                Some(rule),
                None,
            ) => {
                assert_eq!(diagnostic.rule(), rule, "{import}");
                diagnostic.source().clone()
            }
            (
                Err(SourceFailure::Declarations(declarations::Failure::Namespace(diagnostic))),
                None,
                Some(rule),
            ) => {
                assert_eq!(diagnostic.rule(), rule, "{import}");
                diagnostic.source().clone()
            }
            (result, _, _) => panic!("{import}: {result:?}"),
        };
        assert_eq!(diagnostic.primary().source(), fixture.tree(0).source());
    }
}

#[test]
fn seeing_a_source_does_not_inherit_its_imports_into_either_header() {
    for importing in [0, 1] {
        let texts = [
            "see ./body.nct\nfunc inspect(value: Item): void",
            "see ./index.nct\nfunc inspect(value: Item): void {}",
        ];
        let mut texts = texts.map(str::to_owned);
        texts[importing] = format!("use ./data.Data as Item\n{}", texts[importing]);
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &texts[0]),
                ("/app/body.nct", Implementation, &texts[1]),
                ("/app/data/index.nct", Root, "see ./body.nct\nstruct Data"),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data {}",
                ),
            ],
            &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)],
        )
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(importing, 0, &["data"])]);
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("imports must remain physical-source-local")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source().code() == "E0290"
                    && diagnostic.source().primary().source()
                        == fixture.tree(1 - importing).source())
        );
    }
}

#[test]
fn imported_nominals_do_not_grant_hidden_fields_or_structural_construction() {
    for (root_record, definition, signature, expression) in [
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func inspect(value: Data): i32",
            "value.hidden",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func inspect(take value: Data): i32",
            "(value^).hidden",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func build(): Data",
            "Data { shown: 1, hidden: 2 }",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32 }",
            "func build(): Data",
            "Data { shown: 1 }",
        ),
        (
            "struct Data",
            "struct Data {}",
            "func build(): Data",
            "Data {}",
        ),
    ] {
        let app = format!("use ./data.Data\nsee ./body.nct\n{signature}");
        let app_body = format!("use ./data.Data\nsee ./index.nct\n{signature} {{ {expression} }}");
        let data = format!("see ./body.nct\n{root_record}");
        let data_body = format!("see ./index.nct\n{definition}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &app),
                ("/app/body.nct", Implementation, &app_body),
                ("/app/data/index.nct", Root, &data),
                ("/app/data/body.nct", Implementation, &data_body),
            ],
            &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)],
        )
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(0, 0, &["data"]), (1, 0, &["data"])]);
        let Err(SourceFailure::Checking(accepted::Failure::Body {
            failure: Failure::Rejected(diagnostic),
            ..
        })) = ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("expected a source permission rejection: {expression}")
        };
        assert_eq!(
            diagnostic.rule(),
            if expression.ends_with(".hidden") {
                Rule::InaccessibleField
            } else {
                Rule::InaccessibleRepresentation
            }
        );
        assert_eq!(
            diagnostic.source().primary().source(),
            fixture.tree(1).source()
        );
    }
}

#[test]
fn visible_field_audiences_and_complete_definition_views_do_not_depend_on_compiler_schema_knowledge()
 {
    for restrict in [false, true] {
        let data = format!(
            "see ./body.nct\nstruct Data {{\n{}shown: i32\n}}",
            if restrict {
                "#visibility: \"./\"\n"
            } else {
                ""
            }
        );
        let fixture = Fixture::new(&[
            ("/app/index.nct", Root, "use ./data.Data\nsee ./body.nct\nfunc inspect(value: Data): i32"),
            ("/app/body.nct", Implementation, "use ./data.Data\nsee ./index.nct\nfunc inspect(value: Data): i32 { value.shown }"),
            ("/app/data/index.nct", Root, &data),
            ("/app/data/body.nct", Implementation, "see ./index.nct\nstruct Data { shown: i32\nhidden: i32 }\nfunc build(): Data { Data { shown: 1, hidden: 2 } }\nfunc inspect(value: Data): i32 { value.hidden }"),
        ], &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)])
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(0, 0, &["data"]), (1, 0, &["data"])]);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if restrict {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("restricted field: {result:?}")
            };
            assert_eq!(diagnostic.rule(), Rule::InaccessibleField);
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(1).source()
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
    for see_representation in [false, true] {
        let caller = format!(
            "see ./index.nct\n{}func inspect(value: Data): i32 {{ value.hidden }}",
            if see_representation {
                "see ./representation.nct\n"
            } else {
                ""
            }
        );
        let mut edges = vec![(0, 0, 1), (0, 1, 2), (1, 0, 0), (2, 0, 0)];
        if see_representation {
            edges.push((2, 1, 1));
        }
        let fixture = Fixture::new(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./representation.nct\nsee ./caller.nct\nstruct Data\nfunc inspect(value: Data): i32",
                ),
                (
                    "/app/representation.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data { hidden: i32 }",
                ),
                ("/app/caller.nct", Implementation, &caller),
            ],
            &edges,
        );
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if see_representation {
            assert!(result.is_ok(), "{result:?}");
        } else {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("transitive complete representation: {result:?}")
            };
            assert_eq!(diagnostic.rule(), Rule::InaccessibleField);
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(2).source()
            );
        }
    }
}

#[test]
fn qualified_namespace_reexports_use_one_target_for_headers_values_calls_and_member_chains() {
    let fixture = nocter_test_support::qualified_namespace_fixture();
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let current = source.declarations().current().unwrap();
    let program = current.program();
    let result = program
        .resolve_constant_in(
            current.lookup_scope(),
            program.symbols().get("RESULT").unwrap(),
        )
        .unwrap();
    assert_eq!(
        source.unit().constant_value(result).unwrap().node(),
        &nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(42))
    );
    let values = program
        .resolve_in(
            current.lookup_scope(),
            program.symbols().get("values").unwrap(),
        )
        .unwrap();
    let PayloadSchema::Array(array) = program
        .data_schema(program.codes().get(values).unwrap().result())
        .unwrap()
    else {
        panic!("array result");
    };
    assert_eq!(array.length(), 2);
    assert!(source.unit().bind(&current).is_ok());
}

fn namespace_access_fixture(body: &str) -> Fixture {
    let body = format!("use ./data/bridge as api\nsee ./index.nct\nfunc run(): i32 {{ {body} }}");
    Fixture::new(&[
        ("/app/index.nct", Root, "use ./data/bridge as api\nsee ./body.nct\nfunc run(): i32"),
        ("/app/body.nct", Implementation, &body),
        ("/app/data/bridge/index.nct", Root, "use /data as inner\nsee ./body.nct\nconst func safe(): i32"),
        ("/app/data/bridge/body.nct", Implementation, "use /data as inner\nsee ./index.nct\nconst func safe(): i32 { inner.hidden() }"),
        ("/app/data/index.nct", Root, "see ./body.nct\n#visibility: \"./\"\nconst func hidden(): i32\n#visibility: \"./\"\nstruct Hidden\nstruct Data { shown: i32 }\nfunc make(): Data"),
        ("/app/data/body.nct", Implementation, "see ./index.nct\nconst func hidden(): i32 { 42 }\nstruct Hidden {}\nstruct Data { shown: i32\nhidden: i32 }\nfunc make(): Data { Data { shown: 42, hidden: 1 } }"),
    ], &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2), (4, 0, 5), (5, 0, 4)])
    .with_modules(&[(&[], &[0, 1]), (&["data", "bridge"], &[2, 3]), (&["data"], &[4, 5])])
    .with_uses(&[(0, 0, &["data", "bridge"]), (1, 0, &["data", "bridge"]), (2, 0, &["data"]), (3, 0, &["data"])])
}

#[test]
fn qualified_namespace_paths_preserve_source_permission_and_never_become_values_or_owners() {
    for (body, expected) in [
        ("api.safe()", None),
        ("api.inner.make().shown", None),
        (
            "let call = api.inner.make\nlet next = call^\nnext().shown",
            None,
        ),
        ("api.inner.hidden()", Some(Rule::InaccessibleName)),
        (
            "let values: [api.inner.Hidden; 0] = []\n42",
            Some(Rule::InaccessibleName),
        ),
        ("api.inner.make().hidden", Some(Rule::InaccessibleField)),
        (
            "let value = api.inner.Data { shown: 42, hidden: 1 }\n42",
            Some(Rule::InaccessibleRepresentation),
        ),
        ("let ns = api\n42", Some(Rule::NamespaceValue)),
        ("let api = 42\napi", Some(Rule::ShadowedName)),
        ("api.inner.absent()", Some(Rule::UnknownName)),
        ("api.inner", Some(Rule::NamespaceValue)),
        ("api.inner.make^", Some(Rule::MissingTransfer)),
    ] {
        let fixture = namespace_access_fixture(body);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if let Some(expected) = expected {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("{body}: {result:?}")
            };
            assert_eq!(diagnostic.rule(), expected, "{body}");
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(1).source()
            );
        } else {
            assert!(result.is_ok(), "{body}: {result:?}");
        }
    }
}

#[test]
fn qualified_headers_check_audiences_even_for_empty_arrays() {
    for annotation in ["api.inner.Hidden", "[api.inner.Hidden; 0]", "api.inner"] {
        let root = format!(
            "use ./data/bridge as api\nsee ./body.nct\nfunc inspect(value: {annotation}): void"
        );
        let body = format!(
            "use ./data/bridge as api\nsee ./index.nct\nfunc inspect(value: {annotation}): void {{}}"
        );
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &root),
                ("/app/body.nct", Implementation, &body),
                ("/app/data/bridge/index.nct", Root, "use /data as inner"),
                (
                    "/app/data/index.nct",
                    Root,
                    "see ./body.nct\n#visibility: \"./\"\nstruct Hidden",
                ),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Hidden {}",
                ),
            ],
            &[(0, 0, 1), (1, 0, 0), (3, 0, 4), (4, 0, 3)],
        )
        .with_modules(&[
            (&[], &[0, 1]),
            (&["data", "bridge"], &[2]),
            (&["data"], &[3, 4]),
        ])
        .with_uses(&[
            (0, 0, &["data", "bridge"]),
            (1, 0, &["data", "bridge"]),
            (2, 0, &["data"]),
        ]);
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("inaccessible header: {annotation}");
        };
        let expected = if annotation == "api.inner" {
            "E0290"
        } else {
            "E0412"
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source().code() == expected
                    && diagnostic.source().primary().source() == fixture.tree(0).source()),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn matching_qualified_header_spellings_cannot_choose_different_module_targets() {
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "use ./a as item\nsee ./body.nct\nfunc inspect(value: item.Data): void",
            ),
            (
                "/app/body.nct",
                Implementation,
                "use ./b as item\nsee ./index.nct\nfunc inspect(value: item.Data): void {}",
            ),
            ("/app/a/index.nct", Root, "see ./body.nct\nstruct Data"),
            (
                "/app/a/body.nct",
                Implementation,
                "see ./index.nct\nstruct Data {}",
            ),
            ("/app/b/index.nct", Root, "see ./body.nct\nstruct Data"),
            (
                "/app/b/body.nct",
                Implementation,
                "see ./index.nct\nstruct Data {}",
            ),
        ],
        &[
            (0, 0, 1),
            (1, 0, 0),
            (2, 0, 3),
            (3, 0, 2),
            (4, 0, 5),
            (5, 0, 4),
        ],
    )
    .with_modules(&[(&[], &[0, 1]), (&["a"], &[2, 3]), (&["b"], &[4, 5])])
    .with_uses(&[(0, 0, &["a"]), (1, 0, &["b"])]);
    let Err(SourceFailure::Modules(declarations::ModuleSourcesFailure::Contract(diagnostic))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("identical path spelling must not merge different targets");
    };
    assert_eq!(
        diagnostic.rule(),
        nocter_declaration_lowering::DeclarationContractRule::MismatchedBody
    );
    assert_eq!(
        diagnostic.source().primary().source(),
        fixture.tree(1).source()
    );
    assert_eq!(
        diagnostic.source().notes()[0].origin().source(),
        fixture.tree(0).source()
    );
}

#[test]
fn namespace_aliases_collide_with_selected_types_and_authored_values_in_either_order() {
    for imports in [
        "use ./data.Data as api\nuse ./data as api",
        "use ./data as api\nuse ./data.Data as api",
        "use ./data as api\nuse ./data as api",
    ] {
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, imports),
                ("/app/data/index.nct", Root, "see ./body.nct\nstruct Data"),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data {}",
                ),
            ],
            &[(1, 0, 2), (2, 0, 1)],
        )
        .with_modules(&[(&[], &[0]), (&["data"], &[1, 2])])
        .with_uses(&[(0, 0, &["data"]), (0, 1, &["data"])]);
        let Err(SourceFailure::Declarations(declarations::Failure::Namespace(diagnostic))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("colliding import bindings: {imports}");
        };
        assert_eq!(
            diagnostic.rule(),
            nocter_declaration_lowering::NamespaceRule::NameCollision
        );
        assert_eq!(
            diagnostic.source().primary().source(),
            fixture.tree(0).source()
        );
        assert_eq!(diagnostic.source().notes().len(), 1);
    }
}

fn block_import_fixture(body: &str, uses: &[(usize, &[&str])]) -> Fixture {
    let body = format!("see ./index.nct\nfunc run(): i32 {{ {body} }}");
    let uses = uses
        .iter()
        .map(|&(occurrence, target)| (1, occurrence, target))
        .collect::<Vec<_>>();
    Fixture::new(&[
        ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
        ("/app/body.nct", Implementation, &body),
        ("/app/data/index.nct", Root, "see ./body.nct\nconst func answer(): i32\nstruct Data { shown: i32 }\n#visibility: \"./\"\nstruct Hidden"),
        ("/app/data/body.nct", Implementation, "see ./index.nct\nconst func answer(): i32 { 42 }\nstruct Data { shown: i32 }\nstruct Hidden {}"),
        ("/app/other/index.nct", Root, "see ./body.nct\nconst func answer(): i32"),
        ("/app/other/body.nct", Implementation, "see ./index.nct\nconst func answer(): i32 { 7 }"),
    ], &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2), (4, 0, 5), (5, 0, 4)])
    .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3]), (&["other"], &[4, 5])])
    .with_uses(&uses)
}

#[test]
fn lexical_imports_retain_parents_without_leaking_to_siblings_or_after_the_block() {
    for (body, uses, expected) in [
        (
            "use ./data as api\nlet first = api.answer()\nlet inner = if true { use ./other as api\napi.answer() } else { 0 }\nif first == 42 && inner == 7 { api.answer() } else { 1 }",
            vec![(0, &["data"][..]), (1, &["other"][..])],
            None,
        ),
        (
            "let inner = if true { use ./data as api\napi.answer() } else { 0 }\napi.answer()",
            vec![(0, &["data"][..])],
            Some(Rule::UnknownName),
        ),
        (
            "if true { use ./data as api\napi.answer() } else { api.answer() }",
            vec![(0, &["data"][..])],
            Some(Rule::UnknownName),
        ),
        (
            "let inner = if true { use ./data.Data as Local\n42 } else { 0 }\nlet value: [Local; 0] = []\n42",
            vec![(0, &["data"][..])],
            Some(Rule::UnknownType),
        ),
        (
            "let api = 1\nif true { use ./data as api\n42 } else { 0 }",
            vec![(0, &["data"][..])],
            Some(Rule::ShadowedName),
        ),
        (
            "use ./data as api\nlet api = 1\n42",
            vec![(0, &["data"][..])],
            Some(Rule::ShadowedName),
        ),
        (
            "let call: func(i32): i32 = (api: i32): i32 { use ./data as api\n42 }\n42",
            vec![(0, &["data"][..])],
            Some(Rule::ShadowedName),
        ),
    ] {
        let fixture = block_import_fixture(body, &uses);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if let Some(expected) = expected {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("{body}: {result:?}");
            };
            assert_eq!(diagnostic.rule(), expected, "{body}");
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(1).source()
            );
        } else {
            assert!(result.is_ok(), "{body}: {result:?}");
        }
    }
}

#[test]
fn unreachable_block_imports_still_validate_selection_audiences_kinds_and_duplicate_names() {
    use nocter_declaration_lowering::ImportRule;
    for (imports, expected) in [
        ("use ./data.Absent", ImportRule::MissingImportedName),
        ("use ./data.Hidden", ImportRule::InaccessibleImportedName),
        ("use ./data.answer", ImportRule::NonTypeSelection),
    ] {
        let body = format!("if false {{ {imports}\n42 }} else {{ 42 }}");
        let fixture = block_import_fixture(&body, &[(0, &["data"])]);
        let Err(SourceFailure::Declarations(declarations::Failure::Import(diagnostic))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("{imports}");
        };
        assert_eq!(diagnostic.rule(), expected);
        assert_eq!(
            diagnostic.source().primary().source(),
            fixture.tree(1).source()
        );
    }
    let fixture = block_import_fixture(
        "if false { use ./data as api\nuse ./other as api\n42 } else { 42 }",
        &[(0, &["data"]), (1, &["other"])],
    );
    assert!(matches!(
        ModuleSourceUnit::prepare(&fixture.input()),
        Err(SourceFailure::Declarations(
            declarations::Failure::Namespace(_)
        ))
    ));
}
