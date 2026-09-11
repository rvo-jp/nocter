/// Compiler-owned field vocabulary inside package directive records.
///
/// Dependency aliases remain authored names and are intentionally not classified here.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum PackageFieldName {
    Name,
    Version,
    Git,
    Revision,
    Archive,
    Path,
    Commit,
    Sha256,
    Module,
}

impl PackageFieldName {
    pub(crate) const PACKAGE: &'static [Self] = &[Self::Name, Self::Version];
    pub(crate) const DEPENDENCY: &'static [Self] = &[
        Self::Git,
        Self::Revision,
        Self::Archive,
        Self::Path,
        Self::Commit,
        Self::Sha256,
    ];
    pub(crate) const TARGET: &'static [Self] = &[Self::Name, Self::Module];

    pub(crate) fn from_spelling(spelling: &str) -> Option<Self> {
        match spelling {
            "name" => Some(Self::Name),
            "version" => Some(Self::Version),
            "git" => Some(Self::Git),
            "revision" => Some(Self::Revision),
            "archive" => Some(Self::Archive),
            "path" => Some(Self::Path),
            "commit" => Some(Self::Commit),
            "sha256" => Some(Self::Sha256),
            "module" => Some(Self::Module),
            _ => None,
        }
    }

    pub(crate) const fn spelling(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Version => "version",
            Self::Git => "git",
            Self::Revision => "revision",
            Self::Archive => "archive",
            Self::Path => "path",
            Self::Commit => "commit",
            Self::Sha256 => "sha256",
            Self::Module => "module",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::PackageFieldName;

    #[test]
    fn accepted_schema_fields_are_unique_and_round_trip() {
        let fields = PackageFieldName::PACKAGE
            .iter()
            .chain(PackageFieldName::DEPENDENCY)
            .chain(PackageFieldName::TARGET)
            .copied()
            .collect::<BTreeSet<_>>();
        for field in fields {
            assert_eq!(
                PackageFieldName::from_spelling(field.spelling()),
                Some(field)
            );
        }
        assert_eq!(PackageFieldName::from_spelling("unknown"), None);
    }
}
