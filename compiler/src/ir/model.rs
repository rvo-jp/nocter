use crate::abi::ValueLayout;
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
    WriteStr {
        fd: I32Value,
        text: StrValue,
    },
    SetI32 {
        destination: I32Location,
        value: I32Value,
    },
    SetU8 {
        destination: U8Location,
        value: U8Value,
    },
    SetUsize {
        destination: UsizeLocation,
        value: UsizeValue,
    },
    SetBool {
        destination: BoolLocation,
        value: BoolValue,
    },
    SetStr {
        destination: StrLocation,
        value: StrValue,
    },
    SetSlice {
        destination: SliceLocation,
        value: SliceValue,
    },
    #[allow(dead_code)]
    ReserveAggregateSlot {
        slot_index: usize,
        layout: ValueLayout,
    },
    #[allow(dead_code)]
    StoreAggregateUsize {
        destination: AggregateLocation,
        offset: u32,
        value: UsizeValue,
    },
    #[allow(dead_code)]
    StoreAggregateI32 {
        destination: AggregateLocation,
        offset: u32,
        value: I32Value,
    },
    #[allow(dead_code)]
    StoreAggregateU8 {
        destination: AggregateLocation,
        offset: u32,
        value: U8Value,
    },
    #[allow(dead_code)]
    StoreAggregateBool {
        destination: AggregateLocation,
        offset: u32,
        value: BoolValue,
    },
    #[allow(dead_code)]
    LoadAggregateUsize {
        destination: UsizeLocation,
        source: AggregateLocation,
        offset: u32,
    },
    #[allow(dead_code)]
    LoadAggregateI32 {
        destination: I32Location,
        source: AggregateLocation,
        offset: u32,
    },
    #[allow(dead_code)]
    LoadAggregateU8 {
        destination: U8Location,
        source: AggregateLocation,
        offset: u32,
    },
    #[allow(dead_code)]
    LoadAggregateBool {
        destination: BoolLocation,
        source: AggregateLocation,
        offset: u32,
    },
    #[allow(dead_code)]
    CopyAggregate {
        destination: AggregateLocation,
        source: AggregateLocation,
        layout: ValueLayout,
    },
    CopyAggregateRange {
        destination: AggregateLocation,
        destination_offset: u32,
        source: AggregateLocation,
        source_offset: u32,
        layout: ValueLayout,
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
    AddUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    SubtractUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    MultiplyUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    DivideUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    RemainderUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    ShiftLeftUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    ShiftRightUsize {
        destination: UsizeLocation,
        left: UsizeValue,
        right: UsizeValue,
    },
    #[allow(dead_code)]
    CallI32 {
        destination: I32Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleI32 {
        destination: I32Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallU8 {
        destination: U8Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleU8 {
        destination: U8Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallUsize {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleUsize {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallStr {
        destination: StrLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleStr {
        destination: StrLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallSlice {
        destination: SliceLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleSlice {
        destination: SliceLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    #[allow(dead_code)]
    CallAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    #[allow(dead_code)]
    CallDirectAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        layout: ValueLayout,
    },
    CallFallibleDirectAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        layout: ValueLayout,
        failure_mode: FallibleFailureMode,
    },
    CallFallibleAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
    },
    CallVoid {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallFallibleVoid {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: FallibleFailureMode,
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
    While {
        condition_instructions: Vec<Instruction>,
        condition: BoolValue,
        body_instructions: Vec<Instruction>,
    },
    Break,
    Continue,
    PropagateFailure,
    TrapOnFailure,
    CheckFailure {
        failure_mode: FallibleFailureMode,
    },
    ReturnFallibleSuccess,
    ReturnFallibleFailure {
        code: StrValue,
        message: StrValue,
    },
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FallibleFailureMode {
    Propagate,
    PropagateWithCleanup {
        code: StrLocation,
        message: StrLocation,
        instructions: Vec<Instruction>,
    },
    Trap,
    Catch {
        code: StrLocation,
        message: StrLocation,
        instructions: Vec<Instruction>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum AggregateLocation {
    Return,
    DirectReturn,
    Parameter(usize),
    DirectParameter { start_index: usize },
    Slot(usize),
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
    U8ZeroExtend(Box<U8Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum U8Location {
    Return,
    Parameter(usize),
    Local(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum U8Value {
    Const(u8),
    Location(U8Location),
    StrIndex {
        source: StrLocation,
        index: UsizeValue,
    },
    StaticStrIndex {
        bytes: Vec<u8>,
        index: UsizeValue,
    },
    SliceIndex {
        source: SliceLocation,
        index: UsizeValue,
    },
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
    U8ZeroExtend(Box<U8Value>),
    StrLen(StrLocation),
    SliceLen(SliceLocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarArgument {
    I32(I32Value),
    U8(U8Value),
    Usize(UsizeValue),
    Bool(BoolValue),
    Str(StrValue),
    Slice(SliceValue),
    Borrow(BorrowArgument),
    AggregateIndirect(AggregateArgument),
    AggregateDirect(DirectAggregateArgument),
}

impl ScalarArgument {
    pub(crate) fn abi_word_count(&self) -> usize {
        match self {
            Self::I32(_)
            | Self::U8(_)
            | Self::Usize(_)
            | Self::Bool(_)
            | Self::Borrow(_)
            | Self::AggregateIndirect(_) => 1,
            Self::Str(_) | Self::Slice(_) => 2,
            Self::AggregateDirect(argument) => argument.words,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AggregateArgument {
    pub(crate) source: AggregateArgumentSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectAggregateArgument {
    pub(crate) source: AggregateArgumentSource,
    pub(crate) layout: ValueLayout,
    pub(crate) words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateArgumentSource {
    Slot(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BorrowArgument {
    pub(crate) source: BorrowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowSource {
    I32(I32Location),
    U8(U8Location),
    Usize(UsizeLocation),
    Bool(BoolLocation),
    AggregateSlot(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StrValue {
    StaticBytes(Vec<u8>),
    Location(StrLocation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrLocation {
    Return,
    Parameter(usize),
    Local(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SliceValue {
    Location(SliceLocation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SliceLocation {
    Return,
    Parameter(usize),
    Local(usize),
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
    U8,
    Usize,
    Bool,
    Str,
    Slice {
        is_readwrite: bool,
    },
    Aggregate {
        layout: ValueLayout,
    },
    DirectAggregate {
        layout: ValueLayout,
        words: usize,
    },
    Borrow {
        is_readwrite: bool,
        inner: Box<Type>,
    },
    Void,
    Never,
    Fallible(Box<Type>),
}

impl Type {
    pub(crate) fn success_type(&self) -> &Type {
        match self {
            Self::Fallible(success) => success,
            Self::I32
            | Self::U8
            | Self::Usize
            | Self::Bool
            | Self::Str
            | Self::Slice { .. }
            | Self::Aggregate { .. }
            | Self::DirectAggregate { .. }
            | Self::Borrow { .. }
            | Self::Void
            | Self::Never => self,
        }
    }
}
