//! Source-neutral header reuse retains its original semantic authority and binds fresh physical views.
use super::{ModuleContext, ModuleSourcesFailure, ScopedModuleSources, syntax};
use crate::access_declarations::{BodySources, Failure, ModuleSources};
use nocter_compile_input::{CompileUnitInput, ModuleIdentity, ModuleSourceKind};
use nocter_declarations::value_access::Program;
use nocter_model::CompilationTarget;
use nocter_syntax::{DeclarationSyntaxSurface, project_declaration_syntax};
use std::sync::Arc;

/// Reusable complete headers and source-dependency guards, without a current syntax tree.
/// Header reuse grants neither body acceptance nor evaluated constant/static values.
#[derive(Clone, Debug)]
pub struct ModuleQuery {
    program: Arc<Program>,
    target: CompilationTarget,
    sources: Box<[SourceRecipe]>,
    dependencies: BodySources,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceRecipe {
    module: ModuleIdentity,
    path: Box<str>,
    kind: ModuleSourceKind,
    surface: DeclarationSyntaxSurface,
}
impl ModuleContext<'_> {
    /// Capture reusable headers after actual required-header source guards have been retained.
    /// # Errors
    /// Rejects a count-dependent snapshot that was sealed without the required source proof.
    pub fn query(&self) -> Result<ModuleQuery, ModuleSourcesFailure> {
        require_header_sources(&self.binding, self.dependencies.as_ref())?;
        Ok(ModuleQuery {
            program: Arc::clone(&self.program),
            target: self.binding.paired.inventory.surface.target(),
            sources: source_recipes(&self.binding)?,
            dependencies: self.dependencies.clone().unwrap_or_default(),
        })
    }
}
impl ModuleQuery {
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Validate current selected topology/names/headers and bind the retained authority to it.
    /// A changed source inventory, header, name domain or required supplier invalidates reuse.
    /// # Errors
    /// Reports current source-selection errors or `ChangedHeader` without replacing retained facts.
    pub fn bind<'syntax>(
        &self,
        input: &CompileUnitInput<'syntax>,
    ) -> Result<ModuleContext<'syntax>, ModuleSourcesFailure> {
        let current = ModuleSources::prepare(input)?.pair()?.reserve_names()?;
        if current.binding.paired.inventory.surface.target() != self.target
            || source_recipes(&current.binding)? != self.sources
            || !current.headers.has_same_reserved_layout(&self.program)
        {
            return Err(ModuleSourcesFailure::Access(Failure::ChangedHeader));
        }
        let bound = ModuleContext {
            program: Arc::clone(&self.program),
            binding: current.binding,
            dependencies: Some(self.dependencies.clone()),
        };
        self.dependencies
            .verify(&bound.current()?)
            .map_err(|_| ModuleSourcesFailure::Access(Failure::ChangedHeader))?;
        Ok(bound)
    }
}
impl<'syntax> ScopedModuleSources<'syntax> {
    /// Seal current headers and attach exact required-value source guards for later reuse.
    /// # Errors
    /// Rejects missing count-owner guards, invalid headers or guards from another source authority.
    pub fn seal_with_dependencies(
        self,
        dependencies: BodySources,
    ) -> Result<ModuleContext<'syntax>, ModuleSourcesFailure> {
        require_header_sources(&self.binding, Some(&dependencies))?;
        let mut context = self.seal()?;
        dependencies
            .verify(&context.current()?)
            .map_err(|_| ModuleSourcesFailure::Access(Failure::ChangedHeader))?;
        context.dependencies = Some(dependencies);
        Ok(context)
    }
}
fn source_recipes(
    binding: &super::ModuleSourceBinding<'_>,
) -> Result<Box<[SourceRecipe]>, ModuleSourcesFailure> {
    binding
        .paired
        .inventory
        .source_entries()
        .map(|(source, physical)| {
            let syntax = syntax(&binding.paired, source)?;
            Ok(SourceRecipe {
                module: physical.module().clone(),
                path: physical.canonical_path().into(),
                kind: physical.kind(),
                surface: project_declaration_syntax(syntax).surface().clone(),
            })
        })
        .collect()
}

fn require_header_sources(
    binding: &super::ModuleSourceBinding<'_>,
    dependencies: Option<&BodySources>,
) -> Result<(), ModuleSourcesFailure> {
    if binding
        .catalog
        .sources
        .expression_ids()
        .any(|id| dependencies.is_none_or(|sources| !sources.contains(id)))
    {
        Err(ModuleSourcesFailure::Access(Failure::MissingValueEvidence))
    } else {
        Ok(())
    }
}
