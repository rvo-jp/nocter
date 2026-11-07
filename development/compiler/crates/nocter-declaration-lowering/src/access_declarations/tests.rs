use std::{fs, path::PathBuf};

use nocter_declarations::value_access::ResultReferent;
use nocter_json::Value;
use nocter_model::ValueAccessMode;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::*;

fn input(text: &str) -> (SourceMap, SyntaxTree) {
    let mut sources = SourceMap::new();
    let source = sources
        .add_bytes(SourceName::new("unit.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(source).unwrap(), ParseGoal::V2CoreUnit);
    (sources, tree)
}

fn bound<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> BoundSyntax<'a> {
    BoundSyntax::new(sources.get(tree.source()).unwrap(), tree).unwrap()
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("expected fixture object")
    };
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .unwrap()
        .value
}

fn string(value: &Value) -> &str {
    let Value::String(value) = value else {
        panic!("expected fixture string")
    };
    value
}

#[test]
fn corpus_separates_syntax_header_and_body_rejections() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("expected cases")
    };
    let mut totals = [0; 3];
    for case in cases {
        let text = fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let (sources, tree) = input(&text);
        let outcome = Query::prepare(bound(&sources, &tree));
        match string(field(case, "expectation")) {
            "syntax-reject" => {
                assert!(matches!(outcome, Err(Failure::Syntax(_))));
                totals[0] += 1;
            }
            "declaration-reject" => {
                let Err(Failure::Headers(diagnostics)) = outcome else {
                    panic!("expected header failure: {outcome:?}")
                };
                let Value::Object(members) = case else {
                    unreachable!()
                };
                let expected = members
                    .iter()
                    .find(|member| member.name.as_ref() == "diagnostic_count")
                    .map_or(1, |member| {
                        let Value::Number(number) = &member.value else {
                            panic!("expected diagnostic count")
                        };
                        number.parse::<usize>().unwrap()
                    });
                assert_eq!(diagnostics.len(), expected);
                let primary = string(field(case, "primary_text"));
                let start = text.find(primary).unwrap();
                assert!(diagnostics.iter().any(|diagnostic| {
                    let offset =
                        diagnostic.source().primary().span().range().start().get() as usize;
                    offset >= start && offset < start + primary.len()
                }));
                for diagnostic in &diagnostics {
                    assert_eq!(diagnostic.source().code(), string(field(case, "code")));
                }
                if let Value::Object(members) = case
                    && let Some(related) = members
                        .iter()
                        .find(|member| member.name.as_ref() == "related_text")
                {
                    let related = string(&related.value);
                    let start = text.find(related).unwrap();
                    for diagnostic in &diagnostics {
                        assert_eq!(diagnostic.source().notes().len(), 1);
                        let offset = diagnostic.source().notes()[0]
                            .origin()
                            .span()
                            .range()
                            .start()
                            .get() as usize;
                        assert!(offset >= start && offset < start + related.len());
                    }
                }
                totals[1] += 1;
            }
            "accept" | "check-reject" => {
                let query = outcome.unwrap();
                let context = query.bind(bound(&sources, &tree)).unwrap();
                assert_eq!(context.program().callables().len(), context.sources.len());
                for (id, callable) in context.program().callables().iter() {
                    let current = context.callable_source(id).unwrap();
                    assert_eq!(
                        context.syntax().text_at(current.name().range()),
                        context.program().symbols().spelling(callable.name())
                    );
                }
                totals[2] += 1; // A body rejection deliberately remains valid declaration input.
            }
            other => panic!("unknown expectation {other}"),
        }
    }
    assert_eq!(totals, [6, 6, 38]);
}

#[test]
fn body_changes_rebind_one_authority_without_reinferring_a_mode_or_header() {
    let (first_sources, first_tree) = input("func update(value: &i32): &i32 { return value }\n");
    let query = Query::prepare(bound(&first_sources, &first_tree)).unwrap();
    let first = query.bind(bound(&first_sources, &first_tree)).unwrap();
    let (new_sources, new_tree) = input(
        "// moved text\nfunc update(value: &i32): &i32 {\nlet local = 20\nreturn &local\n}\n",
    );
    let second = query.bind(bound(&new_sources, &new_tree)).unwrap();
    assert!(Arc::ptr_eq(&first.program, &second.program));
    for (id, callable) in first.program().callables().iter() {
        assert_eq!(callable.result().mode(), ValueAccessMode::Update);
        let source = second.callable_source(id).unwrap();
        assert!(new_tree.node(source.body()).is_some());
        assert!(first_tree.node(source.body()).is_none());
        assert_eq!(source.name().source(), new_tree.source());
        assert_eq!(
            callable.result_referent(),
            &ResultReferent::Inputs(callable.parameters().into())
        );
    }
    let (changed_sources, changed_tree) =
        input("func update(value: ^i32): &i32 { return value }\n");
    assert!(matches!(
        query.bind(bound(&changed_sources, &changed_tree)),
        Err(Failure::ChangedHeader)
    ));
}

#[test]
fn reversing_declaration_order_preserves_all_semantic_ids_and_no_borrow_shapes() {
    let first = "func alpha(edit: &i32): &i32 { return edit }\nfunc beta(value: ^error): ^error { return ^value }\n";
    let second = "func beta(value: ^error): ^error { return ^value }\nfunc alpha(edit: &i32): &i32 { return edit }\n";
    let (sources_a, tree_a) = input(first);
    let (sources_b, tree_b) = input(second);
    let a = Query::prepare(bound(&sources_a, &tree_a)).unwrap();
    let b = Query::prepare(bound(&sources_b, &tree_b)).unwrap();
    assert_eq!(a.program().symbols(), b.program().symbols());
    assert_eq!(a.program().callables(), b.program().callables());
    assert_eq!(a.program().parameters(), b.program().parameters());
    for (_, callable) in a.program().callables().iter() {
        assert!(a.program().data_kind(callable.result().data()).is_some());
        let data = a.program().data_kind(callable.result().data()).unwrap();
        assert_eq!(a.program().data_type(data), Some(callable.result().data()));
    }
}

#[test]
fn private_alternative_and_empty_bounds_are_header_facts_not_body_inference() {
    for source in [
        "func edit(left: &i32, right: &i32): &i32 { return left }\n",
        "func edit(): &i32 { loop {} }\n",
    ] {
        let (sources, tree) = input(source);
        let query = Query::prepare(bound(&sources, &tree)).unwrap();
        let (_, callable) = query.program().callables().iter().next().unwrap();
        assert_eq!(
            callable.result_referent(),
            &ResultReferent::Inputs(callable.parameters().into())
        );
    }
}

#[test]
fn wrong_goal_syntax_errors_and_changed_headers_cannot_publish_a_context() {
    let (sources, tree) = input("func f(): void {}\n");
    let current = parse(sources.get(tree.source()).unwrap(), ParseGoal::SourceFile);
    assert!(matches!(
        Query::prepare(bound(&sources, &current)),
        Err(Failure::WrongParseGoal)
    ));
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let (broken_sources, broken_tree) = input("func f(): void { let value = }\n");
    assert!(matches!(
        query.bind(bound(&broken_sources, &broken_tree)),
        Err(Failure::Syntax(_))
    ));
}
