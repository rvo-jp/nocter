use std::fmt;

use crate::{
    CallableContractDiagnostic, CallableContractError, CompileUnitInput, GenericError,
    HeaderDefinitionError, HeaderError, ImportDiagnostic, ImportError, LoweredDeclarations,
    ModuleIdentity, NamespaceDiagnostic, PreludeError, ReservationError, SourceDiagnostic,
    SurfaceDiagnostic, SurfaceError, TypeBindingError, TypeNormalizationError,
    analyze_callable_contracts, apply_standard_prelude, bind_header_type_syntax,
    collect_declaration_surface, define_declaration_headers, normalize_header_types,
    prepare_authored_imports, prepare_declaration_headers, prepare_generic_binders,
};

#[derive(Debug)]
pub enum DeclarationLoweringError {
    Surface(SurfaceDiagnostic),
    InternalSurface(SurfaceError),
    CallableContract(CallableContractDiagnostic),
    InternalContract(CallableContractError),
    Reservation(ReservationError),
    Namespace(NamespaceDiagnostic),
    InternalHeader(HeaderError),
    Generic(GenericError),
    Import(ImportDiagnostic),
    InternalImport(ImportError),
    Prelude(PreludeError),
    TypeBinding(TypeBindingError),
    TypeNormalization(TypeNormalizationError),
    Definition(HeaderDefinitionError),
}

impl DeclarationLoweringError {
    /// Returns the common public diagnostic for a source-backed language-rule failure.
    ///
    /// `None` identifies a stage error that has not yet crossed a public diagnostic boundary or an
    /// internal compiler inconsistency. Consumers must not manufacture a public code for it.
    #[must_use]
    pub fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Surface(diagnostic) => Some(diagnostic.source()),
            Self::CallableContract(diagnostic) => Some(diagnostic.source()),
            Self::Namespace(diagnostic) => Some(diagnostic.source()),
            Self::Import(diagnostic) => Some(diagnostic.source()),
            Self::Definition(error) => error.source_diagnostic(),
            Self::InternalSurface(_)
            | Self::InternalContract(_)
            | Self::Reservation(_)
            | Self::InternalHeader(_)
            | Self::Generic(_)
            | Self::InternalImport(_)
            | Self::Prelude(_)
            | Self::TypeBinding(_)
            | Self::TypeNormalization(_) => None,
        }
    }
}

impl fmt::Display for DeclarationLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => error.fmt(formatter),
            Self::InternalSurface(error) => error.fmt(formatter),
            Self::CallableContract(error) => error.fmt(formatter),
            Self::InternalContract(error) => error.fmt(formatter),
            Self::Reservation(error) => error.fmt(formatter),
            Self::Namespace(error) => error.fmt(formatter),
            Self::InternalHeader(error) => error.fmt(formatter),
            Self::Generic(error) => error.fmt(formatter),
            Self::Import(error) => error.fmt(formatter),
            Self::InternalImport(error) => error.fmt(formatter),
            Self::Prelude(error) => error.fmt(formatter),
            Self::TypeBinding(error) => error.fmt(formatter),
            Self::TypeNormalization(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DeclarationLoweringError {}

/// Lowers one discovery-owned compile unit through the complete declaration pipeline.
///
/// This is the production entry point. Individual passes remain exposed for focused tests and
/// compiler development, but consumers do not select or reorder them.
///
/// # Errors
///
/// Returns the exact failing stage. Source-backed module-surface, callable-contract, namespace,
/// and freeze-time declaration rules are already projected to common diagnostics;
/// remaining stage errors stay typed until their diagnostic mappings are completed.
pub fn lower_compile_unit_declarations(
    input: &CompileUnitInput<'_>,
    prelude: &ModuleIdentity,
) -> Result<LoweredDeclarations, DeclarationLoweringError> {
    let surface = match collect_declaration_surface(input) {
        Ok(surface) => surface,
        Err(error) => {
            return match SurfaceDiagnostic::project(error, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Surface(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalSurface(internal)),
            };
        }
    };
    let contracts = match analyze_callable_contracts(&surface) {
        Ok(contracts) => contracts,
        Err(error) => {
            return match CallableContractDiagnostic::project(error, &surface) {
                Ok(diagnostic) => Err(DeclarationLoweringError::CallableContract(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalContract(internal)),
            };
        }
    };
    let reserved = crate::reservation::reserve_with_contracts(surface, contracts)
        .map_err(DeclarationLoweringError::Reservation)?;
    let headers = match prepare_declaration_headers(reserved) {
        Ok(headers) => headers,
        Err(HeaderError::Namespace(violation)) => {
            return match NamespaceDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Namespace(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalHeader(
                    HeaderError::Namespace(internal),
                )),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalHeader(internal)),
    };
    let generics = prepare_generic_binders(headers).map_err(DeclarationLoweringError::Generic)?;
    let imports = match prepare_authored_imports(generics) {
        Ok(imports) => imports,
        Err(ImportError::Namespace(violation)) => {
            return match NamespaceDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Namespace(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalImport(
                    ImportError::Namespace(internal),
                )),
            };
        }
        Err(ImportError::Rule(violation)) => {
            return match ImportDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Import(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalImport(ImportError::Rule(
                    internal,
                ))),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalImport(internal)),
    };
    let namespaces =
        apply_standard_prelude(imports, prelude).map_err(DeclarationLoweringError::Prelude)?;
    let bound =
        bind_header_type_syntax(namespaces).map_err(DeclarationLoweringError::TypeBinding)?;
    let normalized =
        normalize_header_types(bound).map_err(DeclarationLoweringError::TypeNormalization)?;
    define_declaration_headers(normalized).map_err(DeclarationLoweringError::Definition)
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    use crate::test_support::{module_use, source_use};
    use crate::{
        CallableContractRule, CompileUnitInput, DeclarationLoweringError, ImportRule,
        ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind, NamespaceRule,
        PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
        lower_compile_unit_declarations,
    };

    #[test]
    fn production_pipeline_projects_surface_diagnostics() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "use ./private\n");
        let implementation_id = add_source(
            &mut sources,
            "/app/private.nct",
            "pub func exposed(): void {}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
        let input = CompileUnitInput::new(
            &sources,
            vec![package(
                "workspace:app",
                "app",
                "/app/nocter.nct",
                &manifest,
            )],
            vec![ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                vec![
                    ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                    ModuleSourceInput::new(
                        "/app/private.nct",
                        ModuleSourceKind::Implementation,
                        &implementation,
                    ),
                ],
            )],
            vec![source_use(&root, 0, "/app/private.nct")],
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();

        assert_eq!(
            error.source_diagnostic().map(crate::SourceDiagnostic::code),
            Some("E0230")
        );
        assert!(matches!(error, DeclarationLoweringError::Surface(_)));
    }

    #[test]
    fn production_pipeline_projects_duplicate_name_tokens_in_canonical_order() {
        let text = "func duplicate(): void {}\nfunc duplicate(value: usize): void {}\n";
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let input = CompileUnitInput::new(
            &sources,
            vec![package(
                "workspace:app",
                "app",
                "/app/nocter.nct",
                &manifest,
            )],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Namespace(diagnostic) = error else {
            panic!("duplicate name did not produce a namespace diagnostic");
        };

        assert_eq!(diagnostic.rule(), NamespaceRule::NameCollision);
        assert_eq!(diagnostic.source().code(), "E0241");
        assert_eq!(diagnostic.source().primary().source(), root_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.rfind("duplicate").unwrap()).unwrap()
        );
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0]
                .origin()
                .span()
                .range()
                .start()
                .get(),
            u32::try_from(text.find("duplicate").unwrap()).unwrap()
        );
    }

    #[test]
    fn production_pipeline_projects_reserved_declaration_names() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "struct usize {}\n");
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let input = CompileUnitInput::new(
            &sources,
            vec![package(
                "workspace:app",
                "app",
                "/app/nocter.nct",
                &manifest,
            )],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Namespace(diagnostic) = error else {
            panic!("reserved name did not produce a namespace diagnostic");
        };

        assert_eq!(diagnostic.rule(), NamespaceRule::ReservedName);
        assert_eq!(diagnostic.source().code(), "E0240");
        assert_eq!(diagnostic.source().primary().source(), root_id);
        assert!(diagnostic.source().primary().token().is_some());
    }

    #[test]
    fn production_pipeline_projects_visibility_above_the_package_root() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "");
        let child_id = add_source(
            &mut sources,
            "/app/parser/index.nct",
            "pub(../../) func exposed(): void {}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let child = parse_source(&sources, child_id, ParseGoal::ModuleSource);
        let input = CompileUnitInput::new(
            &sources,
            vec![package(
                "workspace:app",
                "app",
                "/app/nocter.nct",
                &manifest,
            )],
            vec![
                module("workspace:app", &[], "/app/index.nct", &root),
                module(
                    "workspace:app",
                    &["parser"],
                    "/app/parser/index.nct",
                    &child,
                ),
            ],
            Vec::new(),
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Namespace(diagnostic) = error else {
            panic!("invalid visibility did not produce a namespace diagnostic");
        };

        assert_eq!(diagnostic.rule(), NamespaceRule::VisibilityAbovePackageRoot);
        assert_eq!(diagnostic.source().code(), "E0242");
        assert_eq!(diagnostic.source().primary().source(), child_id);
        assert!(diagnostic.source().primary().node().is_some());
    }

    #[test]
    fn production_pipeline_projects_import_access_with_its_declaration() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Hidden\n");
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "struct Hidden {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::PackageFile);
        let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::ModuleSource);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            &sources,
            vec![
                package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/nocter.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Import(diagnostic) = error else {
            panic!("inaccessible import did not produce an import diagnostic");
        };

        assert_eq!(diagnostic.rule(), ImportRule::InaccessibleImportedName);
        assert_eq!(diagnostic.source().code(), "E0412");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0].origin().source(),
            dependency_id
        );
        assert!(diagnostic.source().notes()[0].origin().token().is_some());
    }

    #[test]
    fn production_pipeline_projects_missing_imported_names() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Missing\n");
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "pub struct Present {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::PackageFile);
        let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::ModuleSource);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            &sources,
            vec![
                package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/nocter.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Import(diagnostic) = error else {
            panic!("missing name did not produce an import diagnostic");
        };

        assert_eq!(diagnostic.rule(), ImportRule::MissingImportedName);
        assert_eq!(diagnostic.source().code(), "E0260");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert!(diagnostic.source().notes().is_empty());
    }

    #[test]
    fn import_collisions_reuse_the_namespace_diagnostic() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
        let app_id = add_source(
            &mut sources,
            "/app/index.nct",
            "use dep.Value as Item\n\nstruct Item {}\n",
        );
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::PackageFile);
        let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::ModuleSource);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            &sources,
            vec![
                package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/nocter.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Namespace(diagnostic) = error else {
            panic!("import collision did not produce a namespace diagnostic");
        };

        assert_eq!(diagnostic.rule(), NamespaceRule::NameCollision);
        assert_eq!(diagnostic.source().code(), "E0241");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(diagnostic.source().notes()[0].origin().source(), app_id);
        assert!(diagnostic.source().notes()[0].origin().token().is_some());
    }

    #[test]
    fn production_pipeline_projects_callable_contract_diagnostics() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "pub func run(): void\n");
        let standard_id = add_source(&mut sources, "/std/index.nct", "");
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
        let standard_manifest =
            parse_source(&sources, standard_manifest_id, ParseGoal::PackageFile);
        let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
        let standard = parse_source(&sources, standard_id, ParseGoal::ModuleSource);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
        let prelude_identity =
            ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        let input = CompileUnitInput::new(
            &sources,
            vec![
                package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
                package(
                    "toolchain:std",
                    "std",
                    "/std/nocter.nct",
                    &standard_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("toolchain:std", &[], "/std/index.nct", &standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &prelude,
                ),
            ],
            vec![],
        );

        let error = lower_compile_unit_declarations(&input, &prelude_identity).unwrap_err();
        assert_eq!(
            error.source_diagnostic().map(crate::SourceDiagnostic::code),
            Some("E0250")
        );
        let DeclarationLoweringError::CallableContract(diagnostic) = error else {
            panic!("missing body did not produce a callable contract diagnostic");
        };
        assert_eq!(diagnostic.rule(), CallableContractRule::MissingBody);
        assert_eq!(diagnostic.source().code(), "E0250");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().node().is_some());
        assert_eq!(diagnostic.source().notes(), []);
    }

    fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
        sources
            .add_bytes(SourceName::new(name), text.as_bytes())
            .unwrap()
    }

    fn parse_source(
        sources: &SourceMap,
        source: nocter_source::SourceId,
        goal: ParseGoal,
    ) -> SyntaxTree {
        let tree = parse(sources.get(source).unwrap(), goal);
        assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
        tree
    }

    fn package<'syntax>(
        identity: &str,
        name: &str,
        path: &str,
        manifest: &'syntax SyntaxTree,
    ) -> PackageInput<'syntax> {
        PackageInput::new(
            PackageIdentity::new(identity),
            name,
            PackageMode::Declared,
            Some(PackageDeclarationInput::new(path, manifest)),
        )
    }

    fn module<'syntax>(
        identity: &str,
        path: &[&str],
        source_path: &str,
        source: &'syntax SyntaxTree,
    ) -> ModuleInput<'syntax> {
        ModuleInput::new(
            ModuleIdentity::new(PackageIdentity::new(identity), path.iter().copied()),
            vec![ModuleSourceInput::new(
                source_path,
                ModuleSourceKind::Root,
                source,
            )],
        )
    }
}
