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
pub(crate) enum CallTarget {
    SameFile(String),
}

impl CallTarget {
    pub(crate) fn same_file(name: impl Into<String>) -> Self {
        Self::SameFile(name.into())
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            Self::SameFile(name) => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Instruction {
    WriteStaticStderr(Vec<u8>),
    SetI32 {
        destination: I32Location,
        value: I32Value,
    },
    SetBool {
        destination: BoolLocation,
        value: BoolValue,
    },
    AddI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    SubtractI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    MultiplyI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    DivideI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    RemainderI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    ShiftLeftI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    ShiftRightI32 {
        destination: I32Location,
        left: I32Value,
        right: I32Value,
    },
    #[allow(dead_code)]
    CallI32 {
        destination: I32Location,
        target: CallTarget,
        arguments: Vec<I32Value>,
    },
    #[allow(dead_code)]
    CallBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<I32Value>,
    },
    TailCall {
        target: CallTarget,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolLocation {
    Return,
    Local(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoolValue {
    Const(bool),
    Location(BoolLocation),
    Not(Box<BoolValue>),
    Logical {
        operator: BoolLogicalOperator,
        left: Box<BoolValue>,
        right: Box<BoolValue>,
    },
    I32Comparison {
        operator: I32ComparisonOperator,
        left: I32Value,
        right: I32Value,
    },
    BoolComparison {
        operator: BoolComparisonOperator,
        left: Box<BoolValue>,
        right: Box<BoolValue>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolLogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum I32ComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolComparisonOperator {
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Type {
    I32,
    Bool,
    Void,
    Fallible(Box<Type>),
}

impl Type {
    pub(crate) fn success_type(&self) -> &Type {
        match self {
            Self::Fallible(success) => success,
            Self::I32 | Self::Bool | Self::Void => self,
        }
    }
}
