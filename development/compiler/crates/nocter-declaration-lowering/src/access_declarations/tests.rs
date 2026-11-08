use super::*;
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
        "func f(take value: error): error { return value }",
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
        "func beta(take x: error): error { x }\nfunc alpha(): void {}",
        ParseGoal::V2CoreUnit,
    );
    let (b, tb) = input(
        "func alpha(): void {}\nfunc beta(take x: error): error { x }",
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
fn corpus_freezes_all_twenty_six_syntax_valid_private_units() {
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
    let mut counts = [0; 2];
    for case in cases {
        let text = std::fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let (sources, tree) = input(&text, ParseGoal::V2CoreUnit);
        let result = Query::prepare(bound(&sources, &tree));
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(matches!(result, Err(Failure::Syntax(_))));
            counts[1] += 1;
        } else {
            let q = result.unwrap();
            assert!(q.bind(bound(&sources, &tree)).is_ok());
            counts[0] += 1;
        }
    }
    assert_eq!(counts, [26, 4]);
}
