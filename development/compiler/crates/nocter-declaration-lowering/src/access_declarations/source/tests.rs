use super::*;
use crate::access_declarations::{
    Preparation, Query,
    tests::{bound, input},
};
use nocter_syntax::ParseGoal;

fn code(context: &Context<'_>, name: &str) -> AccessCallableId {
    context
        .program()
        .resolve_code(context.program().symbols().get(name).unwrap())
        .unwrap()
}

#[test]
fn exact_dependencies_rebind_current_coordinates_and_reject_changed_initializers_or_helpers() {
    let original =
        "const COUNT: usize = 2\nconst func helper(): usize { COUNT }\nfunc unrelated(): i32 { 1 }";
    let (sources, tree) = input(original, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let helper = context.body_source(code(&context, "helper")).unwrap();
    let initializer = context.body_source(code(&context, "COUNT")).unwrap();
    let mut dependencies = BodySources::default();
    dependencies.insert(&helper).unwrap();
    dependencies.insert(&initializer).unwrap();

    // Different source identity, current coordinates and an unrelated body edit preserve the guard.
    let changed = format!("// moved source\n{}", original.replace("{ 1 }", "{ 42 }"));
    let (current_sources, current_tree) = input(&changed, ParseGoal::V2CoreUnit);
    let current = query.bind(bound(&current_sources, &current_tree)).unwrap();
    dependencies.verify(&current).unwrap();
    let root = helper.bind(&current).unwrap().body();
    assert_eq!(
        current
            .syntax()
            .text_at(current.syntax().tree().node(root).unwrap().range()),
        Some("{ COUNT }")
    );

    for changed in [
        original.replace("= 2", "= 3"),
        original.replace("{ COUNT }", "{ COUNT + 1 }"),
    ] {
        let (changed_sources, changed_tree) = input(&changed, ParseGoal::V2CoreUnit);
        let changed = query.bind(bound(&changed_sources, &changed_tree)).unwrap();
        assert_eq!(
            dependencies.verify(&changed),
            Err(BodySourceFailure::ChangedBody)
        );
    }
    // Equal text and numeric IDs from another construction are not this declaration authority.
    let foreign = Query::prepare(bound(&sources, &tree)).unwrap();
    let foreign = foreign.bind(bound(&sources, &tree)).unwrap();
    assert_eq!(
        dependencies.verify(&foreign),
        Err(BodySourceFailure::ChangedAuthority)
    );
}

#[test]
fn merging_dependencies_keeps_the_strongest_prefix_in_either_insertion_order() {
    let (sources, tree) = input(
        "struct Late {}\nfunc first(): void {}\nfunc second(): void {}",
        ParseGoal::V2CoreUnit,
    );
    let syntax = bound(&sources, &tree);
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let early = preparation.current();
    let first = code(&early, "first");
    let early_source = early.body_source(first).unwrap();
    assert!(!early.program().header_ready(first));
    early_source.bind(&early).unwrap();
    let later = preparation.context(first).unwrap();
    let later_source = later.body_source(first).unwrap();
    for pair in [
        [&early_source, &later_source],
        [&later_source, &early_source],
    ] {
        let mut dependencies = BodySources::default();
        for source in pair {
            dependencies.insert(source).unwrap();
        }
        assert_eq!(
            dependencies.verify(&early),
            Err(BodySourceFailure::ChangedAuthority)
        );
        dependencies.verify(&later).unwrap();
    }
    let complete = preparation.finish().unwrap();
    let complete = complete.bind(syntax).unwrap();
    later_source.bind(&complete).unwrap();
}

#[test]
fn conflicting_evidence_cannot_replace_an_existing_source_dependency() {
    let original = "func run(): i32 { 1 }";
    let (sources, tree) = input(original, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let run = code(&context, "run");
    let mut dependencies = BodySources::default();
    dependencies
        .insert(&context.body_source(run).unwrap())
        .unwrap();

    let (changed_sources, changed_tree) = input("func run(): i32 { 2 }", ParseGoal::V2CoreUnit);
    let changed = query.bind(bound(&changed_sources, &changed_tree)).unwrap();
    assert_eq!(
        dependencies.insert(&changed.body_source(run).unwrap()),
        Err(BodySourceFailure::ChangedBody)
    );
    dependencies.verify(&context).unwrap();

    let foreign = Query::prepare(bound(&sources, &tree)).unwrap();
    let foreign = foreign.bind(bound(&sources, &tree)).unwrap();
    assert_eq!(
        dependencies.insert(&foreign.body_source(run).unwrap()),
        Err(BodySourceFailure::ChangedAuthority)
    );
    dependencies.verify(&context).unwrap();
}
