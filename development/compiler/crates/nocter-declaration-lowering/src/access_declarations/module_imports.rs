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
#[derive(Debug)]
pub(super) struct ModuleExports {
    scopes: Box<[ScopeId]>,
}
impl ModuleExports {
    /// Prerequisites are explicit until qualified namespace and lexical block bindings are closed.
    pub fn require_supported(paired: &PairedModuleSources<'_>) -> Result<(), ModuleSourcesFailure> {
        let inventory = &paired.inventory;
        let mut pending = inventory
            .block_imports()
            .iter()
            .map(crate::SurfaceBlockImport::node)
            .collect::<Vec<_>>();
        for import in inventory.imports() {
            let authored = crate::imports::syntax::read(
                syntax(paired, import.source())?.tree(),
                import.node(),
            )
            .map_err(|_| integrity())?;
            if authored.selected.is_none() {
                pending.push(import.node());
            }
        }
        if pending.is_empty() {
            Ok(())
        } else {
            Err(ModuleSourcesFailure::Access(
                Failure::ModuleImportBindingsRequired(pending.into()),
            ))
        }
    }
    pub fn bind(
        paired: &PairedModuleSources<'_>,
        source_scopes: &[ScopeId],
        declarations: &[Option<DeclarationId>],
        reservations: &mut Reservations,
    ) -> Result<Self, ModuleSourcesFailure> {
        let mut bindings = ImportBindings::new(paired, source_scopes, declarations)?;
        for module in paired.inventory.modules_in_dependency_order() {
            for import in paired.inventory.imports().iter().filter(|import| {
                paired.inventory.sources()[import.source().index()].module() == module
            }) {
                bindings.bind(import, reservations)?;
            }
        }
        bindings.finish(reservations)
    }
    pub fn lookup(
        &self,
        paired: &PairedModuleSources<'_>,
        program: &Program,
        module: &ModuleIdentity,
        caller: &ModuleIdentity,
        namespace: Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        let index = paired
            .inventory
            .surface
            .modules()
            .binary_search(module)
            .ok()?;
        program.declaration_visible_from(self.scopes[index], namespace, name, caller)
    }
}
type ExportKey = (Namespace, Symbol);
type ExportNames = BTreeMap<ExportKey, (DeclarationId, Policy)>;

/// Temporary binding work, consumed before the one semantic namespace closes.
struct ImportBindings<'a, 'syntax> {
    paired: &'a PairedModuleSources<'syntax>,
    source_scopes: &'a [ScopeId],
    declarations: &'a [Option<DeclarationId>],
    exports: Vec<ExportNames>,
    locals: BTreeMap<(SurfaceSourceId, Symbol), SyntaxOrigin>,
}
impl<'a, 'syntax> ImportBindings<'a, 'syntax> {
    fn new(
        paired: &'a PairedModuleSources<'syntax>,
        source_scopes: &'a [ScopeId],
        declarations: &'a [Option<DeclarationId>],
    ) -> Result<Self, ModuleSourcesFailure> {
        let inventory = &paired.inventory;
        let modules = inventory.surface.modules();
        let mut exports = vec![BTreeMap::new(); modules.len()];
        for (raw, declaration) in inventory.declaration_entries() {
            if declaration.owner().is_some() || inventory.audience(raw).is_none() {
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
                target,
                Policy::Declaration(raw),
            )?;
        }
        Ok(Self {
            paired,
            source_scopes,
            declarations,
            exports,
            locals: BTreeMap::new(),
        })
    }
    fn bind(
        &mut self,
        import: &crate::SurfaceImport,
        reservations: &mut Reservations,
    ) -> Result<(), ModuleSourcesFailure> {
        let paired = self.paired;
        let inventory = &paired.inventory;
        let modules = inventory.surface.modules();
        let module = inventory.sources()[import.source().index()].module();
        let module_index = modules.binary_search(module).map_err(|_| integrity())?;
        let target_module = modules
            .binary_search(import.target())
            .map_err(|_| integrity())?;
        let authored =
            crate::imports::syntax::read(syntax(paired, import.source())?.tree(), import.node())
                .map_err(|_| integrity())?;
        for selected in authored.selected.ok_or_else(integrity)? {
            let exported = symbol(paired, import.source(), selected.exported)?;
            let local = symbol(paired, import.source(), selected.local)?;
            let local_origin = SyntaxOrigin::Token(selected.local);
            let exported_origin = SyntaxOrigin::Token(selected.exported);
            let spelling = inventory
                .surface
                .symbols()
                .spelling(local)
                .ok_or_else(integrity)?;
            if BuiltinType::from_spelling(spelling).is_some()
                || spelling == ContextualSpelling::UpperSelf.as_str()
            {
                return Err(namespace_failure(
                    paired,
                    NamespaceViolation::reserved_name(local_origin),
                ));
            }
            let Some(&(target, policy)) =
                self.exports[target_module].get(&(Namespace::Type, exported))
            else {
                return Err(import_failure(
                    paired,
                    if self.exports[target_module].contains_key(&(Namespace::Value, exported)) {
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
            if let Some(audience) = inventory.import_audience(import.node())
                && !audience.is_within(target_audience)
            {
                return Err(import_failure(
                    paired,
                    ImportViolation::widening_reexport(
                        SyntaxOrigin::Node(authored.visibility.unwrap_or(import.node())),
                        target_origin,
                    ),
                ));
            }
            if let Some(first) =
                local_type_origin(paired, self.declarations, import.source(), local)?
                    .or_else(|| self.locals.get(&(import.source(), local)).copied())
            {
                return Err(namespace_failure(
                    paired,
                    NamespaceViolation::name_collision(first, local_origin),
                ));
            }
            self.locals.insert((import.source(), local), local_origin);
            reservations
                .bind(self.source_scopes[import.source().index()], local, target)
                .map_err(|_| integrity())?;
            if inventory.import_audience(import.node()).is_some() {
                insert_export(
                    paired,
                    &mut self.exports[module_index],
                    local,
                    target,
                    Policy::Import {
                        node: import.node(),
                        name: selected.local,
                    },
                )?;
            }
        }
        Ok(())
    }
    fn finish(
        self,
        reservations: &mut Reservations,
    ) -> Result<ModuleExports, ModuleSourcesFailure> {
        let scopes = self
            .exports
            .iter()
            .map(|_| reservations.new_scope())
            .collect::<Box<[_]>>();
        for (index, names) in self.exports.into_iter().enumerate() {
            for ((_, name), (target, policy)) in names {
                reservations
                    .bind_export(
                        scopes[index],
                        name,
                        target,
                        policy.audience_handle(self.paired).ok_or_else(integrity)?,
                    )
                    .map_err(|_| integrity())?;
            }
        }
        Ok(ModuleExports { scopes })
    }
}

fn kind(target: DeclarationId) -> Namespace {
    match target {
        DeclarationId::Record(_) => Namespace::Type,
        DeclarationId::Code(_) => Namespace::Value,
    }
}
fn insert_export(
    paired: &PairedModuleSources<'_>,
    exports: &mut ExportNames,
    name: Symbol,
    target: DeclarationId,
    policy: Policy,
) -> Result<(), ModuleSourcesFailure> {
    if let Some((_, first)) = exports.insert((kind(target), name), (target, policy)) {
        return Err(namespace_failure(
            paired,
            NamespaceViolation::name_collision(
                first.origin(paired).ok_or_else(integrity)?,
                policy.origin(paired).ok_or_else(integrity)?,
            ),
        ));
    }
    Ok(())
}
fn local_type_origin(
    paired: &PairedModuleSources<'_>,
    declarations: &[Option<DeclarationId>],
    source: SurfaceSourceId,
    name: Symbol,
) -> Result<Option<SyntaxOrigin>, ModuleSourcesFailure> {
    let visible = std::iter::once(source)
        .chain(
            paired
                .inventory
                .source_visibilities()
                .iter()
                .filter(|edge| edge.source() == source)
                .map(|edge| edge.target()),
        )
        .collect::<BTreeSet<_>>();
    for (raw, declaration) in paired.inventory.declaration_entries() {
        if matches!(declarations[raw.index()], Some(DeclarationId::Record(_)))
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
