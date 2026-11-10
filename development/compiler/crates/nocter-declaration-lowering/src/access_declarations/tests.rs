use super::*;
use nocter_model::{BuiltinType, ValueInputOperation};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};
fn input(text: &str, goal: ParseGoal) -> (SourceMap, SyntaxTree) {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("unit.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), goal);
    (sources, tree)
}
fn bound<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> BoundSyntax<'a> {
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
    let (_, callable) = program.callables().iter().next().unwrap();
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
    assert_eq!(qa.program().callables(), qb.program().callables());
    assert_eq!(qa.program().parameters(), qb.program().parameters());
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
        "struct Shared { value: Missing }\nfunc Shared(): Missing { unknown }",
        "// current coordinates\nfunc Shared(): Missing { other }\nstruct Shared { value: Missing }",
    ] {
        let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
        let Err(Failure::Headers(diagnostics)) = Query::prepare(bound(&sources, &tree)) else {
            panic!("expected namespace rejection: {source}");
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(matches!(
            diagnostic.violation().rule,
            DeclarationRule::RecordShadowsCallable { .. }
        ));
        assert_eq!(diagnostic.violation().position, HeaderPosition::Name);
        let primary = diagnostic.source().primary().span().range();
        let name = source.find("struct Shared").unwrap() + "struct ".len();
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
            query.program().callables().get(id).unwrap().compile_time(),
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
