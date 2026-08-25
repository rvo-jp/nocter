use std::collections::HashMap;
use std::fmt;

use nocter_compile_input::CompileUnitInput;
use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{BodyId, DeclarationSiteId, ModuleId};
use nocter_source::SourceId;
use nocter_syntax::{NodeId, NodeKind, SyntaxTree};

/// The exact syntax body selected for one declaration `BodyId`.
///
/// This is temporary checking input. It never enters the canonical checked program.
#[derive(Clone, Copy, Debug)]
pub struct BodySource<'syntax> {
    body: BodyId,
    owner: BodyOwner,
    module: ModuleId,
    syntax: &'syntax SyntaxTree,
    block: NodeId,
}

impl<'syntax> BodySource<'syntax> {
    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn owner(self) -> BodyOwner {
        self.owner
    }

    #[must_use]
    pub const fn module(self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn syntax(self) -> &'syntax SyntaxTree {
        self.syntax
    }

    #[must_use]
    pub const fn block(self) -> NodeId {
        self.block
    }
}

/// Canonical `BodyId`-ordered source inputs for one complete declaration program.
#[derive(Debug)]
pub struct BodySourceCatalog<'syntax>(Box<[BodySource<'syntax>]>);

impl<'syntax> BodySourceCatalog<'syntax> {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = BodySource<'syntax>> + '_ {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn get(&self, body: BodyId) -> Option<BodySource<'syntax>> {
        self.0
            .binary_search_by_key(&body, |source| source.body())
            .ok()
            .map(|index| self.0[index])
    }
}

/// Internal inconsistency at the declaration-to-body-checking boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodySourceError {
    DuplicateSyntaxSource(SourceId),
    MissingModuleProjection(ModuleId),
    InvalidModuleProjection(ModuleId),
    SourceOwnedByTwoModules(SourceId),
    MissingBodyProjection(BodyId),
    DuplicateBodyProjection(BodyId),
    InvalidBodyProjection(BodyId),
    MissingSyntaxSource(BodyId),
    InvalidBodyOwner(BodyId),
    MissingDeclarationSite(DeclarationSiteId),
    BodyOutsideOwnerModule(BodyId),
}

impl fmt::Display for BodySourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSyntaxSource(source) => {
                write!(
                    formatter,
                    "source {source} occurs more than once in checking input"
                )
            }
            Self::MissingModuleProjection(module) => {
                write!(
                    formatter,
                    "module {module:?} has no physical-source projection"
                )
            }
            Self::InvalidModuleProjection(module) => {
                write!(
                    formatter,
                    "module {module:?} has an invalid source projection"
                )
            }
            Self::SourceOwnedByTwoModules(source) => {
                write!(formatter, "source {source} is projected by two modules")
            }
            Self::MissingBodyProjection(body) => {
                write!(formatter, "body {body:?} has no implementation projection")
            }
            Self::DuplicateBodyProjection(body) => {
                write!(
                    formatter,
                    "body {body:?} has more than one implementation projection"
                )
            }
            Self::InvalidBodyProjection(body) => {
                write!(formatter, "body {body:?} is not projected to a block node")
            }
            Self::MissingSyntaxSource(body) => {
                write!(
                    formatter,
                    "body {body:?} projects to an absent syntax source"
                )
            }
            Self::InvalidBodyOwner(body) => {
                write!(formatter, "body {body:?} has an invalid declaration owner")
            }
            Self::MissingDeclarationSite(site) => {
                write!(formatter, "declaration site {site:?} is absent")
            }
            Self::BodyOutsideOwnerModule(body) => {
                write!(formatter, "body {body:?} is outside its owner's module")
            }
        }
    }
}

impl std::error::Error for BodySourceError {}

/// Selects every declaration body from exact source projections.
///
/// This function never scans source containment to discover a body and never reconstructs module
/// ownership from filesystem paths. Both associations come from Phase 2 semantic identities and
/// their explicit source projection.
///
/// # Errors
///
/// Returns [`BodySourceError`] when the declaration program, source index, or syntax input does not
/// describe one complete and mutually consistent compile unit.
pub fn catalog_body_sources<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
) -> Result<BodySourceCatalog<'syntax>, BodySourceError> {
    let syntax = syntax_by_source(input)?;
    let modules = module_by_source(graph, bindings)?;
    let mut bodies = Vec::with_capacity(graph.declarations().bodies().len());

    for (body, declaration) in graph.declarations().bodies().iter() {
        let blocks = bindings.body_blocks(body);
        let [block] = blocks else {
            if blocks.is_empty() {
                return Err(BodySourceError::MissingBodyProjection(body));
            }
            return Err(BodySourceError::DuplicateBodyProjection(body));
        };
        let tree = syntax
            .get(&block.source())
            .copied()
            .ok_or(BodySourceError::MissingSyntaxSource(body))?;
        if tree.node(*block).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::Block) {
            return Err(BodySourceError::InvalidBodyProjection(body));
        }
        let module = body_module(graph, body, declaration.owner())?;
        if modules.get(&tree.source()).copied() != Some(module) {
            return Err(BodySourceError::BodyOutsideOwnerModule(body));
        }
        bodies.push(BodySource {
            body,
            owner: declaration.owner(),
            module,
            syntax: tree,
            block: *block,
        });
    }

    Ok(BodySourceCatalog(bodies.into_boxed_slice()))
}

fn syntax_by_source<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
) -> Result<HashMap<SourceId, &'syntax SyntaxTree>, BodySourceError> {
    let mut result = HashMap::new();
    for module in input.modules() {
        for source in module.sources() {
            let tree = source.syntax();
            if result.insert(tree.source(), tree).is_some() {
                return Err(BodySourceError::DuplicateSyntaxSource(tree.source()));
            }
        }
    }
    Ok(result)
}

fn module_by_source(
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
) -> Result<HashMap<SourceId, ModuleId>, BodySourceError> {
    let mut result = HashMap::new();
    for (module, _) in graph.modules().iter() {
        let mut found = false;
        for source in bindings.module_sources(module).unwrap_or_default() {
            found = true;
            if result.insert(*source, module).is_some() {
                return Err(BodySourceError::SourceOwnedByTwoModules(*source));
            }
        }
        if !found {
            return Err(BodySourceError::MissingModuleProjection(module));
        }
    }
    Ok(result)
}

fn body_module(
    graph: &DeclarationGraph,
    body: BodyId,
    owner: BodyOwner,
) -> Result<ModuleId, BodySourceError> {
    let declarations = graph.declarations();
    let site = match owner {
        BodyOwner::Callable(owner) => declarations
            .callables()
            .get(owner)
            .map(nocter_declarations::CallableDeclaration::site),
        BodyOwner::Drop(owner) => declarations
            .drops()
            .get(owner)
            .map(nocter_declarations::DropDeclaration::site),
        BodyOwner::Test(owner) => declarations
            .tests()
            .get(owner)
            .map(|declaration| declaration.site()),
    }
    .ok_or(BodySourceError::InvalidBodyOwner(body))?;
    graph
        .declaration_sites()
        .get(site)
        .map(|declaration| declaration.module())
        .ok_or(BodySourceError::MissingDeclarationSite(site))
}

#[cfg(test)]
mod tests {
    use nocter_compile_input::{
        CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        PackageInput, PackageMode,
    };
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_model::PackageIdentity;
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{NodeKind, ParseGoal, SyntaxTree, parse};

    use super::{BodySourceError, catalog_body_sources};

    #[test]
    fn body_catalog_is_exact_and_input_order_independent() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(
            &mut sources,
            "/app/index.nct",
            "func first(): void { return }\nfunc second(): void { return }\n",
        );
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::BUILTIN_DECLARATIONS,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, std_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let prelude_identity =
            ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        let mut results = Vec::new();

        for reverse in [false, true] {
            let mut packages = vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package("toolchain:std", "std", "/std/index.nct", &std_manifest),
            ];
            let mut modules = vec![
                module("workspace:app", &[], "/app/index.nct", &app),
                module("toolchain:std", &[], "/std/index.nct", &standard),
                module(
                    "toolchain:std",
                    &["prelude"],
                    "/std/prelude/index.nct",
                    &prelude,
                ),
            ];
            if reverse {
                packages.reverse();
                modules.reverse();
            }
            let input = CompileUnitInput::new(
                nocter_model::CompilationTarget::Arm64Darwin,
                &sources,
                packages,
                modules,
                Vec::new(),
            )
            .with_toolchain(crate::test_support::builtin_toolchain(
                &sources,
                &standard,
                prelude_identity.clone(),
            ));
            let lowered = lower_compile_unit_declarations(&input).unwrap();
            let (program, bindings, _) = lowered.into_checking_parts(&input);
            let catalog = catalog_body_sources(&input, program.graph(), &bindings).unwrap();
            assert_eq!(catalog.len(), 2);
            assert!(!catalog.is_empty());
            let entries: Vec<_> = catalog
                .iter()
                .map(|entry| {
                    assert_eq!(
                        entry
                            .syntax()
                            .node(entry.block())
                            .map(nocter_syntax::SyntaxNode::kind),
                        Some(NodeKind::Block)
                    );
                    assert_eq!(catalog.get(entry.body()).unwrap().block(), entry.block());
                    (
                        entry.body(),
                        entry.owner(),
                        entry.module(),
                        entry.syntax().source(),
                        entry.block(),
                    )
                })
                .collect();
            results.push(entries);
        }

        assert_eq!(results[0], results[1]);
    }

    #[test]
    fn missing_phase_two_projection_is_an_internal_boundary_error() {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(
            &mut sources,
            "/app/index.nct",
            "func main(): void { return }\n",
        );
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::BUILTIN_DECLARATIONS,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
        let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
        let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
        let standard = parse_source(&sources, std_id, ParseGoal::SourceFile);
        let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
        let prelude_identity =
            ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![
                package("workspace:app", "app", "/app/index.nct", &app_manifest),
                package("toolchain:std", "std", "/std/index.nct", &std_manifest),
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
            Vec::new(),
        )
        .with_toolchain(crate::test_support::builtin_toolchain(
            &sources,
            &standard,
            prelude_identity,
        ));
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _, _) = lowered.into_checking_parts(&input);
        let error = catalog_body_sources(
            &input,
            program.graph(),
            &nocter_frontend_bindings::FrontendBindings::default(),
        )
        .unwrap_err();

        assert!(matches!(error, BodySourceError::MissingModuleProjection(_)));
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

    fn package(identity: &str, name: &str, _path: &str, _manifest: &SyntaxTree) -> PackageInput {
        PackageInput::new(PackageIdentity::new(identity), name, PackageMode::Declared)
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
