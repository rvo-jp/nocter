//! Source dependency invalidation shared by multi-pass and incremental analysis.

use crate::resolve::{ImportKind, ImportSourceMap, PreludeSourceMap};
use crate::source::SourceId;
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) fn reverse_dependency_closure(
    changed: &HashSet<SourceId>,
    imports: &ImportSourceMap,
    preludes: &PreludeSourceMap,
) -> HashSet<SourceId> {
    let mut dependents = HashMap::<SourceId, Vec<SourceId>>::new();
    for (span, imported) in imports {
        dependents
            .entry(imported.source)
            .or_default()
            .push(span.source);
        if imported.kind == ImportKind::Source {
            // Physical source files in one directory module form one semantic
            // declaration unit, so invalidation is bidirectional.
            dependents
                .entry(span.source)
                .or_default()
                .push(imported.source);
        }
    }
    for (source, prelude) in preludes {
        dependents.entry(prelude.source).or_default().push(*source);
    }

    let mut affected = changed.clone();
    let mut queue = changed.iter().copied().collect::<VecDeque<_>>();
    while let Some(source) = queue.pop_front() {
        for dependent in dependents.get(&source).into_iter().flatten() {
            if affected.insert(*dependent) {
                queue.push_back(*dependent);
            }
        }
    }
    affected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{ImportAccess, ImportSource};
    use crate::source::ByteSpan;

    fn module_import(source: u32) -> ImportSource {
        ImportSource {
            source: SourceId::new(source),
            access: ImportAccess::Public,
            kind: ImportKind::Module,
        }
    }

    #[test]
    fn follows_transitive_importers_and_prelude_consumers() {
        let imports = ImportSourceMap::from([
            (ByteSpan::new(SourceId::new(2), 0, 1), module_import(1)),
            (ByteSpan::new(SourceId::new(3), 0, 1), module_import(2)),
        ]);
        let preludes = PreludeSourceMap::from([(SourceId::new(4), module_import(3))]);

        let affected =
            reverse_dependency_closure(&HashSet::from([SourceId::new(1)]), &imports, &preludes);

        assert_eq!(
            affected,
            HashSet::from([
                SourceId::new(1),
                SourceId::new(2),
                SourceId::new(3),
                SourceId::new(4),
            ])
        );
    }

    #[test]
    fn source_files_in_one_module_invalidate_each_other() {
        let imports = ImportSourceMap::from([(
            ByteSpan::new(SourceId::new(2), 0, 1),
            ImportSource {
                source: SourceId::new(1),
                access: ImportAccess::Public,
                kind: ImportKind::Source,
            },
        )]);

        let affected = reverse_dependency_closure(
            &HashSet::from([SourceId::new(2)]),
            &imports,
            &PreludeSourceMap::new(),
        );

        assert_eq!(
            affected,
            HashSet::from([SourceId::new(1), SourceId::new(2)])
        );
    }
}
