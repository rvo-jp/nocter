use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_compile_input::{ModuleIdentity, ModuleSourceKind, PackageMode};
use nocter_source::SourceId;
use nocter_syntax::NodeKind;

use crate::DiscoveredUnit;

/// Canonical, syntax-content-independent topology that can affect declaration semantics.
///
/// Source contents belong to source/module surface queries. This value records only the selected
/// packages, modules, physical source membership, top-level dependency resolutions, targets, and
/// toolchain attachments. Body-local imports are intentionally excluded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTopologySurface {
    canonical: Box<[u8]>,
}

impl SemanticTopologySurface {
    #[must_use]
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}

impl DiscoveredUnit {
    /// Freezes declaration-relevant discovery decisions without retaining source-arena identity.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when a retained resolution does not belong to exactly one
    /// discovered source tree. Discovery normally guarantees this before publishing the unit.
    pub fn semantic_topology_surface(
        &self,
    ) -> Result<SemanticTopologySurface, SemanticTopologyError> {
        let sources = canonical_sources(self)?;
        let mut canonical = Vec::new();
        encode(self.target.name().as_bytes(), &mut canonical);
        encode_packages(self, &mut canonical);
        encode_modules(self, &mut canonical);
        encode_resolutions(self, &sources, &mut canonical)?;
        encode_targets(self, &mut canonical);
        encode_toolchain(self, &mut canonical)?;

        Ok(SemanticTopologySurface {
            canonical: canonical.into_boxed_slice(),
        })
    }
}

fn encode_packages(unit: &DiscoveredUnit, output: &mut Vec<u8>) {
    let mut packages = unit.packages.iter().collect::<Vec<_>>();
    packages.sort_unstable_by_key(|package| &package.identity);
    for package in packages {
        output.push(0x10);
        encode(package.identity.as_str().as_bytes(), output);
        encode(package.display_name.as_bytes(), output);
        output.push(package_mode_code(package.mode));
        for (alias, identity) in &package.dependencies {
            output.push(0x11);
            encode(alias.as_bytes(), output);
            encode(identity.as_str().as_bytes(), output);
        }
    }
    let mut roots = unit.root_packages.iter().collect::<Vec<_>>();
    roots.sort_unstable();
    for root in roots {
        output.push(0x12);
        encode(root.as_str().as_bytes(), output);
    }
}

fn encode_modules(unit: &DiscoveredUnit, output: &mut Vec<u8>) {
    let mut modules = unit.modules.iter().collect::<Vec<_>>();
    modules.sort_unstable_by_key(|module| module.identity());
    for module in modules {
        output.push(0x20);
        encode_module(module.identity(), output);
        let mut sources = module.sources().iter().collect::<Vec<_>>();
        sources.sort_unstable_by(|left, right| {
            source_kind_code(left.kind())
                .cmp(&source_kind_code(right.kind()))
                .then_with(|| left.canonical_path().cmp(right.canonical_path()))
        });
        for source in sources {
            output.push(0x21);
            output.push(source_kind_code(source.kind()));
            encode(source.canonical_path().as_bytes(), output);
        }
    }
}

fn encode_resolutions(
    unit: &DiscoveredUnit,
    sources: &[CanonicalSource<'_>],
    output: &mut Vec<u8>,
) -> Result<(), SemanticTopologyError> {
    let ownership = sources
        .iter()
        .map(|source| source.id)
        .collect::<BTreeSet<_>>();
    let mut declarations = BTreeSet::new();
    for declaration in unit
        .source_visibility_resolutions
        .iter()
        .map(nocter_compile_input::SourceVisibilityResolutionInput::declaration)
        .chain(
            unit.use_resolutions
                .iter()
                .map(nocter_compile_input::UseResolutionInput::declaration),
        )
    {
        if !ownership.contains(&declaration.source()) {
            return Err(SemanticTopologyError::UnknownResolutionSource(declaration));
        }
        if !declarations.insert((declaration.source(), declaration.index())) {
            return Err(SemanticTopologyError::DuplicateResolution(declaration));
        }
    }
    for source in sources {
        let tree = &unit.syntax[source.syntax];
        let mut visibility = unit
            .source_visibility_resolutions
            .iter()
            .filter(|resolution| resolution.declaration().source() == source.id)
            .collect::<Vec<_>>();
        visibility.sort_unstable_by(|left, right| {
            left.declaration()
                .index()
                .cmp(&right.declaration().index())
                .then_with(|| left.target_source().cmp(right.target_source()))
        });
        for resolution in visibility {
            require_node_kind(
                tree,
                resolution.declaration(),
                NodeKind::SourceVisibilityDeclaration,
            )?;
            output.push(0x30);
            encode(source.path.as_bytes(), output);
            encode(resolution.target_source().as_bytes(), output);
        }
        let mut uses = unit
            .use_resolutions
            .iter()
            .filter(|resolution| resolution.declaration().source() == source.id)
            .collect::<Vec<_>>();
        uses.sort_unstable_by(|left, right| {
            left.declaration()
                .index()
                .cmp(&right.declaration().index())
                .then_with(|| left.target_module().cmp(right.target_module()))
        });
        for resolution in uses {
            match tree
                .node(resolution.declaration())
                .map(nocter_syntax::SyntaxNode::kind)
            {
                Some(NodeKind::UseDeclaration) => {
                    output.push(0x31);
                    encode(source.path.as_bytes(), output);
                    encode_module(resolution.target_module(), output);
                }
                Some(NodeKind::BlockUseDeclaration) => {}
                actual => {
                    return Err(SemanticTopologyError::InvalidUseResolution {
                        declaration: resolution.declaration(),
                        actual,
                    });
                }
            }
        }
    }
    Ok(())
}

fn encode_targets(unit: &DiscoveredUnit, output: &mut Vec<u8>) {
    let mut targets = unit.package_target_resolutions.iter().collect::<Vec<_>>();
    targets.sort_unstable_by(|left, right| {
        left.module()
            .package()
            .cmp(right.module().package())
            .then_with(|| left.declaration_order().cmp(&right.declaration_order()))
            .then_with(|| left.module().cmp(right.module()))
            .then_with(|| left.name().cmp(right.name()))
            .then_with(|| left.kind().name().cmp(right.kind().name()))
    });
    for target in targets {
        output.push(0x40);
        encode(target.name().as_bytes(), output);
        encode(target.kind().name().as_bytes(), output);
        output.extend_from_slice(&target.declaration_order().to_be_bytes());
        encode_module(target.module(), output);
    }
}

fn encode_toolchain(
    unit: &DiscoveredUnit,
    output: &mut Vec<u8>,
) -> Result<(), SemanticTopologyError> {
    let toolchain = unit
        .toolchain
        .as_ref()
        .ok_or(SemanticTopologyError::MissingToolchain)?;
    output.push(0x50);
    encode(toolchain.standard_package().as_str().as_bytes(), output);
    encode_module(toolchain.prelude(), output);
    let mut attachments = toolchain
        .structural_attachments()
        .iter()
        .collect::<Vec<_>>();
    attachments.sort_unstable_by(|left, right| {
        left.attachment()
            .cmp(&right.attachment())
            .then_with(|| left.module().cmp(right.module()))
    });
    for attachment in attachments {
        output.push(0x51);
        encode(attachment.attachment().name().as_bytes(), output);
        encode_module(attachment.module(), output);
    }
    let mut standard_roles = toolchain.standard_roles().iter().collect::<Vec<_>>();
    standard_roles.sort_unstable_by(|left, right| {
        left.role()
            .cmp(&right.role())
            .then_with(|| left.module().cmp(right.module()))
            .then_with(|| left.kind().as_str().cmp(right.kind().as_str()))
            .then_with(|| left.name().cmp(right.name()))
    });
    for locator in standard_roles {
        output.push(0x52);
        encode(locator.role().name().as_bytes(), output);
        encode_module(locator.module(), output);
        encode(locator.kind().as_str().as_bytes(), output);
        encode(locator.name().as_bytes(), output);
    }
    let mut primitive_roles = toolchain.primitive_roles().iter().collect::<Vec<_>>();
    primitive_roles.sort_unstable_by(|left, right| {
        left.role()
            .cmp(&right.role())
            .then_with(|| left.module().cmp(right.module()))
            .then_with(|| left.name().cmp(right.name()))
    });
    for locator in primitive_roles {
        output.push(0x53);
        encode(locator.role().name().as_bytes(), output);
        encode_module(locator.module(), output);
        encode(locator.name().as_bytes(), output);
    }
    let mut builtins = toolchain.builtin_types().iter().collect::<Vec<_>>();
    builtins.sort_unstable_by(|left, right| {
        left.builtin()
            .cmp(&right.builtin())
            .then_with(|| left.module().cmp(right.module()))
            .then_with(|| left.name().cmp(right.name()))
    });
    for locator in builtins {
        output.push(0x54);
        encode(locator.builtin().spelling().as_bytes(), output);
        encode_module(locator.module(), output);
        encode(locator.name().as_bytes(), output);
    }
    Ok(())
}

struct CanonicalSource<'unit> {
    id: SourceId,
    path: &'unit str,
    syntax: usize,
}

fn canonical_sources(
    unit: &DiscoveredUnit,
) -> Result<Vec<CanonicalSource<'_>>, SemanticTopologyError> {
    let mut sources = unit
        .modules
        .iter()
        .flat_map(|module| module.sources().iter())
        .map(|source| {
            let tree = unit.syntax.get(source.syntax_index()).ok_or_else(|| {
                SemanticTopologyError::MissingSyntax(source.canonical_path().into())
            })?;
            Ok(CanonicalSource {
                id: tree.source(),
                path: source.canonical_path(),
                syntax: source.syntax_index(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    sources.sort_unstable_by_key(|source| source.path);
    let mut ownership = BTreeMap::new();
    let mut paths = BTreeMap::new();
    for source in &sources {
        if ownership.insert(source.id, source.path).is_some() {
            return Err(SemanticTopologyError::DuplicateSource(source.id));
        }
        if paths.insert(source.path, source.id).is_some() {
            return Err(SemanticTopologyError::DuplicateSourcePath(
                source.path.into(),
            ));
        }
    }
    Ok(sources)
}

fn require_node_kind(
    tree: &nocter_syntax::SyntaxTree,
    declaration: nocter_syntax::NodeId,
    expected: NodeKind,
) -> Result<(), SemanticTopologyError> {
    let actual = tree.node(declaration).map(nocter_syntax::SyntaxNode::kind);
    if actual == Some(expected) {
        Ok(())
    } else {
        Err(SemanticTopologyError::InvalidVisibilityResolution {
            declaration,
            actual,
        })
    }
}

fn encode_module(module: &ModuleIdentity, output: &mut Vec<u8>) {
    encode(module.package().as_str().as_bytes(), output);
    output.extend_from_slice(&(module.path().len() as u64).to_be_bytes());
    for segment in module.path() {
        encode(segment.as_bytes(), output);
    }
}

const fn package_mode_code(mode: PackageMode) -> u8 {
    match mode {
        PackageMode::Declared => 0,
        PackageMode::SingleFile => 1,
    }
}

const fn source_kind_code(kind: ModuleSourceKind) -> u8 {
    match kind {
        ModuleSourceKind::Root => 0,
        ModuleSourceKind::SingleFile => 1,
        ModuleSourceKind::Implementation => 2,
    }
}

fn encode(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTopologyError {
    MissingToolchain,
    MissingSyntax(Box<str>),
    DuplicateSource(SourceId),
    DuplicateSourcePath(Box<str>),
    UnknownResolutionSource(nocter_syntax::NodeId),
    DuplicateResolution(nocter_syntax::NodeId),
    InvalidVisibilityResolution {
        declaration: nocter_syntax::NodeId,
        actual: Option<NodeKind>,
    },
    InvalidUseResolution {
        declaration: nocter_syntax::NodeId,
        actual: Option<NodeKind>,
    },
}

impl fmt::Display for SemanticTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid discovered semantic topology: {self:?}")
    }
}

impl std::error::Error for SemanticTopologyError {}
