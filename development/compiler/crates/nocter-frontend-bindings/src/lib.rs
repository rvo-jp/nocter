//! Immutable syntax identities selected by declaration lowering for semantic checking.
//!
//! This contract deliberately contains neither display ranges nor documentation. Checking may
//! consume these exact selections, while `nocter-source-index` independently serves diagnostics
//! and editor features.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use nocter_model::{
    AssociatedTypeId, BodyId, BuiltinType, CallableId, InterfaceId, ModuleId, NominalTypeId,
    ParameterId, Symbol, TypeId,
};
use nocter_source::SourceId;
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};

mod access;

use access::SourceAccessTableBuilder;
pub use access::{SourceAccessError, SourceAccessTable};

/// Closed source-local name authority selected by declaration lowering.
///
/// A source sees its own declarations, declarations from its direct `see` targets, and its
/// own authored imports. Consumers must not reconstruct this table from module membership.
#[derive(Clone, Debug, Default)]
pub struct SourceNamespaceTable {
    namespaces: HashMap<SourceId, SourceNamespace>,
}

#[derive(Clone, Debug, Default)]
struct SourceNamespace {
    authored: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
    fallback: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
}

impl SourceNamespaceTable {
    /// Resolves one exact source-local name.
    #[must_use]
    pub fn lookup(
        &self,
        source: SourceId,
        name: Symbol,
    ) -> Option<nocter_declarations::ExportedEntity> {
        let namespace = self.namespaces.get(&source)?;
        lookup_namespace(&namespace.authored, name)
            .or_else(|| lookup_namespace(&namespace.fallback, name))
    }

    /// Resolves only a declaration, direct see, or import authored for this source.
    #[must_use]
    pub fn lookup_authored(
        &self,
        source: SourceId,
        name: Symbol,
    ) -> Option<nocter_declarations::ExportedEntity> {
        lookup_namespace(&self.namespaces.get(&source)?.authored, name)
    }
}

fn lookup_namespace(
    namespace: &[(Symbol, nocter_declarations::ExportedEntity)],
    name: Symbol,
) -> Option<nocter_declarations::ExportedEntity> {
    namespace
        .binary_search_by_key(&name, |(candidate, _)| *candidate)
        .ok()
        .map(|index| namespace[index].1)
}

/// One exact declaration identity selected by declaration lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendDeclaration {
    BuiltinType(BuiltinType),
    NominalType(NominalTypeId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Callable(CallableId),
}

/// One authored associated-type selection after its semantic identity has been normalized.
///
/// The structural type store deliberately does not retain source occurrences. This companion
/// fact lets checking validate concrete applicability and report the exact authored selection
/// without searching declarations or reconstructing syntax ownership.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssociatedProjectionUse {
    base: TypeId,
    associated: AssociatedTypeId,
    origin: SyntaxOrigin,
}

/// Conflicting targets projected for one authored block import.
///
/// Import resolution owns this decision. A conflict therefore indicates an inconsistent
/// lowering transaction rather than an authored namespace error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateBlockImport {
    declaration: NodeId,
    existing: ModuleId,
    duplicate: ModuleId,
}

impl DuplicateBlockImport {
    #[must_use]
    pub const fn declaration(self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn existing(self) -> ModuleId {
        self.existing
    }

    #[must_use]
    pub const fn duplicate(self) -> ModuleId {
        self.duplicate
    }
}

impl fmt::Display for DuplicateBlockImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "block import {:?} resolves to both {:?} and {:?}",
            self.declaration, self.existing, self.duplicate
        )
    }
}

impl std::error::Error for DuplicateBlockImport {}

impl AssociatedProjectionUse {
    #[must_use]
    pub const fn new(base: TypeId, associated: AssociatedTypeId, origin: SyntaxOrigin) -> Self {
        Self {
            base,
            associated,
            origin,
        }
    }

    #[must_use]
    pub const fn base(self) -> TypeId {
        self.base
    }

    #[must_use]
    pub const fn associated(self) -> AssociatedTypeId {
        self.associated
    }

    #[must_use]
    pub const fn origin(self) -> SyntaxOrigin {
        self.origin
    }
}

/// Exact, syntax-stable inputs selected before semantic checking starts.
#[derive(Clone, Debug, Default)]
pub struct FrontendBindings {
    module_sources: BTreeMap<ModuleId, Box<[SourceId]>>,
    body_blocks: BTreeMap<BodyId, Box<[NodeId]>>,
    parameter_declarations: BTreeMap<ParameterId, Box<[SyntaxToken]>>,
    declarations: HashMap<SyntaxToken, Box<[FrontendDeclaration]>>,
    associated_projection_uses: Box<[AssociatedProjectionUse]>,
    block_imports: HashMap<NodeId, ModuleId>,
    source_namespaces: SourceNamespaceTable,
    source_access: SourceAccessTable,
}

impl FrontendBindings {
    #[must_use]
    pub fn module_sources(&self, module: ModuleId) -> Option<&[SourceId]> {
        self.module_sources.get(&module).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn body_blocks(&self, body: BodyId) -> &[NodeId] {
        self.body_blocks.get(&body).map_or(&[], AsRef::as_ref)
    }

    #[must_use]
    pub fn parameter_declarations(&self, parameter: ParameterId) -> &[SyntaxToken] {
        self.parameter_declarations
            .get(&parameter)
            .map_or(&[], AsRef::as_ref)
    }

    #[must_use]
    pub fn declarations(&self, token: SyntaxToken) -> &[FrontendDeclaration] {
        self.declarations.get(&token).map_or(&[], AsRef::as_ref)
    }

    /// Returns normalized associated selections in deterministic source order.
    #[must_use]
    pub const fn associated_projection_uses(&self) -> &[AssociatedProjectionUse] {
        &self.associated_projection_uses
    }

    #[must_use]
    pub fn block_import(&self, declaration: NodeId) -> Option<ModuleId> {
        self.block_imports.get(&declaration).copied()
    }

    /// Resolves one exact source-local name selected by declaration lowering.
    #[must_use]
    pub fn source_name(
        &self,
        source: SourceId,
        name: Symbol,
    ) -> Option<nocter_declarations::ExportedEntity> {
        self.source_namespaces.lookup(source, name)
    }

    /// Returns the closed source-local name authority for semantic consumers.
    #[must_use]
    pub const fn source_namespaces(&self) -> &SourceNamespaceTable {
        &self.source_namespaces
    }

    /// Returns the direct-source authority used for private declaration access.
    #[must_use]
    pub const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }
}

/// Sole construction authority for [`FrontendBindings`].
#[derive(Debug, Default)]
pub struct FrontendBindingsBuilder {
    module_sources: BTreeMap<ModuleId, Vec<SourceId>>,
    body_blocks: BTreeMap<BodyId, Vec<NodeId>>,
    parameter_declarations: BTreeMap<ParameterId, Vec<SyntaxToken>>,
    declarations: HashMap<SyntaxToken, Vec<FrontendDeclaration>>,
    associated_projection_uses: Vec<AssociatedProjectionUse>,
    block_imports: HashMap<NodeId, ModuleId>,
    source_namespaces: HashMap<SourceId, SourceNamespaceBuilder>,
    source_access: SourceAccessTableBuilder,
}

#[derive(Debug, Default)]
struct SourceNamespaceBuilder {
    authored: Vec<(Symbol, nocter_declarations::ExportedEntity)>,
    fallback: Vec<(Symbol, nocter_declarations::ExportedEntity)>,
}

impl FrontendBindingsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_module_source(&mut self, module: ModuleId, source: SourceId) {
        self.module_sources.entry(module).or_default().push(source);
        self.source_access.define_source_module(source, module);
    }

    pub fn add_body_block(&mut self, body: BodyId, block: NodeId) {
        self.body_blocks.entry(body).or_default().push(block);
    }

    pub fn add_parameter_declaration(&mut self, parameter: ParameterId, token: SyntaxToken) {
        self.parameter_declarations
            .entry(parameter)
            .or_default()
            .push(token);
    }

    pub fn add_declaration(&mut self, token: SyntaxToken, declaration: FrontendDeclaration) {
        self.declarations
            .entry(token)
            .or_default()
            .push(declaration);
    }

    pub fn add_associated_projection_use(&mut self, projection: AssociatedProjectionUse) {
        self.associated_projection_uses.push(projection);
    }

    /// Records the semantic target selected for one block import.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateBlockImport`] when the declaration already has a target.
    pub fn add_block_import(
        &mut self,
        declaration: NodeId,
        target: ModuleId,
    ) -> Result<(), DuplicateBlockImport> {
        match self.block_imports.entry(declaration) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(target);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => Err(DuplicateBlockImport {
                declaration,
                existing: *entry.get(),
                duplicate: target,
            }),
        }
    }

    pub fn define_source_namespace(
        &mut self,
        source: SourceId,
        authored: impl IntoIterator<Item = (Symbol, nocter_declarations::ExportedEntity)>,
        fallback: impl IntoIterator<Item = (Symbol, nocter_declarations::ExportedEntity)>,
    ) {
        self.source_namespaces.insert(
            source,
            SourceNamespaceBuilder {
                authored: authored.into_iter().collect(),
                fallback: fallback.into_iter().collect(),
            },
        );
    }

    pub fn define_source_access(
        &mut self,
        source: SourceId,
        directly_visible: impl IntoIterator<Item = SourceId>,
    ) {
        self.source_access.define_source(source, directly_visible);
    }

    pub fn define_declaration_site_source(
        &mut self,
        site: nocter_model::DeclarationSiteId,
        source: SourceId,
    ) {
        self.source_access.define_site(site, source);
    }

    pub fn define_nominal_representation_source(
        &mut self,
        nominal: NominalTypeId,
        source: SourceId,
        contract_private: bool,
    ) {
        self.source_access
            .define_representation(nominal, source, contract_private);
    }

    #[must_use]
    pub fn finish(self) -> FrontendBindings {
        FrontendBindings {
            module_sources: self
                .module_sources
                .into_iter()
                .map(|(module, mut sources)| {
                    sources.sort_unstable();
                    sources.dedup();
                    (module, sources.into_boxed_slice())
                })
                .collect(),
            body_blocks: self
                .body_blocks
                .into_iter()
                .map(|(body, blocks)| (body, blocks.into_boxed_slice()))
                .collect(),
            parameter_declarations: self
                .parameter_declarations
                .into_iter()
                .map(|(parameter, mut declarations)| {
                    declarations.sort_unstable_by_key(|token| {
                        (token.source(), token.range().start(), token.range().end())
                    });
                    declarations.dedup();
                    (parameter, declarations.into_boxed_slice())
                })
                .collect(),
            declarations: self
                .declarations
                .into_iter()
                .map(|(token, declarations)| (token, declarations.into_boxed_slice()))
                .collect(),
            associated_projection_uses: {
                let mut uses = self.associated_projection_uses;
                uses.sort_unstable_by_key(|projection| {
                    let (source, kind, index) = match projection.origin() {
                        SyntaxOrigin::Node(node) => (node.source(), 0_u8, node.index()),
                        SyntaxOrigin::Token(token) => {
                            (token.source(), 1_u8, token.lexical().index())
                        }
                    };
                    (
                        source,
                        kind,
                        index,
                        projection.base(),
                        projection.associated(),
                    )
                });
                uses.dedup();
                uses.into_boxed_slice()
            },
            source_namespaces: SourceNamespaceTable {
                namespaces: self
                    .source_namespaces
                    .into_iter()
                    .map(|(source, mut entries)| {
                        entries.authored.sort_unstable_by_key(|(name, _)| *name);
                        entries.authored.dedup_by_key(|(name, _)| *name);
                        entries.fallback.sort_unstable_by_key(|(name, _)| *name);
                        entries.fallback.dedup_by_key(|(name, _)| *name);
                        (
                            source,
                            SourceNamespace {
                                authored: entries.authored.into_boxed_slice(),
                                fallback: entries.fallback.into_boxed_slice(),
                            },
                        )
                    })
                    .collect(),
            },
            source_access: self.source_access.finish(),
            block_imports: self.block_imports,
        }
    }
}
