/// Stable compiler-owned reasons encoded in ARM64 `brk` instructions.
///
/// These values distinguish failures in native images without requiring a runtime. New target
/// code must allocate a reason here rather than embedding an unrelated immediate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64RuntimeTrap {
    MirTrap,
    MirUnreachable,
    Bounds,
    ExplicitTrap,
    ExplicitUnreachable,
    AllocationFailure,
    ExactSizeIteratorViolation,
    RegionReleaseFailure,
    ProcessIndexOutOfBounds,
    ErrorNodeCorruption,
    ErrorReleaseFailure,
}

impl Arm64RuntimeTrap {
    pub(crate) const fn immediate(self) -> u16 {
        match self {
            Self::MirTrap => 1,
            Self::MirUnreachable => 2,
            Self::Bounds => 3,
            Self::ExplicitTrap => 4,
            Self::ExplicitUnreachable => 5,
            Self::AllocationFailure => 6,
            Self::ExactSizeIteratorViolation => 7,
            Self::RegionReleaseFailure => 8,
            Self::ProcessIndexOutOfBounds => 9,
            Self::ErrorNodeCorruption => 10,
            Self::ErrorReleaseFailure => 11,
        }
    }
}
