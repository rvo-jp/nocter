use nocter_syntax::Keyword;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageIdentity(Box<str>);

impl PackageIdentity {
    #[must_use]
    pub fn new(identity: impl Into<Box<str>>) -> Self {
        Self(identity.into())
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleIdentity {
    package: PackageIdentity,
    path: Box<[Box<str>]>,
}

impl ModuleIdentity {
    #[must_use]
    pub fn new<S>(package: PackageIdentity, path: impl IntoIterator<Item = S>) -> Self
    where
        S: Into<Box<str>>,
    {
        Self {
            package,
            path: path
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn path(&self) -> &[Box<str>] {
        &self.path
    }
}

/// Reports whether one authored directory-module segment is canonical Nocter syntax.
#[must_use]
pub fn is_valid_module_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    !bytes.is_empty()
        && segment != "_"
        && !bytes[0].is_ascii_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        && Keyword::from_spelling(segment).is_none()
}
