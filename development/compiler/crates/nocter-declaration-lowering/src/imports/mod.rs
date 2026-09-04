mod access;
mod prelude;
mod projection;
mod syntax;
mod violation;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    ExportedEntity, ImportDeclaration, ImportTarget, ImportedName, ProgramBuildError, Visibility,
};
use nocter_model::{ImportId, ModuleId, Symbol};
use nocter_source::SourceId;
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};

use crate::visibility::{VisibilityResolutionError, resolve_authored};
use crate::{
    NamespaceViolation, PreparedGenerics, ReservedEntity, SurfaceDeclarationId, SurfaceSourceId,
};
use access::{module_index_by_id, module_index_by_identity, visibility_is_within, visible_from};
use projection::project_import;

pub use prelude::PreparedNamespaces;
pub(crate) use prelude::{NamespaceDefinitionError, apply_toolchain_profile};
pub use violation::{ImportRule, ImportViolation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportError {
    Rule(ImportViolation),
    Namespace(NamespaceViolation),
    Program(ProgramBuildError),
    MissingSource(SurfaceSourceId),
    InvalidSyntax(NodeId),
    UnknownModule(NodeId),
    InvalidVisibility(NodeId),
    DependencyCycle(ModuleId),
    InconsistentSource(SourceId),
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::Namespace(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::Program(error) => error.fmt(formatter),
            Self::MissingSource(source) => {
                write!(formatter, "surface source {source:?} is missing")
            }
            Self::InvalidSyntax(declaration) => {
                write!(formatter, "import {declaration:?} has inconsistent syntax")
            }
            Self::UnknownModule(declaration) => {
                write!(formatter, "import {declaration:?} names an unknown module")
            }
            Self::InvalidVisibility(declaration) => {
                write!(formatter, "import {declaration:?} has invalid visibility")
            }
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

impl From<ImportViolation> for ImportError {
    fn from(violation: ImportViolation) -> Self {
        Self::Rule(violation)
    }
}

impl From<NamespaceViolation> for ImportError {
    fn from(violation: NamespaceViolation) -> Self {
        Self::Namespace(violation)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NamespaceBinding {
    pub(super) entity: ExportedEntity,
    pub(super) visibility: Visibility,
    origin: SyntaxOrigin,
}

pub(super) type ModuleNamespace = Box<[(Symbol, NamespaceBinding)]>;

/// Generic scopes plus resolved authored module namespaces and semantic import declarations.
#[derive(Debug)]
pub struct PreparedImports<'syntax> {
    pub(crate) generics: PreparedGenerics<'syntax>,
    pub(super) namespaces: Box<[ModuleNamespace]>,
    pub(super) source_namespaces: Box<[ModuleNamespace]>,
    import_ids: Box<[Option<ImportId>]>,
    import_paths: Box<[Option<SyntaxOrigin>]>,
}

impl PreparedImports<'_> {
    #[must_use]
    pub const fn generics(&self) -> &PreparedGenerics<'_> {
        &self.generics
    }

    #[must_use]
    pub fn lookup_local(&self, source: SurfaceSourceId, name: Symbol) -> Option<ExportedEntity> {
        lookup(self.source_namespaces.get(source.index())?, name).map(|binding| binding.entity)
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

    pub(super) fn import_path(&self, surface_index: usize) -> Option<SyntaxOrigin> {
        self.import_paths.get(surface_index).copied().flatten()
    }
}

/// Resolves authored module imports and re-exports after declaration and generic identities exist.
///
/// Source-visibility edges do not enter the semantic import arena. Synthetic prelude fallback is a later
/// input-owned layer and is intentionally absent from this authored namespace pass.
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
    let mut source_namespaces = vec![BTreeMap::new(); generics.headers.reserved.sources.len()];
    collect_direct_declarations(&generics, &mut namespaces, &mut source_namespaces)?;
    apply_source_visibility(&generics, &mut source_namespaces)?;

    let import_count = generics.headers.reserved.imports.len();
    let mut import_ids = vec![None; import_count];
    let mut import_paths = vec![None; import_count];
    resolve_authored_imports(
        &mut generics,
        &mut namespaces,
        &mut source_namespaces,
        &mut import_ids,
        &mut import_paths,
    )?;

    Ok(PreparedImports {
        generics,
        namespaces: namespaces
            .into_iter()
            .map(|namespace| namespace.into_iter().collect::<Vec<_>>().into_boxed_slice())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        source_namespaces: source_namespaces
            .into_iter()
            .map(|namespace| namespace.into_iter().collect::<Vec<_>>().into_boxed_slice())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        import_ids: import_ids.into_boxed_slice(),
        import_paths: import_paths.into_boxed_slice(),
    })
}

fn resolve_authored_imports(
    generics: &mut PreparedGenerics<'_>,
    namespaces: &mut [BTreeMap<Symbol, NamespaceBinding>],
    source_namespaces: &mut [BTreeMap<Symbol, NamespaceBinding>],
    import_ids: &mut [Option<ImportId>],
    import_paths: &mut [Option<SyntaxOrigin>],
) -> Result<(), ImportError> {
    let groups = group_imports(generics)?;
    let order = dependency_order(generics, &groups.dependencies)?;
    for module_index in order {
        for import_index in &groups.by_module[module_index] {
            let import = generics.headers.reserved.imports[*import_index].clone();
            let target_index =
                module_index_by_identity(&generics.headers.reserved, import.target())
                    .ok_or(ImportError::UnknownModule(import.node()))?;
            let target_module = generics.headers.reserved.module_ids[target_index];
            let source = generics
                .headers
                .reserved
                .sources
                .get(import.source().index())
                .ok_or(ImportError::MissingSource(import.source()))?;
            let source_kind = source.kind();
            let tree = source.syntax();
            let authored = syntax::read(tree, import.node())?;
            import_paths[*import_index] = Some(SyntaxOrigin::Node(authored.path));
            let visibility = resolve_authored(
                &generics.headers.reserved,
                import.source(),
                authored.visibility,
            )
            .map_err(|error| import_visibility_error(import.node(), error))?;
            if authored.visibility.is_some()
                && let Some(alias) = authored.namespace_alias
            {
                return Err(
                    ImportViolation::namespace_alias_reexport(SyntaxOrigin::Token(alias)).into(),
                );
            }
            let importing_module = generics.headers.reserved.module_ids[module_index];
            let source_index = import.source().index();
            let resolved = if let Some(selected) = authored.selected {
                resolve_selected(
                    generics,
                    &namespaces[target_index],
                    import.node(),
                    ImportAccess {
                        importing_module,
                        target_module,
                        visibility,
                        authored_visibility: authored.visibility,
                    },
                    selected,
                )?
            } else {
                let token = match authored.namespace_alias {
                    Some(alias) => alias,
                    None => syntax::final_path_name(tree, import.node(), authored.path)?,
                };
                let name = symbol(generics, import.node(), token)?;
                validate_local_name(generics, import.node(), name, token)?;
                ResolvedImport::Namespace {
                    local_name: name,
                    local_token: token,
                    aliased: authored.namespace_alias.is_some(),
                    target: ExportedEntity::Module(target_module),
                }
            };
            validate_collisions(&source_namespaces[source_index], resolved.bindings())?;
            let declaration = ImportDeclaration::new(
                importing_module,
                visibility,
                resolved.target(target_module),
            );
            let id = generics.headers.reserved.program.add_import(declaration);
            import_ids[*import_index] = Some(id);
            project_import(
                generics,
                import.node(),
                authored.path,
                id,
                target_module,
                &resolved,
            )?;
            insert_resolved_bindings(
                &resolved,
                visibility,
                authored.visibility,
                &mut BindingDestination {
                    source_kind,
                    module_index,
                    source_index,
                    modules: namespaces,
                    sources: source_namespaces,
                },
            );
        }
    }
    Ok(())
}

struct BindingDestination<'namespace> {
    source_kind: crate::ModuleSourceKind,
    module_index: usize,
    source_index: usize,
    modules: &'namespace mut [BTreeMap<Symbol, NamespaceBinding>],
    sources: &'namespace mut [BTreeMap<Symbol, NamespaceBinding>],
}

fn insert_resolved_bindings(
    resolved: &ResolvedImport,
    visibility: Visibility,
    authored_visibility: Option<NodeId>,
    destination: &mut BindingDestination<'_>,
) {
    for (name, entity, origin) in resolved.bindings() {
        let binding = NamespaceBinding {
            entity,
            visibility,
            origin: SyntaxOrigin::Token(origin),
        };
        destination.sources[destination.source_index].insert(name, binding);
        if authored_visibility.is_some()
            && matches!(
                destination.source_kind,
                crate::ModuleSourceKind::Root | crate::ModuleSourceKind::SingleFile
            )
        {
            destination.modules[destination.module_index].insert(name, binding);
        }
    }
}

#[derive(Debug)]
enum ResolvedImport {
    Namespace {
        local_name: Symbol,
        local_token: SyntaxToken,
        aliased: bool,
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

#[derive(Clone, Copy, Debug)]
struct ImportAccess {
    importing_module: ModuleId,
    target_module: ModuleId,
    visibility: Visibility,
    authored_visibility: Option<NodeId>,
}

impl ResolvedImport {
    fn bindings(&self) -> Vec<(Symbol, ExportedEntity, SyntaxToken)> {
        match self {
            Self::Namespace {
                local_name,
                local_token,
                aliased: _,
                target,
            } => vec![(*local_name, *target, *local_token)],
            Self::Selected(names) => names
                .iter()
                .map(|name| (name.local_name, name.target, name.local_token))
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
    source_namespaces: &mut [BTreeMap<Symbol, NamespaceBinding>],
) -> Result<(), ImportError> {
    let reserved = &generics.headers.reserved;
    for (index, declaration) in reserved.declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if declaration.owner().is_some() {
            continue;
        }
        let Some(name) = generics.headers.name(id) else {
            continue;
        };
        let representative = reserved.contracts.representative(id);
        let Some(entity) = reserved.entity(representative).and_then(exported_entity) else {
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
        let binding = NamespaceBinding {
            entity,
            visibility,
            origin: SyntaxOrigin::Token(
                declaration
                    .name()
                    .ok_or(ImportError::InvalidSyntax(declaration.node()))?,
            ),
        };
        insert_binding(
            &mut source_namespaces[declaration.source().index()],
            name,
            binding,
        )?;
        let source = reserved
            .sources
            .get(declaration.source().index())
            .ok_or(ImportError::MissingSource(declaration.source()))?;
        if representative != id
            || !matches!(
                source.kind(),
                crate::ModuleSourceKind::Root | crate::ModuleSourceKind::SingleFile
            )
        {
            continue;
        }
        if let Some(first) = namespaces[module_index].insert(name, binding) {
            let second = declaration
                .name()
                .ok_or(ImportError::InvalidSyntax(declaration.node()))?;
            return Err(NamespaceViolation::name_collision(
                first.origin,
                SyntaxOrigin::Token(second),
            )
            .into());
        }
    }
    Ok(())
}

fn apply_source_visibility(
    generics: &PreparedGenerics<'_>,
    namespaces: &mut [BTreeMap<Symbol, NamespaceBinding>],
) -> Result<(), ImportError> {
    let direct = namespaces.to_vec();
    for see in generics
        .headers
        .reserved
        .source_visibilities
        .iter()
        .copied()
    {
        let target = direct
            .get(see.target().index())
            .ok_or(ImportError::MissingSource(see.target()))?;
        let destination = namespaces
            .get_mut(see.source().index())
            .ok_or(ImportError::MissingSource(see.source()))?;
        for (name, binding) in target {
            insert_binding(destination, *name, *binding)?;
        }
    }
    Ok(())
}

fn insert_binding(
    namespace: &mut BTreeMap<Symbol, NamespaceBinding>,
    name: Symbol,
    binding: NamespaceBinding,
) -> Result<(), ImportError> {
    if let Some(first) = namespace.get(&name).copied() {
        if first.entity == binding.entity {
            return Ok(());
        }
        return Err(NamespaceViolation::name_collision(first.origin, binding.origin).into());
    }
    namespace.insert(name, binding);
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
        let target_index = module_index_by_identity(reserved, import.target())
            .ok_or(ImportError::UnknownModule(import.node()))?;
        dependencies[module_index].insert(target_index);
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
    access: ImportAccess,
    selected: Vec<syntax::SelectedNameSyntax>,
) -> Result<ResolvedImport, ImportError> {
    let mut resolved = Vec::with_capacity(selected.len());
    let mut local_names = BTreeMap::new();
    for selected in selected {
        let exported_name = symbol(generics, declaration, selected.exported)?;
        let local_name = symbol(generics, declaration, selected.local)?;
        validate_local_name(generics, declaration, local_name, selected.local)?;
        if let Some(first) = local_names.insert(local_name, selected.local) {
            return Err(NamespaceViolation::name_collision(
                SyntaxOrigin::Token(first),
                SyntaxOrigin::Token(selected.local),
            )
            .into());
        }
        let binding = target_namespace
            .get(&exported_name)
            .copied()
            .ok_or_else(|| {
                ImportViolation::missing_imported_name(SyntaxOrigin::Token(selected.exported))
            })?;
        if !visible_from(
            &generics.headers.reserved,
            binding.visibility,
            access.importing_module,
            access.target_module,
        ) {
            return Err(ImportViolation::inaccessible_imported_name(
                SyntaxOrigin::Token(selected.exported),
                binding.origin,
            )
            .into());
        }
        if !binding.entity.is_selectable_type() {
            return Err(ImportViolation::non_type_selection(SyntaxOrigin::Token(
                selected.exported,
            ))
            .into());
        }
        if access.visibility != Visibility::Private
            && !visibility_is_within(
                &generics.headers.reserved,
                access.visibility,
                binding.visibility,
            )
        {
            let visibility = access
                .authored_visibility
                .ok_or(ImportError::InvalidVisibility(declaration))?;
            return Err(ImportViolation::widening_reexport(
                SyntaxOrigin::Node(visibility),
                binding.origin,
            )
            .into());
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
    bindings: Vec<(Symbol, ExportedEntity, SyntaxToken)>,
) -> Result<(), ImportError> {
    let mut local = BTreeMap::new();
    for (name, _, token) in bindings {
        let origin = SyntaxOrigin::Token(token);
        if let Some(first) = local.insert(name, origin) {
            return Err(NamespaceViolation::name_collision(first, origin).into());
        }
        if let Some(first) = namespace.get(&name) {
            return Err(NamespaceViolation::name_collision(first.origin, origin).into());
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
    token: SyntaxToken,
) -> Result<(), ImportError> {
    let spelling = generics
        .headers
        .reserved
        .program
        .symbols()
        .spelling(name)
        .ok_or(ImportError::InvalidSyntax(declaration))?;
    if nocter_syntax::BuiltinType::from_spelling(spelling).is_some() || spelling == "Self" {
        Err(NamespaceViolation::reserved_name(SyntaxOrigin::Token(token)).into())
    } else {
        Ok(())
    }
}

const fn exported_entity(entity: ReservedEntity) -> Option<ExportedEntity> {
    match entity {
        ReservedEntity::BuiltinType(builtin) => Some(ExportedEntity::BuiltinType(builtin)),
        ReservedEntity::NominalType(id) => Some(ExportedEntity::NominalType(id)),
        ReservedEntity::TypeAlias(id) => Some(ExportedEntity::TypeAlias(id)),
        ReservedEntity::Interface(id) => Some(ExportedEntity::Interface(id)),
        ReservedEntity::Constant(id) => Some(ExportedEntity::Constant(id)),
        ReservedEntity::Static(id) => Some(ExportedEntity::Static(id)),
        ReservedEntity::Callable(id) => Some(ExportedEntity::Callable(id)),
        ReservedEntity::AssociatedType(_)
        | ReservedEntity::Construction(_)
        | ReservedEntity::Instance(_)
        | ReservedEntity::InterfaceImplementation(_)
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
        VisibilityResolutionError::AbovePackageRoot(node) => {
            NamespaceViolation::visibility_above_package_root(SyntaxOrigin::Node(node)).into()
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
