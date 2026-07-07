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
    ReturnI32(i32),
    ReturnVoid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Type {
    I32,
    Void,
}
