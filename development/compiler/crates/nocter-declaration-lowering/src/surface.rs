use std::collections::BTreeMap;
use std::fmt;

use nocter_model::{CompilationTarget, SymbolTable};
use nocter_source::{SourceId, SourceMap};
use nocter_syntax::{
    NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, declaration_name_token,
};

use crate::topology::{PreparedCompileUnit, UseResolutionKey, prepare_compile_unit};
use crate::{
    CompileUnitInput, LoweringError, ModuleIdentity, ModuleSourceKind, PackageInput, UseTargetInput,
};
use nocter_target_selection::TargetSelection;

/// Temporary identity of a declaration surface entry before semantic domains are reserved.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceDeclarationId(usize);

impl SurfaceDeclarationId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Temporary identity of a canonical module source in one declaration inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceSourceId(usize);

impl SurfaceSourceId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// The semantic declaration domain selected solely from closed syntax shape.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SurfaceDeclarationKind {
    Function,
    Primitive,
    TypeAlias,
    Struct,
    Field,
    Enum,
    Variant,
    Interface,
    AssociatedType,
    InterfaceMethod,
    Construction,
    ConstructionFunction,
    Literal,
    Instance,
    InherentMethod,
    Coercion,
    Equality,
    Ordering,
    Index,
    Expansion,
    Conformance,
    ConformanceMethod,
    Drop,
    Test,
    OpaqueType,
}

/// One canonical module source retained only for declaration lowering.
#[derive(Clone, Debug)]
pub struct SurfaceSource<'syntax> {
    module: ModuleIdentity,
    canonical_path: Box<str>,
    kind: ModuleSourceKind,
    syntax: &'syntax SyntaxTree,
}

impl<'syntax> SurfaceSource<'syntax> {
    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }

    #[must_use]
    pub const fn canonical_path(&self) -> &str {
        &self.canonical_path
    }

    #[must_use]
    pub const fn kind(&self) -> ModuleSourceKind {
        self.kind
    }

    #[must_use]
    pub const fn syntax(&self) -> &'syntax SyntaxTree {
        self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceImportTarget {
    Source(SurfaceSourceId),
    Module(ModuleIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceImport {
    source: SurfaceSourceId,
    node: NodeId,
    target: SurfaceImportTarget,
}

impl SurfaceImport {
    #[must_use]
    pub const fn source(&self) -> SurfaceSourceId {
        self.source
    }

    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    #[must_use]
    pub const fn target(&self) -> &SurfaceImportTarget {
        &self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceDeclaration {
    source: SurfaceSourceId,
    node: NodeId,
    kind: SurfaceDeclarationKind,
    owner: Option<SurfaceDeclarationId>,
    name: Option<SyntaxToken>,
    visibility: Option<NodeId>,
    target_gate: Option<NodeId>,
}

impl SurfaceDeclaration {
    #[must_use]
    pub const fn source(self) -> SurfaceSourceId {
        self.source
    }

    #[must_use]
    pub const fn node(self) -> NodeId {
        self.node
    }

    #[must_use]
    pub const fn kind(self) -> SurfaceDeclarationKind {
        self.kind
    }

    #[must_use]
    pub const fn owner(self) -> Option<SurfaceDeclarationId> {
        self.owner
    }

    #[must_use]
    pub const fn name(self) -> Option<SyntaxToken> {
        self.name
    }

    #[must_use]
    pub const fn visibility(self) -> Option<NodeId> {
        self.visibility
    }

    #[must_use]
    pub const fn target_gate(self) -> Option<NodeId> {
        self.target_gate
    }
}

/// Canonical, syntax-owned declaration inventory consumed by semantic reservation.
///
/// This representation is deliberately temporary. It retains syntax nodes for the one lowering
/// pass, but it cannot enter the syntax-independent declaration program.
#[derive(Debug)]
pub struct DeclarationSurface<'syntax> {
    target: CompilationTarget,
    source_map: &'syntax SourceMap,
    symbols: SymbolTable,
    packages: Box<[PackageInput<'syntax>]>,
    root_packages: Box<[crate::PackageIdentity]>,
    modules: Box<[ModuleIdentity]>,
    sources: Box<[SurfaceSource<'syntax>]>,
    imports: Box<[SurfaceImport]>,
    package_target_resolutions: Box<[crate::PackageTargetResolutionInput]>,
    declarations: Box<[SurfaceDeclaration]>,
}

impl<'syntax> DeclarationSurface<'syntax> {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn source_map(&self) -> &'syntax SourceMap {
        self.source_map
    }

    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    #[must_use]
    pub const fn packages(&self) -> &[PackageInput<'syntax>] {
        &self.packages
    }

    #[must_use]
    pub const fn modules(&self) -> &[ModuleIdentity] {
        &self.modules
    }

    #[must_use]
    pub const fn sources(&self) -> &[SurfaceSource<'syntax>] {
        &self.sources
    }

    #[must_use]
    pub const fn imports(&self) -> &[SurfaceImport] {
        &self.imports
    }

    #[must_use]
    pub const fn declarations(&self) -> &[SurfaceDeclaration] {
        &self.declarations
    }

    pub(crate) fn into_parts(self) -> SurfaceParts<'syntax> {
        SurfaceParts {
            target: self.target,
            source_map: self.source_map,
            symbols: self.symbols,
            packages: self.packages,
            root_packages: self.root_packages,
            modules: self.modules,
            sources: self.sources,
            imports: self.imports,
            package_target_resolutions: self.package_target_resolutions,
            declarations: self.declarations,
        }
    }
}

pub(crate) struct SurfaceParts<'syntax> {
    pub(crate) target: CompilationTarget,
    pub(crate) source_map: &'syntax SourceMap,
    pub(crate) symbols: SymbolTable,
    pub(crate) packages: Box<[PackageInput<'syntax>]>,
    pub(crate) root_packages: Box<[crate::PackageIdentity]>,
    pub(crate) modules: Box<[ModuleIdentity]>,
    pub(crate) sources: Box<[SurfaceSource<'syntax>]>,
    pub(crate) imports: Box<[SurfaceImport]>,
    pub(crate) package_target_resolutions: Box<[crate::PackageTargetResolutionInput]>,
    pub(crate) declarations: Box<[SurfaceDeclaration]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceError {
    Topology(LoweringError),
    SyntaxErrors(SourceId),
    InvalidRootShape(SourceId),
    InvalidItemShape(NodeId),
    ImplementationVisibility(NodeId),
    ImplementationMember(NodeId),
    MissingConstructionVisibility(NodeId),
    InconsistentUseResolution(NodeId),
    UnknownTargetGate(NodeId),
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topology(error) => error.fmt(formatter),
            Self::SyntaxErrors(source) => {
                write!(formatter, "{source} has lexical or syntax diagnostics")
            }
            Self::InvalidRootShape(source) => {
                write!(formatter, "{source} has an invalid module-root shape")
            }
            Self::InvalidItemShape(node) => {
                write!(
                    formatter,
                    "{node:?} does not contain exactly one declaration"
                )
            }
            Self::ImplementationVisibility(node) => write!(
                formatter,
                "implementation-source declaration {node:?} carries non-private visibility"
            ),
            Self::ImplementationMember(node) => write!(
                formatter,
                "declaration member {node:?} may be authored only in a module root source"
            ),
            Self::MissingConstructionVisibility(node) => write!(
                formatter,
                "module-root construction member {node:?} requires explicit visibility"
            ),
            Self::InconsistentUseResolution(node) => {
                write!(
                    formatter,
                    "use declaration {node:?} lost its resolved target"
                )
            }
            Self::UnknownTargetGate(literal) => {
                write!(formatter, "unknown compilation target in {literal:?}")
            }
        }
    }
}

impl std::error::Error for SurfaceError {}

impl From<LoweringError> for SurfaceError {
    fn from(error: LoweringError) -> Self {
        Self::Topology(error)
    }
}

/// Collects a canonical declaration inventory without resolving any name or type.
///
/// # Errors
///
/// Returns [`SurfaceError`] for invalid compile-unit topology, malformed syntax, an invalid item
/// tree, or a declaration surface forbidden in an implementation source.
pub fn collect_declaration_surface<'syntax>(
    input: &CompileUnitInput<'syntax>,
) -> Result<DeclarationSurface<'syntax>, SurfaceError> {
    let prepared = prepare_compile_unit(input).map_err(|error| match error {
        LoweringError::UnknownTargetGate(literal) => SurfaceError::UnknownTargetGate(literal),
        error => SurfaceError::Topology(error),
    })?;
    let PreparedCompileUnit {
        symbols,
        packages,
        modules,
        use_resolutions,
        package_target_resolutions,
        target_selection,
    } = prepared;
    let mut sources = Vec::new();
    let mut imports = Vec::new();
    let mut declarations = Vec::new();

    for module in &modules {
        let mut module_sources: Vec<_> = module.sources().iter().collect();
        module_sources.sort_unstable_by(|left, right| {
            source_kind_rank(left.kind())
                .cmp(&source_kind_rank(right.kind()))
                .then_with(|| left.canonical_path().cmp(right.canonical_path()))
        });
        for source in module_sources {
            sources.push(SurfaceSource {
                module: module.identity().clone(),
                canonical_path: source.canonical_path().into(),
                kind: source.kind(),
                syntax: source.syntax(),
            });
        }
    }
    let source_by_path: BTreeMap<_, _> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.canonical_path(), SurfaceSourceId(index)))
        .collect();
    for (index, source) in sources.iter().enumerate() {
        collect_source(
            SurfaceSourceId(index),
            source,
            &use_resolutions,
            &target_selection,
            &source_by_path,
            &mut imports,
            &mut declarations,
        )?;
    }

    Ok(DeclarationSurface {
        target: input.target(),
        source_map: input.sources(),
        symbols,
        packages: packages
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        root_packages: input.root_packages().into(),
        modules: modules
            .into_iter()
            .map(|module| module.identity().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        sources: sources.into_boxed_slice(),
        imports: imports.into_boxed_slice(),
        package_target_resolutions: package_target_resolutions
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        declarations: declarations.into_boxed_slice(),
    })
}

const fn source_kind_rank(kind: ModuleSourceKind) -> u8 {
    match kind {
        ModuleSourceKind::Root | ModuleSourceKind::SingleFile => 0,
        ModuleSourceKind::Implementation => 1,
    }
}

fn collect_source(
    source_id: SurfaceSourceId,
    source: &SurfaceSource<'_>,
    use_resolutions: &BTreeMap<UseResolutionKey, &crate::UseResolutionInput>,
    target_selection: &TargetSelection,
    source_by_path: &BTreeMap<&str, SurfaceSourceId>,
    imports: &mut Vec<SurfaceImport>,
    declarations: &mut Vec<SurfaceDeclaration>,
) -> Result<(), SurfaceError> {
    let tree = source.syntax();
    if tree.has_errors() {
        return Err(SurfaceError::SyntaxErrors(tree.source()));
    }
    if tree.root().kind() != NodeKind::ModuleSource {
        return Err(SurfaceError::InvalidRootShape(tree.source()));
    }
    for child in child_nodes(tree, tree.root_id()) {
        match tree.node(child).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::UseDeclaration) => {
                if source.kind() == ModuleSourceKind::Implementation
                    && let Some(visibility) = direct_child(tree, child, NodeKind::Visibility)
                {
                    return Err(SurfaceError::ImplementationVisibility(visibility));
                }
                let resolution = use_resolutions
                    .get(&(child.source(), child.index()))
                    .ok_or(SurfaceError::InconsistentUseResolution(child))?;
                let target = match resolution.target() {
                    UseTargetInput::Source(path) => SurfaceImportTarget::Source(
                        *source_by_path
                            .get(path.as_ref())
                            .ok_or(SurfaceError::InconsistentUseResolution(child))?,
                    ),
                    UseTargetInput::Module(module) => SurfaceImportTarget::Module(module.clone()),
                };
                imports.push(SurfaceImport {
                    source: source_id,
                    node: child,
                    target,
                });
            }
            Some(NodeKind::Item) => {
                if target_selection.item_is_active(child) {
                    collect_item(source_id, source.kind(), tree, child, declarations)?;
                }
            }
            Some(_) | None => return Err(SurfaceError::InvalidRootShape(tree.source())),
        }
    }
    Ok(())
}

fn collect_item(
    source: SurfaceSourceId,
    source_kind: ModuleSourceKind,
    tree: &SyntaxTree,
    item: NodeId,
    declarations: &mut Vec<SurfaceDeclaration>,
) -> Result<(), SurfaceError> {
    let mut target_gate = None;
    let mut declaration = None;
    for child in child_nodes(tree, item) {
        let kind = tree
            .node(child)
            .ok_or(SurfaceError::InvalidItemShape(item))?
            .kind();
        if kind == NodeKind::TargetDirective {
            target_gate = Some(child);
        } else if top_level_kind(kind).is_some() {
            if declaration.replace(child).is_some() {
                return Err(SurfaceError::InvalidItemShape(item));
            }
        } else {
            return Err(SurfaceError::InvalidItemShape(item));
        }
    }
    let declaration = declaration.ok_or(SurfaceError::InvalidItemShape(item))?;
    if source_kind == ModuleSourceKind::Implementation {
        validate_implementation_item(tree, declaration)?;
    } else {
        validate_root_item(tree, declaration)?;
    }
    append_declaration(source, tree, declaration, None, target_gate, declarations)
}

fn append_declaration(
    source: SurfaceSourceId,
    tree: &SyntaxTree,
    node: NodeId,
    owner: Option<SurfaceDeclarationId>,
    target_gate: Option<NodeId>,
    declarations: &mut Vec<SurfaceDeclaration>,
) -> Result<(), SurfaceError> {
    let mut pending = vec![(node, owner, target_gate)];
    while let Some((node, owner, target_gate)) = pending.pop() {
        let syntax_kind = tree
            .node(node)
            .ok_or(SurfaceError::InvalidItemShape(node))?
            .kind();
        let kind = declaration_kind(syntax_kind).ok_or(SurfaceError::InvalidItemShape(node))?;
        let id = SurfaceDeclarationId(declarations.len());
        declarations.push(SurfaceDeclaration {
            source,
            node,
            kind,
            owner,
            name: declaration_name_token(tree, node),
            visibility: direct_child(tree, node, NodeKind::Visibility),
            target_gate,
        });
        for nested in nested_declarations(tree, node)?.into_iter().rev() {
            pending.push((nested, Some(id), None));
        }
    }
    Ok(())
}

fn nested_declarations(tree: &SyntaxTree, root: NodeId) -> Result<Vec<NodeId>, SurfaceError> {
    let mut result = Vec::new();
    let mut pending: Vec<_> = child_nodes(tree, root).rev().collect();
    while let Some(node) = pending.pop() {
        let kind = tree
            .node(node)
            .ok_or(SurfaceError::InvalidItemShape(root))?
            .kind();
        if kind == NodeKind::Block {
            continue;
        }
        if declaration_kind(kind).is_some() {
            result.push(node);
        } else {
            pending.extend(child_nodes(tree, node).rev());
        }
    }
    Ok(result)
}

fn validate_implementation_item(
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<(), SurfaceError> {
    let mut pending = vec![declaration];
    while let Some(node) = pending.pop() {
        let kind = tree
            .node(node)
            .ok_or(SurfaceError::InvalidItemShape(declaration))?
            .kind();
        if kind == NodeKind::Visibility {
            return Err(SurfaceError::ImplementationVisibility(node));
        }
        if matches!(
            kind,
            NodeKind::StructField | NodeKind::AssociatedTypeDeclaration | NodeKind::InterfaceMethod
        ) {
            return Err(SurfaceError::ImplementationMember(node));
        }
        if kind != NodeKind::Block {
            pending.extend(child_nodes(tree, node));
        }
    }
    Ok(())
}

fn validate_root_item(tree: &SyntaxTree, declaration: NodeId) -> Result<(), SurfaceError> {
    if tree.node(declaration).map(nocter_syntax::SyntaxNode::kind)
        != Some(NodeKind::ConstructDeclaration)
    {
        return Ok(());
    }
    for member in child_nodes(tree, declaration) {
        if matches!(
            tree.node(member).map(nocter_syntax::SyntaxNode::kind),
            Some(NodeKind::ConstructionFunction | NodeKind::LiteralDeclaration)
        ) && direct_child(tree, member, NodeKind::Visibility).is_none()
        {
            return Err(SurfaceError::MissingConstructionVisibility(member));
        }
    }
    Ok(())
}

fn child_nodes(tree: &SyntaxTree, node: NodeId) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
    tree.children(node).iter().filter_map(|child| match child {
        SyntaxElement::Node(node) => Some(*node),
        SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
    })
}

fn direct_child(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    child_nodes(tree, node).find(|child| tree.node(*child).is_some_and(|node| node.kind() == kind))
}

fn top_level_kind(kind: NodeKind) -> Option<SurfaceDeclarationKind> {
    match kind {
        NodeKind::FunctionDeclaration => Some(SurfaceDeclarationKind::Function),
        NodeKind::PrimitiveDeclaration => Some(SurfaceDeclarationKind::Primitive),
        NodeKind::TypeAliasDeclaration => Some(SurfaceDeclarationKind::TypeAlias),
        NodeKind::StructDeclaration => Some(SurfaceDeclarationKind::Struct),
        NodeKind::EnumDeclaration => Some(SurfaceDeclarationKind::Enum),
        NodeKind::InterfaceDeclaration => Some(SurfaceDeclarationKind::Interface),
        NodeKind::ConstructDeclaration => Some(SurfaceDeclarationKind::Construction),
        NodeKind::InstanceDeclaration => Some(SurfaceDeclarationKind::Instance),
        NodeKind::ConformDeclaration => Some(SurfaceDeclarationKind::Conformance),
        NodeKind::DropDeclaration => Some(SurfaceDeclarationKind::Drop),
        NodeKind::TestDeclaration => Some(SurfaceDeclarationKind::Test),
        _ => None,
    }
}

fn declaration_kind(kind: NodeKind) -> Option<SurfaceDeclarationKind> {
    if let Some(kind) = top_level_kind(kind) {
        return Some(kind);
    }
    match kind {
        NodeKind::StructField => Some(SurfaceDeclarationKind::Field),
        NodeKind::EnumVariant => Some(SurfaceDeclarationKind::Variant),
        NodeKind::AssociatedTypeDeclaration => Some(SurfaceDeclarationKind::AssociatedType),
        NodeKind::InterfaceMethod => Some(SurfaceDeclarationKind::InterfaceMethod),
        NodeKind::ConstructionFunction => Some(SurfaceDeclarationKind::ConstructionFunction),
        NodeKind::LiteralDeclaration => Some(SurfaceDeclarationKind::Literal),
        NodeKind::InherentMethod => Some(SurfaceDeclarationKind::InherentMethod),
        NodeKind::CoercionDeclaration => Some(SurfaceDeclarationKind::Coercion),
        NodeKind::EqualityOperator => Some(SurfaceDeclarationKind::Equality),
        NodeKind::OrderingOperator => Some(SurfaceDeclarationKind::Ordering),
        NodeKind::IndexOperator => Some(SurfaceDeclarationKind::Index),
        NodeKind::ExpansionOperator => Some(SurfaceDeclarationKind::Expansion),
        NodeKind::ConformMethod => Some(SurfaceDeclarationKind::ConformanceMethod),
        NodeKind::OpaqueResult => Some(SurfaceDeclarationKind::OpaqueType),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
