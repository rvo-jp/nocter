//! Sparse unavailable coverage under one owning payload, independent of physical layout.
use super::{ArrayFrontier, Holder, Path};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Missing {
    Value(Path),
    ArrayPrefix {
        array: Path,
        frontier: ArrayFrontier,
    },
}
impl Missing {
    /// Structural descent only: a region's anchor is not a missing element address.
    pub(super) fn anchor(&self) -> &Path {
        match self {
            Self::Value(path) => path,
            Self::ArrayPrefix { array, .. } => array,
        }
    }
    #[must_use]
    pub fn value(&self) -> Option<&Path> {
        match self {
            Self::Value(path) => Some(path),
            Self::ArrayPrefix { .. } => None,
        }
    }
    pub(super) fn overlaps(&self, path: &Path) -> bool {
        match self {
            Self::Value(hole) => hole.overlaps(path),
            Self::ArrayPrefix { array, .. } => array.overlaps(path),
        }
    }
    pub(super) fn under(&self, path: &Path) -> bool {
        match self {
            Self::Value(hole) => path.is_prefix_of(hole),
            Self::ArrayPrefix { array, .. } => path.is_prefix_of(array),
        }
    }
    pub(super) fn blocks_container(&self, path: &Path) -> bool {
        match self {
            Self::Value(hole) => {
                hole != path && hole.fields().len() <= path.fields().len() && hole.overlaps(path)
            }
            Self::ArrayPrefix { array, .. } => {
                path.fields().len() > array.fields().len() && array.overlaps(path)
            }
        }
    }
    pub(super) fn backing(&self) -> impl Iterator<Item = Holder> + '_ {
        let (path, endpoint) = match self {
            Self::Value(path) => (path, None),
            Self::ArrayPrefix { array, frontier } => {
                (array, Some(Holder::ArrayFrontier(*frontier)))
            }
        };
        path.retained().map(Holder::IndexSnapshot).chain(endpoint)
    }
}
impl From<Path> for Missing {
    fn from(path: Path) -> Self {
        Self::Value(path)
    }
}
