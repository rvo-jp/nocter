use std::collections::{HashMap, HashSet};

use nocter_frontend_bindings::{DuplicateBlockImport, FrontendBindings, FrontendDeclaration};
use nocter_model::{
    AssociatedTypeId, BodyId, DeclarationSiteId, ModuleId, NominalTypeId, ParameterId, TypeId,
};
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};
use nocter_syntax::{NodeId, SyntaxToken};

use crate::SurfaceSource;
use crate::projection_recipe::{
    DocumentationSite, FrontendProjectionRecipe, ProjectionRecipeBuilder, ProjectionRecipeError,
};

/// Sole declaration-lowering write path for a reusable projection recipe.
///
/// Current-generation syntax is converted to stable surface locators immediately. `finish`
/// materializes `SourceIndex` and `FrontendBindings` from that recipe; neither current-generation
/// product is maintained as a parallel authority.
#[derive(Debug)]
pub(crate) struct FrontendProjectionBuilder {
    recipe: ProjectionRecipeBuilder,
    error: Option<ProjectionRecipeError>,
    associated_references: HashSet<(AssociatedTypeId, nocter_syntax::SyntaxOrigin)>,
    block_imports: HashMap<NodeId, ModuleId>,
    binding_count: usize,
}

impl FrontendProjectionBuilder {
    pub(crate) fn new(
        source_map: &SourceMap,
        sources: &[SurfaceSource<'_>],
    ) -> Result<Self, ProjectionRecipeError> {
        Ok(Self {
            recipe: ProjectionRecipeBuilder::new(source_map, sources)?,
            error: None,
            associated_references: HashSet::new(),
            block_imports: HashMap::new(),
            binding_count: 0,
        })
    }

    pub(crate) const fn len(&self) -> usize {
        self.binding_count
    }

    fn retain(&mut self, result: Result<(), ProjectionRecipeError>) {
        if self.error.is_none()
            && let Err(error) = result
        {
            self.error = Some(error);
        }
    }

    pub(crate) fn insert(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
    ) {
        self.binding_count += 1;
        let result = self.recipe.binding(entity, role, origin);
        self.retain(result);
    }

    pub(crate) fn insert_module_source(
        &mut self,
        module: ModuleId,
        source: SourceId,
        role: SourceRole,
        origin: SourceOrigin,
    ) {
        self.binding_count += 1;
        let result = self.recipe.module_source(module, source, role, origin);
        self.retain(result);
    }

    pub(crate) fn insert_body(
        &mut self,
        body: BodyId,
        block: NodeId,
        role: SourceRole,
        origin: SourceOrigin,
    ) {
        self.binding_count += 1;
        let result = self.recipe.body(body, block, role, origin);
        self.retain(result);
    }

    pub(crate) fn insert_parameter(
        &mut self,
        parameter: ParameterId,
        declaration: SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) {
        self.binding_count += 1;
        let result = self.recipe.parameter(parameter, declaration, role, origin);
        self.retain(result);
    }

    pub(crate) fn insert_declaration(
        &mut self,
        declaration: FrontendDeclaration,
        token: SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) {
        self.binding_count += 1;
        let result = self.recipe.declaration(declaration, token, role, origin);
        self.retain(result);
    }

    pub(crate) fn insert_associated_projection_use(
        &mut self,
        base: TypeId,
        associated: AssociatedTypeId,
        syntax: nocter_syntax::SyntaxOrigin,
        origin: SourceOrigin,
    ) {
        if self.associated_references.insert((associated, syntax)) {
            self.binding_count += 1;
            let result = self
                .recipe
                .associated_projection(base, associated, syntax, origin);
            self.retain(result);
        }
    }

    pub(crate) fn insert_block_import(
        &mut self,
        declaration: NodeId,
        target: ModuleId,
    ) -> Result<(), DuplicateBlockImport> {
        match self.block_imports.entry(declaration) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(target);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                Err(DuplicateBlockImport::new(declaration, *entry.get(), target))
            }
        }
    }

    pub(crate) fn define_declaration_site_source(
        &mut self,
        site: DeclarationSiteId,
        source: SourceId,
    ) {
        let result = self.recipe.declaration_site_source(site, source);
        self.retain(result);
    }

    pub(crate) fn define_nominal_representation_source(
        &mut self,
        nominal: NominalTypeId,
        source: SourceId,
        contract_private: bool,
    ) {
        let result = self
            .recipe
            .nominal_representation_source(nominal, source, contract_private);
        self.retain(result);
    }

    pub(crate) fn insert_documentation(&mut self, entity: SemanticEntity, site: DocumentationSite) {
        let result = self.recipe.documentation(entity, site);
        self.retain(result);
    }

    pub(crate) fn insert_occurrence_documentation(
        &mut self,
        entity: SemanticEntity,
        origin: SourceOrigin,
        documented_node: NodeId,
    ) {
        let result = self
            .recipe
            .occurrence_documentation(entity, origin, documented_node);
        self.retain(result);
    }

    pub(crate) fn define_source_namespace(
        &mut self,
        source: SourceId,
        authored: impl IntoIterator<Item = (nocter_model::Symbol, nocter_declarations::ExportedEntity)>,
        fallback: impl IntoIterator<Item = (nocter_model::Symbol, nocter_declarations::ExportedEntity)>,
    ) {
        let result = self.recipe.source_namespace(
            source,
            authored.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            fallback.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        );
        self.retain(result);
    }

    pub(crate) fn define_source_access(
        &mut self,
        source: SourceId,
        directly_visible: impl IntoIterator<Item = SourceId>,
    ) {
        let directly_visible = directly_visible.into_iter().collect::<Vec<_>>();
        let result = self.recipe.source_access(source, &directly_visible);
        self.retain(result);
    }

    pub(crate) fn finish(
        self,
        source_map: &SourceMap,
        sources: &[SurfaceSource<'_>],
    ) -> Result<(FrontendProjectionRecipe, SourceIndex, FrontendBindings), ProjectionRecipeError>
    {
        if let Some(error) = self.error {
            return Err(error);
        }
        let recipe = self.recipe.finish();
        let (source_index, bindings) =
            recipe.materialize(source_map, sources, &self.block_imports)?;
        Ok((recipe, source_index, bindings))
    }
}
