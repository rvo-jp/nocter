//! Authored visibility feeds the semantic namespace; no source is flattened or typed here.
use super::module_contracts::inventory;
use super::*;
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind as Kind, SurfaceSourceId};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::{DeclarationId, DeclarationRule, Namespace, ScopeId};

pub(super) fn source(scoped: &ScopedModuleSources<'_>, path: &str) -> SurfaceSourceId {
    scoped
        .paired()
        .inventory()
        .source_entries()
        .find(|(_, source)| source.canonical_path() == path)
        .unwrap()
        .0
}
pub(super) fn declaration(
    scoped: &ScopedModuleSources<'_>,
    path: &str,
    kind: Kind,
    name: &str,
) -> SurfaceDeclarationId {
    let input = scoped.paired().inventory();
    input
        .declaration_entries()
        .find(|(_, declaration)| {
            declaration.source() == source(scoped, path)
                && declaration.kind() == kind
                && declaration.name().and_then(|token| {
                    input
                        .source_map()
                        .get(token.source())?
                        .text_at(token.range())
                }) == Some(name)
        })
        .unwrap()
        .0
}
fn lookup(
    scoped: &ScopedModuleSources<'_>,
    path: &str,
    namespace: Namespace,
    name: &str,
) -> Option<DeclarationId> {
    let program = scoped.program();
    program.declaration_in(
        scoped.scope(source(scoped, path)).unwrap(),
        namespace,
        program.symbols().get(name).unwrap(),
    )
}

#[test]
fn only_current_and_directly_seen_authored_names_enter_each_scope() {
    for reverse in [false, true] {
        let scoped = inventory(&[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            ("/app/a.nct", Implementation, "see ./index.nct\nsee ./b.nct\nsee ./b.nct\nfunc run(): i32 { assist() }\nfunc assist(): i32 { leaf() }"),
            ("/app/b.nct", Implementation, "func leaf(): i32 { 42 }"),
            ("/app/c.nct", Implementation, "func assist(): i32 { 7 }"),
        ], &[(0, 0, 1), (1, 0, 0), (1, 1, 2), (1, 2, 2)], reverse).pair().unwrap().reserve_names().unwrap();
        let run = scoped
            .declaration(declaration(
                &scoped,
                "/app/index.nct",
                Kind::Function,
                "run",
            ))
            .unwrap();
        assert_eq!(
            scoped.declaration(declaration(&scoped, "/app/a.nct", Kind::Function, "run")),
            Some(run)
        );
        let a = scoped
            .declaration(declaration(&scoped, "/app/a.nct", Kind::Function, "assist"))
            .unwrap();
        let c = scoped
            .declaration(declaration(&scoped, "/app/c.nct", Kind::Function, "assist"))
            .unwrap();
        assert_ne!(a, c);
        assert_eq!(
            lookup(&scoped, "/app/index.nct", Namespace::Value, "assist"),
            Some(a)
        );
        assert_eq!(
            lookup(&scoped, "/app/c.nct", Namespace::Value, "assist"),
            Some(c)
        );
        assert_eq!(
            lookup(&scoped, "/app/index.nct", Namespace::Value, "leaf"),
            None
        );
        assert!(lookup(&scoped, "/app/a.nct", Namespace::Value, "leaf").is_some());
        assert_eq!(
            lookup(&scoped, "/app/b.nct", Namespace::Value, "assist"),
            None
        );
        assert_eq!(lookup(&scoped, "/app/c.nct", Namespace::Value, "run"), None);
        let program = scoped.program();
        assert!(std::ptr::eq(
            program.symbols(),
            scoped.paired().inventory().surface.symbols()
        ));
        assert!(!program.headers_complete());
        assert!(!program.header_ready(run));
        assert!(program.codes().is_empty());
        assert_eq!(
            scoped
                .paired()
                .inventory()
                .declaration_entries()
                .filter_map(|(id, _)| scoped.declaration(id))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
    }
}

#[test]
fn public_fields_share_complete_representation_slots_without_payload_readiness() {
    let scoped = inventory(&[
        ("/app/index.nct", Root, "see ./body.nct\nstruct Item { first: i32\nlast: bool }\nfunc Item(value: i32): i32\nconst LIMIT: i32\nstatic BASE: i32"),
        ("/app/body.nct", Implementation, "see ./index.nct\nstruct Item { hidden: i32\nfirst: i32\nmiddle: bool\nlast: bool }\nfunc Item(value: i32): i32 { value }\nconst LIMIT: i32 = 42\nstatic BASE: i32 = 7"),
    ], &[(0, 0, 1), (1, 0, 0)], false).pair().unwrap().reserve_names().unwrap();
    let contract = declaration(&scoped, "/app/index.nct", Kind::Struct, "Item");
    let representation = declaration(&scoped, "/app/body.nct", Kind::Struct, "Item");
    let Some(DeclarationId::Nominal(record)) = scoped.declaration(contract) else {
        panic!("record")
    };
    assert_eq!(scoped.declaration(representation), Some(record.into()));
    let fields = scoped.field_ids(record).unwrap();
    assert_eq!(fields.len(), 4);
    for (name, position) in [("first", 1), ("last", 3)] {
        let root_field = declaration(&scoped, "/app/index.nct", Kind::Field, name);
        let body_field = declaration(&scoped, "/app/body.nct", Kind::Field, name);
        assert_eq!(scoped.field(root_field), Some(fields[position]));
        assert_eq!(scoped.field(body_field), Some(fields[position]));
    }
    for (name, position) in [("hidden", 0), ("middle", 2)] {
        assert_eq!(
            scoped.field(declaration(&scoped, "/app/body.nct", Kind::Field, name)),
            Some(fields[position])
        );
    }
    let program = scoped.program();
    assert!(program.nominals().is_empty());
    assert!(program.fields().is_empty());
    assert!(program.record(record).is_none());
    assert!(
        fields
            .iter()
            .all(|field| program.fields().get(*field).is_none())
    );
    let data = program.type_identity_named("Item").unwrap();
    assert_eq!(program.type_named("Item"), None);
    assert_eq!(program.data_copyable(data), None);
    assert_eq!(
        lookup(&scoped, "/app/index.nct", Namespace::Value, "first"),
        None
    );
    let constructor = lookup(&scoped, "/app/index.nct", Namespace::Value, "Item").unwrap();
    let DeclarationId::Code(code) = constructor else {
        panic!("code")
    };
    assert_eq!(scoped.parameter_ids(code).unwrap().len(), 1);
    assert_eq!(
        lookup(&scoped, "/app/body.nct", Namespace::Value, "Item"),
        Some(code.into())
    );
    assert_eq!(
        lookup(&scoped, "/app/index.nct", Namespace::Type, "Item"),
        Some(record.into())
    );
    for (kind, name) in [(Kind::Constant, "LIMIT"), (Kind::Static, "BASE")] {
        let contract = scoped
            .declaration(declaration(&scoped, "/app/index.nct", kind, name))
            .unwrap();
        assert_eq!(
            scoped.declaration(declaration(&scoped, "/app/body.nct", kind, name)),
            Some(contract)
        );
        let DeclarationId::Code(code) = contract else {
            panic!("initializer")
        };
        assert!(scoped.parameter_ids(code).unwrap().is_empty());
        assert!(!program.header_ready(contract));
    }
    assert_eq!(
        scoped.scope(source(&scoped, "/app/index.nct")),
        Some(ScopeId::ROOT)
    );
}

#[test]
fn conflicting_seen_helpers_are_reported_at_both_real_declarations() {
    for record in [false, true] {
        let a = if record {
            "struct Shared { value: Missing }"
        } else {
            "func Shared(): Missing { missing }"
        };
        let b = if record {
            "struct Shared { value: Other }"
        } else {
            "func Shared(): Other { other }"
        };
        let selected = inventory(
            &[
                ("/app/index.nct", Root, "see ./a.nct\nsee ./b.nct"),
                ("/app/a.nct", Implementation, a),
                ("/app/b.nct", Implementation, b),
            ],
            &[(0, 0, 1), (0, 1, 2)],
            false,
        )
        .pair()
        .unwrap();
        let a_source = selected
            .inventory()
            .sources()
            .iter()
            .find(|s| s.canonical_path() == "/app/a.nct")
            .unwrap()
            .syntax()
            .source();
        let b_source = selected
            .inventory()
            .sources()
            .iter()
            .find(|s| s.canonical_path() == "/app/b.nct")
            .unwrap()
            .syntax()
            .source();
        let Err(ModuleSourcesFailure::Access(Failure::Headers(diagnostics))) =
            selected.reserve_names()
        else {
            panic!("name conflict")
        };
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert!(matches!(
            diagnostic.violation().rule,
            DeclarationRule::DuplicateValue { .. } | DeclarationRule::DuplicateRecord { .. }
        ));
        assert_eq!(diagnostic.source().primary().source(), b_source);
        assert_eq!(diagnostic.source().notes()[0].origin().source(), a_source);
    }
}

#[test]
fn equal_header_spelling_keeps_distinct_type_meanings_in_each_physical_source() {
    let scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./a.nct\nsee ./c.nct\nfunc relay(value: Data): Data",
            ),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./b.nct\nfunc relay(value: Data): Data { value }",
            ),
            ("/app/b.nct", Implementation, "struct Data { value: i32 }"),
            ("/app/c.nct", Implementation, "struct Data { value: i64 }"),
        ],
        &[(0, 0, 1), (0, 1, 3), (1, 0, 0), (1, 1, 2)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let root_data = lookup(&scoped, "/app/index.nct", Namespace::Type, "Data").unwrap();
    let implementation_data = lookup(&scoped, "/app/a.nct", Namespace::Type, "Data").unwrap();
    assert_ne!(root_data, implementation_data);
    let program = scoped.program();
    let root_scope = scoped.scope(source(&scoped, "/app/index.nct")).unwrap();
    let implementation_scope = scoped.scope(source(&scoped, "/app/a.nct")).unwrap();
    assert_ne!(
        program.type_identity_named_in(root_scope, "Data").unwrap(),
        program
            .type_identity_named_in(implementation_scope, "Data")
            .unwrap()
    );
    let contract = declaration(&scoped, "/app/index.nct", Kind::Function, "relay");
    let definition = declaration(&scoped, "/app/a.nct", Kind::Function, "relay");
    assert_eq!(scoped.declaration(contract), scoped.declaration(definition));
    assert!(!program.header_ready(scoped.declaration(contract).unwrap()));
}

#[test]
fn retained_source_binding_allows_header_writes_and_rejects_foreign_declaration_authorities() {
    let build = || {
        inventory(
            &[
                ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
                (
                    "/app/body.nct",
                    Implementation,
                    "see ./index.nct\nfunc run(): i32 { 42 }",
                ),
            ],
            &[(0, 0, 1), (1, 0, 0)],
            false,
        )
        .pair()
        .unwrap()
        .reserve_names()
        .unwrap()
    };
    let mut scoped = build();
    let DeclarationId::Code(run) = scoped
        .declaration(declaration(
            &scoped,
            "/app/index.nct",
            Kind::Function,
            "run",
        ))
        .unwrap()
    else {
        panic!("code")
    };
    let binding = scoped.source_binding();
    assert!(std::sync::Arc::ptr_eq(&binding, &scoped.source_binding()));
    let early = binding.context_for_code(&scoped, run).unwrap();
    assert!(!early.program().header_ready(run));
    let guard = early.body_source(run).unwrap();
    // This mutable operation must compile with `early` still live; it borrows only `binding`.
    assert_eq!(
        scoped.advance_header_with_values(run, |_| None).unwrap(),
        HeaderProgress::Ready
    );
    let later = binding.context_for_code(&scoped, run).unwrap();
    assert!(!early.program().header_ready(run));
    assert!(later.program().header_ready(run));
    assert!(later.program().preserves_headers(early.program()));
    assert!(guard.bind(&later).is_ok());
    let foreign = build();
    assert!(matches!(
        binding.context_for_code(&foreign, run),
        Err(ModuleSourcesFailure::Access(Failure::ChangedHeader))
    ));
    drop(scoped);
    assert!(guard.bind(&early).is_ok());
    assert!(guard.bind(&later).is_ok());
    assert_eq!(
        later.syntax().text_at(
            later
                .syntax()
                .tree()
                .node(later.code_source(run).unwrap().body().unwrap())
                .unwrap()
                .range()
        ),
        Some("{ 42 }")
    );
}

#[test]
fn root_export_inventory_does_not_publish_helpers_visible_through_see() {
    let scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\n#visibility: \"/\"\n#target: \"arm64-darwin\"\nfunc run(): i32\n#target: \"x64-linux\"\nstruct Unselected",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nfunc run(): i32 { helper() }\nfunc helper(): i32 { 42 }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let binding = scoped.source_binding();
    let exports = binding.exports().collect::<Vec<_>>();
    assert_eq!(exports.len(), 1);
    assert_eq!(
        exports[0].0,
        declaration(&scoped, "/app/index.nct", Kind::Function, "run")
    );
    assert!(matches!(exports[0].2, ModuleAudience::Package(_)));
    assert!(lookup(&scoped, "/app/index.nct", Namespace::Value, "helper").is_some());
    assert!(
        scoped
            .paired()
            .inventory()
            .audience(declaration(
                &scoped,
                "/app/body.nct",
                Kind::Function,
                "helper"
            ))
            .is_none()
    );
}

#[test]
fn field_audiences_cannot_widen_their_root_owner_and_directives_do_not_set_defaults() {
    let scoped = inventory(
        &[
            ("/app/index.nct", Root,
             "see ./body.nct\n#visibility: \"/\"\nstruct Data {\nvalue: i32\n#visibility: \"./\"\nother: i32\n}\nfunc run(): void"),
            ("/app/body.nct", Implementation,
             "see ./index.nct\nstruct Data { value: i32\nother: i32 }\nfunc run(): void {}"),
        ],
        &[(0, 0, 1), (1, 0, 0)], false,
    ).pair().unwrap().reserve_names().unwrap();
    let inventory = scoped.paired().inventory();
    for (kind, name) in [
        (Kind::Struct, "Data"),
        (Kind::Field, "value"),
        (Kind::Field, "other"),
    ] {
        assert!(matches!(
            inventory.audience(declaration(&scoped, "/app/index.nct", kind, name)),
            Some(ModuleAudience::Package(_))
        ));
    }
    assert_eq!(
        inventory.audience(declaration(
            &scoped,
            "/app/index.nct",
            Kind::Function,
            "run"
        )),
        Some(&ModuleAudience::Public)
    );
}
