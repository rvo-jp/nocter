use super::*;
use crate::access_declarations::{
    Preparation, Query,
    tests::{bound, input},
};
use nocter_syntax::ParseGoal;

fn code(context: &Context<'_>, name: &str) -> AccessCodeId {
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
    let root = helper.bind(&current).unwrap().body().unwrap();
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
            Err(SourceMismatch::ChangedSource)
        );
    }
    // Equal text and numeric IDs from another construction are not this declaration authority.
    let foreign = Query::prepare(bound(&sources, &tree)).unwrap();
    let foreign = foreign.bind(bound(&sources, &tree)).unwrap();
    assert_eq!(
        dependencies.verify(&foreign),
        Err(SourceMismatch::ChangedAuthority)
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
            Err(SourceMismatch::ChangedAuthority)
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
        Err(SourceMismatch::ChangedSource)
    );
    dependencies.verify(&context).unwrap();

    let foreign = Query::prepare(bound(&sources, &tree)).unwrap();
    let foreign = foreign.bind(bound(&sources, &tree)).unwrap();
    assert_eq!(
        dependencies.insert(&foreign.body_source(run).unwrap()),
        Err(SourceMismatch::ChangedAuthority)
    );
    dependencies.verify(&context).unwrap();
}

#[test]
fn header_sources_cover_record_fields_and_callable_slots_before_type_publication() {
    use nocter_declarations::value_access::{DeclarationId, HeaderPosition};
    let original =
        "struct Buffer { pair: (i32, bool)\nsize: usize }\nfunc size(value: Buffer): usize { 2 }";
    let (sources, tree) = input(original, ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    let preparation = Preparation::prepare(syntax).unwrap();
    let early = preparation.current();
    let program = early.program();
    let buffer = DeclarationId::Nominal(
        program
            .resolve_nominal(program.symbols().get("Buffer").unwrap())
            .unwrap(),
    );
    let function = DeclarationId::Code(code(&early, "size"));
    assert!(!program.header_ready(buffer));
    assert!(!program.header_ready(function));
    let cases = [
        (buffer, HeaderPosition::Field(0), "(i32, bool)"),
        (buffer, HeaderPosition::Field(1), "usize"),
        (function, HeaderPosition::Parameter(0), "Buffer"),
        (function, HeaderPosition::Result, "usize"),
    ];
    let guards = cases
        .iter()
        .map(|&(owner, position, _)| early.header_source(owner, position).unwrap())
        .collect::<Vec<_>>();
    let query = preparation.finish().unwrap();
    let moved = format!("// current source\n{}", original.replace("{ 2 }", "{ 42 }"));
    let (current_sources, current_tree) = input(&moved, ParseGoal::V2CoreUnit);
    let current = query.bind(bound(&current_sources, &current_tree)).unwrap();
    for (guard, &(declaration, position, expected)) in guards.iter().zip(&cases) {
        assert_eq!(guard.declaration(), declaration);
        assert_eq!(guard.position(), position);
        let node = guard.bind(&current).unwrap();
        assert_eq!(
            current
                .syntax()
                .text_at(current.syntax().tree().node(node).unwrap().range()),
            Some(expected)
        );
    }
    for (declaration, position) in [
        (buffer, HeaderPosition::Name),
        (buffer, HeaderPosition::Parameter(0)),
        (buffer, HeaderPosition::Field(2)),
        (function, HeaderPosition::Field(0)),
        (function, HeaderPosition::Parameter(1)),
    ] {
        assert!(matches!(
            current.header_source(declaration, position),
            Err(SourceMismatch::UnknownSource)
        ));
    }
    let foreign = Query::prepare(syntax).unwrap();
    let foreign = foreign.bind(syntax).unwrap();
    assert_eq!(
        guards[0].bind(&foreign),
        Err(SourceMismatch::ChangedAuthority)
    );
    let (changed_sources, changed_tree) = input(
        &original.replace("size: usize", "size: i32"),
        ParseGoal::V2CoreUnit,
    );
    assert!(matches!(
        query.bind(bound(&changed_sources, &changed_tree)),
        Err(crate::access_declarations::Failure::ChangedHeader)
    ));
}

#[test]
fn an_unresolved_record_type_root_is_source_evidence_without_header_admission() {
    use nocter_declarations::value_access::{DeclarationId, HeaderPosition};
    let (sources, tree) = input("struct Bad { payload: Missing }", ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    let mut preparation = Preparation::prepare(syntax).unwrap();
    let early = preparation.current();
    let owner = DeclarationId::Nominal(
        early
            .program()
            .resolve_nominal(early.program().symbols().get("Bad").unwrap())
            .unwrap(),
    );
    let source = early
        .header_source(owner, HeaderPosition::Field(0))
        .unwrap();
    let node = source.bind(&early).unwrap();
    assert_eq!(
        syntax.text_at(syntax.tree().node(node).unwrap().range()),
        Some("Missing")
    );
    assert!(matches!(
        preparation.advance(owner),
        Err(crate::access_declarations::Failure::Headers(_))
    ));
    assert!(!preparation.current().program().header_ready(owner));
    assert!(preparation.finish().is_err());
}
