use std::fmt;

use nocter_model::ModuleId;
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};

/// The unique semantic owner of one physical source in a prepared program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceContext {
    module: ModuleId,
}

impl SourceContext {
    pub(crate) fn resolve(
        index: &SourceIndex,
        source: SourceId,
    ) -> Result<Self, SourceContextError> {
        let mut selected = None;
        for binding in index.bindings_in(source) {
            if !matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            ) {
                continue;
            }
            let SemanticEntity::Module(module) = binding.entity() else {
                continue;
            };
            if let Some(first) = selected
                && first != module
            {
                return Err(SourceContextError::ConflictingModules {
                    source,
                    first,
                    second: module,
                });
            }
            selected = Some(module);
        }
        selected
            .map(|module| Self { module })
            .ok_or(SourceContextError::MissingModule(source))
    }

    #[must_use]
    pub(crate) const fn module(self) -> ModuleId {
        self.module
    }
}

/// An inconsistent source-to-module projection in an otherwise semantic snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContextError {
    MissingModule(SourceId),
    ConflictingModules {
        source: SourceId,
        first: ModuleId,
        second: ModuleId,
    },
}

impl fmt::Display for SourceContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingModule(source) => {
                write!(formatter, "source {source} has no semantic module owner")
            }
            Self::ConflictingModules {
                source,
                first,
                second,
            } => write!(
                formatter,
                "source {source} has conflicting semantic module owners {first:?} and {second:?}"
            ),
        }
    }
}

impl std::error::Error for SourceContextError {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, ModuleId};
    use nocter_source::{SourceMap, SourceName};
    use nocter_source_index::{SemanticEntity, SourceIndexBuilder, SourceOrigin, SourceRole};
    use nocter_syntax::{ParseGoal, parse};

    use super::{SourceContext, SourceContextError};

    #[test]
    fn source_owner_ignores_references_and_rejects_conflicting_owners() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("/app/index.nct"), b"use ./child\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut modules = ArenaBuilder::<ModuleId, _>::new();
        let owner = modules.insert(());
        let referenced = modules.insert(());

        let mut index = SourceIndexBuilder::new();
        index
            .insert(
                SemanticEntity::Module(referenced),
                SourceRole::Reference,
                origin,
            )
            .unwrap();
        index
            .insert(
                SemanticEntity::Module(owner),
                SourceRole::Declaration,
                origin,
            )
            .unwrap();
        assert_eq!(
            SourceContext::resolve(&index.finish(), source)
                .unwrap()
                .module(),
            owner
        );

        let mut conflicting = SourceIndexBuilder::new();
        conflicting
            .insert(
                SemanticEntity::Module(owner),
                SourceRole::Declaration,
                origin,
            )
            .unwrap();
        conflicting
            .insert(
                SemanticEntity::Module(referenced),
                SourceRole::Implementation,
                origin,
            )
            .unwrap();
        assert!(matches!(
            SourceContext::resolve(&conflicting.finish(), source),
            Err(SourceContextError::ConflictingModules { .. })
        ));
    }
}
