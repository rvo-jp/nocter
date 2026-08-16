mod access;
mod projection;
mod syntax;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    ExportedEntity, ImportDeclaration, ImportScope, ImportTarget, ImportedName, ProgramBuildError,
    Visibility,
};
use nocter_model::{ImportId, ModuleId, Symbol};
use nocter_source::SourceId;
use nocter_source_index::DuplicateSourceBinding;
use nocter_syntax::{NodeId, SyntaxToken};

use crate::visibility::{VisibilityResolutionError, resolve_authored};
use crate::{
    PreparedGenerics, ReservedEntity, SurfaceDeclarationId, SurfaceImportTarget, SurfaceSourceId,
};
use access::{module_index_by_id, module_index_by_identity, visibility_is_within, visible_from};
use projection::project_import;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportError {
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
    MissingSource(SurfaceSourceId),
    InvalidSyntax(NodeId),
    UnknownModule(NodeId),
    MissingImportedName(NodeId),
    InaccessibleImportedName(NodeId),
    WideningReexport(NodeId),
    InvalidLocalName(NodeId),
    DuplicateName { first: NodeId, second: NodeId },
    InvalidVisibility(NodeId),
    VisibilityAbovePackageRoot(NodeId),
    DependencyCycle(ModuleId),
    InconsistentSource(SourceId),
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
            Self::MissingSource(source) => {
                write!(formatter, "surface source {source:?} is missing")
            }
            Self::InvalidSyntax(declaration) => {
                write!(formatter, "import {declaration:?} has inconsistent syntax")
            }
            Self::UnknownModule(declaration) => {
                write!(formatter, "import {declaration:?} names an unknown module")
            }
            Self::MissingImportedName(declaration) => {
                write!(
                    formatter,
                    "import {declaration:?} names an unknown exported name"
                )
            }
            Self::InaccessibleImportedName(declaration) => write!(
                formatter,
                "import {declaration:?} cannot access its selected name"
            ),
            Self::WideningReexport(declaration) => write!(
                formatter,
                "re-export {declaration:?} is wider than its selected name"
            ),
            Self::InvalidLocalName(declaration) => write!(
                formatter,
                "import {declaration:?} introduces a reserved local name"
            ),
            Self::DuplicateName { first, second } => write!(
                formatter,
                "imports or declarations {first:?} and {second:?} introduce the same module name"
            ),
            Self::InvalidVisibility(declaration) => {
                write!(formatter, "import {declaration:?} has invalid visibility")
            }
            Self::VisibilityAbovePackageRoot(declaration) => write!(
                formatter,
                "import {declaration:?} moves visibility above its package root"
            ),
            Self::DependencyCycle(module) => {
                write!(
                    formatter,
                    "module import dependency cycle reaches {module:?}"
                )
            }
            Self::InconsistentSource(source) => {
                write!(formatter, "{source} has an inconsistent import origin")
            }
        }
    }
}

impl std::error::Error for ImportError {}

impl From<ProgramBuildError> for ImportError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DuplicateSourceBinding> for ImportError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

#[derive(Clone, Copy, Debug)]
struct NamespaceBinding {
    entity: ExportedEntity,
    visibility: Visibility,
    origin: NodeId,
}

type ModuleNamespace = Box<[(Symbol, NamespaceBinding)]>;

/// Generic scopes plus resolved authored module namespaces and semantic import declarations.
#[derive(Debug)]
pub struct PreparedImports<'syntax> {
    pub(crate) generics: PreparedGenerics<'syntax>,
    namespaces: Box<[ModuleNamespace]>,
    import_ids: Box<[Option<ImportId>]>,
}

impl PreparedImports<'_> {
    #[must_use]
    pub const fn generics(&self) -> &PreparedGenerics<'_> {
        &self.generics
    }

    #[must_use]
    pub fn lookup_local(&self, module: ModuleId, name: Symbol) -> Option<ExportedEntity> {
        let index = module_index_by_id(&self.generics.headers.reserved, module)?;
        lookup(&self.namespaces[index], name).map(|binding| binding.entity)
    }

    #[must_use]
    pub fn lookup_export(
        &self,
        from: ModuleId,
        module: ModuleId,
        name: Symbol,
    ) -> Option<ExportedEntity> {
        let index = module_index_by_id(&self.generics.headers.reserved, module)?;
        let binding = lookup(&self.namespaces[index], name)?;
        visible_from(
            &self.generics.headers.reserved,
            binding.visibility,
            from,
            module,
        )
        .then_some(binding.entity)
    }

    #[must_use]
    pub fn import_id(&self, surface_index: usize) -> Option<ImportId> {
        self.import_ids.get(surface_index).copied().flatten()
    }
}

/// Resolves authored module imports and re-exports after declaration and generic identities exist.
///
/// Same-module source imports do not enter the semantic import arena. Synthetic prelude fallback
/// is a later input-owned layer and is intentionally absent from this authored namespace pass.
///
/// # Errors
///
/// Returns [`ImportError`] for invalid visibility, missing or inaccessible selected names,
/// collisions, widening re-exports, inconsistent source projection, or an invalid dependency
/// graph.
pub fn prepare_authored_imports(
    mut generics: PreparedGenerics<'_>,
) -> Result<PreparedImports<'_>, ImportError> {
    let module_count = generics.headers.reserved.modules.len();
    let mut namespaces = vec![BTreeMap::new(); module_count];
    collect_direct_declarations(&generics, &mut namespaces)?;

    let groups = group_imports(&generics)?;
    let order = dependency_order(&generics, &groups.dependencies)?;
    let import_count = generics.headers.reserved.imports.len();
    let mut import_ids = vec![None; import_count];

    for module_index in order {
        for import_index in &groups.by_module[module_index] {
            let import = generics.headers.reserved.imports[*import_index].clone();
            let SurfaceImportTarget::Module(target_identity) = import.target() else {
                continue;
            };
            let target_index =
                module_index_by_identity(&generics.headers.reserved, target_identity)
                    .ok_or(ImportError::UnknownModule(import.node()))?;
            let target_module = generics.headers.reserved.module_ids[target_index];
            let source = generics
                .headers
                .reserved
                .sources
                .get(import.source().index())
                .ok_or(ImportError::MissingSource(import.source()))?;
            let tree = source.syntax();
            let authored = syntax::read(tree, import.node())?;
            let visibility = resolve_authored(
                &generics.headers.reserved,
                import.source(),
                authored.visibility,
            )
            .map_err(|error| import_visibility_error(import.node(), error))?;
            let importing_module = generics.headers.reserved.module_ids[module_index];

            let resolved = if let Some(selected) = authored.selected {
                resolve_selected(
                    &generics,
                    &namespaces[target_index],
                    import.node(),
                    importing_module,
                    target_module,
                    visibility,
                    selected,
                )?
            } else {
                let token = syntax::final_path_name(tree, import.node(), authored.path)?;
                let name = symbol(&generics, import.node(), token)?;
                validate_local_name(&generics, import.node(), name)?;
                ResolvedImport::Namespace {
                    local_name: name,
                    target: ExportedEntity::Module(target_module),
                }
            };
            validate_collisions(
                &namespaces[module_index],
                import.node(),
                resolved.bindings(),
            )?;

            let declaration = ImportDeclaration::new(
                ImportScope::Module(importing_module),
                visibility,
                resolved.target(target_module),
            );
            let id = generics.headers.reserved.program.add_import(declaration);
            import_ids[*import_index] = Some(id);
            project_import(
                &mut generics,
                import.node(),
                authored.path,
                id,
                target_module,
                &resolved,
            )?;
            for (name, entity) in resolved.bindings() {
                namespaces[module_index].insert(
                    name,
                    NamespaceBinding {
                        entity,
                        visibility,
                        origin: import.node(),
                    },
                );
            }
        }
    }

    Ok(PreparedImports {
        generics,
        namespaces: namespaces
            .into_iter()
            .map(|namespace| namespace.into_iter().collect::<Vec<_>>().into_boxed_slice())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        import_ids: import_ids.into_boxed_slice(),
    })
}

#[derive(Debug)]
enum ResolvedImport {
    Namespace {
        local_name: Symbol,
        target: ExportedEntity,
    },
    Selected(Vec<ResolvedSelected>),
}

#[derive(Clone, Copy, Debug)]
struct ResolvedSelected {
    exported_name: Symbol,
    local_name: Symbol,
    exported_token: SyntaxToken,
    local_token: SyntaxToken,
    target: ExportedEntity,
}

impl ResolvedImport {
    fn bindings(&self) -> Vec<(Symbol, ExportedEntity)> {
        match self {
            Self::Namespace { local_name, target } => vec![(*local_name, *target)],
            Self::Selected(names) => names
                .iter()
                .map(|name| (name.local_name, name.target))
                .collect(),
        }
    }

    fn target(&self, module: ModuleId) -> ImportTarget {
        match self {
            Self::Namespace { local_name, .. } => ImportTarget::Namespace {
                module,
                local_name: *local_name,
            },
            Self::Selected(names) => ImportTarget::Selected {
                module,
                names: names
                    .iter()
                    .map(|name| ImportedName::new(name.exported_name, name.local_name, name.target))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        }
    }
}

fn collect_direct_declarations(
    generics: &PreparedGenerics<'_>,
    namespaces: &mut [BTreeMap<Symbol, NamespaceBinding>],
) -> Result<(), ImportError> {
    let reserved = &generics.headers.reserved;
    for (index, declaration) in reserved.declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if declaration.owner().is_some() || reserved.contracts.representative(id) != id {
            continue;
        }
        let Some(name) = generics.headers.name(id) else {
            continue;
        };
        let Some(entity) = reserved.entity(id).and_then(exported_entity) else {
            continue;
        };
        let module = reserved
            .module_for_source(declaration.source())
            .ok_or(ImportError::MissingSource(declaration.source()))?;
        let module_index = module_index_by_id(reserved, module)
            .ok_or(ImportError::MissingSource(declaration.source()))?;
        let visibility = generics
            .headers
            .visibility(id)
            .ok_or(ImportError::InvalidSyntax(declaration.node()))?;
        if let Some(first) = namespaces[module_index].insert(
            name,
            NamespaceBinding {
                entity,
                visibility,
                origin: declaration.node(),
            },
        ) {
            return Err(ImportError::DuplicateName {
                first: first.origin,
                second: declaration.node(),
            });
        }
    }
    Ok(())
}

struct ImportGroups {
    by_module: Vec<Vec<usize>>,
    dependencies: Vec<BTreeSet<usize>>,
}

fn group_imports(generics: &PreparedGenerics<'_>) -> Result<ImportGroups, ImportError> {
    let reserved = &generics.headers.reserved;
    let mut grouped = vec![Vec::new(); reserved.modules.len()];
    let mut dependencies = vec![BTreeSet::new(); reserved.modules.len()];
    for (index, import) in reserved.imports.iter().enumerate() {
        let module = reserved
            .module_for_source(import.source())
            .ok_or(ImportError::MissingSource(import.source()))?;
        let module_index = module_index_by_id(reserved, module)
            .ok_or(ImportError::MissingSource(import.source()))?;
        grouped[module_index].push(index);
        if let SurfaceImportTarget::Module(target) = import.target() {
            let target_index = module_index_by_identity(reserved, target)
                .ok_or(ImportError::UnknownModule(import.node()))?;
            dependencies[module_index].insert(target_index);
        }
    }
    Ok(ImportGroups {
        by_module: grouped,
        dependencies,
    })
}

fn dependency_order(
    generics: &PreparedGenerics<'_>,
    dependencies: &[BTreeSet<usize>],
) -> Result<Vec<usize>, ImportError> {
    let mut remaining: Vec<_> = dependencies.iter().map(BTreeSet::len).collect();
    let mut dependents = vec![BTreeSet::new(); dependencies.len()];
    for (module, targets) in dependencies.iter().enumerate() {
        for target in targets {
            dependents[*target].insert(module);
        }
    }
    let mut ready: BTreeSet<_> = remaining
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect();
    let mut order = Vec::with_capacity(dependencies.len());
    while let Some(module) = ready.pop_first() {
        order.push(module);
        for dependent in &dependents[module] {
            remaining[*dependent] -= 1;
            if remaining[*dependent] == 0 {
                ready.insert(*dependent);
            }
        }
    }
    if order.len() != dependencies.len() {
        let index = remaining
            .iter()
            .position(|count| *count != 0)
            .expect("cyclic dependency has a remaining module");
        return Err(ImportError::DependencyCycle(
            generics.headers.reserved.module_ids[index],
        ));
    }
    Ok(order)
}

fn resolve_selected(
    generics: &PreparedGenerics<'_>,
    target_namespace: &BTreeMap<Symbol, NamespaceBinding>,
    declaration: NodeId,
    importing_module: ModuleId,
    target_module: ModuleId,
    visibility: Visibility,
    selected: Vec<syntax::SelectedNameSyntax>,
) -> Result<ResolvedImport, ImportError> {
    let mut resolved = Vec::with_capacity(selected.len());
    let mut local_names = BTreeSet::new();
    for selected in selected {
        let exported_name = symbol(generics, declaration, selected.exported)?;
        let local_name = symbol(generics, declaration, selected.local)?;
        validate_local_name(generics, declaration, local_name)?;
        if !local_names.insert(local_name) {
            return Err(ImportError::DuplicateName {
                first: declaration,
                second: declaration,
            });
        }
        let binding = target_namespace
            .get(&exported_name)
            .copied()
            .ok_or(ImportError::MissingImportedName(declaration))?;
        if !visible_from(
            &generics.headers.reserved,
            binding.visibility,
            importing_module,
            target_module,
        ) {
            return Err(ImportError::InaccessibleImportedName(declaration));
        }
        if visibility != Visibility::Private
            && !visibility_is_within(&generics.headers.reserved, visibility, binding.visibility)
        {
            return Err(ImportError::WideningReexport(declaration));
        }
        resolved.push(ResolvedSelected {
            exported_name,
            local_name,
            exported_token: selected.exported,
            local_token: selected.local,
            target: binding.entity,
        });
    }
    Ok(ResolvedImport::Selected(resolved))
}

fn validate_collisions(
    namespace: &BTreeMap<Symbol, NamespaceBinding>,
    declaration: NodeId,
    bindings: Vec<(Symbol, ExportedEntity)>,
) -> Result<(), ImportError> {
    let mut local = BTreeSet::new();
    for (name, _) in bindings {
        if !local.insert(name) {
            return Err(ImportError::DuplicateName {
                first: declaration,
                second: declaration,
            });
        }
        if let Some(first) = namespace.get(&name) {
            return Err(ImportError::DuplicateName {
                first: first.origin,
                second: declaration,
            });
        }
    }
    Ok(())
}

fn symbol(
    generics: &PreparedGenerics<'_>,
    declaration: NodeId,
    token: SyntaxToken,
) -> Result<Symbol, ImportError> {
    let source = generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .ok_or(ImportError::InconsistentSource(declaration.source()))?;
    let spelling = source
        .text_at(token.range())
        .ok_or(ImportError::InconsistentSource(declaration.source()))?;
    generics
        .headers
        .reserved
        .program
        .symbols()
        .get(spelling)
        .ok_or(ImportError::InvalidSyntax(declaration))
}

fn validate_local_name(
    generics: &PreparedGenerics<'_>,
    declaration: NodeId,
    name: Symbol,
) -> Result<(), ImportError> {
    let spelling = generics
        .headers
        .reserved
        .program
        .symbols()
        .spelling(name)
        .ok_or(ImportError::InvalidSyntax(declaration))?;
    if nocter_syntax::BuiltinType::from_spelling(spelling).is_some() || spelling == "Self" {
        Err(ImportError::InvalidLocalName(declaration))
    } else {
        Ok(())
    }
}

const fn exported_entity(entity: ReservedEntity) -> Option<ExportedEntity> {
    match entity {
        ReservedEntity::NominalType(id) => Some(ExportedEntity::NominalType(id)),
        ReservedEntity::TypeAlias(id) => Some(ExportedEntity::TypeAlias(id)),
        ReservedEntity::Interface(id) => Some(ExportedEntity::Interface(id)),
        ReservedEntity::Callable(id) => Some(ExportedEntity::Callable(id)),
        ReservedEntity::AssociatedType(_)
        | ReservedEntity::Construction(_)
        | ReservedEntity::Instance(_)
        | ReservedEntity::Conformance(_)
        | ReservedEntity::Drop(_)
        | ReservedEntity::Test(_)
        | ReservedEntity::Variant(_)
        | ReservedEntity::OpaqueType(_) => None,
    }
}

fn import_visibility_error(declaration: NodeId, error: VisibilityResolutionError) -> ImportError {
    match error {
        VisibilityResolutionError::MissingSource(source) => ImportError::MissingSource(source),
        VisibilityResolutionError::Invalid(_) => ImportError::InvalidVisibility(declaration),
        VisibilityResolutionError::AbovePackageRoot(_) => {
            ImportError::VisibilityAbovePackageRoot(declaration)
        }
    }
}

fn lookup(namespace: &ModuleNamespace, name: Symbol) -> Option<NamespaceBinding> {
    namespace
        .binary_search_by_key(&name, |(candidate, _)| *candidate)
        .ok()
        .map(|index| namespace[index].1)
}

#[cfg(test)]
mod tests;
