//! Immutable syntax identities selected by declaration lowering for semantic checking.
//!
//! This contract deliberately contains neither display ranges nor documentation. Checking may
//! consume these exact selections, while `nocter-source-index` independently serves diagnostics
//! and editor features.

use std::collections::{BTreeMap, HashMap};

use nocter_model::{
    AssociatedTypeId, BodyId, CallableId, InterfaceId, ModuleId, NominalTypeId, ParameterId, Symbol,
};
use nocter_source::SourceId;
use nocter_syntax::{NodeId, SyntaxToken};

mod access;

use access::SourceAccessTableBuilder;
pub use access::{SourceAccessError, SourceAccessTable};

/// Closed source-local name authority selected by declaration lowering.
///
/// A source sees its own declarations, declarations from sources it directly includes, and its
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

    /// Resolves only a declaration, direct include, or import authored for this source.
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

/// A declaration kind that toolchain discovery may assign a standard semantic role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendDeclaration {
    NominalType(NominalTypeId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Callable(CallableId),
}

/// Exact, syntax-stable inputs selected before semantic checking starts.
#[derive(Clone, Debug, Default)]
pub struct FrontendBindings {
    module_sources: BTreeMap<ModuleId, Box<[SourceId]>>,
    body_blocks: BTreeMap<BodyId, Box<[NodeId]>>,
    parameter_declarations: BTreeMap<ParameterId, Box<[SyntaxToken]>>,
    declarations: HashMap<SyntaxToken, Box<[FrontendDeclaration]>>,
    block_imports: HashMap<NodeId, ModuleId>,
    source_namespaces: SourceNamespaceTable,
    source_access: SourceAccessTable,
    constant_array_lengths: HashMap<NodeId, u64>,
}

impl FrontendBindings {
    #[must_use]
    pub fn module_sources(&self, module: ModuleId) -> Option<&[SourceId]> {
        self.module_sources.get(&module).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn module_for_source(&self, source: SourceId) -> Option<ModuleId> {
        self.source_access.module_for_source(source).ok()
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

    /// Returns a fixed-array length evaluated by declaration lowering.
    ///
    /// Body checking consumes this value instead of interpreting constant-expression syntax.
    #[must_use]
    pub fn constant_array_length(&self, expression: NodeId) -> Option<u64> {
        self.constant_array_lengths.get(&expression).copied()
    }

    #[must_use]
    pub const fn constant_array_lengths(&self) -> &HashMap<NodeId, u64> {
        &self.constant_array_lengths
    }

    #[must_use]
    pub fn with_block_imports(
        mut self,
        imports: impl IntoIterator<Item = (NodeId, ModuleId)>,
    ) -> Self {
        self.block_imports.extend(imports);
        self
    }
}

/// Sole construction authority for [`FrontendBindings`].
#[derive(Debug, Default)]
pub struct FrontendBindingsBuilder {
    module_sources: BTreeMap<ModuleId, Vec<SourceId>>,
    body_blocks: BTreeMap<BodyId, Vec<NodeId>>,
    parameter_declarations: BTreeMap<ParameterId, Vec<SyntaxToken>>,
    declarations: HashMap<SyntaxToken, Vec<FrontendDeclaration>>,
    source_namespaces: HashMap<SourceId, SourceNamespaceBuilder>,
    source_access: SourceAccessTableBuilder,
    constant_array_lengths: HashMap<NodeId, u64>,
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
        directly_included: impl IntoIterator<Item = SourceId>,
    ) {
        self.source_access.define_source(source, directly_included);
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

    pub fn define_constant_array_length(&mut self, expression: NodeId, length: u64) {
        self.constant_array_lengths.insert(expression, length);
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
            constant_array_lengths: self.constant_array_lengths,
            block_imports: HashMap::new(),
        }
    }
}
