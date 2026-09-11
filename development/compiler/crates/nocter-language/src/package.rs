/// Closed names accepted in the package-directive prefix of a package root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PackageDirectiveName {
    Package,
    Dependencies,
    Executable,
    Test,
}

impl PackageDirectiveName {
    pub const ALL: &'static [Self] = &[
        Self::Package,
        Self::Dependencies,
        Self::Executable,
        Self::Test,
    ];

    #[must_use]
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "package" => Some(Self::Package),
            "dependencies" => Some(Self::Dependencies),
            "executable" => Some(Self::Executable),
            "test" => Some(Self::Test),
            _ => None,
        }
    }

    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Dependencies => "dependencies",
            Self::Executable => "executable",
            Self::Test => "test",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::PackageDirectiveName;

    #[test]
    fn spellings_are_unique_and_round_trip() {
        let spellings = PackageDirectiveName::ALL
            .iter()
            .map(|directive| directive.spelling())
            .collect::<BTreeSet<_>>();

        assert_eq!(spellings.len(), PackageDirectiveName::ALL.len());
        for directive in PackageDirectiveName::ALL {
            assert_eq!(
                PackageDirectiveName::from_spelling(directive.spelling()),
                Some(*directive)
            );
        }
        assert_eq!(PackageDirectiveName::from_spelling("lock"), None);
    }
}
