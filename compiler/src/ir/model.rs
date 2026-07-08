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
    WriteStaticStderr(Vec<u8>),
    SetI32 {
        destination: I32Location,
        value: I32Value,
    },
    AddI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    TailCall {
        function: String,
        arguments: Vec<I32Value>,
    },
    If {
        condition: BoolValue,
        then_instructions: Vec<Instruction>,
        else_instructions: Vec<Instruction>,
    },
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum I32Location {
    Return,
    Parameter(usize),
    Local(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum I32Value {
    Const(i32),
    Location(I32Location),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoolValue {
    Const(bool),
    I32Comparison {
        operator: I32ComparisonOperator,
        left: I32Value,
        right: I32Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum I32ComparisonOperator {
    Equal,
    NotEqual,
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
