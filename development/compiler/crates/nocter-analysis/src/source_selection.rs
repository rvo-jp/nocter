use nocter_source::ByteOffset;
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

/// Selects one semantic authority from overlapping source projections.
///
/// The narrowest range wins. Equal ranges use the same reference/declaration/implementation and
/// entity-family ordering for every editor query, followed by exact semantic identity so arena or
/// projection insertion order can never decide presentation.
pub(crate) fn select_source_binding<'a>(
    bindings: impl Iterator<Item = &'a SourceBinding>,
    include: impl Fn(&SourceBinding) -> bool,
) -> Option<SourceBinding> {
    bindings
        .filter(|binding| include(binding))
        .min_by_key(|binding| source_binding_key(binding))
        .copied()
}

pub(crate) fn source_binding_key(
    binding: &SourceBinding,
) -> (u32, u8, u8, SemanticEntity, ByteOffset, ByteOffset) {
    let range = binding.origin().span().range();
    let (role, family, entity) = binding_authority_key(binding);
    (
        range.len(),
        role,
        family,
        entity,
        range.start(),
        range.end(),
    )
}

fn binding_authority_key(binding: &SourceBinding) -> (u8, u8, SemanticEntity) {
    (
        match binding.role() {
            SourceRole::Reference => 0,
            SourceRole::Declaration => 1,
            SourceRole::Implementation => 2,
        },
        entity_family_rank(binding.entity()),
        binding.entity(),
    )
}

const fn entity_family_rank(entity: SemanticEntity) -> u8 {
    match entity {
        SemanticEntity::LocalBinding(..) | SemanticEntity::Capture(..) => 0,
        SemanticEntity::Parameter(_) | SemanticEntity::GenericParameter(_) => 1,
        SemanticEntity::Field(_) | SemanticEntity::Variant(_) => 2,
        SemanticEntity::Callable(_)
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
        | SemanticEntity::Conformance(_)
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

    use super::select_source_binding;

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
            select_source_binding(index.bindings_at(source, ByteOffset::new(0)), |_| true).unwrap();
        assert_eq!(selected.entity(), SemanticEntity::Parameter(parameter));
    }
}
