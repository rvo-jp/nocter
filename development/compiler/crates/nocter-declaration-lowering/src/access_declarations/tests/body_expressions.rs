use super::*;
use nocter_model::{ValueCodeRoot, ValueExpression};

fn code(context: &Context<'_>, name: &str) -> AccessCodeId {
    context
        .program()
        .resolve(context.program().symbols().get(name).unwrap())
        .unwrap()
}

fn text(context: &Context<'_>, source: &BodySource) -> String {
    let node = source.bind(context).unwrap().body();
    context
        .syntax()
        .text_at(context.syntax().tree().node(node).unwrap().range())
        .unwrap()
        .into()
}

#[test]
fn local_type_roots_use_the_shared_projection_without_declaring_functions() {
    let source = "func alpha(unused: i32): void {\nlet a: ([i32; one()], [bool; two()]) = missing\nlet b: [[i32; three()]; four()] = missing\nlet captured = 1\nlet action = (captured; input: [i32; five()]): [bool; six()] { loop {} }\nlet nested: [i32; if true { let inner: [i32; seven()] = missing\n8 } else { 0 }] = missing\n}\nfunc beta(): void { let value: [i32; one()] = missing }";
    let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let owner = code(&context, "alpha");
    let registered = context.body_expressions(owner).unwrap();
    let ids = registered.ids().collect::<Vec<_>>();
    assert_eq!(ids.len(), 8);
    let roots = ids
        .iter()
        .map(|&id| registered.source(id).unwrap())
        .collect::<Vec<_>>();
    let texts = roots
        .iter()
        .map(|source| text(&context, source))
        .collect::<Vec<_>>();
    assert_eq!(
        &texts[..6],
        ["one()", "two()", "three()", "four()", "five()", "six()"]
    );
    assert!(texts[6].starts_with("if true"));
    assert_eq!(texts[7], "seven()");
    for (slot, source) in roots.iter().enumerate() {
        let id = ids[slot];
        assert_eq!(source.code(), ValueCodeRoot::Expression(id));
        assert_eq!(id.owner, owner);
        assert_eq!(usize::try_from(id.slot).unwrap(), slot);
        assert_eq!(source.owner(), owner);
        let executable = source.bind(&context).unwrap();
        assert!(executable.parameters().is_empty());
        assert_eq!(executable.result_origin(), executable.body());
        assert_eq!(
            registered.expression(source.bind(&context).unwrap().body()),
            Some(id)
        );
        assert_eq!(ValueCodeRoot::Expression(id).code().origin(), None);
    }
    let other = ValueExpression {
        owner: code(&context, "beta"),
        slot: 0,
    };
    assert!(registered.source(other).is_none());
    assert!(
        registered
            .source(ValueExpression {
                owner,
                slot: u32::MAX
            })
            .is_none()
    );
    assert!(
        registered
            .expression(context.code_source(owner).unwrap().body())
            .is_none()
    );
    assert_eq!(context.program().codes().len(), 2);
    let beta = context.body_expressions(other.owner).unwrap();
    assert_eq!(text(&context, &beta.source(other).unwrap()), "one()");
    assert_ne!(other, ids[0]);
}

#[test]
fn body_expression_guards_follow_the_lexical_owner_and_rebind_after_sibling_edits() {
    let source = "func beta(): void { let value: [i32; 1] = missing }\nfunc alpha(): void { let local = 40\nlet value: [i32; width()] = missing }";
    let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let owner = code(&context, "alpha");
    let catalog = context.body_expressions(owner).unwrap();
    let id = catalog.ids().next().unwrap();
    let selected = catalog.source(id).unwrap();
    let edited = format!(
        "// move physical nodes\n{}",
        source.replace("[i32; 1]", "[[i32; 2]; 3]")
    );
    let (next_sources, next_tree) = input(&edited, ParseGoal::V2CoreUnit);
    let next = query.bind(bound(&next_sources, &next_tree)).unwrap();
    assert_eq!(text(&next, &selected), "width()");
    assert_eq!(
        next.body_expressions(owner)
            .unwrap()
            .ids()
            .collect::<Vec<_>>(),
        [id]
    );
    let edited = source.replace("let local = 40", "let local = 41");
    let (changed_sources, changed_tree) = input(&edited, ParseGoal::V2CoreUnit);
    let changed = query.bind(bound(&changed_sources, &changed_tree)).unwrap();
    assert_eq!(context.program().codes(), changed.program().codes());
    assert!(matches!(
        selected.bind(&changed),
        Err(SourceMismatch::ChangedSource)
    ));
    assert_eq!(
        text(
            &changed,
            &changed.body_expressions(owner).unwrap().source(id).unwrap()
        ),
        "width()"
    );
    let unrelated = Query::prepare(bound(&sources, &tree)).unwrap();
    let foreign = unrelated.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        selected.bind(&foreign),
        Err(SourceMismatch::ChangedAuthority)
    ));
}

#[test]
fn repetition_counts_share_the_catalog_with_annotations_and_nested_seed_counts() {
    let source = "func run(): void { let values: [i32; annotation()] = [seed(); count()]\nlet nested = [[seed(); inner()]; if true { let local: [i32; inside()] = missing\n1 } else { 0 }] }";
    let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
    let query = Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let owner = code(&context, "run");
    let catalog = context.body_expressions(owner).unwrap();
    let roots = catalog
        .ids()
        .map(|id| text(&context, &catalog.source(id).unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(&roots[..3], ["annotation()", "count()", "inner()"]);
    assert!(roots[3].starts_with("if true"));
    assert_eq!(roots[4], "inside()");
    assert_eq!(roots.len(), 5);
    assert_eq!(context.program().codes().len(), 1);
}
