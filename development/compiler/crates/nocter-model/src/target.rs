use std::fmt;

/// One target name recognized by this compiler release.
///
/// Recognition and implementation availability are deliberately separate. Frontend selection
/// may build a target-specific semantic program for a reserved target, while the target-program
/// boundary rejects it until every required backend component exists.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilationTarget {
    Arm64Darwin,
    X64Linux,
    Arm64Linux,
    X64Windows,
    Arm64Windows,
}

impl CompilationTarget {
    pub const ALL: [Self; 5] = [
        Self::Arm64Darwin,
        Self::X64Linux,
        Self::Arm64Linux,
        Self::X64Windows,
        Self::Arm64Windows,
    ];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "arm64-darwin" => Some(Self::Arm64Darwin),
            "x64-linux" => Some(Self::X64Linux),
            "arm64-linux" => Some(Self::Arm64Linux),
            "x64-windows" => Some(Self::X64Windows),
            "arm64-windows" => Some(Self::Arm64Windows),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Arm64Darwin => "arm64-darwin",
            Self::X64Linux => "x64-linux",
            Self::Arm64Linux => "arm64-linux",
            Self::X64Windows => "x64-windows",
            Self::Arm64Windows => "arm64-windows",
        }
    }

    /// Reports backend completeness, not whether the target name is valid source syntax.
    #[must_use]
    pub const fn is_implemented(self) -> bool {
        matches!(self, Self::Arm64Darwin)
    }
}

impl fmt::Display for CompilationTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::CompilationTarget;

    #[test]
    fn recognized_names_round_trip_without_implying_backend_support() {
        for target in CompilationTarget::ALL {
            assert_eq!(CompilationTarget::from_name(target.name()), Some(target));
        }
        assert!(CompilationTarget::Arm64Darwin.is_implemented());
        assert!(!CompilationTarget::X64Linux.is_implemented());
        assert_eq!(CompilationTarget::from_name("mips-templeos"), None);
    }
}
