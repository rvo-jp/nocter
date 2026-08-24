use std::collections::BTreeMap;
use std::fmt;

use nocter_model::{CompilationTarget, SymbolTable};
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, declaration_name_token,
};

use crate::topology::{
    IncludeResolutionKey, PreparedCompileUnit, UseResolutionKey, prepare_compile_unit,
};
use crate::{CompileUnitInput, LoweringError, ModuleIdentity, ModuleSourceKind, PackageInput};
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
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// The semantic declaration domain selected solely from closed syntax shape.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SurfaceDeclarationKind {
    Constant,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceInclude {
    source: SurfaceSourceId,
    node: NodeId,
    target: SurfaceSourceId,
}

impl SurfaceInclude {
    #[must_use]
    pub const fn source(self) -> SurfaceSourceId {
        self.source
    }

    #[must_use]
    pub const fn node(self) -> NodeId {
        self.node
    }

    #[must_use]
    pub const fn target(self) -> SurfaceSourceId {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceImport {
    source: SurfaceSourceId,
    node: NodeId,
    target: ModuleIdentity,
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
    pub const fn target(&self) -> &ModuleIdentity {
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
    entity_origin: SyntaxOrigin,
    visibility: Option<NodeId>,
    target_gate: Option<NodeId>,
    interface_default: bool,
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

    /// Returns the exact syntax carrying this declaration's semantic identity.
    #[must_use]
    pub const fn entity_origin(self) -> SyntaxOrigin {
        self.entity_origin
    }

    #[must_use]
    pub const fn visibility(self) -> Option<NodeId> {
        self.visibility
    }

    #[must_use]
    pub const fn target_gate(self) -> Option<NodeId> {
        self.target_gate
    }

    /// Returns whether an interface method explicitly declares reusable default behavior.
    #[must_use]
    pub const fn is_interface_default(self) -> bool {
        self.interface_default
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
    includes: Box<[SurfaceInclude]>,
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
    pub const fn includes(&self) -> &[SurfaceInclude] {
        &self.includes
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
            includes: self.includes,
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
    pub(crate) includes: Box<[SurfaceInclude]>,
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
    InvalidNominalContract(NodeId),
    MissingConstructionContractVisibility(NodeId),
    MissingInterfaceContractVisibility(NodeId),
    InconsistentIncludeResolution(NodeId),
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
            Self::InvalidNominalContract(node) => write!(
                formatter,
                "bodyless nominal declaration {node:?} is not an eligible public index contract"
            ),
            Self::MissingConstructionContractVisibility(node) => write!(
                formatter,
                "bodyless public construction contract member {node:?} requires explicit visibility"
            ),
            Self::MissingInterfaceContractVisibility(node) => write!(
                formatter,
                "interface contract member {node:?} requires explicit visibility"
            ),
            Self::InconsistentIncludeResolution(node) => {
                write!(
                    formatter,
                    "include declaration {node:?} lost its resolved target"
                )
            }
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
    collect_declaration_surface_with(input, SyntaxAcceptance::Complete)
}

pub(crate) fn collect_incomplete_body_declaration_surface<'syntax>(
    input: &CompileUnitInput<'syntax>,
) -> Result<DeclarationSurface<'syntax>, SurfaceError> {
    collect_declaration_surface_with(input, SyntaxAcceptance::IncompleteBodies)
}

#[derive(Clone, Copy)]
enum SyntaxAcceptance {
    Complete,
    IncompleteBodies,
}

fn collect_declaration_surface_with<'syntax>(
    input: &CompileUnitInput<'syntax>,
    syntax_acceptance: SyntaxAcceptance,
) -> Result<DeclarationSurface<'syntax>, SurfaceError> {
    let prepared = prepare_compile_unit(input).map_err(|error| match error {
        LoweringError::UnknownTargetGate(literal) => SurfaceError::UnknownTargetGate(literal),
        error => SurfaceError::Topology(error),
    })?;
    let PreparedCompileUnit {
        symbols,
        packages,
        modules,
        include_resolutions,
        use_resolutions,
        package_target_resolutions,
        target_selection,
    } = prepared;
    let mut sources = Vec::new();
    let mut includes = Vec::new();
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
    let source_collection = SourceCollectionInput {
        include_resolutions: &include_resolutions,
        use_resolutions: &use_resolutions,
        target_selection: &target_selection,
        source_by_path: &source_by_path,
    };
    for (index, source) in sources.iter().enumerate() {
        validate_source_syntax(source.syntax(), syntax_acceptance)?;
        collect_source(
            SurfaceSourceId(index),
            source,
            &source_collection,
            &mut includes,
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
        includes: includes.into_boxed_slice(),
        imports: imports.into_boxed_slice(),
        package_target_resolutions: package_target_resolutions
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        declarations: declarations.into_boxed_slice(),
    })
}

struct SourceCollectionInput<'input> {
    include_resolutions:
        &'input BTreeMap<IncludeResolutionKey, &'input crate::IncludeResolutionInput>,
    use_resolutions: &'input BTreeMap<UseResolutionKey, &'input crate::UseResolutionInput>,
    target_selection: &'input TargetSelection,
    source_by_path: &'input BTreeMap<&'input str, SurfaceSourceId>,
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
    input: &SourceCollectionInput<'_>,
    includes: &mut Vec<SurfaceInclude>,
    imports: &mut Vec<SurfaceImport>,
    declarations: &mut Vec<SurfaceDeclaration>,
) -> Result<(), SurfaceError> {
    let tree = source.syntax();
    if tree.root().kind() != NodeKind::SourceFile {
        return Err(SurfaceError::InvalidRootShape(tree.source()));
    }
    for child in child_nodes(tree, tree.root_id()) {
        match tree.node(child).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::IncludeDeclaration) => {
                let resolution = input
                    .include_resolutions
                    .get(&(child.source(), child.index()))
                    .ok_or(SurfaceError::InconsistentIncludeResolution(child))?;
                let target = *input
                    .source_by_path
                    .get(resolution.target_source())
                    .ok_or(SurfaceError::InconsistentIncludeResolution(child))?;
                includes.push(SurfaceInclude {
                    source: source_id,
                    node: child,
                    target,
                });
            }
            Some(NodeKind::UseDeclaration) => {
                if source.kind() == ModuleSourceKind::Implementation
                    && let Some(visibility) = direct_child(tree, child, NodeKind::Visibility)
                {
                    return Err(SurfaceError::ImplementationVisibility(visibility));
                }
                let resolution = input
                    .use_resolutions
                    .get(&(child.source(), child.index()))
                    .ok_or(SurfaceError::InconsistentUseResolution(child))?;
                imports.push(SurfaceImport {
                    source: source_id,
                    node: child,
                    target: resolution.target_module().clone(),
                });
            }
            Some(NodeKind::Item) => {
                if input.target_selection.item_is_active(child) {
                    collect_item(source_id, source.kind(), tree, child, declarations)?;
                }
            }
            Some(_) | None => return Err(SurfaceError::InvalidRootShape(tree.source())),
        }
    }
    Ok(())
}

fn validate_source_syntax(
    tree: &SyntaxTree,
    acceptance: SyntaxAcceptance,
) -> Result<(), SurfaceError> {
    let accepted = !tree.has_errors()
        || matches!(acceptance, SyntaxAcceptance::IncompleteBodies)
            && syntax_errors_are_inside_blocks(tree);
    if accepted {
        Ok(())
    } else {
        Err(SurfaceError::SyntaxErrors(tree.source()))
    }
}

fn syntax_errors_are_inside_blocks(tree: &SyntaxTree) -> bool {
    if !tree.lexed().diagnostics().is_empty() {
        return false;
    }
    let blocks = tree
        .nodes()
        .filter(|(_, node)| node.kind() == NodeKind::Block)
        .map(|(_, node)| node.range())
        .collect::<Vec<_>>();
    tree.diagnostics().iter().all(|diagnostic| {
        let range = diagnostic.span().range();
        blocks.iter().any(|block| block.contains_range(range))
    })
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
        let name = declaration_name_token(tree, node);
        let entity_origin =
            crate::surface_origin::declaration_entity_origin(tree, node, kind, name)
                .ok_or(SurfaceError::InvalidItemShape(node))?;
        let id = SurfaceDeclarationId(declarations.len());
        declarations.push(SurfaceDeclaration {
            source,
            node,
            kind,
            owner,
            name,
            entity_origin,
            visibility: direct_child(tree, node, NodeKind::Visibility),
            target_gate,
            interface_default: kind == SurfaceDeclarationKind::InterfaceMethod
                && direct_child(tree, node, NodeKind::InterfaceDefaultModifier).is_some(),
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
        if kind != NodeKind::Block {
            pending.extend(child_nodes(tree, node));
        }
    }
    if is_bodyless_nominal(tree, declaration) {
        return Err(SurfaceError::InvalidNominalContract(declaration));
    }
    Ok(())
}

fn validate_root_item(tree: &SyntaxTree, declaration: NodeId) -> Result<(), SurfaceError> {
    if is_bodyless_nominal(tree, declaration) {
        if direct_child(tree, declaration, NodeKind::Visibility).is_none() {
            return Err(SurfaceError::InvalidNominalContract(declaration));
        }
        return Ok(());
    }
    match tree.node(declaration).map(nocter_syntax::SyntaxNode::kind) {
        Some(NodeKind::ConstructDeclaration) => {
            for member in child_nodes(tree, declaration) {
                if matches!(
                    tree.node(member).map(nocter_syntax::SyntaxNode::kind),
                    Some(NodeKind::ConstructionFunction | NodeKind::LiteralDeclaration)
                ) && !contains_child_kind(tree, member, NodeKind::Block)
                    && direct_child(tree, member, NodeKind::Visibility).is_none()
                {
                    return Err(SurfaceError::MissingConstructionContractVisibility(member));
                }
            }
        }
        Some(NodeKind::InterfaceDeclaration) => {
            for member in child_nodes(tree, declaration) {
                if tree.node(member).map(nocter_syntax::SyntaxNode::kind)
                    == Some(NodeKind::InterfaceMethod)
                    && direct_child(tree, member, NodeKind::Visibility).is_none()
                {
                    return Err(SurfaceError::MissingInterfaceContractVisibility(member));
                }
            }
        }
        Some(_) | None => {}
    }
    Ok(())
}

fn is_bodyless_nominal(tree: &SyntaxTree, declaration: NodeId) -> bool {
    matches!(
        tree.node(declaration).map(nocter_syntax::SyntaxNode::kind),
        Some(NodeKind::StructDeclaration | NodeKind::EnumDeclaration)
    ) && !tree.children(declaration).iter().any(|element| {
        matches!(
            element,
            SyntaxElement::Token(token)
                if token.kind() == nocter_syntax::TokenKind::Punctuation(
                    nocter_syntax::Punctuation::LeftBrace
                )
        )
    })
}

fn contains_child_kind(tree: &SyntaxTree, root: NodeId, expected: NodeKind) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        for child in child_nodes(tree, node) {
            if tree.node(child).map(nocter_syntax::SyntaxNode::kind) == Some(expected) {
                return true;
            }
            pending.push(child);
        }
    }
    false
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
        NodeKind::ConstantDeclaration => Some(SurfaceDeclarationKind::Constant),
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
