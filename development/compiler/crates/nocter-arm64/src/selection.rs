use nocter_machine::{
    MachineAddressId, MachineBinaryOperation, MachineBlockId, MachineBranchTarget,
    MachineComparisonOperation, MachineComparisonRepresentation, MachineConstant, MachineDataId,
    MachineFunctionId, MachineLayoutKind, MachineOperationId, MachineOperationKind, MachineScalar,
    MachineTerminator, MachineUnaryOperation, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64LoadStoreSize, Arm64Register, Arm64SelectionError,
    Arm64ValuePlan, Arm64ValueStorage, Arm64VirtualRegister,
};

/// Shared immutable inputs for selecting one operation in a machine function.
#[derive(Clone, Copy)]
pub(crate) struct Arm64SelectionContext<'selection> {
    program: &'selection nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    values: &'selection Arm64ValuePlan,
    frame: &'selection Arm64FunctionFrame,
    addresses: &'selection crate::Arm64SelectedAddressPlan,
}

impl<'selection> Arm64SelectionContext<'selection> {
    const fn new(
        program: &'selection nocter_machine::MachineProgram,
        owner: MachineFunctionId,
        values: &'selection Arm64ValuePlan,
        frame: &'selection Arm64FunctionFrame,
        addresses: &'selection crate::Arm64SelectedAddressPlan,
    ) -> Self {
        Self {
            program,
            owner,
            values,
            frame,
            addresses,
        }
    }

    pub(crate) const fn program(self) -> &'selection nocter_machine::MachineProgram {
        self.program
    }

    pub(crate) const fn owner(self) -> MachineFunctionId {
        self.owner
    }

    pub(crate) const fn values(self) -> &'selection Arm64ValuePlan {
        self.values
    }

    pub(crate) const fn frame(self) -> &'selection Arm64FunctionFrame {
        self.frame
    }

    pub(crate) const fn addresses(self) -> &'selection crate::Arm64SelectedAddressPlan {
        self.addresses
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedRegister {
    Virtual(Arm64VirtualRegister),
    Fixed(Arm64Register),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedFloatRegister {
    Virtual(Arm64VirtualRegister),
    Fixed(crate::Arm64FloatRegister),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedStackAddress {
    FrameObject {
        object: crate::Arm64FrameObjectId,
        offset: u64,
    },
    Outgoing(u64),
    Incoming(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedMemoryAddress {
    Stack(Arm64SelectedStackAddress),
    Register {
        base: Arm64SelectedRegister,
        offset: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedInstruction {
    LoadImmediate {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        value: u64,
    },
    LoadDataAddress {
        destination: Arm64SelectedRegister,
        source: MachineDataId,
    },
    LoadPackCallbackAddress {
        destination: Arm64SelectedRegister,
        pack: nocter_machine::MachinePackId,
        kind: crate::Arm64PackCallbackKind,
    },
    Move {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedRegister,
    },
    FloatLoadImmediate {
        size: Arm64DataSize,
        destination: Arm64SelectedFloatRegister,
        bits: u64,
    },
    FloatMove {
        size: Arm64DataSize,
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedFloatRegister,
    },
    FloatMoveFromGeneral {
        size: Arm64DataSize,
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedRegister,
    },
    FloatMoveToGeneral {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedFloatRegister,
    },
    FloatLoadMemory {
        size: Arm64DataSize,
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedMemoryAddress,
    },
    FloatStoreMemory {
        size: Arm64DataSize,
        destination: Arm64SelectedMemoryAddress,
        source: Arm64SelectedFloatRegister,
    },
    FloatNegate {
        size: Arm64DataSize,
        destination: Arm64SelectedFloatRegister,
        operand: Arm64SelectedFloatRegister,
    },
    FloatRound {
        size: Arm64DataSize,
        operation: crate::Arm64FloatRounding,
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedFloatRegister,
    },
    FloatBinary {
        size: Arm64DataSize,
        operation: crate::Arm64FloatBinary,
        destination: Arm64SelectedFloatRegister,
        left: Arm64SelectedFloatRegister,
        right: Arm64SelectedFloatRegister,
    },
    FloatCompare {
        size: Arm64DataSize,
        operation: Arm64SelectedFloatComparisonOperation,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedFloatRegister,
        right: Arm64SelectedFloatRegister,
    },
    CompareFloatBorrowed {
        size: Arm64DataSize,
        operation: Arm64SelectedFloatComparisonOperation,
        offset: u64,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedRegister,
        right: Arm64SelectedRegister,
    },
    /// Losslessly widens one canonical integer value according to the source signedness.
    IntegerConversion {
        size: Arm64DataSize,
        source_bits: u8,
        signed: bool,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedRegister,
    },
    FloatFromInteger {
        source_size: Arm64DataSize,
        target_size: Arm64DataSize,
        signed: bool,
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedRegister,
    },
    FloatWiden {
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedFloatRegister,
    },
    FloatNarrow {
        destination: Arm64SelectedFloatRegister,
        source: Arm64SelectedFloatRegister,
    },
    FloatToInteger {
        signed: bool,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedFloatRegister,
    },
    LoadMemory {
        bytes: u8,
        extension: Arm64SelectedLoadExtension,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedMemoryAddress,
    },
    StoreMemory {
        bytes: u8,
        destination: Arm64SelectedMemoryAddress,
        source: Arm64SelectedRegister,
    },
    ZeroStack {
        destination: Arm64SelectedStackAddress,
        bytes: u64,
    },
    /// Exact copy between storage domains proven distinct during selection.
    CopyMemoryNonOverlapping {
        destination: Arm64SelectedMemoryAddress,
        source: Arm64SelectedMemoryAddress,
        bytes: u64,
    },
    /// Runtime-sized forward copy between non-overlapping pointer ranges.
    CopyMemoryNonOverlappingDynamic {
        destination: Arm64SelectedRegister,
        source: Arm64SelectedRegister,
        bytes: Arm64SelectedRegister,
    },
    ResolveAddress(MachineAddressId),
    IndexAddress {
        destination: Arm64SelectedRegister,
        index: Arm64SelectedRegister,
        domain: Arm64SelectedIndexAddressDomain,
    },
    MemoryAddress {
        destination: Arm64SelectedRegister,
        source: Arm64SelectedMemoryAddress,
    },
    Unary {
        size: Arm64DataSize,
        operation: Arm64SelectedUnaryOperation,
        destination: Arm64SelectedRegister,
        operand: Arm64SelectedRegister,
    },
    Binary {
        size: Arm64DataSize,
        operation: Arm64SelectedBinaryOperation,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedRegister,
        right: Arm64SelectedRegister,
    },
    DarwinSystemCall {
        argument_count: u8,
    },
    DarwinSystemCallPair,
    /// Performs a compiler-owned private anonymous mapping and returns `(value, errno)`.
    DarwinMemoryMap,
    /// Releases a compiler-owned page mapping and returns `(value, errno)`.
    DarwinMemoryUnmap,
    /// Closes one target descriptor and returns `(value, errno)`.
    DarwinDescriptorClose,
    /// Reads once from an owner-certified nonblocking descriptor.
    DarwinDescriptorRead,
    /// Writes once to an owner-certified nonblocking descriptor.
    DarwinDescriptorWrite,
    /// Attempts one nonwaiting observation of an exact child.
    DarwinProcessObserve,
    /// Performs one closed Darwin datagram operation and returns `(value, errno)`.
    DarwinDatagram(crate::DarwinDatagramOperation),
    /// Fills one 64-bit value from the target entropy source and returns its errno.
    DarwinEntropySeedFill,
    /// Performs one Darwin `select` timeout and returns zero or its errno.
    DarwinTimeoutWait,
    /// Observes one Darwin wall-clock timeval and returns normalized fields plus errno.
    DarwinWallClockRead,
    ReadMonotonicCounter,
    ReadMonotonicCounterFrequency,
    /// Calls the compiler-owned descriptor-readiness computation constructor.
    ConstructDescriptorReadiness,
    /// Calls the compiler-owned descriptor-or-deadline computation constructor.
    ConstructDescriptorReadinessOrDeadline,
    ConstructMonotonicDeadline,
    /// Calls the compiler-owned process-completion computation constructor.
    ConstructProcessCompletion,
    /// Constructs one compiler-owned computation that owns and joins two child computations.
    ConstructTaskJoin {
        first_output_offset: u64,
        second_output_offset: u64,
    },
    /// Constructs one compiler-owned computation that selects one of two same-output children.
    ConstructTaskRace {
        winner_offset: u64,
        output_offset: u64,
    },
    /// Calls one source-ABI entry from the compiler-owned plain connection target set.
    CallDarwinNetworkPrimitive(crate::Arm64DarwinNetworkPrimitive),
    /// Transfers one exact unreaped child to the compiler-owned abandonment service.
    CallDarwinProcessAbandon,
    ExitProcess {
        status: Arm64SelectedRegister,
    },
    Break {
        immediate: u16,
    },
    /// Initializes one compiler-owned non-movable lexical allocation context.
    CreateRegion {
        region: crate::Arm64FrameObjectId,
        parent: Arm64SelectedRegister,
    },
    /// Releases every mapping owned by one lexical allocation context.
    ReleaseRegion {
        region: crate::Arm64FrameObjectId,
    },
    /// Releases the owned node chain stored in one initialized error place.
    ReleaseError {
        place: Arm64SelectedMemoryAddress,
    },
    /// Cancels and releases one owning deferred-computation handle.
    ReleaseComputation {
        place: Arm64SelectedMemoryAddress,
    },
    /// Drives the compiler-selected process-entry computation to completion.
    DriveComputation {
        computation: Arm64SelectedMemoryAddress,
        destination: Option<Arm64SelectedMemoryAddress>,
    },
    /// Releases allocation-backed pack storage; stack-backed descriptors carry a zero size.
    ReleasePackAllocation {
        descriptor: Arm64SelectedMemoryAddress,
    },
    ConstructErrorLeaf {
        buffer: crate::Arm64FrameObjectId,
    },
    ConstructErrorContext {
        buffer: crate::Arm64FrameObjectId,
    },
    ReadErrorCode,
    ReadErrorMessage,
    LoadAllocationFailureError,
    /// Borrows and reports one initialized built-in error place without allocation or a fallible
    /// library call.
    ReportError {
        place: Arm64SelectedMemoryAddress,
        buffer: crate::Arm64FrameObjectId,
    },
    /// Captures `argc`, `argv`, and `envp` from the platform entry ABI and counts `envp` once.
    InitializeProcessContext {
        context: crate::Arm64FrameObjectId,
    },
    ReadProcessArgumentCount,
    ReadProcessArgument,
    ReadProcessEnvironmentCount,
    ReadProcessEnvironmentName,
    ReadProcessEnvironmentValue,
    CompareBorrowed {
        size: Arm64LoadStoreSize,
        extension: Arm64SelectedLoadExtension,
        operation: Arm64SelectedComparisonOperation,
        offset: u64,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedRegister,
        right: Arm64SelectedRegister,
    },
    Call(MachineFunctionId),
    CallImported(nocter_machine::MachineImportId),
    CallRegister(Arm64SelectedRegister),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedIndexAddressDomain {
    Fixed {
        pointer: Arm64SelectedRegister,
        length: u64,
        stride: u64,
    },
    View {
        pointer: Arm64SelectedRegister,
        length: Arm64SelectedRegister,
        stride: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedLoadExtension {
    Zero,
    Sign(Arm64DataSize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedUnaryOperation {
    LogicalNot,
    Negate,
    CountLeadingZeros,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedBinaryOperation {
    Add,
    Subtract,
    Multiply,
    MultiplyHigh,
    Divide { signed: bool },
    Remainder { signed: bool },
    ShiftLeft,
    ShiftRight { signed: bool },
    BitwiseXor,
    BitwiseAnd,
    BitwiseOr,
    RotateRight,
    Equal,
    Less { signed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedComparisonOperation {
    Equal,
    Less { signed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedFloatComparisonOperation {
    Equal,
    Less,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64SelectedCopy {
    destination: Arm64SelectedRegister,
    source: Arm64SelectedRegister,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64SelectedFloatCopy {
    size: Arm64DataSize,
    destination: Arm64SelectedFloatRegister,
    source: Arm64SelectedFloatRegister,
}

impl Arm64SelectedFloatCopy {
    #[must_use]
    pub const fn size(self) -> Arm64DataSize {
        self.size
    }

    #[must_use]
    pub const fn destination(self) -> Arm64SelectedFloatRegister {
        self.destination
    }

    #[must_use]
    pub const fn source(self) -> Arm64SelectedFloatRegister {
        self.source
    }
}

impl Arm64SelectedCopy {
    #[must_use]
    pub const fn destination(self) -> Arm64SelectedRegister {
        self.destination
    }

    #[must_use]
    pub const fn source(self) -> Arm64SelectedRegister {
        self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64SelectedMemoryCopy {
    destination: Arm64SelectedStackAddress,
    source: Arm64SelectedStackAddress,
    bytes: u64,
}

impl Arm64SelectedMemoryCopy {
    #[must_use]
    pub const fn destination(self) -> Arm64SelectedStackAddress {
        self.destination
    }

    #[must_use]
    pub const fn source(self) -> Arm64SelectedStackAddress {
        self.source
    }

    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedEdge {
    target: MachineBlockId,
    copies: Box<[Arm64SelectedCopy]>,
    float_copies: Box<[Arm64SelectedFloatCopy]>,
    memory_copies: Box<[Arm64SelectedMemoryCopy]>,
}

impl Arm64SelectedEdge {
    #[must_use]
    pub const fn target(&self) -> MachineBlockId {
        self.target
    }

    #[must_use]
    pub const fn copies(&self) -> &[Arm64SelectedCopy] {
        &self.copies
    }

    #[must_use]
    pub const fn float_copies(&self) -> &[Arm64SelectedFloatCopy] {
        &self.float_copies
    }

    #[must_use]
    pub const fn memory_copies(&self) -> &[Arm64SelectedMemoryCopy] {
        &self.memory_copies
    }

    #[must_use]
    pub const fn has_copies(&self) -> bool {
        !self.copies.is_empty() || !self.float_copies.is_empty() || !self.memory_copies.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedSwitchCase {
    value: u128,
    edge: Arm64SelectedEdge,
}

impl Arm64SelectedSwitchCase {
    pub(crate) const fn new(value: u128, edge: Arm64SelectedEdge) -> Self {
        Self { value, edge }
    }

    #[must_use]
    pub const fn value(&self) -> u128 {
        self.value
    }

    #[must_use]
    pub const fn edge(&self) -> &Arm64SelectedEdge {
        &self.edge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64SelectedTerminator {
    Goto(Arm64SelectedEdge),
    Branch {
        condition: Arm64SelectedRegister,
        then_edge: Arm64SelectedEdge,
        else_edge: Arm64SelectedEdge,
    },
    Switch {
        subject: Box<[Arm64SelectedRegister]>,
        cases: Box<[Arm64SelectedSwitchCase]>,
        fallback: Arm64SelectedEdge,
    },
    Return,
    Suspend {
        computation: Arm64SelectedRegister,
        resume: MachineBlockId,
        result: MachineValueId,
    },
    DeferredReturn(Option<MachineValueId>),
    Exit(Option<Arm64SelectedRegister>),
    Trap,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedBlock {
    instructions: Box<[Arm64SelectedInstruction]>,
    terminator: Arm64SelectedTerminator,
}

impl Arm64SelectedBlock {
    #[must_use]
    pub const fn instructions(&self) -> &[Arm64SelectedInstruction] {
        &self.instructions
    }

    #[must_use]
    pub const fn terminator(&self) -> &Arm64SelectedTerminator {
        &self.terminator
    }
}

/// Target-selected operations, value allocation, and fixed frame for one machine function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedFunction {
    owner: MachineFunctionId,
    values: Arm64ValuePlan,
    frame: Arm64FunctionFrame,
    addresses: crate::Arm64SelectedAddressPlan,
    entry_instructions: Box<[Arm64SelectedInstruction]>,
    blocks: Box<[(MachineBlockId, Arm64SelectedBlock)]>,
    entry: MachineBlockId,
}

impl Arm64SelectedFunction {
    /// Selects the currently executable scalar/call/control slice.
    ///
    /// Unsupported machine operations fail explicitly and never survive as passthrough nodes.
    ///
    /// # Errors
    ///
    /// Rejects malformed machine identities, value/frame planning failures, and operations or
    /// ABI transports outside the currently closed selection slice.
    pub fn build(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
    ) -> Result<Self, Arm64SelectionError> {
        let function = program
            .function(owner)
            .ok_or(Arm64SelectionError::UnknownFunction(owner))?;
        if matches!(
            function.execution(),
            nocter_machine::MachineFunctionExecution::Deferred(_)
        ) {
            return Err(Arm64SelectionError::UnsupportedDeferredFunction(owner));
        }
        Self::build_execution(program, owner, None)
    }

    pub(crate) fn build_deferred(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
        persistent: &crate::Arm64AsyncFrameLayout,
    ) -> Result<Self, Arm64SelectionError> {
        let function = program
            .function(owner)
            .ok_or(Arm64SelectionError::UnknownFunction(owner))?;
        if !matches!(
            function.execution(),
            nocter_machine::MachineFunctionExecution::Deferred(_)
        ) {
            return Err(Arm64SelectionError::UnsupportedDeferredFunction(owner));
        }
        Self::build_execution(program, owner, Some(persistent))
    }

    fn build_execution(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
        persistent: Option<&crate::Arm64AsyncFrameLayout>,
    ) -> Result<Self, Arm64SelectionError> {
        let deferred = persistent.is_some();
        let function = program
            .function(owner)
            .ok_or(Arm64SelectionError::UnknownFunction(owner))?;
        let values = Arm64ValuePlan::build(function)?;
        let frame = Arm64FunctionFrame::build(program, owner, &values)?;
        let addresses =
            crate::Arm64SelectedAddressPlan::build(function, &values, &frame, persistent)?;
        let context = Arm64SelectionContext::new(program, owner, &values, &frame, &addresses);
        let mut entry_instructions = if deferred {
            Vec::new()
        } else {
            crate::call_selection::select_parameters(program, owner, function, &frame)?.into_vec()
        };
        crate::destruction_selection::select_entry(function, &frame, &mut entry_instructions)?;
        let mut blocks = Vec::with_capacity(function.body().blocks().len());
        for (block_id, block) in function.body().blocks() {
            if block_id.index() != blocks.len() {
                return Err(Arm64SelectionError::NonDenseBlock(block_id));
            }
            let mut instructions = Vec::new();
            for operation_id in block.operations() {
                let operation = function.body().operation(*operation_id).ok_or(
                    Arm64SelectionError::UnknownOperation {
                        function: owner,
                        operation: *operation_id,
                    },
                )?;
                select_operation(context, *operation_id, operation, &mut instructions)?;
            }
            let terminator = select_terminator(
                context,
                block_id,
                block.terminator(),
                deferred,
                &mut instructions,
            )?;
            blocks.push((
                block_id,
                Arm64SelectedBlock {
                    instructions: instructions.into_boxed_slice(),
                    terminator,
                },
            ));
        }
        Ok(Self {
            owner,
            values,
            frame,
            addresses,
            entry_instructions: entry_instructions.into_boxed_slice(),
            blocks: blocks.into_boxed_slice(),
            entry: function.body().entry(),
        })
    }

    #[must_use]
    pub const fn owner(&self) -> MachineFunctionId {
        self.owner
    }

    #[must_use]
    pub const fn values(&self) -> &Arm64ValuePlan {
        &self.values
    }

    #[must_use]
    pub const fn frame(&self) -> &Arm64FunctionFrame {
        &self.frame
    }

    #[must_use]
    pub const fn addresses(&self) -> &crate::Arm64SelectedAddressPlan {
        &self.addresses
    }

    #[must_use]
    pub const fn entry_instructions(&self) -> &[Arm64SelectedInstruction] {
        &self.entry_instructions
    }

    #[must_use]
    pub fn block(&self, id: MachineBlockId) -> Option<&Arm64SelectedBlock> {
        self.blocks
            .get(id.index())
            .and_then(|(actual, block)| (*actual == id).then_some(block))
    }

    #[must_use]
    pub fn blocks(&self) -> impl ExactSizeIterator<Item = (MachineBlockId, &Arm64SelectedBlock)> {
        self.blocks.iter().map(|(id, block)| (*id, block))
    }

    #[must_use]
    pub const fn entry(&self) -> MachineBlockId {
        self.entry
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed machine-operation domain is routed exhaustively in one place"
)]
fn select_operation(
    context: Arm64SelectionContext<'_>,
    operation_id: MachineOperationId,
    operation: &nocter_machine::MachineOperation,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let program = context.program();
    let owner = context.owner();
    let values = context.values();
    let frame = context.frame();
    match operation.kind() {
        MachineOperationKind::Constant(constant) => {
            let result = operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
            select_constant(program, owner, *constant, result, values, selected)
        }
        MachineOperationKind::Load { source } => crate::memory_selection::select_load(
            (program, owner),
            *source,
            operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?,
            values,
            frame,
            context.addresses(),
            selected,
        ),
        MachineOperationKind::Store { destination, value } => {
            crate::memory_selection::select_store(
                (program, owner),
                *destination,
                *value,
                values,
                frame,
                context.addresses(),
                selected,
            )
        }
        MachineOperationKind::AddressOf { source } => crate::memory_selection::select_address_of(
            (program, owner),
            *source,
            operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?,
            values,
            context.addresses(),
            selected,
        ),
        MachineOperationKind::Unary {
            operation: unary,
            operand,
        } => select_unary(
            (program, owner),
            operation_id,
            *unary,
            *operand,
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::Binary {
            operation: binary,
            left,
            right,
        } => select_binary(
            (program, owner),
            operation_id,
            *binary,
            (*left, *right),
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::Comparison(comparison) => select_comparison(
            operation_id,
            *comparison,
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::IndexBorrow(index) => {
            crate::structural_selection::select_index_borrow(
                *index,
                operation
                    .result()
                    .ok_or(Arm64SelectionError::MissingResult(operation_id))?,
                values,
                selected,
            )
        }
        MachineOperationKind::BorrowWeakening { source } => {
            crate::structural_selection::select_direct_copy(
                operation_id,
                *source,
                operation
                    .result()
                    .ok_or(Arm64SelectionError::MissingResult(operation_id))?,
                values,
                selected,
            )
        }
        MachineOperationKind::Aggregate(aggregate) => select_aggregate_operation(
            (program, owner),
            operation_id,
            aggregate,
            operation.result(),
            values,
            frame,
            selected,
        ),
        MachineOperationKind::InvokeDrop {
            target,
            place,
            allocation,
        } => crate::destruction_selection::select_drop(
            context,
            operation_id,
            *target,
            *place,
            *allocation,
            selected,
        ),
        MachineOperationKind::SetDropFlag { flag, initialized } => {
            crate::destruction_selection::select_flag_write(*flag, *initialized, frame, selected)
        }
        MachineOperationKind::CreateRegion { parent, region } => {
            crate::region_selection::select_create(
                operation_id,
                *parent,
                *region,
                operation.result(),
                context,
                selected,
            )
        }
        MachineOperationKind::ReleaseRegion { region } => crate::region_selection::select_release(
            operation_id,
            *region,
            operation.result(),
            context,
            selected,
        ),
        MachineOperationKind::ReportError { place } => crate::error_selection::select_report(
            operation_id,
            *place,
            operation.result(),
            context,
            selected,
        ),
        MachineOperationKind::ReleaseError { place } => crate::error_selection::select_release(
            operation_id,
            *place,
            operation.result(),
            context,
            selected,
        ),
        MachineOperationKind::ReleaseComputation { place } => {
            crate::async_release_selection::select(
                operation_id,
                *place,
                operation.result(),
                context,
                selected,
            )
        }
        MachineOperationKind::DriveComputation {
            computation,
            destination,
        } => crate::async_drive_selection::select(
            context,
            operation_id,
            *computation,
            *destination,
            operation.result(),
            selected,
        ),
        MachineOperationKind::Call(call) => crate::call_selection::select_call(
            context,
            operation_id,
            call,
            operation.result(),
            selected,
        ),
        MachineOperationKind::PackLength => crate::pack_selection::select_pack_operation(
            context,
            operation_id,
            crate::pack_selection::Arm64PackOperation::Length(operation.result()),
            selected,
        ),
        MachineOperationKind::PackNext => crate::pack_selection::select_pack_operation(
            context,
            operation_id,
            crate::pack_selection::Arm64PackOperation::Next(operation.result()),
            selected,
        ),
        MachineOperationKind::DestroyPack => crate::pack_selection::select_pack_operation(
            context,
            operation_id,
            crate::pack_selection::Arm64PackOperation::Destroy,
            selected,
        ),
        MachineOperationKind::NumericConversion { operand } => select_numeric_conversion(
            (context.program(), context.owner()),
            operation_id,
            *operand,
            operation.result(),
            values,
            selected,
        ),
    }
}

fn select_aggregate_operation(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation: MachineOperationId,
    aggregate: &nocter_machine::MachineAggregate,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    crate::aggregate_selection::select_aggregate(
        scope.0,
        scope.1,
        aggregate,
        result.ok_or(Arm64SelectionError::MissingResult(operation))?,
        values,
        frame,
        selected,
    )
}

fn select_constant(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    constant: MachineConstant,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if let MachineConstant::Text(data) = constant {
        return crate::memory_selection::select_text_constant(
            program, owner, data, result, values, selected,
        );
    }
    match constant {
        MachineConstant::Float32(bits) => {
            let (destination, size) = floating_value(values, result)?;
            if size != Arm64DataSize::Bits32 {
                return Err(Arm64SelectionError::UnsupportedScalar(result));
            }
            selected.push(Arm64SelectedInstruction::FloatLoadImmediate {
                size,
                destination,
                bits: u64::from(bits),
            });
            return Ok(());
        }
        MachineConstant::Float64(bits) => {
            let (destination, size) = floating_value(values, result)?;
            if size != Arm64DataSize::Bits64 {
                return Err(Arm64SelectionError::UnsupportedScalar(result));
            }
            selected.push(Arm64SelectedInstruction::FloatLoadImmediate {
                size,
                destination,
                bits,
            });
            return Ok(());
        }
        _ => {}
    }
    let bits = match constant {
        MachineConstant::Bool(value) => u128::from(value),
        MachineConstant::Character(value) => u128::from(value),
        MachineConstant::Integer(value) => value.cast_unsigned(),
        MachineConstant::Float32(_) | MachineConstant::Float64(_) | MachineConstant::Text(_) => {
            unreachable!("text and floating constants return before general scalar selection")
        }
    };
    let registers = direct_value(values, result)?;
    for (lane, register) in registers.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::LoadImmediate {
            size: Arm64DataSize::Bits64,
            destination: Arm64SelectedRegister::Virtual(register),
            value: u64::try_from((bits >> (lane * 64)) & u128::from(u64::MAX))
                .expect("one selected constant lane is exactly 64 bits"),
        });
    }
    Ok(())
}

fn select_unary(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: MachineOperationId,
    operation: MachineUnaryOperation,
    operand: MachineValueId,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    let operand_storage = values
        .value(operand)
        .ok_or(Arm64SelectionError::UnknownValue(operand))?;
    if matches!(operand_storage, Arm64ValueStorage::Floating { .. }) {
        let (operand, size) = floating_value(values, operand)?;
        if operation != MachineUnaryOperation::Negate {
            return Err(Arm64SelectionError::UnsupportedScalar(result));
        }
        let (destination, result_size) = floating_value(values, result)?;
        if result_size != size {
            return Err(Arm64SelectionError::UnsupportedScalar(result));
        }
        selected.push(Arm64SelectedInstruction::FloatNegate {
            size,
            destination,
            operand,
        });
        return Ok(());
    }
    let (size, _) = scalar_value(scope.0, scope.1, operand)?;
    selected.push(Arm64SelectedInstruction::Unary {
        size,
        operation: match operation {
            MachineUnaryOperation::LogicalNot => Arm64SelectedUnaryOperation::LogicalNot,
            MachineUnaryOperation::Negate => Arm64SelectedUnaryOperation::Negate,
        },
        destination: one_word(values, result)?,
        operand: one_word(values, operand)?,
    });
    Ok(())
}

fn select_numeric_conversion(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: MachineOperationId,
    operand: MachineValueId,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    match (
        machine_scalar(scope.0, scope.1, operand)?,
        machine_scalar(scope.0, scope.1, result)?,
    ) {
        (
            MachineScalar::Integer {
                bits: source_bits,
                signed,
            },
            MachineScalar::Integer {
                bits: target_bits, ..
            },
        ) if source_bits <= target_bits => {
            selected.push(Arm64SelectedInstruction::IntegerConversion {
                size: integer_data_size(target_bits, result)?,
                source_bits,
                signed,
                destination: one_word(values, result)?,
                source: one_word(values, operand)?,
            });
            Ok(())
        }
        (
            MachineScalar::Integer {
                bits: source_bits,
                signed,
            },
            target @ (MachineScalar::Float32 | MachineScalar::Float64),
        ) => {
            selected.push(Arm64SelectedInstruction::FloatFromInteger {
                source_size: integer_data_size(source_bits, operand)?,
                target_size: float_scalar_size(target),
                signed,
                destination: floating_value(values, result)?.0,
                source: one_word(values, operand)?,
            });
            Ok(())
        }
        (MachineScalar::Float32, MachineScalar::Float64) => {
            selected.push(Arm64SelectedInstruction::FloatWiden {
                destination: floating_value(values, result)?.0,
                source: floating_value(values, operand)?.0,
            });
            Ok(())
        }
        _ => Err(Arm64SelectionError::UnsupportedScalar(result)),
    }
}

fn integer_data_size(
    bits: u8,
    value: MachineValueId,
) -> Result<Arm64DataSize, Arm64SelectionError> {
    match bits {
        1..=32 => Ok(Arm64DataSize::Bits32),
        33..=64 => Ok(Arm64DataSize::Bits64),
        _ => Err(Arm64SelectionError::UnsupportedScalar(value)),
    }
}

const fn float_scalar_size(scalar: MachineScalar) -> Arm64DataSize {
    match scalar {
        MachineScalar::Float32 => Arm64DataSize::Bits32,
        MachineScalar::Float64 => Arm64DataSize::Bits64,
        _ => unreachable!(),
    }
}

fn select_binary(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: MachineOperationId,
    operation: MachineBinaryOperation,
    operands: (MachineValueId, MachineValueId),
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let (left, right) = operands;
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    let left_storage = values
        .value(left)
        .ok_or(Arm64SelectionError::UnknownValue(left))?;
    if matches!(left_storage, Arm64ValueStorage::Floating { .. }) {
        let (left_register, size) = floating_value(values, left)?;
        let (right_register, right_size) = floating_value(values, right)?;
        if right_size != size {
            return Err(Arm64SelectionError::UnsupportedScalar(right));
        }
        match operation {
            MachineBinaryOperation::Add
            | MachineBinaryOperation::Subtract
            | MachineBinaryOperation::Multiply
            | MachineBinaryOperation::Divide => {
                let (destination, result_size) = floating_value(values, result)?;
                if result_size != size {
                    return Err(Arm64SelectionError::UnsupportedScalar(result));
                }
                let operation = match operation {
                    MachineBinaryOperation::Add => crate::Arm64FloatBinary::Add,
                    MachineBinaryOperation::Subtract => crate::Arm64FloatBinary::Subtract,
                    MachineBinaryOperation::Multiply => crate::Arm64FloatBinary::Multiply,
                    MachineBinaryOperation::Divide => crate::Arm64FloatBinary::Divide,
                    _ => unreachable!(),
                };
                selected.push(Arm64SelectedInstruction::FloatBinary {
                    size,
                    operation,
                    destination,
                    left: left_register,
                    right: right_register,
                });
            }
            MachineBinaryOperation::Equal | MachineBinaryOperation::Less => {
                selected.push(Arm64SelectedInstruction::FloatCompare {
                    size,
                    operation: if operation == MachineBinaryOperation::Equal {
                        Arm64SelectedFloatComparisonOperation::Equal
                    } else {
                        Arm64SelectedFloatComparisonOperation::Less
                    },
                    destination: one_word(values, result)?,
                    left: left_register,
                    right: right_register,
                });
            }
            MachineBinaryOperation::Remainder
            | MachineBinaryOperation::ShiftLeft
            | MachineBinaryOperation::ShiftRightSigned
            | MachineBinaryOperation::ShiftRightUnsigned => {
                return Err(Arm64SelectionError::UnsupportedScalar(left));
            }
        }
        return Ok(());
    }
    let (size, signed) = scalar_value(scope.0, scope.1, left)?;
    let selected_operation = match operation {
        MachineBinaryOperation::Add => Arm64SelectedBinaryOperation::Add,
        MachineBinaryOperation::Subtract => Arm64SelectedBinaryOperation::Subtract,
        MachineBinaryOperation::Multiply => Arm64SelectedBinaryOperation::Multiply,
        MachineBinaryOperation::Divide => Arm64SelectedBinaryOperation::Divide { signed },
        MachineBinaryOperation::Remainder => Arm64SelectedBinaryOperation::Remainder { signed },
        MachineBinaryOperation::ShiftLeft => Arm64SelectedBinaryOperation::ShiftLeft,
        MachineBinaryOperation::ShiftRightSigned => {
            Arm64SelectedBinaryOperation::ShiftRight { signed: true }
        }
        MachineBinaryOperation::ShiftRightUnsigned => {
            Arm64SelectedBinaryOperation::ShiftRight { signed: false }
        }
        MachineBinaryOperation::Equal => Arm64SelectedBinaryOperation::Equal,
        MachineBinaryOperation::Less => Arm64SelectedBinaryOperation::Less { signed },
    };
    selected.push(Arm64SelectedInstruction::Binary {
        size,
        operation: selected_operation,
        destination: one_word(values, result)?,
        left: one_word(values, left)?,
        right: one_word(values, right)?,
    });
    Ok(())
}

fn select_comparison(
    operation_id: MachineOperationId,
    comparison: nocter_machine::MachineComparison,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    if let MachineComparisonRepresentation::Scalar(
        scalar @ (MachineScalar::Float32 | MachineScalar::Float64),
    ) = comparison.representation()
    {
        let size = match scalar {
            MachineScalar::Float32 => Arm64DataSize::Bits32,
            MachineScalar::Float64 => Arm64DataSize::Bits64,
            _ => unreachable!(),
        };
        selected.push(Arm64SelectedInstruction::CompareFloatBorrowed {
            size,
            operation: match comparison.operation() {
                MachineComparisonOperation::Equal => Arm64SelectedFloatComparisonOperation::Equal,
                MachineComparisonOperation::Less => Arm64SelectedFloatComparisonOperation::Less,
            },
            offset: 0,
            destination: one_word(values, result)?,
            left: one_word(values, comparison.left())?,
            right: one_word(values, comparison.right())?,
        });
        return Ok(());
    }
    let (size, extension, offset, signed) = match comparison.representation() {
        MachineComparisonRepresentation::Scalar(scalar) => {
            let (size, extension, signed) =
                crate::memory_selection::scalar_memory_representation(scalar)?;
            (size, extension, 0, signed)
        }
        MachineComparisonRepresentation::Tag { offset } => (
            Arm64LoadStoreSize::Byte,
            Arm64SelectedLoadExtension::Zero,
            offset,
            false,
        ),
    };
    let operation = match comparison.operation() {
        MachineComparisonOperation::Equal => Arm64SelectedComparisonOperation::Equal,
        MachineComparisonOperation::Less => Arm64SelectedComparisonOperation::Less { signed },
    };
    selected.push(Arm64SelectedInstruction::CompareBorrowed {
        size,
        extension,
        operation,
        offset,
        destination: one_word(values, result)?,
        left: one_word(values, comparison.left())?,
        right: one_word(values, comparison.right())?,
    });
    Ok(())
}

fn select_terminator(
    context: Arm64SelectionContext<'_>,
    block: MachineBlockId,
    terminator: &MachineTerminator,
    deferred: bool,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    let function = context
        .program()
        .function(context.owner())
        .ok_or(Arm64SelectionError::UnknownFunction(context.owner()))?;
    let values = context.values();
    let frame = context.frame();
    let addresses = context.addresses();
    let switch_context =
        crate::switch_selection::SwitchSelectionContext::new(function, values, frame);
    match terminator {
        MachineTerminator::Goto(target) => Ok(Arm64SelectedTerminator::Goto(select_edge(
            function, target, values, frame,
        )?)),
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => Ok(Arm64SelectedTerminator::Branch {
            condition: one_word(values, *condition)?,
            then_edge: select_edge(function, then_target, values, frame)?,
            else_edge: select_edge(function, else_target, values, frame)?,
        }),
        MachineTerminator::BranchDropFlag {
            flag,
            initialized,
            uninitialized,
        } => {
            let condition = crate::destruction_selection::select_flag_read(*flag, frame, selected)?;
            Ok(Arm64SelectedTerminator::Branch {
                condition,
                then_edge: select_edge(function, initialized, values, frame)?,
                else_edge: select_edge(function, uninitialized, values, frame)?,
            })
        }
        MachineTerminator::SwitchValue {
            subject,
            cases,
            fallback,
        } => crate::switch_selection::select_value(switch_context, *subject, cases, fallback),
        MachineTerminator::SwitchTag {
            subject,
            tag_offset,
            cases,
            fallback,
        } => crate::switch_selection::select_tag(
            switch_context,
            *subject,
            *tag_offset,
            cases,
            fallback,
            addresses,
            selected,
        ),
        MachineTerminator::Suspend {
            computation,
            resume,
        } if deferred => {
            let [] = resume.arguments() else {
                return Err(Arm64SelectionError::EdgeArity(resume.block()));
            };
            let [result] = function
                .body()
                .block(resume.block())
                .map(nocter_machine::MachineBlock::parameters)
                .ok_or(Arm64SelectionError::UnknownBlock(resume.block()))?
            else {
                return Err(Arm64SelectionError::EdgeArity(resume.block()));
            };
            Ok(Arm64SelectedTerminator::Suspend {
                computation: one_word(values, *computation)?,
                resume: resume.block(),
                result: *result,
            })
        }
        MachineTerminator::Suspend { .. } => {
            Err(Arm64SelectionError::UnsupportedAsyncControl(block))
        }
        MachineTerminator::Return(value) if deferred => {
            Ok(Arm64SelectedTerminator::DeferredReturn(*value))
        }
        MachineTerminator::Return(value) => {
            crate::call_selection::select_return(function, block, *value, values, frame, selected)?;
            Ok(Arm64SelectedTerminator::Return)
        }
        MachineTerminator::Exit(value) => Ok(Arm64SelectedTerminator::Exit(
            value.map(|value| one_word(values, value)).transpose()?,
        )),
        MachineTerminator::Trap => Ok(Arm64SelectedTerminator::Trap),
        MachineTerminator::Unreachable => Ok(Arm64SelectedTerminator::Unreachable),
    }
}

pub(crate) fn select_edge(
    function: &nocter_machine::MachineFunction,
    target: &MachineBranchTarget,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
) -> Result<Arm64SelectedEdge, Arm64SelectionError> {
    let parameters = function
        .body()
        .block(target.block())
        .map(nocter_machine::MachineBlock::parameters)
        .ok_or(Arm64SelectionError::UnknownBlock(target.block()))?;
    if parameters.len() != target.arguments().len() {
        return Err(Arm64SelectionError::EdgeArity(target.block()));
    }
    let mut copies = Vec::new();
    let mut float_copies = Vec::new();
    let mut memory_copies = Vec::new();
    for (argument, parameter) in target
        .arguments()
        .iter()
        .copied()
        .zip(parameters.iter().copied())
    {
        match (values.value(argument), values.value(parameter)) {
            (Some(Arm64ValueStorage::Omitted), Some(Arm64ValueStorage::Omitted)) => {}
            (
                Some(Arm64ValueStorage::Direct(sources)),
                Some(Arm64ValueStorage::Direct(destinations)),
            ) if sources.len() == destinations.len() => {
                copies.extend(
                    sources
                        .iter()
                        .zip(destinations)
                        .map(|(source, destination)| Arm64SelectedCopy {
                            destination: Arm64SelectedRegister::Virtual(*destination),
                            source: Arm64SelectedRegister::Virtual(*source),
                        }),
                );
            }
            (
                Some(Arm64ValueStorage::Floating {
                    register: source,
                    bytes: source_bytes,
                }),
                Some(Arm64ValueStorage::Floating {
                    register: destination,
                    bytes: destination_bytes,
                }),
            ) if source_bytes == destination_bytes => {
                float_copies.push(Arm64SelectedFloatCopy {
                    size: match source_bytes {
                        4 => Arm64DataSize::Bits32,
                        8 => Arm64DataSize::Bits64,
                        _ => return Err(Arm64SelectionError::EdgeTransport(target.block())),
                    },
                    destination: Arm64SelectedFloatRegister::Virtual(*destination),
                    source: Arm64SelectedFloatRegister::Virtual(*source),
                });
            }
            (
                Some(Arm64ValueStorage::Memory {
                    size: source_size, ..
                }),
                Some(Arm64ValueStorage::Memory {
                    size: destination_size,
                    ..
                }),
            ) if source_size == destination_size => {
                memory_copies.push(Arm64SelectedMemoryCopy {
                    destination: memory_value_address(frame, parameter)?,
                    source: memory_value_address(frame, argument)?,
                    bytes: *source_size,
                });
            }
            (Some(_), Some(_)) => return Err(Arm64SelectionError::EdgeTransport(target.block())),
            (None, _) => return Err(Arm64SelectionError::UnknownValue(argument)),
            (_, None) => return Err(Arm64SelectionError::UnknownValue(parameter)),
        }
    }
    Ok(Arm64SelectedEdge {
        target: target.block(),
        copies: copies.into_boxed_slice(),
        float_copies: float_copies.into_boxed_slice(),
        memory_copies: memory_copies.into_boxed_slice(),
    })
}

fn memory_value_address(
    frame: &Arm64FunctionFrame,
    value: MachineValueId,
) -> Result<Arm64SelectedStackAddress, Arm64SelectionError> {
    frame
        .memory_value(value)
        .map(|object| Arm64SelectedStackAddress::FrameObject { object, offset: 0 })
        .ok_or(Arm64SelectionError::MemoryValue(value))
}

pub(crate) fn direct_value(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<&[Arm64VirtualRegister], Arm64SelectionError> {
    match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Direct(registers) => Ok(registers),
        Arm64ValueStorage::Omitted => Ok(&[]),
        Arm64ValueStorage::Floating { .. } | Arm64ValueStorage::Memory { .. } => {
            Err(Arm64SelectionError::MemoryValue(value))
        }
    }
}

pub(crate) fn floating_value(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<(Arm64SelectedFloatRegister, Arm64DataSize), Arm64SelectionError> {
    match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Floating { register, bytes: 4 } => Ok((
            Arm64SelectedFloatRegister::Virtual(*register),
            Arm64DataSize::Bits32,
        )),
        Arm64ValueStorage::Floating { register, bytes: 8 } => Ok((
            Arm64SelectedFloatRegister::Virtual(*register),
            Arm64DataSize::Bits64,
        )),
        _ => Err(Arm64SelectionError::UnsupportedScalar(value)),
    }
}

fn scalar_value(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    value: MachineValueId,
) -> Result<(Arm64DataSize, bool), Arm64SelectionError> {
    let ty = program
        .function(owner)
        .and_then(|function| function.body().value(value))
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(value))?;
    match program
        .layouts()
        .get(ty)
        .map(nocter_machine::MachineLayout::kind)
    {
        Some(MachineLayoutKind::Scalar(MachineScalar::Bool)) => Ok((Arm64DataSize::Bits32, false)),
        Some(MachineLayoutKind::Scalar(MachineScalar::Integer { bits, signed })) => {
            let size = match bits {
                1..=32 => Arm64DataSize::Bits32,
                33..=64 => Arm64DataSize::Bits64,
                _ => return Err(Arm64SelectionError::UnsupportedScalar(value)),
            };
            Ok((size, *signed))
        }
        _ => Err(Arm64SelectionError::UnsupportedScalar(value)),
    }
}

fn machine_scalar(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    value: MachineValueId,
) -> Result<MachineScalar, Arm64SelectionError> {
    let ty = program
        .function(owner)
        .and_then(|function| function.body().value(value))
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(value))?;
    match program
        .layouts()
        .get(ty)
        .map(nocter_machine::MachineLayout::kind)
    {
        Some(MachineLayoutKind::Scalar(scalar)) => Ok(*scalar),
        _ => Err(Arm64SelectionError::UnsupportedScalar(value)),
    }
}

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match direct_value(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}
