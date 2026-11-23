//! Formal input binding shape, independent of the source and callable data identity.
use super::{DataReference, ValueDefinition};
use nocter_model::{FieldPath, Symbol, ValueField};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParameterBinding<Name = Symbol> {
    Name(Name),
    Discard,
    Tuple(Box<[Self]>),
}

impl<Name> ParameterBinding<Name> {
    /// Rebind names without changing which parts of the formal input are bound.
    /// # Errors
    /// Returns the first error from the name projection.
    pub fn try_map<T, E>(
        &self,
        map: &mut impl FnMut(&Name) -> Result<T, E>,
    ) -> Result<ParameterBinding<T>, E> {
        Ok(match self {
            Self::Name(name) => ParameterBinding::Name(map(name)?),
            Self::Discard => ParameterBinding::Discard,
            Self::Tuple(items) => ParameterBinding::Tuple(
                items
                    .iter()
                    .map(|item| item.try_map(map))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }

    pub fn names(&self) -> impl Iterator<Item = &Name> {
        self.walk_names((), |(), _| ()).map(|(name, ())| name)
    }

    pub fn named_paths(&self) -> impl Iterator<Item = (&Name, FieldPath)> {
        self.walk_names(FieldPath::default(), |path, index| {
            path.project(ValueField::Tuple(index))
        })
    }

    fn walk_names<Path>(
        &self,
        root: Path,
        mut project: impl FnMut(&Path, usize) -> Path,
    ) -> impl Iterator<Item = (&Name, Path)> {
        let mut pending = vec![(self, root)];
        std::iter::from_fn(move || {
            while let Some((binding, path)) = pending.pop() {
                match binding {
                    Self::Name(name) => return Some((name, path)),
                    Self::Discard => {}
                    Self::Tuple(items) => {
                        pending.extend(
                            items
                                .iter()
                                .enumerate()
                                .rev()
                                .map(|(index, item)| (item, project(&path, index))),
                        );
                    }
                }
            }
            None
        })
    }

    pub(super) fn matches_data(&self, value: &ValueDefinition) -> bool {
        fn matches<Name>(binding: &ParameterBinding<Name>, data: &DataReference) -> bool {
            let ParameterBinding::Tuple(items) = binding else {
                return true;
            };
            let DataReference::Tuple(fields) = data else {
                return false;
            };
            items.len() == fields.as_slice().len()
                && items
                    .iter()
                    .zip(fields.as_slice())
                    .all(|(item, field)| matches(item, field))
        }
        match value {
            ValueDefinition::Data(data) => matches(self, data),
            ValueDefinition::Function(_) => !matches!(self, Self::Tuple(_)),
        }
    }
}
