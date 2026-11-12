//! Retained headers reuse one semantic authority while all locations come from current sources.
use super::*;
use nocter_syntax::SyntaxOrigin;

fn reusable_fixture(prefix: &str, body: &str, unrelated: u64, visibility: &str) -> Fixture {
    let body = format!(
        "{prefix}see ./index.nct\nfunc run(): i32 {{ {body} }}\nfunc unrelated(): i32 {{ {unrelated} }}"
    );
    let root = format!("see ./body.nct\n{visibility}const func answer(): i32");
    Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
            ("/app/body.nct", Implementation, &body),
            ("/app/data/index.nct", Root, &root),
            (
                "/app/data/body.nct",
                Implementation,
                "see ./index.nct\nconst func answer(): i32 { 42 }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)],
    )
    .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
    .with_uses(&[(1, 0, &["data"])])
}

fn retained() -> (declarations::ModuleQuery, Query) {
    let fixture = reusable_fixture("", "use ./data as api\napi.answer()", 1, "");
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let context = source.declarations().current().unwrap();
    let run = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    (
        source.declarations().query().unwrap(),
        Query::prepare(&context, run).unwrap(),
    )
}

#[test]
fn module_rebinding_keeps_original_headers_and_projects_body_queries_into_new_source_identities() {
    let (headers, body) = retained();
    let fixture = reusable_fixture(
        "// current revision\n\n",
        "use ./data as api\napi.answer()",
        2,
        "",
    );
    let rebound = headers.bind(&fixture.input()).unwrap();
    assert!(std::ptr::eq(headers.program(), rebound.program()));
    let context = rebound.current().unwrap();
    let bound = body.bind(&context).unwrap();
    assert_eq!(bound.syntax().tree().source(), fixture.tree(1).source());
    let call = body
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .unwrap();
    let origin = bound.expression_origin(call).unwrap();
    let current = context.source_origin(origin).unwrap();
    assert_eq!(current.source(), fixture.tree(1).source());
    assert_eq!(
        bound.syntax().text_at(current.span().range()),
        Some("api.answer()")
    );
    let fresh = Query::prepare(&context, body.source_owner()).unwrap();
    assert_eq!(fresh.body(), body.body());
    assert!(rebound.query().unwrap().bind(&fixture.input()).is_ok());
}

#[test]
fn changed_runtime_bodies_invalidate_their_body_query_without_invalidating_unchanged_headers() {
    let (headers, body) = retained();
    let fixture = reusable_fixture("", "use ./data as api\napi.answer() + 1", 1, "");
    let rebound = headers.bind(&fixture.input()).unwrap();
    let context = rebound.current().unwrap();
    assert!(matches!(body.bind(&context), Err(Failure::ChangedBody)));
    assert!(Query::prepare(&context, body.source_owner()).is_ok());
}

#[test]
fn changed_audiences_and_lexical_bindings_invalidate_retained_headers_before_body_reuse() {
    let (headers, _) = retained();
    for (body, visibility) in [
        ("use ./data as api\napi.answer()", "#visibility: \"./\"\n"),
        ("use ./data as changed\nchanged.answer()", ""),
    ] {
        let fixture = reusable_fixture("", body, 1, visibility);
        assert!(matches!(
            headers.bind(&fixture.input()),
            Err(declarations::ModuleSourcesFailure::Access(
                declarations::Failure::ChangedHeader
            ))
        ));
    }
}

#[test]
fn header_count_reuse_retains_both_physical_suppliers_and_rejects_changed_actual_source() {
    let headers = {
        let fixture = counts(3);
        let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
        source.declarations().query().unwrap()
    };
    let same = counts(3);
    let rebound = headers.bind(&same.input()).unwrap();
    assert!(rebound.program().preserves_headers(headers.program()));
    let changed = counts(4);
    assert!(matches!(
        headers.bind(&changed.input()),
        Err(declarations::ModuleSourcesFailure::Access(
            declarations::Failure::ChangedHeader
        ))
    ));
    let context = rebound.current().unwrap();
    assert!(
        context
            .lookup_scope_at(SyntaxOrigin::Node(changed.tree(0).root_id()))
            .is_err()
    );
}
