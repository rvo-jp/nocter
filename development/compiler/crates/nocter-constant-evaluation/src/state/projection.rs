//! Executable component ordinals or selections through an owned, previously bounded snapshot.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Projection {
    Field(usize),
    Index { slot: usize, bound: u64 },
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProjectionPath(Box<[Projection]>);
impl ProjectionPath {
    pub fn indices(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.iter().filter_map(|projection| match *projection {
            Projection::Index { slot, bound } => Some((slot, bound)),
            Projection::Field(_) => None,
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
