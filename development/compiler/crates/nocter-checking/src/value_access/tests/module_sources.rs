//! Whole physical source input uses the same staged header/body/value engine as a single source.
use super::source_scopes::Fixture;
use super::*;
use accepted::{ModuleSourceUnit, SourceFailure};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::PayloadSchema;

fn counts(body_increment: u64) -> Fixture {
    let body_count =
        format!("const BASE: usize = 39\nconst func width(): usize {{ BASE + {body_increment} }}");
    Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./a.nct\nsee ./root_count.nct\nfunc run(): [i32; width()]",
            ),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./body_count.nct\nfunc run(): [i32; width()] { [0; 42] }",
            ),
            (
                "/app/root_count.nct",
                Implementation,
                "const BASE: usize = 40\nconst func width(): usize { BASE + 2 }",
            ),
            ("/app/body_count.nct", Implementation, &body_count),
        ],
        &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, 3)],
    )
}

#[test]
fn module_source_coordinator_executes_each_header_supplier_before_accepting_ordinary_bodies() {
    let fixture = counts(3);
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = source.unit().program();
    let run = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    let result = program.codes().get(run).unwrap().result();
    assert!(
        matches!(program.data_schema(result), Some(PayloadSchema::Array(array)) if array.length() == 42)
    );
    let context = source.declarations().current().unwrap();
    source.unit().bind(&context).unwrap();
    assert_eq!(
        context.for_code(run).unwrap().syntax().tree().source(),
        fixture.tree(1).source()
    );
    assert!(source.unit().program().headers_complete());
}

#[test]
fn module_source_coordinator_rejects_real_count_disagreement_at_both_physical_headers() {
    let fixture = counts(4);
    let Err(SourceFailure::Modules(declarations::ModuleSourcesFailure::Contract(error))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("actual header disagreement")
    };
    assert_eq!(
        error.rule(),
        nocter_declaration_lowering::DeclarationContractRule::MismatchedBody
    );
    assert_eq!(error.source().primary().source(), fixture.tree(1).source());
    assert_eq!(
        error.source().notes()[0].origin().source(),
        fixture.tree(0).source()
    );
}

#[test]
fn module_source_coordinator_checks_unused_sources_and_projects_their_actual_rejections() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nfunc run(): i32 { 42 }",
            ),
            (
                "/app/unused.nct",
                Implementation,
                "func unused(): i32 { missing }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Checking(accepted::Failure::Body {
        failure: Failure::Rejected(error),
        ..
    })) = ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("unused body")
    };
    assert_eq!(error.rule(), Rule::UnknownName);
    assert_eq!(error.source().primary().source(), fixture.tree(2).source());
}

#[test]
fn module_source_coordinator_keeps_header_cycle_diagnostics_on_complete_representation_fields() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Node"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Node { next: Node }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Headers(errors))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("record header cycle")
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].source().primary().source(),
        fixture.tree(1).source()
    );
    assert!(matches!(
        errors[0].violation().rule,
        nocter_declarations::value_access::DeclarationRule::RecursivePayload { .. }
    ));
}

#[test]
fn module_source_coordinator_accepts_empty_library_sources_and_rejects_unselected_primitives() {
    let empty = Fixture::new(&[("/app/index.nct", Root, "")], &[]);
    let source = ModuleSourceUnit::prepare(&empty.input()).unwrap();
    assert!(source.unit().program().codes().is_empty());
    let primitive = Fixture::new(
        &[
            ("/app/index.nct", Root, ""),
            (
                "/app/body.nct",
                Implementation,
                "primitive func operation(): i32",
            ),
        ],
        &[],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Primitive(error))) =
        ModuleSourceUnit::prepare(&primitive.input())
    else {
        panic!("unselected primitive")
    };
    assert_eq!(error.rule(), declarations::PrimitiveRule::Unauthorized);
    assert_eq!(
        error.source().primary().source(),
        primitive.tree(1).source()
    );
}

#[test]
fn module_visibility_sealing_checks_signature_names_without_exposing_hidden_representation_fields()
{
    for (root, body, accepted) in [
        (
            "struct Outer\nfunc inspect(value: Outer): void",
            "struct Outer { hidden: Private }\nstruct Private {}\nfunc inspect(value: Outer): void {}",
            true,
        ),
        (
            "struct Outer { shown: Private }",
            "struct Outer { shown: Private }\nstruct Private {}",
            false,
        ),
        (
            "func inspect(value: Private): void",
            "struct Private {}\nfunc inspect(value: Private): void {}",
            false,
        ),
        (
            "func inspect(value: [Private; 0]): void",
            "struct Private {}\nfunc inspect(value: [Private; 0]): void {}",
            false,
        ),
        (
            "func inspect(action: func(Private): void): void",
            "struct Private {}\nfunc inspect(action: func(Private): void): void {}",
            false,
        ),
        (
            "#visibility: \"/\"\nstruct Restricted\nfunc inspect(value: Restricted): void",
            "struct Restricted {}\nfunc inspect(value: Restricted): void {}",
            false,
        ),
        (
            "#visibility: \"/\"\nstruct Restricted\n#visibility: \"./\"\nfunc inspect(value: Restricted): void",
            "struct Restricted {}\nfunc inspect(value: Restricted): void {}",
            true,
        ),
    ] {
        let root = format!("see ./body.nct\n{root}");
        let body = format!("see ./index.nct\n{body}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if accepted {
            assert!(result.is_ok(), "{root}: {result:?}");
        } else {
            let Err(SourceFailure::Declarations(declarations::Failure::Visibility(error))) = result
            else {
                panic!("signature exposure: {root}: {result:?}")
            };
            assert_eq!(
                error.rule(),
                declarations::VisibilityRule::SignatureExposure
            );
            assert_eq!(error.source().primary().source(), fixture.tree(0).source());
            assert_eq!(error.source().notes().len(), 1);
        }
    }
}

#[test]
fn selected_type_imports_and_root_reexports_share_the_original_nominal_identity() {
    use nocter_compile_input::ModuleIdentity;
    use nocter_declarations::value_access::Namespace;
    use nocter_model::PackageIdentity;
    let fixture = Fixture::new(&[
        ("/app/index.nct", Root, "use ./wire.Item as PublicItem\nsee ./body.nct\nfunc inspect(value: PublicItem): void"),
        ("/app/body.nct", Implementation, "use ./wire.Item as PublicItem\nsee ./index.nct\nfunc inspect(value: PublicItem): void {}"),
        ("/app/wire/index.nct", Root, "use ../data.Data as Item"),
        ("/app/data/index.nct", Root, "see ./body.nct\nstruct Data"),
        ("/app/data/body.nct", Implementation, "see ./index.nct\nstruct Data { value: i32 }"),
    ], &[(0, 0, 1), (1, 0, 0), (3, 0, 4), (4, 0, 3)])
    .with_modules(&[(&[], &[0, 1]), (&["wire"], &[2]), (&["data"], &[3, 4])])
    .with_uses(&[(0, 0, &["wire"]), (1, 0, &["wire"]), (2, 0, &["data"])]);
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = source.declarations().program();
    let identity = |path: &[&str]| {
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), path.iter().copied())
    };
    let root = identity(&[]);
    let lookup = |path: &[&str], name| {
        source
            .declarations()
            .lookup_export(
                &identity(path),
                &root,
                Namespace::Type,
                program.symbols().get(name).unwrap(),
            )
            .unwrap()
    };
    assert_eq!(lookup(&[], "PublicItem"), lookup(&["wire"], "Item"));
    assert_eq!(lookup(&[], "PublicItem"), lookup(&["data"], "Data"));
    let current = source.declarations().current().unwrap();
    assert!(source.unit().bind(&current).is_ok());
}

#[test]
fn selected_type_imports_diagnose_missing_private_non_type_and_widening_names_at_the_import() {
    use nocter_declaration_lowering::{ImportRule, NamespaceRule};
    for (import, restriction, expected_import, expected_namespace) in [
        (
            "use ./data.Absent",
            "",
            Some(ImportRule::MissingImportedName),
            None,
        ),
        (
            "use ./data.Hidden",
            "",
            Some(ImportRule::MissingImportedName),
            None,
        ),
        (
            "use ./data.compute",
            "",
            Some(ImportRule::NonTypeSelection),
            None,
        ),
        (
            "use ./data.Data",
            "#visibility: \"./\"\n",
            Some(ImportRule::InaccessibleImportedName),
            None,
        ),
        (
            "use ./data.Data",
            "#visibility: \"/\"\n",
            Some(ImportRule::WideningReexport),
            None,
        ),
        (
            "#visibility: \"/\"\nuse ./data.Data",
            "#visibility: \"/\"\n",
            None,
            None,
        ),
        (
            "use ./data.{Data as Item, Data as Item}",
            "",
            None,
            Some(NamespaceRule::NameCollision),
        ),
        (
            "use ./data.Data as i32",
            "",
            None,
            Some(NamespaceRule::ReservedName),
        ),
    ] {
        let root = format!("see ./body.nct\n{restriction}struct Data\nfunc compute(): i32");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, import),
                ("/app/data/index.nct", Root, &root),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data {}\nstruct Hidden {}\nfunc compute(): i32 { 42 }",
                ),
            ],
            &[(1, 0, 2), (2, 0, 1)],
        )
        .with_modules(&[(&[], &[0]), (&["data"], &[1, 2])])
        .with_uses(&[(0, 0, &["data"])]);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        let diagnostic = match (result, expected_import, expected_namespace) {
            (Ok(_), None, None) => continue,
            (
                Err(SourceFailure::Declarations(declarations::Failure::Import(diagnostic))),
                Some(rule),
                None,
            ) => {
                assert_eq!(diagnostic.rule(), rule, "{import}");
                diagnostic.source().clone()
            }
            (
                Err(SourceFailure::Declarations(declarations::Failure::Namespace(diagnostic))),
                None,
                Some(rule),
            ) => {
                assert_eq!(diagnostic.rule(), rule, "{import}");
                diagnostic.source().clone()
            }
            (result, _, _) => panic!("{import}: {result:?}"),
        };
        assert_eq!(diagnostic.primary().source(), fixture.tree(0).source());
    }
}

#[test]
fn seeing_a_source_does_not_inherit_its_imports_into_either_header() {
    for importing in [0, 1] {
        let texts = [
            "see ./body.nct\nfunc inspect(value: Item): void",
            "see ./index.nct\nfunc inspect(value: Item): void {}",
        ];
        let mut texts = texts.map(str::to_owned);
        texts[importing] = format!("use ./data.Data as Item\n{}", texts[importing]);
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &texts[0]),
                ("/app/body.nct", Implementation, &texts[1]),
                ("/app/data/index.nct", Root, "see ./body.nct\nstruct Data"),
                (
                    "/app/data/body.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data {}",
                ),
            ],
            &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)],
        )
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(importing, 0, &["data"])]);
        let Err(SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("imports must remain physical-source-local")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source().code() == "E0290"
                    && diagnostic.source().primary().source()
                        == fixture.tree(1 - importing).source())
        );
    }
}

#[test]
fn imported_nominals_do_not_grant_hidden_fields_or_structural_construction() {
    for (root_record, definition, signature, expression) in [
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func inspect(value: Data): i32",
            "value.hidden",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func inspect(take value: Data): i32",
            "(value^).hidden",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32\nhidden: i32 }",
            "func build(): Data",
            "Data { shown: 1, hidden: 2 }",
        ),
        (
            "struct Data { shown: i32 }",
            "struct Data { shown: i32 }",
            "func build(): Data",
            "Data { shown: 1 }",
        ),
        (
            "struct Data",
            "struct Data {}",
            "func build(): Data",
            "Data {}",
        ),
    ] {
        let app = format!("use ./data.Data\nsee ./body.nct\n{signature}");
        let app_body = format!("use ./data.Data\nsee ./index.nct\n{signature} {{ {expression} }}");
        let data = format!("see ./body.nct\n{root_record}");
        let data_body = format!("see ./index.nct\n{definition}");
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, &app),
                ("/app/body.nct", Implementation, &app_body),
                ("/app/data/index.nct", Root, &data),
                ("/app/data/body.nct", Implementation, &data_body),
            ],
            &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)],
        )
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(0, 0, &["data"]), (1, 0, &["data"])]);
        let Err(SourceFailure::Checking(accepted::Failure::Body {
            failure: Failure::Rejected(diagnostic),
            ..
        })) = ModuleSourceUnit::prepare(&fixture.input())
        else {
            panic!("expected a source permission rejection: {expression}")
        };
        assert_eq!(
            diagnostic.rule(),
            if expression.ends_with(".hidden") {
                Rule::InaccessibleField
            } else {
                Rule::InaccessibleRepresentation
            }
        );
        assert_eq!(
            diagnostic.source().primary().source(),
            fixture.tree(1).source()
        );
    }
}

#[test]
fn visible_field_audiences_and_complete_definition_views_do_not_depend_on_compiler_schema_knowledge()
 {
    for restrict in [false, true] {
        let data = format!(
            "see ./body.nct\nstruct Data {{\n{}shown: i32\n}}",
            if restrict {
                "#visibility: \"./\"\n"
            } else {
                ""
            }
        );
        let fixture = Fixture::new(&[
            ("/app/index.nct", Root, "use ./data.Data\nsee ./body.nct\nfunc inspect(value: Data): i32"),
            ("/app/body.nct", Implementation, "use ./data.Data\nsee ./index.nct\nfunc inspect(value: Data): i32 { value.shown }"),
            ("/app/data/index.nct", Root, &data),
            ("/app/data/body.nct", Implementation, "see ./index.nct\nstruct Data { shown: i32\nhidden: i32 }\nfunc build(): Data { Data { shown: 1, hidden: 2 } }\nfunc inspect(value: Data): i32 { value.hidden }"),
        ], &[(0, 0, 1), (1, 0, 0), (2, 0, 3), (3, 0, 2)])
        .with_modules(&[(&[], &[0, 1]), (&["data"], &[2, 3])])
        .with_uses(&[(0, 0, &["data"]), (1, 0, &["data"])]);
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if restrict {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("restricted field: {result:?}")
            };
            assert_eq!(diagnostic.rule(), Rule::InaccessibleField);
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(1).source()
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
    for see_representation in [false, true] {
        let caller = format!(
            "see ./index.nct\n{}func inspect(value: Data): i32 {{ value.hidden }}",
            if see_representation {
                "see ./representation.nct\n"
            } else {
                ""
            }
        );
        let mut edges = vec![(0, 0, 1), (0, 1, 2), (1, 0, 0), (2, 0, 0)];
        if see_representation {
            edges.push((2, 1, 1));
        }
        let fixture = Fixture::new(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./representation.nct\nsee ./caller.nct\nstruct Data\nfunc inspect(value: Data): i32",
                ),
                (
                    "/app/representation.nct",
                    Implementation,
                    "see ./index.nct\nstruct Data { hidden: i32 }",
                ),
                ("/app/caller.nct", Implementation, &caller),
            ],
            &edges,
        );
        let result = ModuleSourceUnit::prepare(&fixture.input());
        if see_representation {
            assert!(result.is_ok(), "{result:?}");
        } else {
            let Err(SourceFailure::Checking(accepted::Failure::Body {
                failure: Failure::Rejected(diagnostic),
                ..
            })) = result
            else {
                panic!("transitive complete representation: {result:?}")
            };
            assert_eq!(diagnostic.rule(), Rule::InaccessibleField);
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.tree(2).source()
            );
        }
    }
}
