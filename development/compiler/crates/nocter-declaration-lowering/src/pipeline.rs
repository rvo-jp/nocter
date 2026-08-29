use std::fmt;

use crate::definitions::{HeaderDefinitionError, define_declaration_headers_recovering};
use crate::surface::collect_incomplete_body_declaration_surface;
use crate::toolchain::resolve_toolchain_surface;
use crate::{
    CompileUnitInput, DeclarationContractDiagnostic, DeclarationContractError,
    DeclarationDiagnostics, DeclarationLoweringRecovery, DefinitionDiagnostic, GenericDiagnostic,
    GenericError, HeaderError, ImportDiagnostic, ImportError, LoweredDeclarations,
    NamespaceDiagnostic, PreparedImports, PreparedNamespaces, PreparedTypeBindings, PreparedTypes,
    ReservationError, SourceDiagnostic, SurfaceDiagnostic, SurfaceError, ToolchainError,
    TopologyDiagnostic, TypeBindingDiagnostic, TypeBindingError, TypeNormalizationDiagnostic,
    TypeNormalizationError, analyze_declaration_contracts, apply_toolchain_profile,
    bind_header_type_syntax, collect_declaration_surface, evaluate_header_constants,
    normalize_header_types, prepare_authored_imports, prepare_declaration_headers,
    prepare_generic_binders,
};

#[derive(Clone, Debug)]
pub enum DeclarationLoweringError {
    Topology(TopologyDiagnostic),
    Surface(SurfaceDiagnostic),
    InternalSurface(SurfaceError),
    DeclarationContract(DeclarationContractDiagnostic),
    InternalContract(DeclarationContractError),
    Reservation(ReservationError),
    Namespace(NamespaceDiagnostic),
    InternalHeader(HeaderError),
    Generic(GenericDiagnostic),
    InternalGeneric(GenericError),
    Import(ImportDiagnostic),
    InternalImport(ImportError),
    Toolchain(ToolchainError),
    TypeBinding(TypeBindingDiagnostic),
    InternalTypeBinding(TypeBindingError),
    TypeNormalization(TypeNormalizationDiagnostic),
    InternalTypeNormalization(TypeNormalizationError),
    Definition(DefinitionDiagnostic),
    Declaration(DeclarationDiagnostics),
    InternalDefinition(HeaderDefinitionError),
}

impl DeclarationLoweringError {
    /// Returns the complete common public diagnostic set selected by the rejecting phase.
    ///
    /// An empty slice identifies a stage error that has not crossed a public diagnostic boundary or
    /// an internal compiler inconsistency. Consumers must not manufacture a public code for it.
    #[must_use]
    pub fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        match self {
            Self::Topology(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Surface(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::DeclarationContract(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Namespace(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Generic(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Import(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::TypeBinding(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::TypeNormalization(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Definition(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Declaration(diagnostics) => diagnostics.sources(),
            Self::InternalSurface(_)
            | Self::InternalContract(_)
            | Self::Reservation(_)
            | Self::InternalHeader(_)
            | Self::InternalGeneric(_)
            | Self::InternalImport(_)
            | Self::Toolchain(_)
            | Self::InternalTypeBinding(_)
            | Self::InternalTypeNormalization(_)
            | Self::InternalDefinition(_) => &[],
        }
    }
}

impl fmt::Display for DeclarationLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topology(error) => error.fmt(formatter),
            Self::Surface(error) => error.fmt(formatter),
            Self::InternalSurface(error) => error.fmt(formatter),
            Self::DeclarationContract(error) => error.fmt(formatter),
            Self::InternalContract(error) => error.fmt(formatter),
            Self::Reservation(error) => error.fmt(formatter),
            Self::Namespace(error) => error.fmt(formatter),
            Self::InternalHeader(error) => error.fmt(formatter),
            Self::Generic(error) => error.fmt(formatter),
            Self::InternalGeneric(error) => error.fmt(formatter),
            Self::Import(error) => error.fmt(formatter),
            Self::InternalImport(error) => error.fmt(formatter),
            Self::Toolchain(error) => error.fmt(formatter),
            Self::TypeBinding(error) => error.fmt(formatter),
            Self::InternalTypeBinding(error) => error.fmt(formatter),
            Self::TypeNormalization(error) => error.fmt(formatter),
            Self::InternalTypeNormalization(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
            Self::Declaration(error) => error.fmt(formatter),
            Self::InternalDefinition(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DeclarationLoweringError {}

#[derive(Clone, Debug)]
pub struct DeclarationLoweringFailure {
    error: Box<DeclarationLoweringError>,
    recovery: Option<Box<DeclarationLoweringRecovery>>,
}

impl DeclarationLoweringFailure {
    fn new(error: DeclarationLoweringError, recovery: Option<DeclarationLoweringRecovery>) -> Self {
        Self {
            error: Box::new(error),
            recovery: recovery.map(Box::new),
        }
    }

    fn without_recovery(error: DeclarationLoweringError) -> Self {
        Self::new(error, None)
    }

    #[must_use]
    pub fn current_branch(&self) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DeclarationLoweringError,
        Option<DeclarationLoweringRecovery>,
    ) {
        (*self.error, self.recovery.map(|recovery| *recovery))
    }

    #[must_use]
    pub fn into_error(self) -> DeclarationLoweringError {
        *self.error
    }
}

/// Lowers one discovery-owned compile unit through the complete declaration pipeline.
///
/// This is the production entry point. Individual passes remain exposed for focused tests and
/// compiler development, but consumers do not select or reorder them.
///
/// # Errors
///
/// Returns the exact failing stage. Source-backed module-surface, declaration-contract, namespace,
/// and freeze-time declaration rules are already projected to common diagnostics;
/// remaining stage errors stay typed until their diagnostic mappings are completed.
#[allow(clippy::disallowed_methods)]
pub fn lower_compile_unit_declarations(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringError> {
    lower_compile_unit_declarations_recovering(input)
        .map_err(DeclarationLoweringFailure::into_error)
}

/// Lowers declarations while retaining the immutable declaration snapshot reached before an
/// authored declaration rule rejected the program.
///
/// # Errors
///
/// Returns the exact production lowering error and optional editor recovery. Earlier-stage and
/// internal-integrity failures never expose a recovery program.
pub fn lower_compile_unit_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    lower_complete_declarations_recovering(input)
}

fn lower_complete_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    let normalized =
        prepare_compile_unit_declarations_from(input, collect_declaration_surface(input))
            .map_err(DeclarationLoweringFailure::without_recovery)?;
    finish_declarations_recovering(input, normalized)
}

/// Computes only the source-neutral accepted declaration product for a semantic query.
///
/// Current frontend bindings and source projection are deliberately discarded at this boundary;
/// the query consumer must materialize them from the retained recipe against its current input.
///
/// # Errors
///
/// Returns the same authored or integrity failure as complete declaration lowering.
pub fn lower_reusable_declarations(
    input: &CompileUnitInput<'_>,
) -> Result<crate::ReusableDeclarations, DeclarationLoweringFailure> {
    lower_complete_declarations_recovering(input).map(LoweredDeclarations::into_reusable)
}

/// Lowers declarations from an incomplete-body source while retaining declaration-only facts
/// rejected by an independent authored declaration rule.
///
/// # Errors
///
/// Returns the ordinary declaration failure. A recovery snapshot is present only when the
/// declaration graph and its source projection are both internally consistent.
pub fn lower_incomplete_body_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    let normalized = prepare_compile_unit_declarations_from(
        input,
        collect_incomplete_body_declaration_surface(input),
    )
    .map_err(DeclarationLoweringFailure::without_recovery)?;
    finish_declarations_recovering(input, normalized)
}

fn finish_declarations_recovering(
    input: &CompileUnitInput<'_>,
    normalized: PreparedTypes<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    match define_declaration_headers_recovering(normalized) {
        Ok(lowered) => Ok(lowered),
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            Err(DeclarationLoweringFailure::new(
                project_definition_error(error, input),
                recovery,
            ))
        }
    }
}

fn prepare_compile_unit_declarations_from<'syntax>(
    input: &CompileUnitInput<'syntax>,
    surface: Result<crate::DeclarationSurface<'syntax>, SurfaceError>,
) -> Result<PreparedTypes<'syntax>, DeclarationLoweringError> {
    let surface = match surface {
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
    let toolchain_input = input
        .toolchain()
        .ok_or(DeclarationLoweringError::Toolchain(
            ToolchainError::MissingProfile,
        ))?;
    let toolchain = resolve_toolchain_surface(&surface, toolchain_input)
        .map_err(DeclarationLoweringError::Toolchain)?;
    if let Err(error) =
        crate::surface::validate_builtin_type_authority(&surface, toolchain.builtin_types())
    {
        return match SurfaceDiagnostic::project(error, input) {
            Ok(diagnostic) => Err(DeclarationLoweringError::Surface(diagnostic)),
            Err(internal) => Err(DeclarationLoweringError::InternalSurface(internal)),
        };
    }
    let contracts = match analyze_declaration_contracts(&surface) {
        Ok(contracts) => contracts,
        Err(error) => {
            return match DeclarationContractDiagnostic::project(error, &surface) {
                Ok(diagnostic) => Err(DeclarationLoweringError::DeclarationContract(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalContract(internal)),
            };
        }
    };
    let reserved = crate::reservation::reserve_with_contracts(surface, contracts, toolchain)
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
    let namespaces = prepare_toolchain_namespaces(imports, input)?;
    let bound = bind_types(namespaces, input)?;
    let bound = evaluate_constants(bound, input)?;
    normalize_types(bound, input)
}

fn evaluate_constants<'syntax>(
    bound: PreparedTypeBindings<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypeBindings<'syntax>, DeclarationLoweringError> {
    match evaluate_header_constants(bound) {
        Ok(bound) => Ok(bound),
        Err(HeaderDefinitionError::Rule(violation)) => {
            match DefinitionDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Definition(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalDefinition(
                    HeaderDefinitionError::Rule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalDefinition(internal)),
    }
}

fn prepare_toolchain_namespaces<'syntax>(
    imports: PreparedImports<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedNamespaces<'syntax>, DeclarationLoweringError> {
    match apply_toolchain_profile(imports) {
        Ok(namespaces) => Ok(namespaces),
        Err(ToolchainError::Rule(violation)) => match ImportDiagnostic::project(violation, input) {
            Ok(diagnostic) => Err(DeclarationLoweringError::Import(diagnostic)),
            Err(internal) => Err(DeclarationLoweringError::Toolchain(ToolchainError::Rule(
                internal,
            ))),
        },
        Err(internal) => Err(DeclarationLoweringError::Toolchain(internal)),
    }
}

fn bind_types<'syntax>(
    namespaces: PreparedNamespaces<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypeBindings<'syntax>, DeclarationLoweringError> {
    match bind_header_type_syntax(namespaces) {
        Ok(bound) => Ok(bound),
        Err(TypeBindingError::Rule(violation)) => {
            match TypeBindingDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::TypeBinding(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalTypeBinding(
                    TypeBindingError::Rule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalTypeBinding(internal)),
    }
}

fn project_definition_error(
    error: HeaderDefinitionError,
    input: &CompileUnitInput<'_>,
) -> DeclarationLoweringError {
    match error {
        HeaderDefinitionError::Rule(violation) => {
            match DefinitionDiagnostic::project(violation, input) {
                Ok(diagnostic) => DeclarationLoweringError::Definition(diagnostic),
                Err(internal) => DeclarationLoweringError::InternalDefinition(
                    HeaderDefinitionError::Rule(internal),
                ),
            }
        }
        HeaderDefinitionError::Declaration(diagnostic) => {
            DeclarationLoweringError::Declaration(diagnostic)
        }
        internal => DeclarationLoweringError::InternalDefinition(internal),
    }
}

fn normalize_types<'syntax>(
    bound: PreparedTypeBindings<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypes<'syntax>, DeclarationLoweringError> {
    match normalize_header_types(bound) {
        Ok(normalized) => Ok(normalized),
        Err(TypeNormalizationError::Rule(violation)) => {
            match TypeNormalizationDiagnostic::project(&violation, input) {
                Some(diagnostic) => Err(DeclarationLoweringError::TypeNormalization(diagnostic)),
                None => Err(DeclarationLoweringError::InternalTypeNormalization(
                    TypeNormalizationError::Rule(violation),
                )),
            }
        }
        Err(TypeNormalizationError::RequirementRule(violation)) => {
            match TypeBindingDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::TypeBinding(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalTypeNormalization(
                    TypeNormalizationError::RequirementRule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalTypeNormalization(
            internal,
        )),
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::PackageTargetKind;
    use nocter_source::{SourceMap, SourceName};
    use nocter_source_index::SemanticEntity;
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    use crate::test_support::{module_use, package_target, source_see};
    use crate::{
        CompileUnitInput, DeclarationContractRule, DeclarationLoweringError, DefinitionRule,
        GenericRule, ImportRule, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        NamespaceRule, PackageIdentity, PackageInput, PackageMode, ToolchainInput, TopologyRule,
        TypeBindingRule, TypeNormalizationRule, lower_compile_unit_declarations,
    };

    #[test]
    fn package_targets_retain_resolved_modules_source_order_and_exact_name_origins() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(
            &mut sources,
            "/app/index.nct",
            "#package: { name: \"app\", version: \"0.0.0\", }\n\
             #executable: { name: \"app\", }\n\
             #test: { name: \"unit\", module: \"./tests/unit\", }\n\
             #executable: { name: \"tool\", module: \"./tools/tool\", }\n",
        );
        let test_id = add_source(&mut sources, "/app/tests/unit/index.nct", "");
        let tool_id = add_source(&mut sources, "/app/tools/tool/index.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let tests = parse_source(&sources, test_id, ParseGoal::SourceFile);
        let tool = parse_source(&sources, tool_id, ParseGoal::SourceFile);
        let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, std_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let app_package = PackageIdentity::new("workspace:app");
        let app_root = ModuleIdentity::new(app_package.clone(), Vec::<&str>::new());
        let test_module = ModuleIdentity::new(app_package.clone(), ["tests", "unit"]);
        let tool_module = ModuleIdentity::new(app_package.clone(), ["tools", "tool"]);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package("toolchain:std", "std", "/std/index.nct", &std_manifest),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app_manifest),
                module(
                    "workspace:app",
                    &["tests", "unit"],
                    "/app/tests/unit/index.nct",
                    &tests,
                ),
                module(
                    "workspace:app",
                    &["tools", "tool"],
                    "/app/tools/tool/index.nct",
                    &tool,
                ),
                module("toolchain:std", &[], "/std/index.nct", &standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &prelude,
                ),
            ],
            Vec::new(),
        )
        .with_package_target_resolutions(vec![
            package_target(&sources, &app_manifest, 2, tool_module),
            package_target(&sources, &app_manifest, 0, app_root),
            package_target(&sources, &app_manifest, 1, test_module),
        ])
        .with_toolchain(standard_toolchain(&standard));

        let lowered = lower_compile_unit_declarations(&input).unwrap();
        assert_package_targets(&sources, &lowered);
    }

    #[test]
    fn single_file_mode_creates_one_semantic_executable_target() {
        let mut sources = SourceMap::new();
        let app_id = add_source(
            &mut sources,
            "/tmp/example.nct",
            "func main(): void { return }\n",
        );
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, std_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let app_package = PackageIdentity::new("single:/tmp/example.nct");
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                PackageInput::new(app_package.clone(), "example", PackageMode::SingleFile),
                package("toolchain:std", "std", "/std/index.nct", &std_manifest),
            ],
            vec![
                ModuleInput::new(
                    ModuleIdentity::new(app_package, Vec::<&str>::new()),
                    vec![ModuleSourceInput::new(
                        "/tmp/example.nct",
                        ModuleSourceKind::SingleFile,
                        &app,
                    )],
                ),
                module("toolchain:std", &[], "/std/index.nct", &standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &prelude,
                ),
            ],
            Vec::new(),
        )
        .with_toolchain(standard_toolchain(&standard));

        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let mut targets = lowered.program().package_targets().iter();
        let (target_id, target) = targets
            .next()
            .expect("single-file lowering did not create a target");
        assert!(targets.next().is_none());
        assert_eq!(target.kind(), PackageTargetKind::Executable);
        assert_eq!(target.declaration_order(), 0);
        assert_eq!(
            lowered.program().symbols().spelling(target.name()),
            Some("example")
        );
        let [binding] = lowered
            .source_index()
            .bindings_for(SemanticEntity::PackageTarget(target_id))
        else {
            panic!("single-file target did not retain one source projection")
        };
        assert_eq!(binding.origin().source(), app_id);
        assert_eq!(binding.origin().span().range(), app.root().range());
    }

    fn assert_package_targets(sources: &SourceMap, lowered: &crate::LoweredDeclarations) {
        let targets = lowered
            .program()
            .package_targets()
            .iter()
            .map(|(id, target)| {
                (
                    id,
                    lowered.program().symbols().spelling(target.name()).unwrap(),
                    target.kind(),
                    target.declaration_order(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            targets
                .iter()
                .map(|(_, name, kind, order)| (*name, *kind, *order))
                .collect::<Vec<_>>(),
            [
                ("app", PackageTargetKind::Executable, 0),
                ("unit", PackageTargetKind::Test, 1),
                ("tool", PackageTargetKind::Executable, 2),
            ]
        );
        for (id, name, _, _) in targets {
            let bindings = lowered
                .source_index()
                .bindings_for(SemanticEntity::PackageTarget(id));
            assert_eq!(bindings.len(), 1);
            let origin = bindings[0].origin();
            let text = sources
                .get(origin.source())
                .and_then(|source| source.text_at(origin.span().range()))
                .unwrap();
            assert_eq!(text, format!("\"{name}\""));
        }
    }

    #[test]
    fn production_pipeline_projects_surface_diagnostics() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "see ./private.nct\n");
        let implementation_id = add_source(
            &mut sources,
            "/app/private.nct",
            "pub func exposed(): void {}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
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
            Vec::new(),
        )
        .with_source_visibility_resolutions(vec![source_see(&root, 0, "/app/private.nct")]);

        let error = lower_compile_unit_declarations(&input).unwrap_err();

        assert_eq!(
            error
                .source_diagnostics()
                .first()
                .map(crate::SourceDiagnostic::code),
            Some("E0230")
        );
        assert!(matches!(error, DeclarationLoweringError::Surface(_)));
    }

    #[test]
    fn production_pipeline_projects_a_deterministic_module_cycle() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "use ./a\n");
        let a_id = add_source(&mut sources, "/app/a/index.nct", "use /b\n");
        let b_id = add_source(&mut sources, "/app/b/index.nct", "use /a\n");
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let a = parse_source(&sources, a_id, ParseGoal::SourceFile);
        let b = parse_source(&sources, b_id, ParseGoal::SourceFile);
        let package_identity = PackageIdentity::new("workspace:app");
        let a_identity = ModuleIdentity::new(package_identity.clone(), ["a"]);
        let b_identity = ModuleIdentity::new(package_identity, ["b"]);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
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

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "struct usize {}\n");
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", "");
        let child_id = add_source(
            &mut sources,
            "/app/parser/index.nct",
            "pub(../../) func exposed(): void {}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let child = parse_source(&sources, child_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
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
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let root_id = add_source(&mut sources, "/app/index.nct", text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::SourceFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package("workspace:app", "app", "/app/index.nct", &manifest)],
            vec![module("workspace:app", &[], "/app/index.nct", &root)],
            Vec::new(),
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Hidden\n");
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "struct Hidden {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::SourceFile);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/index.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "use dep.Missing\n");
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "pub struct Present {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::SourceFile);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/index.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
    fn production_pipeline_projects_the_explicit_prelude_path() {
        let text = "use std/prelude.String\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(
            &mut sources,
            "/std/prelude/index.nct",
            "pub struct String {}\n",
        );
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let managed_prelude =
            ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package("toolchain:std", "std", "/std/index.nct", &standard_manifest),
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
            vec![module_use(&app, 0, managed_prelude.clone())],
        )
        .with_toolchain(standard_toolchain(&standard));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
        let DeclarationLoweringError::Import(diagnostic) = error else {
            panic!("explicit prelude import did not produce an import diagnostic");
        };

        assert_eq!(diagnostic.rule(), ImportRule::CompilerManagedPreludeImport);
        assert_eq!(diagnostic.source().code(), "E0262");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().node().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.find("std/prelude").unwrap()).unwrap()
        );
        assert!(diagnostic.source().notes().is_empty());
    }

    #[test]
    fn import_collisions_reuse_the_namespace_diagnostic() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let dependency_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
        let app_id = add_source(
            &mut sources,
            "/app/index.nct",
            "use dep.Value as Item\n\nstruct Item {}\n",
        );
        let dependency_id = add_source(&mut sources, "/dep/index.nct", "pub struct Value {}\n");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let dependency_manifest =
            parse_source(&sources, dependency_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let dependency = parse_source(&sources, dependency_id, ParseGoal::SourceFile);
        let dependency_identity =
            ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package(
                    "resolved:dep",
                    "dep",
                    "/dep/index.nct",
                    &dependency_manifest,
                ),
            ],
            vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("resolved:dep", &[], "/dep/index.nct", &dependency),
            ],
            vec![module_use(&app, 0, dependency_identity)],
        )
        .with_toolchain(crate::test_support::empty_toolchain(ModuleIdentity::new(
            PackageIdentity::new("workspace:app"),
            Vec::<&str>::new(),
        )));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", "pub func run(): void\n");
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package("toolchain:std", "std", "/std/index.nct", &standard_manifest),
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
        )
        .with_toolchain(standard_toolchain(&standard));

        let error = lower_compile_unit_declarations(&input).unwrap_err();
        assert_eq!(
            error
                .source_diagnostics()
                .first()
                .map(crate::SourceDiagnostic::code),
            Some("E0250")
        );
        let DeclarationLoweringError::DeclarationContract(diagnostic) = error else {
            panic!("missing body did not produce a callable contract diagnostic");
        };
        assert_eq!(diagnostic.rule(), DeclarationContractRule::MissingBody);
        assert_eq!(diagnostic.source().code(), "E0250");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().node().is_some());
        assert_eq!(diagnostic.source().notes(), []);
    }

    #[test]
    fn production_pipeline_projects_the_exact_unknown_type_name() {
        let text = "pub func run(value: Missing): void {}\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input).unwrap_err();
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
            let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
            let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
            let app_id = add_source(&mut sources, "/app/index.nct", text);
            let standard_id = add_source(
                &mut sources,
                "/std/index.nct",
                crate::test_support::TEST_BUILTIN_SOURCE,
            );
            let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
            let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
            let standard_manifest =
                parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
            let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
            let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
            let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
            let input = input_with_standard_prelude(
                &sources,
                &app_manifest,
                &app,
                &standard_manifest,
                &standard,
                &prelude,
            );

            let error = lower_compile_unit_declarations(&input).unwrap_err();
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

    #[test]
    fn production_pipeline_projects_a_canonical_alias_cycle() {
        let text = "type A = B\ntype B = A\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input).unwrap_err();
        let DeclarationLoweringError::TypeNormalization(diagnostic) = error else {
            panic!("recursive aliases did not produce a normalization diagnostic");
        };

        assert_eq!(diagnostic.rule(), TypeNormalizationRule::RecursiveAlias);
        assert_eq!(diagnostic.source().code(), "E0310");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.find('A').unwrap()).unwrap()
        );
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0]
                .origin()
                .span()
                .range()
                .start()
                .get(),
            u32::try_from(text.rfind('B').unwrap()).unwrap()
        );
    }

    #[test]
    fn production_pipeline_projects_ambiguous_callable_provenance_at_the_callable() {
        let text = "type Callback = &func(left: &str, right: &str): &str\n";
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input).unwrap_err();
        let DeclarationLoweringError::TypeNormalization(diagnostic) = error else {
            panic!("ambiguous provenance did not produce a normalization diagnostic");
        };

        assert_eq!(
            diagnostic.rule(),
            TypeNormalizationRule::AmbiguousCallableProvenance
        );
        assert_eq!(diagnostic.source().code(), "E0313");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().node().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.find("&func").unwrap()).unwrap()
        );
        assert!(diagnostic.source().notes().is_empty());
    }

    #[test]
    fn production_pipeline_projects_the_exact_unknown_associated_name() {
        let text = concat!(
            "interface Source { pub type Item }\n",
            "func read<T>(value: &T): T.Missing where T impl Source { return value }\n",
        );
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", text);
        let standard_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let standard_manifest = parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let input = input_with_standard_prelude(
            &sources,
            &app_manifest,
            &app,
            &standard_manifest,
            &standard,
            &prelude,
        );

        let error = lower_compile_unit_declarations(&input).unwrap_err();
        let DeclarationLoweringError::TypeNormalization(diagnostic) = error else {
            panic!("unknown associated type did not produce a normalization diagnostic");
        };

        assert_eq!(
            diagnostic.rule(),
            TypeNormalizationRule::UnknownAssociatedType
        );
        assert_eq!(diagnostic.source().code(), "E0311");
        assert_eq!(diagnostic.source().primary().source(), app_id);
        assert!(diagnostic.source().primary().token().is_some());
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            u32::try_from(text.find("Missing").unwrap()).unwrap()
        );
        assert!(diagnostic.source().notes().is_empty());
    }

    #[test]
    fn production_pipeline_projects_declaration_definition_rules() {
        let cases = [
            (
                "func choose<T>(left: &T, right: &T): &T from missing { return }\n",
                DefinitionRule::UnknownResultProvenanceOrigin,
                "missing",
                None,
            ),
            (
                "func choose<T>(left: &T, right: &T): &T from left | left { return }\n",
                DefinitionRule::DuplicateResultProvenanceOrigin,
                "left {",
                Some("left |"),
            ),
            (
                "interface Choose {\n    pub method &self.choose(other: &Self): &Self\n}\n",
                DefinitionRule::AmbiguousBodylessResultProvenance,
                "&Self\n",
                None,
            ),
            (
                "interface Source {\n    pub type Item\n}\nstruct Value {}\ninstance Value { impl Source { .Missing = i32 } }\n",
                DefinitionRule::UnknownAssociatedTypeBinding,
                "Missing",
                None,
            ),
            (
                "interface Source {\n    pub type Item\n}\nstruct Value {}\ninstance Value { impl Source { .Item = i32, .Item = i64 } }\n",
                DefinitionRule::DuplicateAssociatedTypeBinding,
                "Item = i64",
                Some("Item = i32"),
            ),
        ];

        for (text, expected_rule, primary_text, related_text) in cases {
            let mut sources = SourceMap::new();
            let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
            let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
            let app_id = add_source(&mut sources, "/app/index.nct", text);
            let standard_id = add_source(
                &mut sources,
                "/std/index.nct",
                crate::test_support::TEST_BUILTIN_SOURCE,
            );
            let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
            let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
            let standard_manifest =
                parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
            let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
            let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
            let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
            let mut projected = Vec::new();
            for reverse in [false, true] {
                let input = input_with_standard_prelude_ordered(
                    &sources,
                    &app_manifest,
                    &app,
                    &standard_manifest,
                    &standard,
                    &prelude,
                    reverse,
                );
                let error = lower_compile_unit_declarations(&input).unwrap_err();
                let DeclarationLoweringError::Definition(diagnostic) = error else {
                    panic!("authored definition failure did not cross the production boundary")
                };
                projected.push(diagnostic);
            }
            assert_eq!(projected[0], projected[1]);
            let diagnostic = &projected[0];
            assert_eq!(
                diagnostic.source().code(),
                expected_rule.code(),
                "production diagnostic code changed"
            );
            assert_eq!(diagnostic.rule(), expected_rule);
            assert_eq!(diagnostic.source().primary().source(), app_id);
            assert_eq!(
                diagnostic.source().primary().span().range().start().get(),
                u32::try_from(text.rfind(primary_text).unwrap()).unwrap()
            );
            match related_text {
                Some(related_text) => {
                    assert_eq!(diagnostic.source().notes().len(), 1);
                    assert_eq!(
                        diagnostic.source().notes()[0]
                            .origin()
                            .span()
                            .range()
                            .start()
                            .get(),
                        u32::try_from(text.find(related_text).unwrap()).unwrap()
                    );
                }
                None => assert!(diagnostic.source().notes().is_empty()),
            }
        }
    }

    #[test]
    fn declaration_owned_g001_g020_boundaries_are_order_invariant() {
        let cases = [
            (
                "G006",
                "func missing_body(): i32\n",
                "declaration-contract",
                "E0253",
            ),
            ("G007", "enum Empty {}\n", "declaration", "E0200"),
            (
                "G008",
                "interface Source {\n    pub type Item\n    pub type Item\n}\n",
                "namespace",
                "E0241",
            ),
            ("G010", "instance str {}\n", "declaration", "E0201"),
            ("G012", "drop str(&+self) {}\n", "declaration", "E0204"),
            (
                "G013",
                "interface Source {\n    pub type Item\n}\nfunc read<T>(value: T): T.Item<i32> where T impl Source { return value }\n",
                "type-binding",
                "E0292",
            ),
            (
                "G015",
                "type Callback = &func(input: &str): &str from missing\n",
                "type-binding",
                "E0296",
            ),
            (
                "G016",
                "interface Show {\n    pub method &self.show(): i32\n}\ninterface Factory {\n    pub method &self.make(): some Show\n}\n",
                "declaration",
                "E0212",
            ),
            ("G017", "struct Pair<T, T> {}\n", "generic", "E0281"),
            (
                "G018",
                "func equality<T, U>(): T where T = U { return }\n",
                "type-binding",
                "E0301",
            ),
            (
                "G019",
                "interface First where Self impl Second {}\ninterface Second where Self impl First {}\n",
                "declaration",
                "E0327",
            ),
            (
                "G020",
                "interface First { pub method &self.value(): i32 }\ninterface Second { pub method &self.value(): i32 }\ninterface Combined where Self impl First, Self impl Second {}\n",
                "declaration",
                "E0328",
            ),
        ];

        for (grammar_row, text, expected_family, expected_code) in cases {
            let mut sources = SourceMap::new();
            let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
            let standard_manifest_id = add_source(&mut sources, "/std/index.nct", "");
            let app_id = add_source(&mut sources, "/app/index.nct", text);
            let standard_id = add_source(
                &mut sources,
                "/std/index.nct",
                crate::test_support::TEST_BUILTIN_SOURCE,
            );
            let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
            let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
            let standard_manifest =
                parse_source(&sources, standard_manifest_id, ParseGoal::SourceFile);
            let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
            let standard = parse_source(&sources, standard_id, ParseGoal::SourceFile);
            let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
            let mut projected = Vec::new();
            for reverse in [false, true] {
                let input = input_with_standard_prelude_ordered(
                    &sources,
                    &app_manifest,
                    &app,
                    &standard_manifest,
                    &standard,
                    &prelude,
                    reverse,
                );
                let error = lower_compile_unit_declarations(&input).unwrap_err();
                projected.push(public_diagnostic(&error));
            }

            assert_eq!(projected[0], projected[1], "{grammar_row}");
            assert_eq!(projected[0].0, expected_family, "{grammar_row}");
            assert_eq!(projected[0].1.code(), expected_code, "{grammar_row}");
            assert_eq!(projected[0].1.primary().source(), app_id, "{grammar_row}");
        }
    }

    #[test]
    fn reusable_declarations_materialize_against_the_current_source_domain() {
        let mut original_sources = SourceMap::new();
        let original_id = add_source(
            &mut original_sources,
            "/tmp/example.nct",
            "/// Original.\nfunc answer(): i32 {\n    let original_local = 1\n    return original_local\n}\n",
        );
        let standard_id = add_source(
            &mut original_sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let prelude_id = add_source(&mut original_sources, "/std/prelude/index.nct", "");
        let original = parse_source(&original_sources, original_id, ParseGoal::SourceFile);
        let standard = parse_source(&original_sources, standard_id, ParseGoal::SourceFile);
        let prelude = parse_source(&original_sources, prelude_id, ParseGoal::SourceFile);
        let single = PackageIdentity::new("single:/tmp/example.nct");
        let original_input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &original_sources,
            vec![
                PackageInput::new(single.clone(), "example", PackageMode::SingleFile),
                PackageInput::new(
                    PackageIdentity::new("toolchain:std"),
                    "std",
                    PackageMode::Declared,
                ),
            ],
            vec![
                ModuleInput::new(
                    ModuleIdentity::new(single.clone(), Vec::<&str>::new()),
                    vec![ModuleSourceInput::new(
                        "/tmp/example.nct",
                        ModuleSourceKind::SingleFile,
                        &original,
                    )],
                ),
                module("toolchain:std", &[], "/std/index.nct", &standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &prelude,
                ),
            ],
            Vec::new(),
        )
        .with_toolchain(standard_toolchain(&standard));
        let reusable = lower_compile_unit_declarations(&original_input)
            .unwrap()
            .into_reusable();

        let mut current_sources = SourceMap::new();
        let current_id = add_source(
            &mut current_sources,
            "/tmp/example.nct",
            "/// Current documentation.\nfunc answer(): i32 {\n    let changed = 2\n    return changed\n}\n",
        );
        let current_standard_id = add_source(
            &mut current_sources,
            "/std/index.nct",
            crate::test_support::TEST_BUILTIN_SOURCE,
        );
        let current_prelude_id = add_source(&mut current_sources, "/std/prelude/index.nct", "");
        let current = parse_source(&current_sources, current_id, ParseGoal::SourceFile);
        let current_standard =
            parse_source(&current_sources, current_standard_id, ParseGoal::SourceFile);
        let current_prelude =
            parse_source(&current_sources, current_prelude_id, ParseGoal::SourceFile);
        let current_input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &current_sources,
            vec![
                PackageInput::new(single.clone(), "example", PackageMode::SingleFile),
                PackageInput::new(
                    PackageIdentity::new("toolchain:std"),
                    "std",
                    PackageMode::Declared,
                ),
            ],
            vec![
                ModuleInput::new(
                    ModuleIdentity::new(single, Vec::<&str>::new()),
                    vec![ModuleSourceInput::new(
                        "/tmp/example.nct",
                        ModuleSourceKind::SingleFile,
                        &current,
                    )],
                ),
                module("toolchain:std", &[], "/std/index.nct", &current_standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &current_prelude,
                ),
            ],
            Vec::new(),
        )
        .with_toolchain(standard_toolchain(&current_standard));

        reusable.materialize_projection(&current_input).unwrap();
        assert_current_body_symbol_domain(&reusable, &current_input);
    }

    fn assert_current_body_symbol_domain(
        reusable: &crate::ReusableDeclarations,
        current_input: &CompileUnitInput<'_>,
    ) {
        let answer = reusable.program().symbols().get("answer").unwrap();
        assert_eq!(reusable.program().symbols().get("original_local"), None);
        let checking = reusable.checking_branch_for(current_input).unwrap();
        assert_eq!(checking.symbols().get("answer"), Some(answer));
        assert!(checking.symbols().get("changed").is_some());
        assert_eq!(checking.symbols().get("original_local"), None);
    }

    fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
        sources
            .add_bytes(SourceName::new(name), text.as_bytes())
            .unwrap()
    }

    fn public_diagnostic(
        error: &DeclarationLoweringError,
    ) -> (&'static str, crate::SourceDiagnostic) {
        let family = match &error {
            DeclarationLoweringError::Topology(_) => "topology",
            DeclarationLoweringError::Surface(_) => "surface",
            DeclarationLoweringError::DeclarationContract(_) => "declaration-contract",
            DeclarationLoweringError::Namespace(_) => "namespace",
            DeclarationLoweringError::Generic(_) => "generic",
            DeclarationLoweringError::Import(_) => "import",
            DeclarationLoweringError::TypeBinding(_) => "type-binding",
            DeclarationLoweringError::TypeNormalization(_) => "type-normalization",
            DeclarationLoweringError::Definition(_) => "definition",
            DeclarationLoweringError::Declaration(_) => "declaration",
            DeclarationLoweringError::InternalSurface(_)
            | DeclarationLoweringError::InternalContract(_)
            | DeclarationLoweringError::Reservation(_)
            | DeclarationLoweringError::InternalHeader(_)
            | DeclarationLoweringError::InternalGeneric(_)
            | DeclarationLoweringError::InternalImport(_)
            | DeclarationLoweringError::Toolchain(_)
            | DeclarationLoweringError::InternalTypeBinding(_)
            | DeclarationLoweringError::InternalTypeNormalization(_)
            | DeclarationLoweringError::InternalDefinition(_) => {
                panic!("internal failure crossed an authored semantic-boundary fixture")
            }
        };
        let diagnostic = error
            .source_diagnostics()
            .first()
            .expect("projected authored error has no common diagnostic")
            .clone();
        (family, diagnostic)
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

    fn package(identity: &str, name: &str, _path: &str, _manifest: &SyntaxTree) -> PackageInput {
        PackageInput::new(PackageIdentity::new(identity), name, PackageMode::Declared)
    }

    fn input_with_standard_prelude<'syntax>(
        sources: &'syntax SourceMap,
        app_manifest: &'syntax SyntaxTree,
        app: &'syntax SyntaxTree,
        standard_manifest: &'syntax SyntaxTree,
        standard: &'syntax SyntaxTree,
        prelude: &'syntax SyntaxTree,
    ) -> CompileUnitInput<'syntax> {
        input_with_standard_prelude_ordered(
            sources,
            app_manifest,
            app,
            standard_manifest,
            standard,
            prelude,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn input_with_standard_prelude_ordered<'syntax>(
        sources: &'syntax SourceMap,
        app_manifest: &'syntax SyntaxTree,
        app: &'syntax SyntaxTree,
        standard_manifest: &'syntax SyntaxTree,
        standard: &'syntax SyntaxTree,
        prelude: &'syntax SyntaxTree,
        reverse: bool,
    ) -> CompileUnitInput<'syntax> {
        let mut packages = vec![
            package("workspace:app", "app", "/app/index.nct", app_manifest),
            package("toolchain:std", "std", "/std/index.nct", standard_manifest),
        ];
        let mut modules = vec![
            module("workspace:app", &[], "/app/index.nct", app),
            module("toolchain:std", &[], "/std/index.nct", standard),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                prelude,
            ),
        ];
        if reverse {
            packages.reverse();
            modules.reverse();
        }
        CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            sources,
            packages,
            modules,
            Vec::new(),
        )
        .with_toolchain(standard_toolchain(standard))
    }

    fn standard_toolchain(standard: &SyntaxTree) -> ToolchainInput {
        let package = PackageIdentity::new("toolchain:std");
        crate::test_support::test_toolchain(
            ModuleIdentity::new(package.clone(), ["prelude"]),
            &ModuleIdentity::new(package, Vec::<&str>::new()),
            standard,
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
