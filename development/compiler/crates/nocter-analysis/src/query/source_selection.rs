use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::query) enum SourceSelection<T> {
    None,
    Unique(T),
    Ambiguous,
}

impl<T> SourceSelection<T> {
    pub(in crate::query) fn unique(self) -> Option<T> {
        match self {
            Self::Unique(binding) => Some(binding),
            Self::None | Self::Ambiguous => None,
        }
    }
}

/// Selects one semantic authority from overlapping source projections.
///
/// The narrowest range wins. Equally narrow candidates use the same
/// reference/declaration/implementation and entity-family ordering for every editor query. If
/// distinct bindings still have equal authority, the projection is explicitly ambiguous; a dense
/// semantic identity or insertion order must never decide presentation.
pub(in crate::query) fn select_source_binding<'a>(
    bindings: impl Iterator<Item = &'a SourceBinding>,
    see: impl Fn(&SourceBinding) -> bool,
) -> SourceSelection<SourceBinding> {
    select_source_candidates(
        bindings
            .filter(|binding| see(binding))
            .map(|binding| (*binding, *binding)),
    )
}

/// Selects a payload already derived from one source binding without repeating that derivation.
///
/// This is the shared authority rule for semantic queries that need more than the binding itself.
/// Distinct bindings with the same authority remain ambiguous even when their payloads compare
/// equal.
pub(in crate::query) fn select_source_candidates<T>(
    candidates: impl Iterator<Item = (SourceBinding, T)>,
) -> SourceSelection<T> {
    let mut selected = SourceSelection::None;
    let mut selected_binding = None;
    let mut best = None;
    for (binding, payload) in candidates {
        let key = binding_authority_key(&binding);
        match best.map(|best| key.cmp(&best)) {
            None | Some(std::cmp::Ordering::Less) => {
                best = Some(key);
                selected_binding = Some(binding);
                selected = SourceSelection::Unique(payload);
            }
            Some(std::cmp::Ordering::Equal) => {
                if selected_binding != Some(binding) {
                    selected = SourceSelection::Ambiguous;
                }
            }
            Some(std::cmp::Ordering::Greater) => {}
        }
    }
    selected
}

fn binding_authority_key(binding: &SourceBinding) -> (u32, u8, u8) {
    (
        binding.origin().span().range().len(),
        match binding.role() {
            SourceRole::Reference => 0,
            SourceRole::Declaration => 1,
            SourceRole::Implementation => 2,
        },
        entity_family_rank(binding.entity()),
    )
}

const fn entity_family_rank(entity: SemanticEntity) -> u8 {
    match entity {
        SemanticEntity::LocalBinding(..) | SemanticEntity::Capture(..) => 0,
        SemanticEntity::Parameter(_) | SemanticEntity::GenericParameter(_) => 1,
        SemanticEntity::Field(_) | SemanticEntity::Variant(_) => 2,
        SemanticEntity::Callable(_)
        | SemanticEntity::BuiltinType(_)
        | SemanticEntity::Constant(_)
        | SemanticEntity::NominalType(_)
        | SemanticEntity::TypeAlias(_)
        | SemanticEntity::Interface(_)
        | SemanticEntity::AssociatedType(_) => 3,
        SemanticEntity::Module(_)
        | SemanticEntity::Package(_)
        | SemanticEntity::PackageTarget(_) => 4,
        SemanticEntity::Import(_)
        | SemanticEntity::DeclarationSite(_)
        | SemanticEntity::Construction(_)
        | SemanticEntity::Instance(_)
        | SemanticEntity::InterfaceImplementation(_)
        | SemanticEntity::Drop(_)
        | SemanticEntity::Test(_)
        | SemanticEntity::Requirement(_)
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..)
        | SemanticEntity::OpaqueType(_) => 5,
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, CallableId, ParameterId};
    use nocter_source::{ByteOffset, SourceMap, SourceName};
    use nocter_source_index::{SemanticEntity, SourceIndexBuilder, SourceOrigin, SourceRole};
    use nocter_syntax::{ParseGoal, parse};

    use super::{SourceSelection, select_source_binding};

    #[test]
    fn equal_ranges_use_role_then_entity_family_without_insertion_order() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("selection.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut callables = ArenaBuilder::<CallableId, ()>::new();
        let callable = callables.insert(());
        let mut parameters = ArenaBuilder::<ParameterId, ()>::new();
        let parameter = parameters.insert(());

        let build = |reverse: bool| {
            let mut builder = SourceIndexBuilder::new();
            let entries = if reverse {
                [
                    (
                        SemanticEntity::Parameter(parameter),
                        SourceRole::Declaration,
                    ),
                    (SemanticEntity::Callable(callable), SourceRole::Reference),
                ]
            } else {
                [
                    (SemanticEntity::Callable(callable), SourceRole::Reference),
                    (
                        SemanticEntity::Parameter(parameter),
                        SourceRole::Declaration,
                    ),
                ]
            };
            for (entity, role) in entries {
                builder.insert(entity, role, origin).unwrap();
            }
            builder.finish()
        };

        for index in [build(false), build(true)] {
            let selected =
                select_source_binding(index.bindings_at(source, ByteOffset::new(0)), |_| true)
                    .unique()
                    .unwrap();
            assert_eq!(selected.entity(), SemanticEntity::Callable(callable));
            assert_eq!(selected.role(), SourceRole::Reference);
        }
    }

    #[test]
    fn equal_role_and_range_prefer_the_more_specific_entity_family() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("selection.nct"), b"value\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut callables = ArenaBuilder::<CallableId, ()>::new();
        let callable = callables.insert(());
        let mut parameters = ArenaBuilder::<ParameterId, ()>::new();
        let parameter = parameters.insert(());
        let mut builder = SourceIndexBuilder::new();
        builder
            .insert(
                SemanticEntity::Callable(callable),
                SourceRole::Declaration,
                origin,
            )
            .unwrap();
        builder
            .insert(
                SemanticEntity::Parameter(parameter),
                SourceRole::Declaration,
                origin,
            )
            .unwrap();
        let index = builder.finish();

        let selected =
            select_source_binding(index.bindings_at(source, ByteOffset::new(0)), |_| true)
                .unique()
                .unwrap();
        assert_eq!(selected.entity(), SemanticEntity::Parameter(parameter));
    }

    #[test]
    fn equal_authority_from_distinct_entities_is_ambiguous() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("selection.nct"), b"value\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut callables = ArenaBuilder::<CallableId, ()>::new();
        let first = callables.insert(());
        let second = callables.insert(());

        for entities in [[first, second], [second, first]] {
            let mut builder = SourceIndexBuilder::new();
            for callable in entities {
                builder
                    .insert(
                        SemanticEntity::Callable(callable),
                        SourceRole::Declaration,
                        origin,
                    )
                    .unwrap();
            }
            let index = builder.finish();
            assert_eq!(
                select_source_binding(index.bindings_at(source, ByteOffset::new(0)), |_| true),
                SourceSelection::Ambiguous
            );
        }
    }
}
