//! Explicit selected-module source fixtures, without semantic or filesystem discovery authority.
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode, SourceVisibilityResolutionInput,
};
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxTree, child_node_iter, parse};

/// Exact selected sources and dependency occurrences; no declaration or body evidence is created.
pub struct ValueModuleFixture {
    sources: SourceMap,
    trees: Vec<SyntaxTree>,
    files: Vec<(String, ModuleSourceKind)>,
    edges: Vec<(usize, usize, usize)>,
    modules: Vec<(Vec<String>, Vec<usize>)>,
    uses: Vec<(usize, usize, Vec<String>)>,
}
impl ValueModuleFixture {
    /// # Panics
    /// Panics if a supplied source cannot be stored or parsed by the selected private grammar.
    #[must_use]
    pub fn new(files: &[(&str, ModuleSourceKind, &str)], edges: &[(usize, usize, usize)]) -> Self {
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
            modules: vec![(Vec::new(), (0..files.len()).collect())],
            uses: Vec::new(),
        }
    }
    /// Select exact module paths and member-source indices; no directory discovery is performed.
    #[must_use]
    pub fn with_modules(mut self, modules: &[(&[&str], &[usize])]) -> Self {
        self.modules = modules
            .iter()
            .map(|(path, files)| {
                (
                    path.iter().map(|part| (*part).to_string()).collect(),
                    files.to_vec(),
                )
            })
            .collect();
        self
    }
    /// Supply exact source/occurrence/target-module import resolutions for production validation.
    /// Occurrences include top-level and block imports in physical source order.
    #[must_use]
    pub fn with_uses(mut self, uses: &[(usize, usize, &[&str])]) -> Self {
        self.uses = uses
            .iter()
            .map(|(source, occurrence, target)| {
                (
                    *source,
                    *occurrence,
                    target.iter().map(|part| (*part).to_string()).collect(),
                )
            })
            .collect();
        self
    }
    /// # Panics
    /// Panics for inconsistent fixture indices or missing authored dependency occurrences.
    #[must_use]
    pub fn input(&self) -> CompileUnitInput<'_> {
        let package = PackageIdentity::new("workspace:app");
        CompileUnitInput::for_goal(
            ParseGoal::V2CoreUnit,
            CompilationTarget::Arm64Darwin,
            &self.sources,
            vec![PackageInput::new(
                package.clone(),
                "app",
                PackageMode::Declared,
            )],
            self.modules
                .iter()
                .map(|(path, files)| {
                    ModuleInput::new(
                        ModuleIdentity::new(package.clone(), path.iter().map(String::as_str)),
                        files
                            .iter()
                            .map(|&index| {
                                ModuleSourceInput::new(
                                    self.files[index].0.as_str(),
                                    self.files[index].1,
                                    &self.trees[index],
                                )
                            })
                            .collect(),
                    )
                })
                .collect(),
            self.uses
                .iter()
                .map(|(source, occurrence, target)| {
                    let tree = &self.trees[*source];
                    let mut imports = tree
                        .nodes()
                        .filter(|(_, node)| {
                            matches!(
                                node.kind(),
                                NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
                            )
                        })
                        .collect::<Vec<_>>();
                    imports.sort_by_key(|(_, node)| node.range().start());
                    let node = imports[*occurrence].0;
                    nocter_compile_input::UseResolutionInput::new(
                        node,
                        ModuleIdentity::new(package.clone(), target.iter().map(String::as_str)),
                    )
                })
                .collect(),
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
        )
    }
    /// # Panics
    /// Panics if the requested physical source index is outside this fixture.
    #[must_use]
    pub fn tree(&self, index: usize) -> &SyntaxTree {
        &self.trees[index]
    }
}

/// Shared authored module corpus for checking, required execution and native qualification.
/// # Panics
/// Panics when the repository fixture is missing or cannot form its explicit selected input.
#[must_use]
pub fn qualified_namespace_fixture() -> ValueModuleFixture {
    use ModuleSourceKind::{Implementation, Root};
    let root = crate::repository_root()
        .join("development/compiler/tests/fixtures/value-modules/qualified-namespaces");
    let entries = [
        ("index.nct", Root),
        ("body.nct", Implementation),
        ("facade/index.nct", Root),
        ("data/index.nct", Root),
        ("data/body.nct", Implementation),
    ];
    let texts = entries
        .iter()
        .map(|(path, _)| std::fs::read_to_string(root.join(path)).expect("module source fixture"))
        .collect::<Vec<_>>();
    let files = entries
        .iter()
        .map(|(path, kind)| (format!("/app/{path}"), *kind))
        .collect::<Vec<_>>();
    let files = files
        .iter()
        .zip(&texts)
        .map(|((path, kind), text)| (path.as_str(), *kind, text.as_str()))
        .collect::<Vec<_>>();
    ValueModuleFixture::new(&files, &[(0, 0, 1), (1, 0, 0), (3, 0, 4), (4, 0, 3)])
        .with_modules(&[(&[], &[0, 1]), (&["facade"], &[2]), (&["data"], &[3, 4])])
        .with_uses(&[
            (0, 0, &["facade"]),
            (1, 0, &["facade"]),
            (1, 1, &["facade"]),
            (1, 2, &["data"]),
            (1, 3, &["data"]),
            (2, 0, &["data"]),
        ])
}
