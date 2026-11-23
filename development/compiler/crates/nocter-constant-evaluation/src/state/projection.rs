//! Executable component ordinals or selections through an owned, previously bounded snapshot.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Projection {
    Field(usize),
    /// A field of one declared enum case. Resolving it requires that case to be active.
    EnumField {
        tag: u64,
        field: usize,
    },
    Index {
        slot: usize,
        bound: u64,
    },
}

/// An address with any dynamic array selections resolved to their saved indices.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Coordinate {
    Field(usize),
    EnumField { tag: u64, field: usize },
}
impl Coordinate {
    pub(super) fn array_index(self) -> Option<u64> {
        match self {
            Self::Field(index) => u64::try_from(index).ok(),
            Self::EnumField { .. } => None,
        }
    }

    pub(super) fn data(self, ty: &super::Type) -> Option<usize> {
        match self {
            Self::Field(index) => ty.component(index),
            Self::EnumField { tag, field } => ty.enum_field(tag, field),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProjectionPath(Box<[Projection]>);
impl ProjectionPath {
    pub fn indices(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.iter().filter_map(|projection| match *projection {
            Projection::Index { slot, bound } => Some((slot, bound)),
            Projection::Field(_) | Projection::EnumField { .. } => None,
        })
    }
}
impl std::ops::Deref for ProjectionPath {
    type Target = [Projection];
    fn deref(&self) -> &[Projection] {
        &self.0
    }
}
impl FromIterator<Projection> for ProjectionPath {
    fn from_iter<I: IntoIterator<Item = Projection>>(items: I) -> Self {
        Self(items.into_iter().collect())
    }
}
impl From<Vec<Projection>> for ProjectionPath {
    fn from(items: Vec<Projection>) -> Self {
        Self(items.into())
    }
}
impl From<Vec<usize>> for ProjectionPath {
    fn from(items: Vec<usize>) -> Self {
        items.into_iter().map(Projection::Field).collect()
    }
}
impl From<&[usize]> for ProjectionPath {
    fn from(items: &[usize]) -> Self {
        items.iter().copied().map(Projection::Field).collect()
    }
}

/// Missing coverage under one owner. A prefix is a region, never a selected element address.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Missing {
    Value(ProjectionPath),
    ArrayPrefix {
        base: ProjectionPath,
        endpoint: usize,
        bound: u64,
    },
}
impl Missing {
    pub fn indices(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        let (base, endpoint) = match self {
            Self::Value(path) => (path, None),
            Self::ArrayPrefix {
                base,
                endpoint,
                bound,
            } => (base, Some((*endpoint, *bound))),
        };
        base.indices().chain(endpoint)
    }
}
impl From<ProjectionPath> for Missing {
    fn from(path: ProjectionPath) -> Self {
        Self::Value(path)
    }
}
impl From<Vec<Projection>> for Missing {
    fn from(path: Vec<Projection>) -> Self {
        Self::Value(path.into())
    }
}
impl From<Vec<usize>> for Missing {
    fn from(path: Vec<usize>) -> Self {
        Self::Value(path.into())
    }
}
impl From<&[usize]> for Missing {
    fn from(path: &[usize]) -> Self {
        Self::Value(path.into())
    }
}
