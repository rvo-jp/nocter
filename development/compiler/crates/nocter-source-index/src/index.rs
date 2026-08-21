use std::collections::HashSet;
use std::fmt;

use nocter_source::{ByteOffset, SourceId};

use crate::{SemanticEntity, SourceOrigin};

/// The meaning of one semantic-to-source projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceRole {
    Declaration,
    Implementation,
    Reference,
}

/// Assignment capability known for one exact semantic occurrence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceAccess {
    Readonly,
    Writable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceBinding {
    entity: SemanticEntity,
    role: SourceRole,
    origin: SourceOrigin,
    access: Option<SourceAccess>,
}

impl SourceBinding {
    #[must_use]
    pub const fn entity(self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub const fn role(self) -> SourceRole {
        self.role
    }

    #[must_use]
    pub const fn origin(self) -> SourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn access(self) -> Option<SourceAccess> {
        self.access
    }
}

/// Immutable bidirectional projection between source coordinates and semantic identities.
///
/// The two sorted arrays contain the same bindings in different index orders. This keeps semantic
/// lookup and editor offset lookup deterministic without mutating either program after lowering.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceIndex {
    by_entity: Box<[SourceBinding]>,
    by_source: Box<[SourceBinding]>,
}

impl SourceIndex {
    #[must_use]
    pub fn bindings_for(&self, entity: SemanticEntity) -> &[SourceBinding] {
        let start = self
            .by_entity
            .partition_point(|binding| binding.entity < entity);
        let end = self
            .by_entity
            .partition_point(|binding| binding.entity <= entity);
        &self.by_entity[start..end]
    }

    pub fn bindings_at(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> impl Iterator<Item = &SourceBinding> {
        let start = self
            .by_source
            .partition_point(|binding| binding.origin.source() < source);
        let end = self
            .by_source
            .partition_point(|binding| binding.origin.source() <= source);
        self.by_source[start..end].iter().filter(move |binding| {
            let range = binding.origin.span().range();
            range.start() <= offset && offset < range.end()
        })
    }

    /// Returns every binding projected into one source in deterministic range order.
    pub fn bindings_in(&self, source: SourceId) -> impl Iterator<Item = &SourceBinding> {
        let start = self
            .by_source
            .partition_point(|binding| binding.origin.source() < source);
        let end = self
            .by_source
            .partition_point(|binding| binding.origin.source() <= source);
        self.by_source[start..end].iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.by_entity.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }

    /// Consumes this immutable projection and opens the sole extension boundary for a later
    /// semantic stage.
    ///
    /// Existing bindings remain subject to duplicate detection. Finishing the returned builder
    /// recreates both deterministic lookup orders.
    #[must_use]
    pub fn into_builder(self) -> SourceIndexBuilder {
        let bindings = self.by_entity.into_vec();
        let unique = bindings
            .iter()
            .map(|binding| (binding.entity, binding.role, binding.origin))
            .collect();
        SourceIndexBuilder { bindings, unique }
    }
}

#[derive(Debug, Default)]
pub struct SourceIndexBuilder {
    bindings: Vec<SourceBinding>,
    unique: HashSet<(SemanticEntity, SourceRole, SourceOrigin)>,
}

impl SourceIndexBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            unique: HashSet::new(),
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Records one projection.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateSourceBinding`] when the exact entity, role, and source origin were
    /// already recorded. Multiple references and separate contract/implementation origins remain
    /// valid distinct bindings.
    pub fn insert(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.insert_binding(entity, role, origin, None)
    }

    /// Records one projection with occurrence-specific assignment capability.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateSourceBinding`] when the exact projection was already recorded.
    pub fn insert_with_access(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
        access: SourceAccess,
    ) -> Result<(), DuplicateSourceBinding> {
        self.insert_binding(entity, role, origin, Some(access))
    }

    fn insert_binding(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
        access: Option<SourceAccess>,
    ) -> Result<(), DuplicateSourceBinding> {
        let binding = SourceBinding {
            entity,
            role,
            origin,
            access,
        };
        if !self.unique.insert((entity, role, origin)) {
            return Err(DuplicateSourceBinding(binding));
        }
        self.bindings.push(binding);
        Ok(())
    }

    #[must_use]
    pub fn finish(self) -> SourceIndex {
        let mut by_entity = self.bindings;
        by_entity.sort_unstable_by_key(entity_sort_key);
        let mut by_source = by_entity.clone();
        by_source.sort_unstable_by_key(source_sort_key);
        SourceIndex {
            by_entity: by_entity.into_boxed_slice(),
            by_source: by_source.into_boxed_slice(),
        }
    }
}

fn entity_sort_key(
    binding: &SourceBinding,
) -> (
    SemanticEntity,
    SourceRole,
    SourceId,
    ByteOffset,
    ByteOffset,
    u8,
    usize,
) {
    (
        binding.entity,
        binding.role,
        binding.origin.source(),
        binding.origin.span().range().start(),
        binding.origin.span().range().end(),
        binding.origin.syntax().sort_key().0,
        binding.origin.syntax().sort_key().1,
    )
}

fn source_sort_key(
    binding: &SourceBinding,
) -> (
    SourceId,
    ByteOffset,
    ByteOffset,
    SourceRole,
    SemanticEntity,
    u8,
    usize,
) {
    (
        binding.origin.source(),
        binding.origin.span().range().start(),
        binding.origin.span().range().end(),
        binding.role,
        binding.entity,
        binding.origin.syntax().sort_key().0,
        binding.origin.syntax().sort_key().1,
    )
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DuplicateSourceBinding(SourceBinding);

impl DuplicateSourceBinding {
    #[must_use]
    pub const fn binding(self) -> SourceBinding {
        self.0
    }
}

impl fmt::Debug for DuplicateSourceBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DuplicateSourceBinding")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for DuplicateSourceBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("semantic entity already has this exact source binding")
    }
}

impl std::error::Error for DuplicateSourceBinding {}

#[cfg(test)]
mod tests {
    use nocter_declarations::{DeclarationProgramBuilder, ModuleNamespace, ModulePath, Visibility};
    use nocter_model::{DeclarationSiteId, ModuleId, Symbol, SymbolTable};
    use nocter_source::{ByteOffset, SourceFile, SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxElement, SyntaxToken, SyntaxTree, parse};

    use super::{SourceAccess, SourceIndexBuilder, SourceRole};
    use crate::{SemanticEntity, SourceOrigin};

    #[test]
    fn source_projection_is_separate_and_bidirectional() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("index.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        assert!(!tree.has_errors());

        let (module, site) = declaration_ids();
        let name = find_token(&tree, sources.get(source).unwrap(), "main");
        let declaration_origin = SourceOrigin::from_token(&tree, name).unwrap();
        let module_origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut builder = SourceIndexBuilder::new();
        builder
            .insert(
                SemanticEntity::DeclarationSite(site),
                SourceRole::Declaration,
                declaration_origin,
            )
            .unwrap();
        builder
            .insert(
                SemanticEntity::Module(module),
                SourceRole::Implementation,
                module_origin,
            )
            .unwrap();
        let index = builder.finish();

        assert_eq!(
            index
                .bindings_for(SemanticEntity::DeclarationSite(site))
                .len(),
            1
        );
        assert_eq!(index.bindings_at(source, ByteOffset::new(1)).count(), 1);
        assert_eq!(index.bindings_at(source, ByteOffset::new(6)).count(), 2);
        assert_eq!(index.bindings_at(source, ByteOffset::new(31)).count(), 0);
        assert_eq!(index.bindings_in(source).count(), 2);
    }

    #[test]
    fn duplicate_binding_is_rejected_without_collapsing_distinct_roles() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("index.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let (_, site) = declaration_ids();
        let entity = SemanticEntity::DeclarationSite(site);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut builder = SourceIndexBuilder::new();

        builder
            .insert(entity, SourceRole::Declaration, origin)
            .unwrap();
        builder
            .insert(entity, SourceRole::Implementation, origin)
            .unwrap();
        assert!(
            builder
                .insert(entity, SourceRole::Declaration, origin)
                .is_err()
        );
    }

    #[test]
    fn occurrence_access_is_retained_without_weakening_projection_identity() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("index.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let (_, site) = declaration_ids();
        let entity = SemanticEntity::DeclarationSite(site);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let mut builder = SourceIndexBuilder::new();

        builder
            .insert_with_access(
                entity,
                SourceRole::Reference,
                origin,
                SourceAccess::Readonly,
            )
            .unwrap();
        assert!(
            builder
                .insert_with_access(
                    entity,
                    SourceRole::Reference,
                    origin,
                    SourceAccess::Writable,
                )
                .is_err(),
            "one semantic occurrence cannot carry contradictory access facts"
        );
        let index = builder.finish();
        assert_eq!(
            index.bindings_for(entity)[0].access(),
            Some(SourceAccess::Readonly)
        );
    }

    #[test]
    fn a_later_stage_can_extend_without_losing_duplicate_detection() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("index.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let (module, site) = declaration_ids();
        let module_origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let name = find_token(&tree, sources.get(source).unwrap(), "main");
        let site_origin = SourceOrigin::from_token(&tree, name).unwrap();
        let mut initial = SourceIndexBuilder::new();
        initial
            .insert(
                SemanticEntity::Module(module),
                SourceRole::Implementation,
                module_origin,
            )
            .unwrap();

        let mut extended = initial.finish().into_builder();
        assert!(
            extended
                .insert(
                    SemanticEntity::Module(module),
                    SourceRole::Implementation,
                    module_origin,
                )
                .is_err()
        );
        extended
            .insert(
                SemanticEntity::DeclarationSite(site),
                SourceRole::Declaration,
                site_origin,
            )
            .unwrap();
        let index = extended.finish();

        assert_eq!(index.len(), 2);
        assert_eq!(index.bindings_for(SemanticEntity::Module(module)).len(), 1);
    }

    #[test]
    fn syntax_origins_cannot_cross_source_trees() {
        let mut sources = SourceMap::new();
        let first = sources
            .add_bytes(
                SourceName::new("first.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let second = sources
            .add_bytes(
                SourceName::new("second.nct"),
                b"func main(): void { return }\n",
            )
            .unwrap();
        let first_tree = parse(sources.get(first).unwrap(), ParseGoal::ModuleSource);
        let second_tree = parse(sources.get(second).unwrap(), ParseGoal::ModuleSource);
        let first_name = find_token(&first_tree, sources.get(first).unwrap(), "main");

        assert!(SourceOrigin::from_node(&second_tree, first_tree.root_id()).is_err());
        assert!(SourceOrigin::from_token(&second_tree, first_name).is_err());
    }

    fn declaration_ids() -> (ModuleId, DeclarationSiteId) {
        let symbols = SymbolTable::from_spellings(["app"]);
        let app_name = symbols.get("app").unwrap();
        build_declaration_ids(symbols, app_name)
    }

    fn build_declaration_ids(
        symbols: SymbolTable,
        app_name: Symbol,
    ) -> (ModuleId, DeclarationSiteId) {
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let package = builder
            .add_package(
                nocter_model::PackageIdentity::new("workspace:app"),
                app_name,
            )
            .unwrap();
        let module = builder.add_module(package, ModulePath::root()).unwrap();
        builder
            .define_module_namespace(module, ModuleNamespace::default())
            .unwrap();
        let site = builder
            .add_declaration_site(module, Visibility::Private)
            .unwrap();
        let _program = builder.finish().unwrap();
        (module, site)
    }

    fn find_token(tree: &SyntaxTree, source: &SourceFile, spelling: &str) -> SyntaxToken {
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            for child in tree.children(node) {
                match child {
                    SyntaxElement::Node(child) => pending.push(*child),
                    SyntaxElement::Token(token)
                        if source.text_at(token.range()) == Some(spelling) =>
                    {
                        return *token;
                    }
                    SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
                }
            }
        }
        panic!("expected token {spelling}");
    }
}
