use super::*;
use nocter_declarations::value_access::Namespace;
use nocter_model::{BuiltinType, ValueInputOperation};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};
mod body_expressions;
mod source_roles;
pub(super) fn input(text: &str, goal: ParseGoal) -> (SourceMap, SyntaxTree) {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("unit.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), goal);
    (sources, tree)
}
pub(super) fn bound<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> BoundSyntax<'a> {
    BoundSyntax::new(sources.get(tree.source()).unwrap(), tree).unwrap()
}
#[test]
fn input_modes_and_value_result_come_only_from_the_header() {
    let (sources, tree) = input(
        "func f(value: i32, edit target: i32, take owned: error): error { missing }",
        ParseGoal::V2CoreUnit,
    );
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let program = query.program();
    let (_, callable) = program.codes().iter().next().unwrap();
    assert_eq!(
        program.data_kind(callable.result()),
        Some(BuiltinType::Error)
    );
    let modes: Vec<_> = callable
        .parameters()
        .iter()
        .map(|id| program.parameters().get(*id).unwrap().operation())
        .collect();
    assert_eq!(
        modes,
        [
            ValueInputOperation::Read,
            ValueInputOperation::Edit,
            ValueInputOperation::Take
        ]
    );
}
#[test]
fn body_and_trivia_edits_rebind_without_redeciding_headers() {
    let (a, ta) = input(
        "func f(take value: error): error { return value^ }",
        ParseGoal::V2CoreUnit,
    );
    let query = Query::prepare(bound(&a, &ta)).unwrap();
    let (b, tb) = input(
        "// new source\nfunc f(take value: error): error { unknown }",
        ParseGoal::V2CoreUnit,
    );
    let context = query.bind(bound(&b, &tb)).unwrap();
    let same = query.bind(bound(&a, &ta)).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &context.program_handle(),
        &same.program_handle()
    ));
    let (c, tc) = input(
        "func f(value: error): error { return value }",
        ParseGoal::V2CoreUnit,
    );
    assert!(matches!(
        query.bind(bound(&c, &tc)),
        Err(Failure::ChangedHeader)
    ));
}
#[test]
fn canonical_reservation_is_independent_of_source_order() {
    let (a, ta) = input(
        "func beta(take x: error): error { x^ }\nfunc alpha(): void {}",
        ParseGoal::V2CoreUnit,
    );
    let (b, tb) = input(
        "func alpha(): void {}\nfunc beta(take x: error): error { x^ }",
        ParseGoal::V2CoreUnit,
    );
    let qa = Query::prepare(bound(&a, &ta)).unwrap();
    let qb = Query::prepare(bound(&b, &tb)).unwrap();
    assert_eq!(qa.program().codes(), qb.program().codes());
    assert_eq!(qa.program().parameters(), qb.program().parameters());
}

#[test]
fn field_and_formal_input_reservations_are_independent_of_source_definition_order() {
    use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
    use nocter_model::{ValueFunctionOrigin, ValueInvocation};
    let records = "copy struct Item { value: i32\nflag: bool }";
    let functions = [
        "func apply(action: func(i32): i32, edit item: Item): i32 { unknown }",
        "func identity(value: i32): i32 { value }",
    ];
    let first = format!("{records}\n{}\n{}", functions[0], functions[1]);
    let second = format!("{}\n{}\n{records}", functions[1], functions[0]);
    let (sources_a, tree_a) = input(&first, ParseGoal::V2CoreUnit);
    let (sources_b, tree_b) = input(&second, ParseGoal::V2CoreUnit);
    let a = Query::prepare(bound(&sources_a, &tree_a)).unwrap();
    let b = Query::prepare(bound(&sources_b, &tree_b)).unwrap();
    assert_eq!(a.program().codes(), b.program().codes());
    assert_eq!(a.program().parameters(), b.program().parameters());
    assert_eq!(a.program().records(), b.program().records());
    assert_eq!(a.program().fields(), b.program().fields());
    let program = a.program();
    let apply = program
        .resolve(program.symbols().get("apply").unwrap())
        .unwrap();
    let parameter_id = program.codes().get(apply).unwrap().parameters()[0];
    let parameter = program.parameters().get(parameter_id).unwrap();
    assert_eq!(
        program.data_schema(parameter.data()),
        Some(PayloadSchema::Function(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter_id),
            invocation: ValueInvocation::Read,
        }))
    );
    assert_eq!((parameter.owner(), parameter.position()), (apply, 0));
}
#[test]
fn header_errors_are_local_and_cannot_be_repaired_by_a_body() {
    for text in [
        "func f(): void {}\nfunc f(): void {}",
        "func f(x: i32, x: i32): void {}",
        "func i32(): void {}",
        "func f(f: i32): void {}",
    ] {
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let Err(Failure::Headers(diagnostics)) = Query::prepare(bound(&sources, &tree)) else {
            panic!("{text}")
        };
        assert!(!diagnostics.is_empty());
    }
}

#[test]
fn namespace_closure_precedes_type_resolution_and_keeps_current_conflict_locations() {
    use nocter_declarations::value_access::{DeclarationRule, HeaderPosition};
    for source in [
        "struct Shared { value: Missing }\nfunc Shared(): Missing { unknown }\nfunc Shared(): Missing { other }",
        "// current coordinates\nfunc Shared(): Missing { other }\nstruct Shared { value: Missing }\nfunc Shared(): Missing { unknown }",
    ] {
        let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
        let Err(Failure::Headers(diagnostics)) = Query::prepare(bound(&sources, &tree)) else {
            panic!("expected namespace rejection: {source}");
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(matches!(
            diagnostic.violation().rule,
            DeclarationRule::DuplicateValue { .. }
        ));
        assert_eq!(diagnostic.violation().position, HeaderPosition::Name);
        let primary = diagnostic.source().primary().span().range();
        let name = source.rfind("func Shared").unwrap() + "func ".len();
        assert_eq!(primary.start().get() as usize, name);
        assert_eq!(primary.end().get() as usize, name + "Shared".len());
        assert_eq!(diagnostic.source().notes().len(), 1);
    }
}
#[test]
fn wrong_goal_and_retired_source_cannot_publish_declarations() {
    let (sources, tree) = input("func f(): void {}", ParseGoal::SourceFile);
    assert!(matches!(
        Query::prepare(bound(&sources, &tree)),
        Err(Failure::WrongParseGoal)
    ));
    let (sources, tree) = input("func f(x: ^error): ^error { ^x }", ParseGoal::V2CoreUnit);
    assert!(matches!(
        Query::prepare(bound(&sources, &tree)),
        Err(Failure::Syntax(_))
    ));
}
#[test]
fn corpus_validates_every_header_and_rebinds_valid_declarations() {
    use nocter_json::Value;
    fn field<'a>(v: &'a Value, name: &str) -> &'a Value {
        let Value::Object(m) = v else {
            panic!("object")
        };
        &m.iter().find(|m| m.name.as_ref() == name).unwrap().value
    }
    fn string(v: &Value) -> &str {
        let Value::String(s) = v else {
            panic!("string")
        };
        s
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/value-bindings");
    let manifest =
        nocter_json::parse(&std::fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases")
    };
    let mut counts = [0; 3];
    for case in cases {
        let text = std::fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let (sources, tree) = input(&text, ParseGoal::V2CoreUnit);
        let result = Query::prepare(bound(&sources, &tree));
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(matches!(result, Err(Failure::Syntax(_))));
            counts[1] += 1;
        } else if string(field(case, "expectation")) == "header-reject" {
            let Err(Failure::Headers(diagnostics)) = result else {
                panic!("expected authored header rejection");
            };
            assert_eq!(
                diagnostics[0].source().code(),
                string(field(case, "diagnostic"))
            );
            counts[2] += 1;
        } else {
            let q = result.unwrap();
            assert!(q.bind(bound(&sources, &tree)).is_ok());
            counts[0] += 1;
        }
    }
    assert!(counts.iter().all(|&count| count > 0));
}

#[test]
fn callback_contracts_freeze_formal_identity_before_body_use_and_rebind_exactly() {
    use nocter_declarations::value_access::PayloadSchema;
    use nocter_model::ValueFunctionOrigin;
    let (sources, tree) = input(
        "func apply(action: func(edit i32): i32): i32 { unknown }",
        ParseGoal::V2CoreUnit,
    );
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let (id, parameter) = query.program().parameters().iter().next().unwrap();
    let Some(PayloadSchema::Function(witness)) = query.program().data_schema(parameter.data())
    else {
        panic!("formal schema");
    };
    assert_eq!(witness.origin, ValueFunctionOrigin::Parameter(id));
    assert_eq!(
        parameter.contract().unwrap().inputs[0].operation,
        ValueInputOperation::Edit
    );
    assert_eq!(query.program().data_copyable(parameter.data()), Some(false));
    let (edited, edited_tree) = input(
        "// changed body only\nfunc apply(action: func(edit i32): i32): i32 { 1 }",
        ParseGoal::V2CoreUnit,
    );
    let context = query.bind(bound(&edited, &edited_tree)).unwrap();
    assert!(Arc::ptr_eq(
        &context.program_handle(),
        &query.bind(bound(&sources, &tree)).unwrap().program_handle()
    ));
    let (changed, changed_tree) = input(
        "func apply(action: func(take i32): i32): i32 { 1 }",
        ParseGoal::V2CoreUnit,
    );
    assert!(matches!(
        query.bind(bound(&changed, &changed_tree)),
        Err(Failure::ChangedHeader)
    ));
}

#[test]
fn const_capability_is_header_authority_and_not_body_inference() {
    use nocter_model::CompileTimeGuarantee::{Evaluatable, RuntimeOnly};
    let text = "const func promised(): i32 { missing }\nfunc ordinary(): i32 { 1 }";
    let (a, ta) = input(text, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&a, &ta)).unwrap();
    for (name, expected) in [("promised", Evaluatable), ("ordinary", RuntimeOnly)] {
        let id = query
            .program()
            .resolve(query.program().symbols().get(name).unwrap())
            .unwrap();
        assert_eq!(
            query.program().codes().get(id).unwrap().compile_time(),
            expected
        );
    }
    let (b, tb) = input(&text.replace("missing", "4"), ParseGoal::V2CoreUnit);
    assert!(query.bind(bound(&b, &tb)).is_ok());
    let (c, tc) = input(&text.replace("const func", "func"), ParseGoal::V2CoreUnit);
    assert!(matches!(
        query.bind(bound(&c, &tc)),
        Err(Failure::ChangedHeader)
    ));
    let changed = Query::prepare(bound(&c, &tc)).unwrap();
    assert!(matches!(
        changed.bind(bound(&a, &ta)),
        Err(Failure::ChangedHeader)
    ));
}

#[test]
fn array_headers_request_their_real_length_code_without_inventing_a_schema() {
    use nocter_declarations::value_access::{CodeKind, HeaderPosition};
    for (source, annotation, position) in [
        (
            "struct Data { values: [i32; count()] }",
            "[i32; count()]",
            HeaderPosition::Field(0),
        ),
        (
            "func f(values: [i32; 0]): void {}",
            "[i32; 0]",
            HeaderPosition::Parameter(0),
        ),
        (
            "func f(): (bool, [i32; count()]) {}",
            "(bool, [i32; count()])",
            HeaderPosition::Result,
        ),
        (
            "func f(action: func(edit [i32; 2]): void): void {}",
            "func(edit [i32; 2]): void",
            HeaderPosition::Parameter(0),
        ),
        (
            "const DATA: [i32; 0] = missing",
            "[i32; 0]",
            HeaderPosition::Result,
        ),
        (
            "static DATA: [i32; 2] = missing",
            "[i32; 2]",
            HeaderPosition::Result,
        ),
    ] {
        let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
        assert!(!tree.has_errors(), "{source}: {:?}", tree.diagnostics());
        let syntax = bound(&sources, &tree);
        let mut preparation = Preparation::prepare(syntax).unwrap();
        let owner = preparation.declaration_ids().next().unwrap();
        let early = preparation.current();
        let guard = early.header_source(owner, position).unwrap();
        assert_eq!(
            syntax.text_at(tree.node(guard.bind(&early).unwrap()).unwrap().range()),
            Some(annotation)
        );
        let Err(Failure::RequiredValues(required)) = preparation.advance(owner) else {
            panic!("required length request: {source}");
        };
        assert_eq!(required.len(), 1);
        assert!(!preparation.current().program().header_ready(owner));
        let context = preparation.context(required[0]).unwrap();
        let code = context.program().codes().get(required[0]).unwrap();
        assert_eq!(code.name(), None);
        assert_eq!(
            code.kind(),
            CodeKind::TypeExpression(nocter_declarations::value_access::TypeExpressionSource {
                declaration: owner,
                position
            })
        );
        assert_eq!(
            context.program().data_kind(code.result()),
            Some(BuiltinType::Usize)
        );
    }
}

#[test]
fn unready_array_header_roots_preserve_the_exact_ordinary_length_expression() {
    use nocter_declarations::value_access::{DeclarationId, HeaderPosition};
    for (source, position) in [
        (
            "struct Data { values: [i32; if true { size() } else { 0 }] }",
            HeaderPosition::Field(0),
        ),
        (
            "func f(values: [i32; if true { size() } else { 0 }]): void {}",
            HeaderPosition::Parameter(0),
        ),
        (
            "func f(): [i32; if true { size() } else { 0 }] {}",
            HeaderPosition::Result,
        ),
    ] {
        let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
        let syntax = bound(&sources, &tree);
        let preparation = Preparation::prepare(syntax).unwrap();
        let context = preparation.current();
        let name = if source.starts_with("struct") {
            "Data"
        } else {
            "f"
        };
        let owner: DeclarationId = context
            .program()
            .declaration(
                if source.starts_with("struct") {
                    Namespace::Type
                } else {
                    Namespace::Value
                },
                context.program().symbols().get(name).unwrap(),
            )
            .unwrap();
        assert!(!context.program().header_ready(owner));
        let guard = context.header_source(owner, position).unwrap();
        let root = guard.bind(&context).unwrap();
        let ValueTypeSyntax::Data(DataTypeSyntax::Array { element, length }) =
            value_type_syntax(syntax, root).unwrap()
        else {
            panic!("array syntax");
        };
        assert!(matches!(*element, DataTypeSyntax::Named(_)));
        assert_eq!(
            syntax.text_at(tree.node(length).unwrap().range()),
            Some("if true { size() } else { 0 }")
        );
        let projection = nocter_syntax::BodySyntaxProjection::for_root(&tree, length).unwrap();
        let locator = projection
            .locator(nocter_syntax::SyntaxOrigin::Node(length))
            .unwrap();
        assert_eq!(
            projection.resolve(locator),
            Some(nocter_syntax::SyntaxOrigin::Node(length))
        );
        assert!(!context.program().header_ready(owner));
    }
}

#[test]
fn nested_header_lengths_have_distinct_code_roots_and_neutral_source_recipes() {
    use nocter_declarations::value_access::{CodeKind, HeaderPosition};
    let source = "struct Data { values: ([i32; inner()], [bool; outer()]) }\nfunc f(action: func(edit [i32; p()]): [void; q()]): [bool; r()] { missing }";
    let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let early = preparation.current();
    let expressions = preparation
        .code_ids()
        .filter(|&id| {
            matches!(
                early.code_source(id).unwrap().origin(),
                nocter_syntax::SyntaxOrigin::Node(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(expressions.len(), 5);
    assert_eq!(preparation.declaration_ids().count(), 2);
    let expected = [
        ("inner()", "Data", HeaderPosition::Field(0)),
        ("outer()", "Data", HeaderPosition::Field(0)),
        ("p()", "f", HeaderPosition::Parameter(0)),
        ("q()", "f", HeaderPosition::Parameter(0)),
        ("r()", "f", HeaderPosition::Result),
    ];
    for (&id, &(text, owner, position)) in expressions.iter().zip(&expected) {
        let context = preparation.context(id).unwrap();
        let code = context.program().codes().get(id).unwrap();
        let CodeKind::TypeExpression(site) = code.kind() else {
            panic!("anonymous code");
        };
        assert_eq!(code.name(), None);
        assert_eq!(
            site.declaration,
            context
                .program()
                .declaration(
                    if owner == "Data" {
                        Namespace::Type
                    } else {
                        Namespace::Value
                    },
                    context.program().symbols().get(owner).unwrap()
                )
                .unwrap()
        );
        assert_eq!(site.position, position);
        assert!(!context.program().header_ready(site.declaration));
        let source = context.code_source(id).unwrap();
        assert!(source.parameters().is_empty());
        assert_eq!(
            source.origin(),
            nocter_syntax::SyntaxOrigin::Node(source.body().unwrap())
        );
        assert_eq!(source.result_origin(), source.body().unwrap());
        assert_eq!(
            syntax.text_at(tree.node(source.body().unwrap()).unwrap().range()),
            Some(text)
        );
        let guard = context.body_source(id).unwrap();
        assert_eq!(guard.owner(), id);
        assert_eq!(
            guard.bind(&context).unwrap().body().unwrap(),
            source.body().unwrap()
        );
    }
    let moved = source.replace("{ missing }", "{\n other\n}");
    let (next_sources, next_tree) = input(&moved, ParseGoal::V2CoreUnit);
    let next = bound(&next_sources, &next_tree);
    let projection = project_declaration_syntax(next);
    assert_eq!(projection.surface(), &preparation.surface);
    let rebound = preparation.source_recipes.bind(&projection).unwrap();
    for (&id, &(text, _, _)) in expressions.iter().zip(&expected) {
        let source = rebound.codes.get(&id).unwrap();
        assert_eq!(
            next.text_at(next_tree.node(source.body().unwrap()).unwrap().range()),
            Some(text)
        );
        assert_eq!(source.name(), None);
    }
    let changed = source.replace("inner()", "other()");
    let (changed_sources, changed_tree) = input(&changed, ParseGoal::V2CoreUnit);
    assert_ne!(
        project_declaration_syntax(bound(&changed_sources, &changed_tree)).surface(),
        &preparation.surface
    );
}

#[test]
fn a_header_requests_all_missing_lengths_in_authored_order_without_publishing_a_partial_shape() {
    let text = "struct Data { a: ([i32; one()], [i32; two()])\nb: [[i32; three()]; four()] }";
    let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let owner = preparation.declaration_ids().next().unwrap();
    let HeaderProgress::RequireValues(required) =
        preparation.advance_with_values(owner, |_| None).unwrap()
    else {
        panic!("length prerequisites");
    };
    let context = preparation.current();
    let expressions = required
        .iter()
        .map(|&id| {
            syntax
                .text_at(
                    tree.node(context.code_source(id).unwrap().body().unwrap())
                        .unwrap()
                        .range(),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(expressions, ["one()", "two()", "three()", "four()"]);
    assert!(!context.program().header_ready(owner));
    assert!(context.program().fields().is_empty());
}

#[test]
fn malformed_completed_lengths_leave_raw_definition_loading_retryable_and_require_source_evidence()
{
    use nocter_model::{ConstantValue, FrozenData, FrozenNode};
    let (sources, tree) = input(
        "struct Data { values: [i32; count()] }",
        ParseGoal::V2CoreUnit,
    );
    let syntax = bound(&sources, &tree);
    let bad = FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Bool(true))]).unwrap();
    let good = FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Integer(42))]).unwrap();
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let owner = preparation.declaration_ids().next().unwrap();
    assert!(matches!(
        preparation.advance_with_values(owner, |_| Some(&bad)),
        Err(Failure::InconsistentSyntax)
    ));
    assert!(!preparation.current().program().header_ready(owner));
    assert_eq!(
        preparation
            .advance_with_values(owner, |_| Some(&good))
            .unwrap(),
        HeaderProgress::Ready
    );
    assert!(matches!(
        preparation.finish(),
        Err(Failure::MissingValueEvidence)
    ));
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let owner = preparation.declaration_ids().next().unwrap();
    preparation
        .advance_with_values(owner, |_| Some(&good))
        .unwrap();
    assert!(matches!(
        preparation.finish_with_dependencies(BodySources::default()),
        Err(Failure::MissingValueEvidence)
    ));
}
mod primitives;
