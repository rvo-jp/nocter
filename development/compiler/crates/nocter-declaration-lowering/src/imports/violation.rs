use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule for authored module imports and re-exports.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportRule {
    MissingImportedName,
    InaccessibleImportedName,
    WideningReexport,
    CompilerManagedPreludeImport,
    NonTypeSelection,
    NamespaceAliasReexport,
}

impl ImportRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingImportedName => "E0260",
            Self::InaccessibleImportedName => "E0412",
            Self::WideningReexport => "E0261",
            Self::CompilerManagedPreludeImport => "E0262",
            Self::NonTypeSelection => "E0264",
            Self::NamespaceAliasReexport => "E0265",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingImportedName => "selected import name does not exist in the target module",
            Self::InaccessibleImportedName => {
                "selected import name is outside its visibility boundary"
            }
            Self::WideningReexport => {
                "re-export visibility is wider than the selected name's visibility"
            }
            Self::CompilerManagedPreludeImport => {
                "source code imports the compiler-managed standard prelude"
            }
            Self::NonTypeSelection => "selected imports can introduce only types and interfaces",
            Self::NamespaceAliasReexport => "a module namespace alias cannot be re-exported",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::MissingImportedName => {
                "correct the selected name or export it from the target module"
            }
            Self::InaccessibleImportedName => {
                "import a public name or move the import inside the declared visibility boundary"
            }
            Self::WideningReexport => {
                "narrow the re-export visibility to the selected name's visibility boundary"
            }
            Self::CompilerManagedPreludeImport => {
                "remove the import; the standard prelude is available implicitly"
            }
            Self::NonTypeSelection => {
                "import the owning module namespace and access this name through that namespace"
            }
            Self::NamespaceAliasReexport => {
                "remove pub or re-export the module under its canonical name"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::InaccessibleImportedName | Self::WideningReexport => {
                Some("the selected name is declared here")
            }
            Self::MissingImportedName
            | Self::CompilerManagedPreludeImport
            | Self::NonTypeSelection
            | Self::NamespaceAliasReexport => None,
        }
    }
}

/// Exact syntax subjects for one authored import-rule violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportViolation {
    rule: ImportRule,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
}

impl ImportViolation {
    #[must_use]
    pub const fn missing_imported_name(name: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::MissingImportedName,
            primary: name,
            related: None,
        }
    }

    #[must_use]
    pub const fn inaccessible_imported_name(name: SyntaxOrigin, declaration: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::InaccessibleImportedName,
            primary: name,
            related: Some(declaration),
        }
    }

    #[must_use]
    pub const fn widening_reexport(visibility: SyntaxOrigin, declaration: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::WideningReexport,
            primary: visibility,
            related: Some(declaration),
        }
    }

    #[must_use]
    pub const fn compiler_managed_prelude_import(path: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::CompilerManagedPreludeImport,
            primary: path,
            related: None,
        }
    }

    #[must_use]
    pub const fn non_type_selection(name: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::NonTypeSelection,
            primary: name,
            related: None,
        }
    }

    #[must_use]
    pub const fn namespace_alias_reexport(alias: SyntaxOrigin) -> Self {
        Self {
            rule: ImportRule::NamespaceAliasReexport,
            primary: alias,
            related: None,
        }
    }

    #[must_use]
    pub const fn rule(self) -> ImportRule {
        self.rule
    }

    #[must_use]
    pub const fn primary(self) -> SyntaxOrigin {
        self.primary
    }

    #[must_use]
    pub const fn related(self) -> Option<SyntaxOrigin> {
        self.related
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::ImportRule;

    #[test]
    fn import_rules_own_distinct_diagnostic_codes() {
        let rules = [
            ImportRule::MissingImportedName,
            ImportRule::InaccessibleImportedName,
            ImportRule::WideningReexport,
            ImportRule::CompilerManagedPreludeImport,
            ImportRule::NonTypeSelection,
            ImportRule::NamespaceAliasReexport,
        ];
        let codes = rules
            .into_iter()
            .map(ImportRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), rules.len());
    }
}
