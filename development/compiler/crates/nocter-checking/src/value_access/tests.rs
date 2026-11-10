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
fn recognized_local_array_annotations_are_authored_rejections_not_integrity_failures() {
    for annotation in [
        "[i32; count()]",
        "(bool, [i32; 0])",
        "func(edit [i32; 2]): void",
    ] {
        let text = format!("func f(): void {{ let value: {annotation} = missing }}");
        let Err(Failure::Rejected(diagnostic)) = queries(&text) else {
            panic!("array normalization is not admitted: {text}");
        };
        assert_eq!(diagnostic.rule(), Rule::UnknownType);
        assert_eq!(diagnostic.source().code(), "E0370");
    }
}
