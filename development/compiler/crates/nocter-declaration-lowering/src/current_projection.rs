use std::collections::{BTreeMap, HashMap};
use std::fmt;

use nocter_compile_input::{CompileUnitInput, ModuleSourceKind};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::ModuleId;
use nocter_source_index::SourceIndex;
use nocter_syntax::{NodeId, NodeKind, SyntaxNode, descendant_node_iter};
use nocter_target_selection::TargetSelectionError;

use crate::{ModuleIdentity, ProjectionRecipeError, ReusableDeclarations, SurfaceSource};

/// Current-generation source projection materialized from one reusable declaration result.
#[derive(Debug)]
pub struct CurrentDeclarationProjection {
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
    checking_symbols: crate::current_symbols::CurrentCheckingSymbols,
}

impl CurrentDeclarationProjection {
    #[must_use]
    pub const fn frontend_bindings(&self) -> &FrontendBindings {
        &self.frontend_bindings
    }

    #[must_use]
    pub const fn checking_symbols(&self) -> &crate::CurrentCheckingSymbols {
        &self.checking_symbols
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        FrontendBindings,
        SourceIndex,
        crate::current_symbols::CurrentCheckingSymbols,
    ) {
        (
            self.frontend_bindings,
            self.source_index,
            self.checking_symbols,
        )
    }
}

impl ReusableDeclarations {
    /// Rebinds declaration identities into one current compile input without repeating declaration
    /// collection, topology validation, or semantic lowering.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the current source domain differs from the recipe or when
    /// discovery's body-import projection is incomplete or inconsistent.
    pub fn materialize_projection(
        &self,
        input: &CompileUnitInput<'_>,
    ) -> Result<CurrentDeclarationProjection, CurrentProjectionError> {
        let sources = canonical_sources(input);
        let checking_symbols = crate::current_symbols::CurrentCheckingSymbols::from_sources(
            input.sources(),
            &sources,
        )?;
        let block_imports = current_block_imports(input, self, &sources)?;
        let (source_index, frontend_bindings) =
            self.projection_recipe()
                .materialize(input.sources(), &sources, &block_imports)?;
        Ok(CurrentDeclarationProjection {
            frontend_bindings,
            source_index,
            checking_symbols,
        })
    }
}

pub(crate) fn canonical_sources<'syntax>(
    input: &CompileUnitInput<'syntax>,
) -> Vec<SurfaceSource<'syntax>> {
    let mut modules = input.modules().iter().collect::<Vec<_>>();
    modules.sort_unstable_by_key(|module| module.identity());
    let mut sources = Vec::new();
    for module in modules {
        let mut module_sources = module.sources().iter().collect::<Vec<_>>();
        module_sources.sort_unstable_by(|left, right| {
            source_kind_rank(left.kind())
                .cmp(&source_kind_rank(right.kind()))
                .then_with(|| left.canonical_path().cmp(right.canonical_path()))
        });
        sources.extend(module_sources.into_iter().map(|source| {
            SurfaceSource::new(
                module.identity().clone(),
                source.canonical_path(),
                source.kind(),
                source.syntax(),
            )
        }));
    }
    sources
}

fn current_block_imports(
    input: &CompileUnitInput<'_>,
    declarations: &ReusableDeclarations,
    sources: &[SurfaceSource<'_>],
) -> Result<HashMap<NodeId, ModuleId>, CurrentProjectionError> {
    let selection = input
        .target_selection()
        .map_err(CurrentProjectionError::TargetSelection)?;
    let mut imports = HashMap::new();
    let mut resolved = BTreeMap::new();
    for resolution in input.use_resolutions() {
        let declaration = resolution.declaration();
        if !selection.use_is_active(declaration) {
            continue;
        }
        let tree = input
            .syntax_tree(declaration.source())
            .ok_or(CurrentProjectionError::InvalidUseResolution(declaration))?;
        match tree.node(declaration).map(SyntaxNode::kind) {
            Some(NodeKind::UseDeclaration) => {}
            Some(NodeKind::BlockUseDeclaration) => {
                let key = (declaration.source(), declaration.index());
                if resolved.insert(key, declaration).is_some() {
                    return Err(CurrentProjectionError::DuplicateUseResolution(declaration));
                }
                let target = declarations
                    .module_binding(resolution.target_module())
                    .ok_or_else(|| {
                        CurrentProjectionError::UnknownModule(resolution.target_module().clone())
                    })?;
                imports.insert(declaration, target);
            }
            _ => return Err(CurrentProjectionError::InvalidUseResolution(declaration)),
        }
    }
    for source in sources {
        for declaration in descendant_node_iter(source.syntax(), source.syntax().root_id()) {
            if source.syntax().node(declaration).map(SyntaxNode::kind)
                == Some(NodeKind::BlockUseDeclaration)
                && selection.use_is_active(declaration)
                && !resolved.contains_key(&(declaration.source(), declaration.index()))
            {
                return Err(CurrentProjectionError::MissingUseResolution(declaration));
            }
        }
    }
    Ok(imports)
}

const fn source_kind_rank(kind: ModuleSourceKind) -> u8 {
    match kind {
        ModuleSourceKind::Root => 0,
        ModuleSourceKind::SingleFile => 1,
        ModuleSourceKind::Implementation => 2,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentProjectionError {
    Projection(ProjectionRecipeError),
    TargetSelection(TargetSelectionError),
    InvalidUseResolution(NodeId),
    DuplicateUseResolution(NodeId),
    MissingUseResolution(NodeId),
    UnknownModule(ModuleIdentity),
    CurrentSymbols(crate::CurrentSymbolError),
}

impl fmt::Display for CurrentProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current declaration projection: {self:?}"
        )
    }
}

impl std::error::Error for CurrentProjectionError {}

impl From<ProjectionRecipeError> for CurrentProjectionError {
    fn from(error: ProjectionRecipeError) -> Self {
        Self::Projection(error)
    }
}

impl From<crate::CurrentSymbolError> for CurrentProjectionError {
    fn from(error: crate::CurrentSymbolError) -> Self {
        Self::CurrentSymbols(error)
    }
}
