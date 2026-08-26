use std::collections::{HashMap, VecDeque};

use nocter_declarations::{DeclarationGraph, ExportedEntity, ModuleNamespace};
use nocter_model::{ModuleId, Symbol};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceIndex};

/// The shortest semantic spelling of every entity reachable from one requesting module.
///
/// The complete view is derived once per editor query context. Rendering nested types or a batch
/// of completion details therefore does not repeatedly traverse the module graph, and all related
/// presentations use the same deterministic alias choice.
#[derive(Debug)]
pub(crate) struct VisibleSpellings {
    by_entity: HashMap<ExportedEntity, Box<[Symbol]>>,
}

impl VisibleSpellings {
    pub(crate) fn new(graph: &DeclarationGraph, from: ModuleId) -> Self {
        let mut by_entity = HashMap::new();
        let mut module_paths = HashMap::from([(from, Vec::new())]);
        let mut pending = VecDeque::from([(from, Vec::new(), true)]);

        while let Some((module, prefix, local)) = pending.pop_front() {
            if module_paths
                .get(&module)
                .is_none_or(|selected| selected != &prefix)
            {
                continue;
            }
            let Some(namespace) = graph.module_namespaces().get(module) else {
                continue;
            };
            visit_authored(
                graph,
                from,
                module,
                namespace,
                &prefix,
                local,
                &mut by_entity,
                &mut module_paths,
                &mut pending,
            );
            if local {
                visit_fallback(
                    namespace,
                    &prefix,
                    &mut by_entity,
                    &mut module_paths,
                    &mut pending,
                );
            }
        }

        Self {
            by_entity: by_entity
                .into_iter()
                .map(|(entity, spelling)| (entity, spelling.into_boxed_slice()))
                .collect(),
        }
    }

    pub(crate) fn for_source(
        graph: &DeclarationGraph,
        from: ModuleId,
        source_index: &SourceIndex,
        source: SourceId,
    ) -> Self {
        let mut visible = Self::new(graph, from);
        for (name, entity) in source_index.visible_names_in(source) {
            let Some(entity) = exported_entity(entity) else {
                continue;
            };
            let candidate = vec![name];
            if visible.by_entity.get(&entity).is_none_or(|current| {
                (candidate.len(), candidate.as_slice()) < (current.len(), current.as_ref())
            }) {
                visible
                    .by_entity
                    .insert(entity, candidate.into_boxed_slice());
            }
        }
        visible
    }

    pub(crate) fn get(&self, entity: ExportedEntity) -> Option<&[Symbol]> {
        self.by_entity.get(&entity).map(AsRef::as_ref)
    }
}

const fn exported_entity(entity: SemanticEntity) -> Option<ExportedEntity> {
    match entity {
        SemanticEntity::Module(id) => Some(ExportedEntity::Module(id)),
        SemanticEntity::NominalType(id) => Some(ExportedEntity::NominalType(id)),
        SemanticEntity::TypeAlias(id) => Some(ExportedEntity::TypeAlias(id)),
        SemanticEntity::Interface(id) => Some(ExportedEntity::Interface(id)),
        SemanticEntity::Constant(id) => Some(ExportedEntity::Constant(id)),
        SemanticEntity::Callable(id) => Some(ExportedEntity::Callable(id)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_authored(
    graph: &DeclarationGraph,
    from: ModuleId,
    module: ModuleId,
    namespace: &ModuleNamespace,
    prefix: &[Symbol],
    local: bool,
    by_entity: &mut HashMap<ExportedEntity, Vec<Symbol>>,
    module_paths: &mut HashMap<ModuleId, Vec<Symbol>>,
    pending: &mut VecDeque<(ModuleId, Vec<Symbol>, bool)>,
) {
    for entry in namespace.authored().iter().copied() {
        if !local && !graph.is_visible_from(entry.visibility(), from, module) {
            continue;
        }
        visit_entry(
            prefix,
            entry.name(),
            entry.target(),
            by_entity,
            module_paths,
            pending,
        );
    }
}

fn visit_fallback(
    namespace: &ModuleNamespace,
    prefix: &[Symbol],
    by_entity: &mut HashMap<ExportedEntity, Vec<Symbol>>,
    module_paths: &mut HashMap<ModuleId, Vec<Symbol>>,
    pending: &mut VecDeque<(ModuleId, Vec<Symbol>, bool)>,
) {
    for entry in namespace.fallback().iter().copied() {
        if namespace.lookup_authored(entry.name()).is_some() {
            continue;
        }
        visit_entry(
            prefix,
            entry.name(),
            entry.target(),
            by_entity,
            module_paths,
            pending,
        );
    }
}

fn visit_entry(
    prefix: &[Symbol],
    name: Symbol,
    entity: ExportedEntity,
    by_entity: &mut HashMap<ExportedEntity, Vec<Symbol>>,
    module_paths: &mut HashMap<ModuleId, Vec<Symbol>>,
    pending: &mut VecDeque<(ModuleId, Vec<Symbol>, bool)>,
) {
    let mut spelling = prefix.to_vec();
    spelling.push(name);
    if is_better(by_entity.get(&entity), &spelling) {
        by_entity.insert(entity, spelling.clone());
    }
    if let ExportedEntity::Module(module) = entity
        && is_better(module_paths.get(&module), &spelling)
    {
        module_paths.insert(module, spelling.clone());
        pending.push_back((module, spelling, false));
    }
}

fn is_better(current: Option<&Vec<Symbol>>, candidate: &[Symbol]) -> bool {
    current.is_none_or(|current| (candidate.len(), candidate) < (current.len(), current.as_slice()))
}
