use std::fmt;
use std::marker::PhantomData;

macro_rules! machine_ids {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(usize);

            impl $name {
                #[must_use]
                pub const fn index(self) -> usize {
                    self.0
                }
            }

            impl MachineId for $name {
                fn new(index: usize) -> Self {
                    Self(index)
                }

                fn index(self) -> usize {
                    self.index()
                }
            }

            impl fmt::Debug for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(formatter, "{}({})", stringify!($name), self.0)
                }
            }
        )+
    };
}

pub(crate) trait MachineId: Copy {
    fn new(index: usize) -> Self;
    fn index(self) -> usize;
}

machine_ids! {
    MachineFunctionId,
    MachineBlockId,
    MachineValueId,
    MachineStackId,
    MachineAddressId,
    MachinePackId,
    MachineOperationId,
    MachineDropFlagId,
    MachineLinkageId,
    MachineDataId,
    MachineDestructionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MachineTable<I, T> {
    values: Box<[T]>,
    identity: PhantomData<fn() -> I>,
}

impl<I: MachineId, T> MachineTable<I, T> {
    pub(crate) fn from_values(values: impl Into<Box<[T]>>) -> Self {
        Self {
            values: values.into(),
            identity: PhantomData,
        }
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = (I, &T)> {
        self.values
            .iter()
            .enumerate()
            .map(|(index, value)| (I::new(index), value))
    }

    pub(crate) const fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) const fn values(&self) -> &[T] {
        &self.values
    }
}
