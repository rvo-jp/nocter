use std::fmt;

use nocter_model::Symbol;

use crate::{ExportedEntity, Visibility};

/// One authored name in a module's canonical namespace.
///
/// The visibility is the effective boundary of this binding, after resolving relative source
/// syntax and any re-export restriction. The target may belong to another module when this entry
/// is a re-export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceEntry {
    name: Symbol,
    target: ExportedEntity,
    visibility: Visibility,
}

impl NamespaceEntry {
    #[must_use]
    pub const fn new(name: Symbol, target: ExportedEntity, visibility: Visibility) -> Self {
        Self {
            name,
            target,
            visibility,
        }
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn target(self) -> ExportedEntity {
        self.target
    }

    #[must_use]
    pub const fn visibility(self) -> Visibility {
        self.visibility
    }
}

/// One compiler-selected prelude fallback.
///
/// Fallback entries are deliberately distinct from authored entries: local lookup may use them,
/// but export lookup never can. An authored name with the same symbol shadows the fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallbackEntry {
    name: Symbol,
    target: ExportedEntity,
}

impl FallbackEntry {
    #[must_use]
    pub const fn new(name: Symbol, target: ExportedEntity) -> Self {
        Self { name, target }
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn target(self) -> ExportedEntity {
        self.target
    }
}

/// The immutable value/type namespace selected during declaration lowering for one module.
///
/// Both arrays are sorted by canonical `Symbol`. Keeping the fallback separate preserves the
/// language rule that it is shadowable and cannot be re-exported.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleNamespace {
    authored: Box<[NamespaceEntry]>,
    fallback: Box<[FallbackEntry]>,
}

impl ModuleNamespace {
    /// Builds a canonical namespace.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateNamespaceName`] if either input contains the same symbol twice.
    pub fn new(
        authored: impl IntoIterator<Item = NamespaceEntry>,
        fallback: impl IntoIterator<Item = FallbackEntry>,
    ) -> Result<Self, DuplicateNamespaceName> {
        let mut authored: Vec<_> = authored.into_iter().collect();
        authored.sort_unstable_by_key(|entry| entry.name());
        reject_duplicate_names(authored.iter().map(|entry| entry.name()))?;

        let mut fallback: Vec<_> = fallback.into_iter().collect();
        fallback.sort_unstable_by_key(|entry| entry.name());
        reject_duplicate_names(fallback.iter().map(|entry| entry.name()))?;

        Ok(Self {
            authored: authored.into_boxed_slice(),
            fallback: fallback.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn authored(&self) -> &[NamespaceEntry] {
        &self.authored
    }

    #[must_use]
    pub const fn fallback(&self) -> &[FallbackEntry] {
        &self.fallback
    }

    /// Looks up the exact name visible inside this module.
    ///
    /// Authored entries take precedence over compiler-selected prelude fallback entries.
    #[must_use]
    pub fn lookup_local(&self, name: Symbol) -> Option<ExportedEntity> {
        self.lookup_authored(name)
            .map(NamespaceEntry::target)
            .or_else(|| {
                self.fallback
                    .binary_search_by_key(&name, |entry| entry.name())
                    .ok()
                    .map(|index| self.fallback[index].target())
            })
    }

    /// Looks up only the authored surface. Prelude fallback is never exportable.
    #[must_use]
    pub fn lookup_authored(&self, name: Symbol) -> Option<NamespaceEntry> {
        self.authored
            .binary_search_by_key(&name, |entry| entry.name())
            .ok()
            .map(|index| self.authored[index])
    }
}

fn reject_duplicate_names(
    names: impl IntoIterator<Item = Symbol>,
) -> Result<(), DuplicateNamespaceName> {
    let mut previous = None;
    for name in names {
        if previous == Some(name) {
            return Err(DuplicateNamespaceName(name));
        }
        previous = Some(name);
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DuplicateNamespaceName(Symbol);

impl DuplicateNamespaceName {
    #[must_use]
    pub const fn name(self) -> Symbol {
        self.0
    }
}

impl fmt::Debug for DuplicateNamespaceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DuplicateNamespaceName")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for DuplicateNamespaceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("module namespace contains a duplicate name")
    }
}

impl std::error::Error for DuplicateNamespaceName {}

#[cfg(test)]
mod tests {
    use nocter_model::SymbolTable;

    use super::{FallbackEntry, ModuleNamespace, NamespaceEntry};
    use crate::{DeclarationProgramBuilder, ExportedEntity, ModulePath, Visibility};

    #[test]
    fn authored_names_shadow_but_do_not_merge_with_fallback_names() {
        let symbols = SymbolTable::from_spellings(["app", "first", "second", "value"]);
        let app_name = symbols.get("app").unwrap();
        let first_name = symbols.get("first").unwrap();
        let second_name = symbols.get("second").unwrap();
        let name = symbols.get("value").unwrap();
        let mut program =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let package = program.add_package(app_name).unwrap();
        let first = program
            .add_module(package, ModulePath::from_segments([first_name]))
            .unwrap();
        let second = program
            .add_module(package, ModulePath::from_segments([second_name]))
            .unwrap();
        let authored_target = ExportedEntity::Module(first);
        let fallback_target = ExportedEntity::Module(second);
        let namespace = ModuleNamespace::new(
            [NamespaceEntry::new(
                name,
                authored_target,
                Visibility::Private,
            )],
            [FallbackEntry::new(name, fallback_target)],
        )
        .unwrap();

        assert_eq!(namespace.lookup_local(name), Some(authored_target));
        assert_eq!(
            namespace.lookup_authored(name).map(NamespaceEntry::target),
            Some(authored_target)
        );
    }

    #[test]
    fn duplicate_names_are_rejected_within_each_layer() {
        let symbols = SymbolTable::from_spellings(["app", "value"]);
        let app_name = symbols.get("app").unwrap();
        let name = symbols.get("value").unwrap();
        let mut program =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let package = program.add_package(app_name).unwrap();
        let module = program.add_module(package, ModulePath::root()).unwrap();
        let target = ExportedEntity::Module(module);

        assert!(
            ModuleNamespace::new(
                [
                    NamespaceEntry::new(name, target, Visibility::Public),
                    NamespaceEntry::new(name, target, Visibility::Private),
                ],
                [],
            )
            .is_err()
        );
        assert!(
            ModuleNamespace::new(
                [],
                [
                    FallbackEntry::new(name, target),
                    FallbackEntry::new(name, target),
                ],
            )
            .is_err()
        );
    }
}
