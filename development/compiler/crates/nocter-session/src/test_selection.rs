/// User-visible selection of package-declared test targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestTargetSelector {
    All,
    Named(Box<str>),
}

impl TestTargetSelector {
    #[must_use]
    pub fn named(name: impl Into<Box<str>>) -> Self {
        Self::Named(name.into())
    }
}
