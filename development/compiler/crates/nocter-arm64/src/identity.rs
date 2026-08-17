use std::fmt;

macro_rules! arm64_ids {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(pub(crate) usize);

            impl fmt::Debug for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(formatter, "{}({})", stringify!($name), self.0)
                }
            }
        )+
    };
}

arm64_ids! {
    Arm64FunctionId,
    Arm64DataId,
}
