#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IrModule {
    pub(crate) functions: Vec<Function>,
}

impl IrModule {
    pub(crate) fn new(functions: Vec<Function>) -> Self {
        Self { functions }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) return_type: Type,
    pub(crate) instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Instruction {
    // Lowering will produce this for top-level `fail` once error construction is wired in.
    #[allow(dead_code)]
    WriteStaticStderr(Vec<u8>),
    ReturnI32(i32),
    ReturnVoid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Type {
    I32,
    Void,
    Fallible(Box<Type>),
}

impl Type {
    pub(crate) fn success_type(&self) -> &Type {
        match self {
            Self::Fallible(success) => success,
            Self::I32 | Self::Void => self,
        }
    }
}
