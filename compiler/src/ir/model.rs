use crate::source::SourceId;

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
    pub(crate) target: CallTarget,
    pub(crate) return_type: Type,
    pub(crate) instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CallTarget {
    SameFile(String),
    Imported { source: SourceId, name: String },
}

impl CallTarget {
    pub(crate) fn same_file(name: impl Into<String>) -> Self {
        Self::SameFile(name.into())
    }

    pub(crate) fn imported(source: SourceId, name: impl Into<String>) -> Self {
        Self::Imported {
            source,
            name: name.into(),
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
    SetUsize {
        destination: UsizeLocation,
        value: UsizeValue,
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
        arguments: Vec<ScalarArgument>,
    },
    #[allow(dead_code)]
    CallUsize {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    #[allow(dead_code)]
    CallBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    TailCall {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    Trap,
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
pub(crate) enum UsizeLocation {
    Return,
    Parameter(usize),
    Local(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UsizeValue {
    Const(u64),
    Location(UsizeLocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarArgument {
    I32(I32Value),
    Usize(UsizeValue),
    Bool(BoolValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoolLocation {
    Return,
    Parameter(usize),
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
    UsizeComparison {
        operator: I32ComparisonOperator,
        left: UsizeValue,
        right: UsizeValue,
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
    Usize,
    Bool,
    Void,
    Never,
    Fallible(Box<Type>),
}

impl Type {
    pub(crate) fn success_type(&self) -> &Type {
        match self {
            Self::Fallible(success) => success,
            Self::I32 | Self::Usize | Self::Bool | Self::Void | Self::Never => self,
        }
    }
}
