//! Immutable syntax identities selected by declaration lowering for semantic checking.
//!
//! This contract deliberately contains neither display ranges nor documentation. Checking may
//! consume these exact selections, while `nocter-source-index` independently serves diagnostics
//! and editor features.

use std::collections::{BTreeMap, HashMap};

use nocter_model::{
    AssociatedTypeId, BodyId, CallableId, InterfaceId, ModuleId, NominalTypeId, ParameterId,
};
use nocter_source::SourceId;
use nocter_syntax::{NodeId, SyntaxToken};

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

    #[must_use]
    pub fn block_import(&self, declaration: NodeId) -> Option<ModuleId> {
        self.block_imports.get(&declaration).copied()
    }
}

/// Sole construction authority for [`FrontendBindings`].
#[derive(Debug, Default)]
pub struct FrontendBindingsBuilder {
    module_sources: BTreeMap<ModuleId, Vec<SourceId>>,
    body_blocks: BTreeMap<BodyId, Vec<NodeId>>,
    parameter_declarations: BTreeMap<ParameterId, Vec<SyntaxToken>>,
    declarations: HashMap<SyntaxToken, Vec<FrontendDeclaration>>,
    block_imports: HashMap<NodeId, ModuleId>,
}

impl FrontendBindingsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_module_source(&mut self, module: ModuleId, source: SourceId) {
        self.module_sources.entry(module).or_default().push(source);
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

    pub fn set_block_import(&mut self, declaration: NodeId, module: ModuleId) -> Option<ModuleId> {
        self.block_imports.insert(declaration, module)
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
            block_imports: self.block_imports,
        }
    }
}
