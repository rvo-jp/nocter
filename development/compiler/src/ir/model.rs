use crate::abi::{ReturnPassing, ValueLayout};
use crate::outcomes::OutcomeLayer;
use crate::outcomes::storage::OutcomeStorageLayout;
use crate::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IrModule {
    pub(crate) entry: CallTarget,
    pub(crate) functions: Vec<Function>,
}

impl IrModule {
    #[cfg(test)]
    pub(crate) fn new(functions: Vec<Function>) -> Self {
        Self::with_entry(CallTarget::same_file("main"), functions)
    }

    pub(crate) fn with_entry(entry: CallTarget, functions: Vec<Function>) -> Self {
        Self { entry, functions }
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
    WriteSlice {
        fd: I32Value,
        bytes: SliceValue,
    },
    ReadSlice {
        destination: UsizeLocation,
        fd: I32Value,
        buffer: SliceValue,
        failure_mode: OutcomeFailureMode,
    },
    OpenRead {
        destination: I32Location,
        path: UsizeValue,
        flags: UsizeValue,
        mode: UsizeValue,
        failure_mode: OutcomeFailureMode,
    },
    CloseFd {
        fd: I32Value,
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
    RegionEnter {
        destination: UsizeLocation,
    },
    SetCurrentAllocationContext {
        state: UsizeValue,
        kind: UsizeValue,
    },
    RegionRelease {
        state: UsizeValue,
        parent_state: UsizeValue,
        parent_kind: UsizeValue,
    },
    SetUsizeFromBorrow {
        destination: UsizeLocation,
        source: BorrowSource,
    },
    SetBool {
        destination: BoolLocation,
        value: BoolValue,
    },
    SetStr {
        destination: StrLocation,
        value: StrValue,
    },
    SetStrRawParts {
        destination: StrLocation,
        pointer: UsizeValue,
        len: UsizeValue,
    },
    SetStrSubview {
        destination: StrLocation,
        source: StrValue,
        start: UsizeValue,
        len: UsizeValue,
    },
    SetSlice {
        destination: SliceLocation,
        value: SliceValue,
    },
    SetSliceRawParts {
        destination: SliceLocation,
        pointer: UsizeValue,
        len: UsizeValue,
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
    StoreAggregateUsizeIndexed {
        destination: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
        value: UsizeValue,
    },
    #[allow(dead_code)]
    StoreAggregateI32 {
        destination: AggregateLocation,
        offset: u32,
        value: I32Value,
    },
    StoreAggregateI32Indexed {
        destination: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
        value: I32Value,
    },
    #[allow(dead_code)]
    StoreAggregateU16 {
        destination: AggregateLocation,
        offset: u32,
        value: u16,
    },
    #[allow(dead_code)]
    StoreAggregateU32 {
        destination: AggregateLocation,
        offset: u32,
        value: u32,
    },
    #[allow(dead_code)]
    StoreAggregateU8 {
        destination: AggregateLocation,
        offset: u32,
        value: U8Value,
    },
    StoreAggregateU8Indexed {
        destination: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
        value: U8Value,
    },
    #[allow(dead_code)]
    StoreAggregateBool {
        destination: AggregateLocation,
        offset: u32,
        value: BoolValue,
    },
    StoreAggregateBoolIndexed {
        destination: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
        value: BoolValue,
    },
    #[allow(dead_code)]
    LoadAggregateUsize {
        destination: UsizeLocation,
        source: AggregateLocation,
        offset: u32,
    },
    LoadAggregateUsizeIndexed {
        destination: UsizeLocation,
        source: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
    },
    #[allow(dead_code)]
    LoadAggregateI32 {
        destination: I32Location,
        source: AggregateLocation,
        offset: u32,
    },
    LoadAggregateI32Indexed {
        destination: I32Location,
        source: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
    },
    #[allow(dead_code)]
    LoadAggregateU8 {
        destination: U8Location,
        source: AggregateLocation,
        offset: u32,
    },
    LoadAggregateU8Indexed {
        destination: U8Location,
        source: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
    },
    #[allow(dead_code)]
    LoadAggregateBool {
        destination: BoolLocation,
        source: AggregateLocation,
        offset: u32,
    },
    LoadAggregateBoolIndexed {
        destination: BoolLocation,
        source: AggregateLocation,
        base_offset: u32,
        index: UsizeValue,
        length: u64,
        stride: u32,
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
    CopySliceElementToAggregate {
        destination: AggregateLocation,
        source: SliceLocation,
        index: SliceElementIndex,
        layout: ValueLayout,
    },
    CopyAggregateToSliceElement {
        destination: SliceLocation,
        index: SliceElementIndex,
        source: AggregateLocation,
        layout: ValueLayout,
    },
    DarwinSyscall {
        destination: AggregateLocation,
        arity: u8,
        number: UsizeValue,
        arguments: Vec<UsizeValue>,
    },
    CopyStrToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        text: StrValue,
    },
    CopyPointerBytes {
        destination: UsizeValue,
        source: UsizeValue,
        byte_count: UsizeValue,
    },
    CopyAggregateToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        source: AggregateLocation,
        layout: ValueLayout,
    },
    CopyPointerToAggregate {
        destination: AggregateLocation,
        pointer: UsizeValue,
        offset: UsizeValue,
        layout: ValueLayout,
    },
    LoadU8FromPointer {
        destination: U8Location,
        pointer: UsizeValue,
        offset: UsizeValue,
    },
    LoadI32FromPointer {
        destination: I32Location,
        pointer: UsizeValue,
        offset: UsizeValue,
    },
    LoadUsizeFromPointer {
        destination: UsizeLocation,
        pointer: UsizeValue,
        offset: UsizeValue,
    },
    LoadBoolFromPointer {
        destination: BoolLocation,
        pointer: UsizeValue,
        offset: UsizeValue,
    },
    LoadStrFromPointer {
        destination: StrLocation,
        pointer: UsizeValue,
        offset: UsizeValue,
    },
    StoreU8ToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        value: U8Value,
    },
    StoreI32ToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        value: I32Value,
    },
    StoreUsizeToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        value: UsizeValue,
    },
    StoreBoolToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        value: BoolValue,
    },
    StoreStrToPointer {
        pointer: UsizeValue,
        offset: UsizeValue,
        value: StrValue,
    },
    StoreU8ToSliceIndex {
        destination: SliceLocation,
        index: UsizeValue,
        value: U8Value,
    },
    StoreI32ToSliceIndex {
        destination: SliceLocation,
        index: UsizeValue,
        value: I32Value,
    },
    StoreUsizeToSliceIndex {
        destination: SliceLocation,
        index: UsizeValue,
        value: UsizeValue,
    },
    StoreBoolToSliceIndex {
        destination: SliceLocation,
        index: UsizeValue,
        value: BoolValue,
    },
    StoreStrToSliceIndex {
        destination: SliceLocation,
        index: UsizeValue,
        value: StrValue,
    },
    AddU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    SubtractU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    MultiplyU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    DivideU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    RemainderU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    ShiftLeftU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
    },
    ShiftRightU8 {
        destination: U8Location,
        left: U8Value,
        right: U8Value,
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
    CallOutcomeI32 {
        destination: I32Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    #[allow(dead_code)]
    CallU8 {
        destination: U8Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeU8 {
        destination: U8Location,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    #[allow(dead_code)]
    CallUsize {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeUsize {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    CallBorrow {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeBorrow {
        destination: UsizeLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    #[allow(dead_code)]
    CallBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeBool {
        destination: BoolLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    #[allow(dead_code)]
    CallStr {
        destination: StrLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeStr {
        destination: StrLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    #[allow(dead_code)]
    CallSlice {
        destination: SliceLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeSlice {
        destination: SliceLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
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
    CallOutcomeDirectAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        layout: ValueLayout,
        failure_mode: OutcomeFailureMode,
    },
    CallOutcomeAggregate {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    CallVoid {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    CallOutcomeVoid {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        failure_mode: OutcomeFailureMode,
    },
    CallComposedOutcome {
        destination: ComposedOutcomeDestination,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        outer: OutcomeLayer,
        inner: OutcomeLayer,
        outer_mode: OutcomeFailureMode,
        inner_mode: OutcomeFailureMode,
    },
    CallStoredOutcome {
        destination: AggregateLocation,
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
        storage: OutcomeStorageLayout,
        payload_type: Type,
    },
    IfStoredOutcomeTag {
        source: AggregateLocation,
        tag_offset: u32,
        success_instructions: Vec<Instruction>,
        outcome_instructions: Vec<Instruction>,
    },
    CheckStoredFallible {
        source: AggregateLocation,
        tag_offset: u32,
        error_offset: u32,
        success_instructions: Vec<Instruction>,
        failure_mode: OutcomeFailureMode,
    },
    LoadStoredOutcomePayload {
        destination: ComposedOutcomeDestination,
        source: AggregateLocation,
        offset: u32,
    },
    ReturnStoredOutcome {
        source: AggregateLocation,
        storage: OutcomeStorageLayout,
        payload_type: Type,
    },
    TailCall {
        target: CallTarget,
        arguments: Vec<ScalarArgument>,
    },
    ProcessExit {
        code: I32Value,
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
        failure_mode: OutcomeFailureMode,
    },
    ReturnOutcomeSuccess,
    ReturnOptionalNone,
    ReturnFallibleFailure {
        code: StrValue,
        message: StrValue,
    },
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutcomeFailureMode {
    Propagate,
    PropagateWithCleanup {
        code: StrLocation,
        message: StrLocation,
        instructions: Vec<Instruction>,
    },
    Trap,
    Handle {
        instructions: Vec<Instruction>,
    },
    Recover {
        instructions: Vec<Instruction>,
    },
    Catch {
        code: StrLocation,
        message: StrLocation,
        instructions: Vec<Instruction>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposedOutcomeDestination {
    I32(I32Location),
    U8(U8Location),
    Usize(UsizeLocation),
    Borrow(UsizeLocation),
    Bool(BoolLocation),
    Str(StrLocation),
    Slice(SliceLocation),
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
    SliceIndex {
        source: SliceLocation,
        index: UsizeValue,
    },
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
    ProcessArgCount,
    ProcessEnvironmentCount,
    CurrentAllocationState,
    CurrentAllocationKind,
    U8ZeroExtend(Box<U8Value>),
    StrLen(StrLocation),
    SliceLen(SliceLocation),
    SliceIndex {
        source: SliceLocation,
        index: Box<UsizeValue>,
    },
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
    BorrowParameter(usize),
    BorrowLocal(UsizeLocation),
    SliceIndex {
        source: SliceLocation,
        index: SliceElementIndex,
        element: SliceElementAddressKind,
    },
    AggregateSlot(usize),
    AggregateSlotField {
        slot_index: usize,
        offset: u32,
    },
    AggregateParameter(usize),
    AggregateParameterField {
        parameter_index: usize,
        offset: u32,
    },
    PointerOffset {
        pointer: UsizeLocation,
        offset: UsizeLocation,
        field_offset: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SliceElementAddressKind {
    U8,
    I32,
    Usize,
    Bool,
    Str,
    Aggregate { stride: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SliceElementIndex {
    Const(u64),
    Location(UsizeLocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StrValue {
    StaticBytes(Vec<u8>),
    Location(StrLocation),
    ProcessArg {
        index: UsizeValue,
    },
    ProcessEnvironmentName {
        index: UsizeValue,
    },
    ProcessEnvironmentValue {
        index: UsizeValue,
    },
    SliceIndex {
        source: SliceLocation,
        index: UsizeValue,
    },
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
    StrBytes(StrValue),
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
    SliceIndex {
        source: SliceLocation,
        index: UsizeValue,
    },
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
    StrComparison {
        operator: BoolComparisonOperator,
        left: StrValue,
        right: StrValue,
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
    Error,
    Void,
    Never,
    Optional(Box<Type>),
    Fallible(Box<Type>),
    ComposedOutcome {
        outer: OutcomeLayer,
        inner: OutcomeLayer,
        payload: Box<Type>,
    },
}

impl Type {
    pub(crate) fn outer_outcome_layer(&self) -> Option<OutcomeLayer> {
        match self {
            Self::Optional(_) => Some(OutcomeLayer::Optional),
            Self::Fallible(_) => Some(OutcomeLayer::Fallible),
            Self::ComposedOutcome { outer, .. } => Some(*outer),
            _ => None,
        }
    }

    pub(crate) fn contains_outcome_layer(&self, expected: OutcomeLayer) -> bool {
        match self {
            Self::Optional(_) => expected == OutcomeLayer::Optional,
            Self::Fallible(_) => expected == OutcomeLayer::Fallible,
            Self::ComposedOutcome { outer, inner, .. } => *outer == expected || *inner == expected,
            _ => false,
        }
    }

    pub(crate) fn single_outcome(&self) -> Option<(OutcomeLayer, &Type)> {
        match self {
            Self::Optional(payload) => Some((OutcomeLayer::Optional, payload)),
            Self::Fallible(payload) => Some((OutcomeLayer::Fallible, payload)),
            _ => None,
        }
    }

    pub(crate) fn success_type(&self) -> &Type {
        match self {
            Self::Optional(payload) => payload,
            Self::Fallible(success) => success,
            Self::ComposedOutcome { payload, .. } => payload,
            Self::I32
            | Self::U8
            | Self::Usize
            | Self::Bool
            | Self::Str
            | Self::Slice { .. }
            | Self::Aggregate { .. }
            | Self::DirectAggregate { .. }
            | Self::Borrow { .. }
            | Self::Error
            | Self::Void
            | Self::Never => self,
        }
    }

    pub(crate) fn success_return_passing(&self) -> Option<ReturnPassing> {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => {
                Some(ReturnPassing::Direct { words: 1 })
            }
            Self::Str | Self::Slice { .. } => Some(ReturnPassing::Direct { words: 2 }),
            Self::Aggregate { .. } => Some(ReturnPassing::IndirectPointer),
            Self::DirectAggregate { words, .. } => Some(ReturnPassing::Direct { words: *words }),
            Self::Error => None,
            Self::Void => Some(ReturnPassing::Void),
            Self::Never => Some(ReturnPassing::Never),
            Self::Optional(payload) => payload.success_return_passing(),
            Self::Fallible(success) => success.success_return_passing(),
            Self::ComposedOutcome { payload, .. } => payload.success_return_passing(),
            Self::Borrow { .. } => Some(ReturnPassing::Direct { words: 1 }),
        }
    }
}
