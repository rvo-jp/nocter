use std::num::{NonZeroU32, NonZeroU64};

/// Deterministic resource limits shared by every compile-time evaluation entry.
///
/// Limits count semantic plan operations and source-call depth, not host instructions or elapsed
/// time. The same input therefore reaches the same limit on every compiler host.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeEvaluationLimits {
    steps: NonZeroU64,
    call_depth: NonZeroU32,
}

impl CompileTimeEvaluationLimits {
    #[must_use]
    pub const fn new(steps: NonZeroU64, call_depth: NonZeroU32) -> Self {
        Self { steps, call_depth }
    }

    #[must_use]
    pub const fn steps(self) -> NonZeroU64 {
        self.steps
    }

    #[must_use]
    pub const fn call_depth(self) -> NonZeroU32 {
        self.call_depth
    }
}

impl Default for CompileTimeEvaluationLimits {
    fn default() -> Self {
        Self {
            steps: NonZeroU64::new(1_000_000).expect("nonzero compile-time step limit"),
            call_depth: NonZeroU32::new(256).expect("nonzero compile-time call-depth limit"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CompileTimeEvaluationLimits;

    #[test]
    fn default_limits_are_explicit_semantic_counts() {
        let limits = CompileTimeEvaluationLimits::default();

        assert_eq!(limits.steps().get(), 1_000_000);
        assert_eq!(limits.call_depth().get(), 256);
    }
}
