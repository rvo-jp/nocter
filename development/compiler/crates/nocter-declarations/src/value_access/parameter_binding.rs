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
        match value {
            ValueDefinition::Data(data) => self.matches_data_reference(data),
            ValueDefinition::Function(_) => !matches!(self, Self::Tuple(_)),
        }
    }

    /// Check the given data reference after transparent names have been resolved.
    #[must_use]
    pub fn matches_data_reference(&self, data: &DataReference) -> bool {
        self.matches_tuple_shape(data, &mut |data| match data {
            DataReference::Tuple(items) => Some(items.as_slice().iter()),
            _ => None,
        })
    }

    pub(super) fn matches_tuple_shape<Data, Children>(
        &self,
        data: Data,
        tuple_fields: &mut impl FnMut(Data) -> Option<Children>,
    ) -> bool
    where
        Children: ExactSizeIterator<Item = Data>,
    {
        match self {
            Self::Name(_) | Self::Discard => true,
            Self::Tuple(items) => tuple_fields(data).is_some_and(|fields| {
                items.len() == fields.len()
                    && items
                        .iter()
                        .zip(fields)
                        .all(|(item, field)| item.matches_tuple_shape(field, tuple_fields))
            }),
        }
    }
}
