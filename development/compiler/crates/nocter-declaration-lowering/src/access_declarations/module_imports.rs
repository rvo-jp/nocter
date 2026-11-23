//! Authored export/import policy prepares bindings; semantic names remain in Program's namespace.
use super::module_scopes::{integrity, symbol, syntax};
use super::{Failure, ModuleAudience, ModuleSourcesFailure, PairedModuleSources};
use crate::{
    ImportDiagnostic, ImportViolation, ModuleIdentity, NamespaceDiagnostic, NamespaceViolation,
    SurfaceDeclarationId, SurfaceSourceId,
};
use nocter_declarations::value_access::{DeclarationId, Namespace, Program, Reservations, ScopeId};
use nocter_model::{BuiltinType, Symbol};
use nocter_syntax::{ContextualSpelling, NodeId, SyntaxOrigin, SyntaxToken};

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

mod lexical;
pub(super) use lexical::LexicalScopes;

#[derive(Clone, Copy)]
struct ImportSite<'a> {
    source: SurfaceSourceId,
    node: NodeId,
    target: &'a ModuleIdentity,
    scope: ScopeId,
}

#[derive(Clone, Copy, Debug)]
enum Policy {
    Declaration(SurfaceDeclarationId),
    Import { node: NodeId, name: SyntaxToken },
}
impl Policy {
    fn audience<'a>(self, paired: &'a PairedModuleSources<'_>) -> Option<&'a ModuleAudience> {
        match self {
            Self::Declaration(raw) => paired.inventory.audience(raw),
            Self::Import { node, .. } => paired.inventory.import_audience(node),
        }
    }
    fn audience_handle(self, paired: &PairedModuleSources<'_>) -> Option<Arc<ModuleAudience>> {
        match self {
            Self::Declaration(raw) => paired.inventory.audiences.declaration_handle(raw),
            Self::Import { node, .. } => paired.inventory.audiences.import_handle(node),
        }
    }
    fn origin(self, paired: &PairedModuleSources<'_>) -> Option<SyntaxOrigin> {
        Some(SyntaxOrigin::Token(match self {
            Self::Declaration(raw) => paired.inventory.declarations()[raw.index()].name()?,
            Self::Import { name, .. } => name,
        }))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ModuleExports {
    scopes: Arc<BTreeMap<ModuleIdentity, ScopeId>>,
}
impl ModuleExports {
    pub fn bind(
        paired: &PairedModuleSources<'_>,
        source_scopes: &[ScopeId],
        body_scopes: &BTreeMap<NodeId, ScopeId>,
        declarations: &[Option<DeclarationId>],
        reservations: &mut Reservations,
    ) -> Result<(Self, LexicalScopes), ModuleSourcesFailure> {
        let mut bindings = ImportBindings::new(paired, source_scopes, declarations, reservations)?;
        for module in paired.inventory.modules_in_dependency_order() {
            for import in paired.inventory.imports().iter().filter(|import| {
                paired.inventory.sources()[import.source().index()].module() == module
            }) {
                bindings.bind(
                    ImportSite {
                        source: import.source(),
                        node: import.node(),
                        target: import.target(),
                        scope: source_scopes[import.source().index()],
                    },
                    reservations,
                )?;
            }
        }
        let lexical = bindings.bind_blocks(body_scopes, reservations)?;
        Ok((bindings.finish(reservations)?, lexical))
    }
    pub fn lookup(
        &self,
        program: &Program,
        module: &ModuleIdentity,
        caller: &ModuleIdentity,
        namespace: Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        let scope = *self.scopes.get(module)?;
        program.declaration_visible_from(scope, namespace, name, caller)
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExportNamespace {
    Value,
    Type,
    Qualifier,
}
#[derive(Clone, Copy, Debug)]
enum ExportTarget {
    Declaration(DeclarationId),
    Namespace(usize),
}
impl ExportTarget {
    fn namespace(self) -> ExportNamespace {
        match self {
            Self::Declaration(DeclarationId::Code(_)) => ExportNamespace::Value,
            Self::Declaration(DeclarationId::Record(_)) => ExportNamespace::Type,
            Self::Namespace(_) => ExportNamespace::Qualifier,
        }
    }
}
type ExportKey = (ExportNamespace, Symbol);
type ExportNames = BTreeMap<ExportKey, (ExportTarget, Policy)>;

/// Temporary binding work, consumed before the one semantic namespace closes.
struct ImportBindings<'a, 'syntax> {
    paired: &'a PairedModuleSources<'syntax>,
    source_scopes: &'a [ScopeId],
    declarations: &'a [Option<DeclarationId>],
    exports: Vec<ExportNames>,
    locals: BTreeMap<ScopeId, BTreeMap<(ExportNamespace, Symbol), SyntaxOrigin>>,
    module_scopes: Box<[ScopeId]>,
}
impl<'a, 'syntax> ImportBindings<'a, 'syntax> {
    fn new(
        paired: &'a PairedModuleSources<'syntax>,
        source_scopes: &'a [ScopeId],
        declarations: &'a [Option<DeclarationId>],
        reservations: &mut Reservations,
    ) -> Result<Self, ModuleSourcesFailure> {
        let inventory = &paired.inventory;
        let modules = inventory.surface.modules();
        let mut exports = vec![BTreeMap::new(); modules.len()];
        for (raw, declaration) in inventory.declaration_entries() {
            if !super::module_sources::has_source_name(declaration)
                || inventory.audience(raw).is_none()
            {
                continue;
            }
            let target = declarations[raw.index()].ok_or_else(integrity)?;
            let name = symbol(
                paired,
                declaration.source(),
                declaration.name().ok_or_else(integrity)?,
            )?;
            let module = modules
                .binary_search(inventory.sources()[declaration.source().index()].module())
                .map_err(|_| integrity())?;
            insert_export(
                paired,
                &mut exports[module],
                name,
                ExportTarget::Declaration(target),
                Policy::Declaration(raw),
            )?;
        }
        Ok(Self {
            paired,
            source_scopes,
            declarations,
            exports,
            locals: BTreeMap::new(),
            module_scopes: modules.iter().map(|_| reservations.new_scope()).collect(),
        })
    }
    fn bind(
        &mut self,
        import: ImportSite<'_>,
        reservations: &mut Reservations,
    ) -> Result<(), ModuleSourcesFailure> {
        let paired = self.paired;
        let inventory = &paired.inventory;
        let modules = inventory.surface.modules();
        let module = inventory.sources()[import.source.index()].module();
        let module_index = modules.binary_search(module).map_err(|_| integrity())?;
        let target_module = modules
            .binary_search(import.target)
            .map_err(|_| integrity())?;
        let authored =
            crate::imports::syntax::read(syntax(paired, import.source)?.tree(), import.node)
                .map_err(|_| integrity())?;
        if authored.selected.is_none() {
            return self.bind_namespace_import(
                &import,
                &authored,
                module_index,
                target_module,
                reservations,
            );
        }
        let selected_names = authored.selected.ok_or_else(integrity)?;
        for selected in selected_names {
            let exported = symbol(paired, import.source, selected.exported)?;
            let local = symbol(paired, import.source, selected.local)?;
            let exported_origin = SyntaxOrigin::Token(selected.exported);
            let Some(&(target, policy)) =
                self.exports[target_module].get(&(ExportNamespace::Type, exported))
            else {
                return Err(import_failure(
                    paired,
                    if self.exports[target_module].contains_key(&(ExportNamespace::Value, exported))
                        || self.exports[target_module]
                            .contains_key(&(ExportNamespace::Qualifier, exported))
                    {
                        ImportViolation::non_type_selection(exported_origin)
                    } else {
                        ImportViolation::missing_imported_name(exported_origin)
                    },
                ));
            };
            let target_audience = policy.audience(paired).ok_or_else(integrity)?;
            let target_origin = policy.origin(paired).ok_or_else(integrity)?;
            if !target_audience.contains(module) {
                return Err(import_failure(
                    paired,
                    ImportViolation::inaccessible_imported_name(exported_origin, target_origin),
                ));
            }
            if let Some(audience) = inventory.import_audience(import.node)
                && !audience.is_within(target_audience)
            {
                return Err(import_failure(
                    paired,
                    ImportViolation::widening_reexport(
                        SyntaxOrigin::Node(authored.visibility.unwrap_or(import.node)),
                        target_origin,
                    ),
                ));
            }
            self.validate_local(
                import.source,
                import.scope,
                local,
                selected.local,
                ExportNamespace::Type,
            )?;
            let ExportTarget::Declaration(declaration) = target else {
                return Err(integrity());
            };
            reservations
                .bind(import.scope, local, declaration)
                .map_err(|_| integrity())?;
            if inventory.import_audience(import.node).is_some() {
                insert_export(
                    paired,
                    &mut self.exports[module_index],
                    local,
                    target,
                    Policy::Import {
                        node: import.node,
                        name: selected.local,
                    },
                )?;
            }
        }
        Ok(())
    }
    fn bind_namespace_import(
        &mut self,
        import: &ImportSite<'_>,
        authored: &crate::imports::syntax::ImportSyntax,
        module_index: usize,
        target_module: usize,
        reservations: &mut Reservations,
    ) -> Result<(), ModuleSourcesFailure> {
        let paired = self.paired;
        let inventory = &paired.inventory;
        let name = match authored.namespace_alias {
            Some(alias) => alias,
            None => crate::imports::syntax::final_path_name(
                syntax(paired, import.source)?.tree(),
                import.node,
                authored.path,
            )
            .map_err(|_| integrity())?,
        };
        let local = symbol(paired, import.source, name)?;
        self.validate_local(
            import.source,
            import.scope,
            local,
            name,
            ExportNamespace::Qualifier,
        )?;
        reservations
            .bind_namespace(import.scope, local, self.module_scopes[target_module], None)
            .map_err(|_| integrity())?;
        if inventory.import_audience(import.node).is_some() {
            insert_export(
                paired,
                &mut self.exports[module_index],
                local,
                ExportTarget::Namespace(target_module),
                Policy::Import {
                    node: import.node,
                    name,
                },
            )?;
        }
        Ok(())
    }
    fn validate_local(
        &mut self,
        source: SurfaceSourceId,
        scope: ScopeId,
        local: Symbol,
        token: SyntaxToken,
        namespace: ExportNamespace,
    ) -> Result<(), ModuleSourcesFailure> {
        let paired = self.paired;
        let origin = SyntaxOrigin::Token(token);
        let spelling = paired
            .inventory
            .surface
            .symbols()
            .spelling(local)
            .ok_or_else(integrity)?;
        if BuiltinType::from_spelling(spelling).is_some()
            || spelling == ContextualSpelling::UpperSelf.as_str()
        {
            return Err(namespace_failure(
                paired,
                NamespaceViolation::reserved_name(origin),
            ));
        }
        let first = (if scope == self.source_scopes[source.index()] {
            local_origin(paired, self.declarations, source, local, namespace)?
        } else {
            None
        })
        .or_else(|| {
            self.locals.get(&scope).and_then(|names| {
                names
                    .iter()
                    .find(|((kind, name), _)| {
                        *name == local && namespaces_conflict(namespace, *kind)
                    })
                    .map(|(_, origin)| *origin)
            })
        });
        if let Some(first) = first {
            return Err(namespace_failure(
                paired,
                NamespaceViolation::name_collision(first, origin),
            ));
        }
        self.locals
            .entry(scope)
            .or_default()
            .insert((namespace, local), origin);
        Ok(())
    }
    fn finish(
        self,
        reservations: &mut Reservations,
    ) -> Result<ModuleExports, ModuleSourcesFailure> {
        let scopes = self.module_scopes;
        for (index, names) in self.exports.into_iter().enumerate() {
            for ((_, name), (target, policy)) in names {
                let audience = policy.audience_handle(self.paired).ok_or_else(integrity)?;
                match target {
                    ExportTarget::Declaration(target) => reservations
                        .bind_export(scopes[index], name, target, audience)
                        .map_err(|_| integrity())?,
                    ExportTarget::Namespace(target) => reservations
                        .bind_namespace(scopes[index], name, scopes[target], Some(audience))
                        .map_err(|_| integrity())?,
                }
            }
        }
        let modules = self.paired.inventory.surface.modules();
        if modules.len() != scopes.len() {
            return Err(integrity());
        }
        Ok(ModuleExports {
            scopes: Arc::new(modules.iter().cloned().zip(scopes).collect()),
        })
    }
}

fn namespaces_conflict(left: ExportNamespace, right: ExportNamespace) -> bool {
    left == right || left == ExportNamespace::Qualifier || right == ExportNamespace::Qualifier
}

fn insert_export(
    paired: &PairedModuleSources<'_>,
    exports: &mut ExportNames,
    name: Symbol,
    target: ExportTarget,
    policy: Policy,
) -> Result<(), ModuleSourcesFailure> {
    let namespace = target.namespace();
    if let Some((_, (_, first))) = exports
        .iter()
        .find(|((kind, symbol), _)| *symbol == name && namespaces_conflict(namespace, *kind))
    {
        return Err(namespace_failure(
            paired,
            NamespaceViolation::name_collision(
                first.origin(paired).ok_or_else(integrity)?,
                policy.origin(paired).ok_or_else(integrity)?,
            ),
        ));
    }
    exports.insert((namespace, name), (target, policy));
    Ok(())
}
fn local_origin(
    paired: &PairedModuleSources<'_>,
    declarations: &[Option<DeclarationId>],
    source: SurfaceSourceId,
    name: Symbol,
    namespace: ExportNamespace,
) -> Result<Option<SyntaxOrigin>, ModuleSourcesFailure> {
    let visible = super::module_sources::visible_sources(paired, source).collect::<BTreeSet<_>>();
    for (raw, declaration) in paired.inventory.declaration_entries() {
        if super::module_sources::has_source_name(declaration)
            && declarations[raw.index()].is_some_and(|declaration| {
                namespaces_conflict(
                    namespace,
                    ExportTarget::Declaration(declaration).namespace(),
                )
            })
            && visible.contains(&declaration.source())
        {
            let token = declaration.name().ok_or_else(integrity)?;
            if symbol(paired, declaration.source(), token)? == name {
                return Ok(Some(SyntaxOrigin::Token(token)));
            }
        }
    }
    Ok(None)
}
fn import_failure(
    paired: &PairedModuleSources<'_>,
    violation: ImportViolation,
) -> ModuleSourcesFailure {
    ImportDiagnostic::project_surface(violation, &paired.inventory.surface).map_or_else(
        |_| integrity(),
        |diagnostic| ModuleSourcesFailure::Access(Failure::Import(Box::new(diagnostic))),
    )
}
fn namespace_failure(
    paired: &PairedModuleSources<'_>,
    violation: NamespaceViolation,
) -> ModuleSourcesFailure {
    NamespaceDiagnostic::project_surface(violation, &paired.inventory.surface).map_or_else(
        |_| integrity(),
        |diagnostic| ModuleSourcesFailure::Access(Failure::Namespace(Box::new(diagnostic))),
    )
}
