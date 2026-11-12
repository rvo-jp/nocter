//! Resolve real physical headers without writing or publishing the shared declaration authority.
use super::module_contracts::inventory;
use super::*;
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind as Kind};
use definitions::{CodeDefinition, HeaderScope, Projection, Recipe};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::{
    DataReference, DeclarationId, DeclarationRule, HeaderPosition, ValueDefinition,
};
use std::collections::BTreeMap;

fn occurrence<'a>(
    scoped: &'a ScopedModuleSources<'_>,
    path: &str,
    kind: Kind,
    name: &str,
) -> (
    SurfaceDeclarationId,
    BoundSyntax<'a>,
    nocter_declarations::value_access::ScopeId,
) {
    let input = scoped.paired().inventory();
    let (source_id, source) = input
        .source_entries()
        .find(|(_, s)| s.canonical_path() == path)
        .unwrap();
    let syntax = bound(input.source_map(), source.syntax());
    let id = input
        .declaration_entries()
        .find(|(_, declaration)| {
            declaration.source() == source_id
                && declaration.kind() == kind
                && declaration.name().and_then(|token| {
                    input
                        .source_map()
                        .get(token.source())?
                        .text_at(token.range())
                }) == Some(name)
        })
        .unwrap()
        .0;
    (id, syntax, scoped.scope(source_id).unwrap())
}

fn project_function(
    scoped: &ScopedModuleSources<'_>,
    path: &str,
    name: &str,
) -> Projection<CodeDefinition> {
    let (raw, syntax, lookup) = occurrence(scoped, path, Kind::Function, name);
    let Some(DeclarationId::Code(code)) = scoped.declaration(raw) else {
        panic!("code")
    };
    let header = headers::function(
        syntax,
        scoped.paired().inventory().declarations()[raw.index()].node(),
    )
    .unwrap();
    let sources = source::Sources::new(
        BTreeMap::from([(code, CodeSource::from_header(&header))]),
        BTreeMap::new(),
    )
    .unwrap();
    definitions::project_code(
        &scoped.program(),
        HeaderScope { syntax, lookup },
        code,
        &Recipe::from(header),
        sources.codes.get(&code).unwrap(),
        &sources,
        &mut |_| None,
    )
    .unwrap()
}

#[test]
fn repeated_headers_resolve_in_their_own_physical_scopes_without_publication() {
    for same_binding in [false, true] {
        let body = if same_binding {
            "see ./index.nct\nsee ./data.nct\nfunc relay(value: Data): Data { value }"
        } else {
            "see ./index.nct\nsee ./other.nct\nfunc relay(value: Data): Data { value }"
        };
        let body_target = if same_binding { 2 } else { 3 };
        let scoped = inventory(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./body.nct\nsee ./data.nct\nfunc relay(value: Data): Data",
                ),
                ("/app/body.nct", Implementation, body),
                (
                    "/app/data.nct",
                    Implementation,
                    "struct Data { value: i32 }",
                ),
                (
                    "/app/other.nct",
                    Implementation,
                    "struct Data { value: i64 }",
                ),
            ],
            &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, body_target)],
            true,
        )
        .pair()
        .unwrap()
        .reserve_names()
        .unwrap();
        let root = project_function(&scoped, "/app/index.nct", "relay");
        let body = project_function(&scoped, "/app/body.nct", "relay");
        assert!(root.progress.missing.is_empty() && root.progress.violations.is_empty());
        assert!(body.progress.missing.is_empty() && body.progress.violations.is_empty());
        let root = root.definition.unwrap();
        let body = body.definition.unwrap();
        assert_eq!(root == body, same_binding);
        for (path, definition) in [("/app/index.nct", root), ("/app/body.nct", body)] {
            let (raw, _, scope) = occurrence(&scoped, path, Kind::Function, "relay");
            let program = scoped.program();
            let expected = program
                .resolve_data_in(scope, program.symbols().get("Data").unwrap())
                .unwrap();
            assert!(matches!(expected, DataReference::Record(_)));
            assert_eq!(
                definition.parameters[0].data,
                ValueDefinition::Data(expected.clone())
            );
            assert_eq!(definition.result, ValueDefinition::Data(expected));
            assert!(!program.header_ready(scoped.declaration(raw).unwrap()));
            assert!(program.codes().is_empty() && program.records().is_empty());
        }
    }
}

#[test]
fn unknown_header_names_are_local_violations_and_foreign_tokens_are_integrity_failures() {
    let scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nfunc relay(value: Missing): Missing",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nfunc relay(value: Missing): Missing { value }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let projected = project_function(&scoped, "/app/body.nct", "relay");
    assert!(projected.definition.is_none());
    assert!(projected.progress.missing.is_empty());
    assert_eq!(projected.progress.violations.len(), 2);
    assert_eq!(
        projected.progress.violations[0].position,
        HeaderPosition::Parameter(0)
    );
    assert_eq!(
        projected.progress.violations[1].position,
        HeaderPosition::Result
    );
    assert!(
        projected
            .progress
            .violations
            .iter()
            .all(|v| matches!(v.rule, DeclarationRule::UnknownDataName { .. }))
    );
    let (raw, syntax, _) = occurrence(&scoped, "/app/body.nct", Kind::Function, "relay");
    let Some(DeclarationId::Code(code)) = scoped.declaration(raw) else {
        panic!("code")
    };
    let header = headers::function(
        syntax,
        scoped.paired().inventory().declarations()[raw.index()].node(),
    )
    .unwrap();
    let sources = source::Sources::new(
        BTreeMap::from([(code, CodeSource::from_header(&header))]),
        BTreeMap::new(),
    )
    .unwrap();
    let (_, wrong_tree, lookup) = occurrence(&scoped, "/app/index.nct", Kind::Function, "relay");
    assert!(matches!(
        definitions::project_code(
            &scoped.program(),
            HeaderScope {
                syntax: wrong_tree,
                lookup
            },
            code,
            &Recipe::from(header),
            sources.codes.get(&code).unwrap(),
            &sources,
            &mut |_| None
        ),
        Err(Failure::InconsistentSyntax)
    ));
}

#[test]
fn complete_representation_fields_use_the_definition_scope_and_keep_field_order() {
    let scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nstruct Boxed { shown: Data }",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nsee ./data.nct\nstruct Boxed { hidden: bool\nshown: Data }",
            ),
            (
                "/app/data.nct",
                Implementation,
                "struct Data { value: i32 }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let (raw, syntax, lookup) = occurrence(&scoped, "/app/body.nct", Kind::Struct, "Boxed");
    let Some(DeclarationId::Record(record)) = scoped.declaration(raw) else {
        panic!("record")
    };
    let header = headers::record(
        syntax,
        scoped.paired().inventory().declarations()[raw.index()].node(),
    )
    .unwrap();
    let program = scoped.program();
    let sources = source::Sources::new(BTreeMap::new(), BTreeMap::new()).unwrap();
    let projected = definitions::project_record(
        &program,
        HeaderScope { syntax, lookup },
        record,
        &header,
        &sources,
        &mut |_| None,
    )
    .unwrap();
    assert!(projected.progress.missing.is_empty() && projected.progress.violations.is_empty());
    let definition = projected.definition.unwrap();
    assert!(!definition.copy);
    assert_eq!(
        definition
            .fields
            .iter()
            .map(|f| program.symbols().spelling(f.name).unwrap())
            .collect::<Vec<_>>(),
        ["hidden", "shown"]
    );
    assert_eq!(
        definition.fields[1].data,
        program
            .resolve_data_in(lookup, program.symbols().get("Data").unwrap())
            .unwrap()
    );
    let (_, _, root_lookup) = occurrence(&scoped, "/app/index.nct", Kind::Struct, "Boxed");
    assert!(
        program
            .resolve_data_in(root_lookup, program.symbols().get("Data").unwrap())
            .is_err()
    );
    assert!(program.records().is_empty() && program.fields().is_empty());
    assert!(!program.header_ready(record));
}

#[test]
fn empty_representation_still_requires_its_actual_syntax_source() {
    let scoped = inventory(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Empty"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Empty {}",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let (raw, syntax, lookup) = occurrence(&scoped, "/app/body.nct", Kind::Struct, "Empty");
    let Some(DeclarationId::Record(record)) = scoped.declaration(raw) else {
        panic!("record")
    };
    let header = headers::record(
        syntax,
        scoped.paired().inventory().declarations()[raw.index()].node(),
    )
    .unwrap();
    let (_, foreign, _) = occurrence(&scoped, "/app/index.nct", Kind::Struct, "Empty");
    let sources = source::Sources::new(BTreeMap::new(), BTreeMap::new()).unwrap();
    let program = scoped.program();
    assert!(
        definitions::project_record(
            &program,
            HeaderScope { syntax, lookup },
            record,
            &header,
            &sources,
            &mut |_| None
        )
        .unwrap()
        .definition
        .unwrap()
        .fields
        .is_empty()
    );
    assert!(matches!(
        definitions::project_record(
            &program,
            HeaderScope {
                syntax: foreign,
                lookup
            },
            record,
            &header,
            &sources,
            &mut |_| None
        ),
        Err(Failure::InconsistentSyntax)
    ));
}
