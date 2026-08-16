use std::fmt;

use crate::{
    CallableContractDiagnostic, CallableContractError, CompileUnitInput, GenericDiagnostic,
    GenericError, HeaderDefinitionError, HeaderError, ImportDiagnostic, ImportError,
    LoweredDeclarations, ModuleIdentity, NamespaceDiagnostic, PreludeError, ReservationError,
    SourceDiagnostic, SurfaceDiagnostic, SurfaceError, TopologyDiagnostic, TypeBindingDiagnostic,
    TypeBindingError, TypeNormalizationError, analyze_callable_contracts, apply_standard_prelude,
    bind_header_type_syntax, collect_declaration_surface, define_declaration_headers,
    normalize_header_types, prepare_authored_imports, prepare_declaration_headers,
    prepare_generic_binders,
};

#[derive(Debug)]
pub enum DeclarationLoweringError {
    Topology(TopologyDiagnostic),
    Surface(SurfaceDiagnostic),
    InternalSurface(SurfaceError),
    CallableContract(CallableContractDiagnostic),
    InternalContract(CallableContractError),
    Reservation(ReservationError),
    Namespace(NamespaceDiagnostic),
    InternalHeader(HeaderError),
    Generic(GenericDiagnostic),
    InternalGeneric(GenericError),
    Import(ImportDiagnostic),
    InternalImport(ImportError),
    Prelude(PreludeError),
    TypeBinding(TypeBindingDiagnostic),
    InternalTypeBinding(TypeBindingError),
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
            Self::Topology(diagnostic) => Some(diagnostic.source()),
            Self::Surface(diagnostic) => Some(diagnostic.source()),
            Self::CallableContract(diagnostic) => Some(diagnostic.source()),
            Self::Namespace(diagnostic) => Some(diagnostic.source()),
            Self::Generic(diagnostic) => Some(diagnostic.source()),
            Self::Import(diagnostic) => Some(diagnostic.source()),
            Self::TypeBinding(diagnostic) => Some(diagnostic.source()),
            Self::Definition(error) => error.source_diagnostic(),
            Self::InternalSurface(_)
            | Self::InternalContract(_)
            | Self::Reservation(_)
            | Self::InternalHeader(_)
            | Self::InternalGeneric(_)
            | Self::InternalImport(_)
            | Self::Prelude(_)
            | Self::InternalTypeBinding(_)
            | Self::TypeNormalization(_) => None,
        }
    }
}

impl fmt::Display for DeclarationLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topology(error) => error.fmt(formatter),
            Self::Surface(error) => error.fmt(formatter),
            Self::InternalSurface(error) => error.fmt(formatter),
            Self::CallableContract(error) => error.fmt(formatter),
            Self::InternalContract(error) => error.fmt(formatter),
            Self::Reservation(error) => error.fmt(formatter),
            Self::Namespace(error) => error.fmt(formatter),
            Self::InternalHeader(error) => error.fmt(formatter),
            Self::Generic(error) => error.fmt(formatter),
            Self::InternalGeneric(error) => error.fmt(formatter),
            Self::Import(error) => error.fmt(formatter),
            Self::InternalImport(error) => error.fmt(formatter),
            Self::Prelude(error) => error.fmt(formatter),
            Self::TypeBinding(error) => error.fmt(formatter),
            Self::InternalTypeBinding(error) => error.fmt(formatter),
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
        Err(SurfaceError::Topology(crate::LoweringError::Rule(violation))) => {
            return match TopologyDiagnostic::project(&violation, input) {
                Some(diagnostic) => Err(DeclarationLoweringError::Topology(diagnostic)),
                None => Err(DeclarationLoweringError::InternalSurface(
                    SurfaceError::Topology(crate::LoweringError::Rule(violation)),
                )),
            };
        }
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
    let generics = match prepare_generic_binders(headers) {
        Ok(generics) => generics,
        Err(GenericError::Rule(violation)) => {
            return match GenericDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Generic(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalGeneric(
                    GenericError::Rule(internal),
                )),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalGeneric(internal)),
    };
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
    let bound = match bind_header_type_syntax(namespaces) {
        Ok(bound) => bound,
        Err(TypeBindingError::Rule(violation)) => {
            return match TypeBindingDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::TypeBinding(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalTypeBinding(
                    TypeBindingError::Rule(internal),
                )),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalTypeBinding(internal)),
    };
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
        CallableContractRule, CompileUnitInput, DeclarationLoweringError, GenericRule, ImportRule,
        ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind, NamespaceRule,
        PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, TopologyRule,
        TypeBindingRule, lower_compile_unit_declarations,
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
    fn production_pipeline_projects_a_deterministic_module_cycle() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "use ./a\n");
        let a_id = add_source(&mut sources, "/app/a/index.nct", "use /b\n");
        let b_id = add_source(&mut sources, "/app/b/index.nct", "use /a\n");
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let a = parse_source(&sources, a_id, ParseGoal::ModuleSource);
        let b = parse_source(&sources, b_id, ParseGoal::ModuleSource);
        let package_identity = PackageIdentity::new("workspace:app");
        let a_identity = ModuleIdentity::new(package_identity.clone(), ["a"]);
        let b_identity = ModuleIdentity::new(package_identity, ["b"]);
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
                module("workspace:app", &["a"], "/app/a/index.nct", &a),
                module("workspace:app", &["b"], "/app/b/index.nct", &b),
            ],
            vec![
                module_use(&root, 0, a_identity.clone()),
                module_use(&a, 0, b_identity),
                module_use(&b, 0, a_identity),
            ],
        );

        let error = lower_compile_unit_declarations(
            &input,
            &ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]),
        )
        .unwrap_err();
        let DeclarationLoweringError::Topology(diagnostic) = error else {
            panic!("module cycle did not produce a topology diagnostic");
        };

        assert_eq!(diagnostic.rule(), TopologyRule::ModuleImportCycle);
        assert_eq!(diagnostic.source().code(), "E0271");
        assert_eq!(diagnostic.source().primary().source(), a_id);
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(diagnostic.source().notes()[0].origin().source(), b_id);
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
    fn production_pipeline_projects_duplicate_generic_binder_tokens() {
        let text = "pub struct Broken<T, T> {}\n";
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
        let DeclarationLoweringError::Generic(diagnostic) = error else {
            panic!("duplicate binder did not produce a generic diagnostic");
        };

        assert_eq!(diagnostic.rule(), GenericRule::DuplicateBinder);
        assert_eq!(diagnostic.source().code(), "E0281");
        assert_eq!(diagnostic.source().primary().source(), root_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.rfind('T').unwrap()).unwrap()
        );
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0]
                .origin()
                .span()
                .range()
                .start()
                .get(),
            u32::try_from(text.find('T').unwrap()).unwrap()
        );
    }

    #[test]
    fn production_pipeline_distinguishes_nested_generic_shadowing() {
        let text = concat!(
            "pub struct Pair<T> {}\n",
            "instance Pair<T> {\n",
            "    pub method &self.identity<T>(value: T): T { value }\n",
            "}\n",
        );
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
        let DeclarationLoweringError::Generic(diagnostic) = error else {
            panic!("nested shadowing did not produce a generic diagnostic");
        };

        assert_eq!(diagnostic.rule(), GenericRule::ShadowingBinder);
        assert_eq!(diagnostic.source().code(), "E0282");
        assert_eq!(diagnostic.source().primary().source(), root_id);
        assert_eq!(diagnostic.source().notes().len(), 1);
        let method_binder = text.find("identity<T>").unwrap() + "identity<".len();
        let inherited_binder = text.find("instance Pair<T>").unwrap() + "instance Pair<".len();
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(method_binder).unwrap()
        );
        assert_eq!(
            diagnostic.source().notes()[0]
                .origin()
                .span()
                .range()
                .start()
                .get(),
            u32::try_from(inherited_binder).unwrap()
        );
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

    #[test]
    fn production_pipeline_projects_the_exact_unknown_type_name() {
        let text = "pub func run(value: Missing): void {}\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
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
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input, &prelude_identity).unwrap_err();
        let DeclarationLoweringError::TypeBinding(diagnostic) = error else {
            panic!("unknown type did not produce a type-binding diagnostic");
        };

        assert_eq!(diagnostic.rule(), TypeBindingRule::UnknownTypeContextName);
        assert_eq!(diagnostic.source().code(), "E0290");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.find("Missing").unwrap()).unwrap()
        );
        assert!(diagnostic.source().notes().is_empty());
    }

    #[test]
    fn production_pipeline_projects_both_duplicate_callable_parameter_names() {
        let text = "pub func install(callback: &func(value: usize, value: usize): void): void {}\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
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
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input, &prelude_identity).unwrap_err();
        let DeclarationLoweringError::TypeBinding(diagnostic) = error else {
            panic!("duplicate callable parameter did not produce a type-binding diagnostic");
        };

        assert_eq!(
            diagnostic.rule(),
            TypeBindingRule::DuplicateCallableParameter
        );
        assert_eq!(diagnostic.source().code(), "E0295");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.rfind("value").unwrap()).unwrap()
        );
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0]
                .origin()
                .span()
                .range()
                .start()
                .get(),
            u32::try_from(text.find("value").unwrap()).unwrap()
        );
    }

    #[test]
    fn production_pipeline_projects_exact_callable_provenance_origins() {
        let cases = [
            (
                "pub func install(callback: &func(input: &str): &str from missing): void {}\n",
                TypeBindingRule::UnknownProvenanceOrigin,
                "missing",
                None,
            ),
            (
                "pub func install(callback: &func(input: &str): &str from input | input): void {}\n",
                TypeBindingRule::DuplicateProvenanceOrigin,
                "input",
                Some("input"),
            ),
        ];
        for (text, expected_rule, primary_name, related_name) in cases {
            let mut sources = SourceMap::new();
            let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
            let standard_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
            let app_id = add_source(&mut sources, "/app/index.nct", text);
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
            let input = input_with_standard_prelude(
                &sources,
                &app_manifest,
                &app,
                &standard_manifest,
                &standard,
                &prelude,
            );

            let error = lower_compile_unit_declarations(&input, &prelude_identity).unwrap_err();
            let DeclarationLoweringError::TypeBinding(diagnostic) = error else {
                panic!("invalid provenance did not produce a type-binding diagnostic");
            };

            assert_eq!(diagnostic.rule(), expected_rule);
            assert_eq!(diagnostic.source().primary().source(), app_id);
            assert_eq!(
                diagnostic.source().primary().span().range().start().get(),
                u32::try_from(text.rfind(primary_name).unwrap()).unwrap()
            );
            if let Some(related_name) = related_name {
                assert_eq!(diagnostic.source().notes().len(), 1);
                let clause = text.find(" from ").unwrap();
                assert_eq!(
                    diagnostic.source().notes()[0]
                        .origin()
                        .span()
                        .range()
                        .start()
                        .get(),
                    u32::try_from(text[clause..].find(related_name).unwrap() + clause).unwrap()
                );
            } else {
                assert!(diagnostic.source().notes().is_empty());
            }
        }
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

    fn input_with_standard_prelude<'syntax>(
        sources: &'syntax SourceMap,
        app_manifest: &'syntax SyntaxTree,
        app: &'syntax SyntaxTree,
        standard_manifest: &'syntax SyntaxTree,
        standard: &'syntax SyntaxTree,
        prelude: &'syntax SyntaxTree,
    ) -> CompileUnitInput<'syntax> {
        CompileUnitInput::new(
            sources,
            vec![
                package("workspace:app", "app", "/app/nocter.nct", app_manifest),
                package("toolchain:std", "std", "/std/nocter.nct", standard_manifest),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", app),
                module("toolchain:std", &[], "/std/index.nct", standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    prelude,
                ),
            ],
            Vec::new(),
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
