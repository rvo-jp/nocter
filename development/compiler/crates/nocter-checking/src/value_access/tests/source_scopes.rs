//! Actual body typing over physical source views. No whole-module Unit or evaluation claim.
use super::*;
use ModuleSourceKind::{Implementation, Root};
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode, SourceVisibilityResolutionInput,
};
use nocter_model::{AccessCodeId, CompilationTarget, PackageIdentity, ValueFunctionOrigin};
use nocter_syntax::{NodeKind, child_node_iter};
use std::collections::HashSet;

struct Fixture {
    sources: SourceMap,
    trees: Vec<SyntaxTree>,
    files: Vec<(String, ModuleSourceKind)>,
    edges: Vec<(usize, usize, usize)>,
}
impl Fixture {
    fn new(files: &[(&str, ModuleSourceKind, &str)], edges: &[(usize, usize, usize)]) -> Self {
        let mut sources = SourceMap::new();
        let trees = files
            .iter()
            .map(|(path, _, text)| {
                let id = sources
                    .add_bytes(SourceName::new(*path), text.as_bytes())
                    .unwrap();
                let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
                assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
                tree
            })
            .collect();
        Self {
            sources,
            trees,
            files: files
                .iter()
                .map(|(path, role, _)| (path.to_string(), *role))
                .collect(),
            edges: edges.into(),
        }
    }
    fn prepare(&self) -> declarations::ScopedModuleSources<'_> {
        let package = PackageIdentity::new("workspace:app");
        let input = CompileUnitInput::for_goal(
            ParseGoal::V2CoreUnit,
            CompilationTarget::Arm64Darwin,
            &self.sources,
            vec![PackageInput::new(
                package.clone(),
                "app",
                PackageMode::Declared,
            )],
            vec![ModuleInput::new(
                ModuleIdentity::new(package, Vec::<&str>::new()),
                self.files
                    .iter()
                    .zip(&self.trees)
                    .map(|((path, role), tree)| ModuleSourceInput::new(path.as_str(), *role, tree))
                    .collect(),
            )],
            Vec::new(),
        )
        .with_source_visibility_resolutions(
            self.edges
                .iter()
                .map(|&(source, occurrence, target)| {
                    let node = child_node_iter(&self.trees[source], self.trees[source].root_id())
                        .filter(|node| {
                            self.trees[source].node(*node).unwrap().kind()
                                == NodeKind::SourceVisibilityDeclaration
                        })
                        .nth(occurrence)
                        .unwrap();
                    SourceVisibilityResolutionInput::new(node, self.files[target].0.as_str())
                })
                .collect(),
        );
        let mut scoped = declarations::ModuleSources::prepare(&input)
            .unwrap()
            .pair()
            .unwrap()
            .reserve_names()
            .unwrap();
        let mut seen = HashSet::new();
        let mut pending = scoped
            .paired()
            .inventory()
            .declaration_entries()
            .filter_map(|(raw, _)| scoped.declaration(raw))
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|id| {
                !matches!(
                    scoped.advance_header_with_values(*id, |_| None).unwrap(),
                    declarations::HeaderProgress::Ready
                )
            });
            assert!(
                pending.len() < before,
                "fixture has unresolved header dependencies"
            );
        }
        scoped
    }
}
fn code(scoped: &declarations::ScopedModuleSources<'_>, path: &str, name: &str) -> AccessCodeId {
    let (source, _) = scoped
        .paired()
        .inventory()
        .source_entries()
        .find(|(_, s)| s.canonical_path() == path)
        .unwrap();
    let program = scoped.program();
    program
        .resolve_code_in(
            scoped.scope(source).unwrap(),
            program.symbols().get(name).unwrap(),
        )
        .unwrap()
}

#[test]
fn source_views_select_direct_helpers_and_bind_queries_only_to_their_actual_body() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./b.nct\nfunc run(): i32 { helper() }",
            ),
            ("/app/b.nct", Implementation, "func helper(): i32 { 42 }"),
            (
                "/app/c.nct",
                Implementation,
                "func helper(): i64 { 7 }\nfunc other(): i64 { helper() }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
    );
    let scoped = fixture.prepare();
    let run = code(&scoped, "/app/index.nct", "run");
    let other = code(&scoped, "/app/c.nct", "other");
    let run_context = scoped.context_for_code(run).unwrap();
    let other_context = scoped.context_for_code(other).unwrap();
    assert_ne!(run_context.lookup_scope(), other_context.lookup_scope());
    for (id, context, helper_path) in [
        (run, &run_context, "/app/b.nct"),
        (other, &other_context, "/app/c.nct"),
    ] {
        let query = Query::prepare(context, id).unwrap();
        let target = code(&scoped, helper_path, "helper");
        assert!(query.body().expressions().iter().any(|(_, expr)| matches!(expr.kind, ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(actual), .. } if actual == target)));
        assert!(query.bind(context).is_ok());
        let foreign = if id == run {
            &other_context
        } else {
            &run_context
        };
        assert!(query.bind(foreign).is_err());
    }
}

#[test]
fn seen_sources_do_not_propagate_their_own_see_edges_into_body_lookup() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./b.nct\nfunc run(): i32 { leaf() }",
            ),
            ("/app/b.nct", Implementation, "see ./c.nct"),
            ("/app/c.nct", Implementation, "func leaf(): i32 { 42 }"),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2), (2, 0, 3)],
    );
    let scoped = fixture.prepare();
    let run = code(&scoped, "/app/index.nct", "run");
    let context = scoped.context_for_code(run).unwrap();
    let Err(Failure::Rejected(diagnostic)) = Query::prepare(&context, run) else {
        panic!("transitive name")
    };
    assert_eq!(diagnostic.rule(), Rule::UnknownName);
    assert_eq!(
        diagnostic.source().primary().source(),
        context.syntax().tree().source()
    );
}

#[test]
fn local_shadow_checks_use_current_visibility_rather_than_the_root_sources_scope() {
    for direct in [false, true] {
        let text = if direct {
            "see ./index.nct\nsee ./helpers.nct\nfunc run(): i32 { let helper = 42\nhelper }"
        } else {
            "see ./index.nct\nfunc run(): i32 { let helper = 42\nhelper }"
        };
        let edges = if direct {
            vec![(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, 2)]
        } else {
            vec![(0, 0, 1), (0, 1, 2), (1, 0, 0)]
        };
        let fixture = Fixture::new(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./a.nct\nsee ./helpers.nct\nfunc run(): i32",
                ),
                ("/app/a.nct", Implementation, text),
                (
                    "/app/helpers.nct",
                    Implementation,
                    "func helper(): i32 { 7 }",
                ),
            ],
            &edges,
        );
        let scoped = fixture.prepare();
        let run = code(&scoped, "/app/index.nct", "run");
        let context = scoped.context_for_code(run).unwrap();
        match Query::prepare(&context, run) {
            Ok(_) => assert!(!direct),
            Err(Failure::Rejected(error)) => {
                assert!(direct);
                assert_eq!(error.rule(), Rule::ShadowedName);
            }
            Err(other) => panic!("{other:?}"),
        }
    }
}

#[test]
fn annotations_and_constructions_keep_distinct_same_spelling_record_identities() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./b.nct\nfunc run(): i32 { let item: Data = Data { value: 42 }\nitem.value }",
            ),
            ("/app/b.nct", Implementation, "struct Data { value: i32 }"),
            (
                "/app/c.nct",
                Implementation,
                "struct Data { value: bool }\nfunc other(): bool { let item: Data = Data { value: true }\nitem.value }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
    );
    let scoped = fixture.prepare();
    let mut types = Vec::new();
    for (path, name) in [("/app/a.nct", "run"), ("/app/c.nct", "other")] {
        let id = code(&scoped, path, name);
        let context = scoped.context_for_code(id).unwrap();
        let expected = context
            .program()
            .type_named_in(context.lookup_scope(), "Data")
            .unwrap();
        types.push(expected);
        let query = Query::prepare(&context, id).unwrap();
        assert_eq!(
            query.body().locals().iter().next().unwrap().1.data,
            expected
        );
    }
    assert_ne!(types[0], types[1]);
}

#[test]
fn constant_requests_and_static_places_select_the_current_physical_scope() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./b.nct\nfunc run(): i32 { VALUE }\nfunc slot(): i32 { SLOT }",
            ),
            (
                "/app/b.nct",
                Implementation,
                "const VALUE: i32 = 42\nstatic SLOT: i32 = 42",
            ),
            (
                "/app/c.nct",
                Implementation,
                "const VALUE: i64 = 7\nstatic SLOT: i64 = 7\nfunc other(): i64 { VALUE }\nfunc slot(): i64 { SLOT }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
    );
    let scoped = fixture.prepare();
    let mut constants = Vec::new();
    for (path, name, supplier) in [
        ("/app/a.nct", "run", "/app/b.nct"),
        ("/app/c.nct", "other", "/app/c.nct"),
    ] {
        let id = code(&scoped, path, name);
        let context = scoped.context_for_code(id).unwrap();
        let Err(Failure::RequiredConstant { initializer, .. }) = Query::prepare(&context, id)
        else {
            panic!("constant dependency")
        };
        assert_eq!(initializer, code(&scoped, supplier, "VALUE"));
        constants.push(initializer);
        let slot = code(&scoped, path, "slot");
        let context = scoped.context_for_code(slot).unwrap();
        let query = Query::prepare(&context, slot).unwrap();
        let static_id = code(&scoped, supplier, "SLOT");
        assert!(query.body().locals().iter().any(|(_, local)| matches!(local.origin, LocalOrigin::Static(value) if value.initializer == static_id)));
    }
    assert_ne!(constants[0], constants[1]);
}
