use std::collections::BTreeSet;
use std::fmt;

use nocter_machine::{
    MachineArgumentAbi, MachineFrameField, MachineFunctionExecution, MachineFunctionId,
    MachineFunctionKind, MachinePackAbi, MachineResultAbi, MachineResultLocation,
    MachineValueClass,
};

use crate::{
    Arm64AsyncFrameField, Arm64AsyncFrameLayout, Arm64AsyncFrameLayoutError, Arm64FrameLayout,
    Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObjectId,
};

/// One constructor input and its target frame destination.
///
/// `transport` is the already-selected Machine ABI. Native materialization must only move bytes
/// from that transport into `destination`; it cannot reclassify the parameter type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncParameterCapture {
    source: nocter_machine::MachineStackId,
    transport: MachineArgumentAbi,
    destination: Arm64AsyncFrameField,
}

impl Arm64AsyncParameterCapture {
    #[must_use]
    pub const fn source(self) -> nocter_machine::MachineStackId {
        self.source
    }

    #[must_use]
    pub const fn transport(self) -> MachineArgumentAbi {
        self.transport
    }

    #[must_use]
    pub const fn destination(self) -> Arm64AsyncFrameField {
        self.destination
    }
}

/// The compiler-owned pack pointer accepted by a deferred constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncPackCapture {
    transport: MachinePackAbi,
    destination: Arm64AsyncFrameField,
}

impl Arm64AsyncPackCapture {
    #[must_use]
    pub const fn transport(self) -> MachinePackAbi {
        self.transport
    }

    #[must_use]
    pub const fn destination(self) -> Arm64AsyncFrameField {
        self.destination
    }
}

/// Closed ARM64 plan for constructing one allocation-backed deferred computation.
///
/// Machine remains the authority for the initial ownership set and callable transport. This plan
/// only proves that every initial input has one heap destination and that the constructor returns
/// the uniform one-word owning handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64AsyncFunctionPlan {
    owner: MachineFunctionId,
    frame: Arm64AsyncFrameLayout,
    parameters: Box<[Arm64AsyncParameterCapture]>,
    pack: Option<Arm64AsyncPackCapture>,
    result_register: u8,
    constructor_frame: Arm64AsyncConstructorFrame,
    cancellation: crate::async_cancellation::Arm64AsyncCancellationPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncConstructorFrame {
    layout: Arm64FrameLayout,
    parameters: Box<[Arm64FrameObjectId]>,
    allocation_context: Arm64FrameObjectId,
    process_context: Option<Arm64FrameObjectId>,
    mapping: Arm64FrameObjectId,
}

impl Arm64AsyncConstructorFrame {
    pub(crate) const fn layout(&self) -> &Arm64FrameLayout {
        &self.layout
    }

    pub(crate) fn parameter(&self, index: usize) -> Option<Arm64FrameObjectId> {
        self.parameters.get(index).copied()
    }

    pub(crate) const fn allocation_context(&self) -> Arm64FrameObjectId {
        self.allocation_context
    }

    pub(crate) const fn process_context(&self) -> Option<Arm64FrameObjectId> {
        self.process_context
    }

    pub(crate) const fn mapping(&self) -> Arm64FrameObjectId {
        self.mapping
    }
}

impl Arm64AsyncFunctionPlan {
    /// Selects the complete constructor capture plan without inspecting MIR or source types.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or immediate function, a non-callable deferred entry, inconsistent
    /// initial-frame fields, malformed parameter/pack transport, or a non-uniform async handle.
    pub fn build(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
    ) -> Result<Self, Arm64AsyncFunctionPlanError> {
        let function = program
            .function(owner)
            .ok_or(Arm64AsyncFunctionPlanError::UnknownFunction(owner))?;
        let MachineFunctionExecution::Deferred(machine_frame) = function.execution() else {
            return Err(Arm64AsyncFunctionPlanError::ImmediateFunction(owner));
        };
        let MachineFunctionKind::Callable(abi) = function.kind() else {
            return Err(Arm64AsyncFunctionPlanError::NonCallable(owner));
        };
        if function.body().parameters().len() != abi.arguments().len() {
            return Err(Arm64AsyncFunctionPlanError::ParameterArity(owner));
        }

        let frame = Arm64AsyncFrameLayout::build(program, owner)?;
        let expected = function
            .body()
            .parameters()
            .iter()
            .copied()
            .map(MachineFrameField::Stack)
            .chain(abi.pack().map(|_| MachineFrameField::Pack))
            .collect::<BTreeSet<_>>();
        let actual = machine_frame
            .initial()
            .fields()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual != expected || actual.len() != machine_frame.initial().fields().len() {
            return Err(Arm64AsyncFunctionPlanError::InitialFields(owner));
        }

        let parameters = function
            .body()
            .parameters()
            .iter()
            .copied()
            .zip(abi.arguments().iter().copied())
            .map(|(source, transport)| {
                let destination = frame.stack_object(source).ok_or(
                    Arm64AsyncFunctionPlanError::MissingParameter {
                        function: owner,
                        parameter: source,
                    },
                )?;
                validate_parameter(owner, function, source, transport, destination)?;
                Ok(Arm64AsyncParameterCapture {
                    source,
                    transport,
                    destination,
                })
            })
            .collect::<Result<Vec<_>, Arm64AsyncFunctionPlanError>>()?
            .into_boxed_slice();
        let pack = match (abi.pack(), frame.pack_input()) {
            (None, None) => None,
            (Some(transport), Some(destination)) => Some(Arm64AsyncPackCapture {
                transport,
                destination,
            }),
            (None, Some(_)) | (Some(_), None) => {
                return Err(Arm64AsyncFunctionPlanError::Pack(owner));
            }
        };
        let result_register = validate_result(owner, abi.result())?;
        let constructor_frame = build_constructor_frame(&frame, &parameters)?;
        let cancellation =
            crate::async_cancellation::Arm64AsyncCancellationPlan::build(owner, function, &frame)?;
        Ok(Self {
            owner,
            frame,
            parameters,
            pack,
            result_register,
            constructor_frame,
            cancellation,
        })
    }

    #[must_use]
    pub const fn owner(&self) -> MachineFunctionId {
        self.owner
    }

    #[must_use]
    pub const fn frame(&self) -> &Arm64AsyncFrameLayout {
        &self.frame
    }

    #[must_use]
    pub const fn parameters(&self) -> &[Arm64AsyncParameterCapture] {
        &self.parameters
    }

    #[must_use]
    pub const fn pack(&self) -> Option<Arm64AsyncPackCapture> {
        self.pack
    }

    #[must_use]
    pub const fn result_register(&self) -> u8 {
        self.result_register
    }

    /// Materializes the allocation and capture entry for this deferred function.
    ///
    /// # Errors
    ///
    /// Rejects a foreign or immediate native target, pack transfer until its ownership-preserving
    /// representation is implemented, malformed constructor storage, or code emission failure.
    pub fn materialize_constructor(
        &self,
        target: crate::Arm64FunctionTarget,
    ) -> Result<crate::Arm64Code, crate::Arm64AsyncConstructorError> {
        crate::async_constructor_code::materialize(self, target)
    }

    /// Materializes the completed-output transfer entry for this deferred function.
    ///
    /// # Errors
    ///
    /// Rejects a foreign or immediate native target, invalid runtime register assignments,
    /// malformed output placement, or code emission failure.
    pub fn materialize_consume(
        &self,
        target: crate::Arm64FunctionTarget,
    ) -> Result<crate::Arm64Code, crate::Arm64AsyncConsumeError> {
        crate::async_consume_code::materialize(self, target)
    }

    /// Materializes state-dispatched cancellation and exact owned-resource cleanup.
    ///
    /// # Errors
    ///
    /// Rejects foreign lifecycle or destruction targets, malformed async-frame projections,
    /// transferred argument packs until their owning heap representation is implemented, and
    /// native code emission failures.
    pub fn materialize_cancel(
        &self,
        target: crate::Arm64FunctionTarget,
        functions: &crate::Arm64FunctionTargets,
    ) -> Result<crate::Arm64Code, crate::Arm64AsyncCancelError> {
        crate::async_cancel_code::materialize(self, target, functions)
    }

    pub(crate) const fn constructor_frame(&self) -> &Arm64AsyncConstructorFrame {
        &self.constructor_frame
    }

    pub(crate) const fn cancellation(
        &self,
    ) -> &crate::async_cancellation::Arm64AsyncCancellationPlan {
        &self.cancellation
    }
}

fn build_constructor_frame(
    frame: &Arm64AsyncFrameLayout,
    parameters: &[Arm64AsyncParameterCapture],
) -> Result<Arm64AsyncConstructorFrame, Arm64AsyncFunctionPlanError> {
    let mut builder = Arm64FrameLayoutBuilder::new();
    let parameters = parameters
        .iter()
        .map(|capture| {
            builder.add_object(
                capture.destination().size(),
                capture.destination().alignment(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let allocation_context = builder.add_object(8, 8)?;
    let process_context = frame
        .process_context()
        .map(|_| builder.add_object(8, 8))
        .transpose()?;
    let mapping = builder.add_object(8, 8)?;
    Ok(Arm64AsyncConstructorFrame {
        layout: builder.finish()?,
        parameters: parameters.into_boxed_slice(),
        allocation_context,
        process_context,
        mapping,
    })
}

fn validate_parameter(
    owner: MachineFunctionId,
    function: &nocter_machine::MachineFunction,
    source: nocter_machine::MachineStackId,
    transport: MachineArgumentAbi,
    destination: Arm64AsyncFrameField,
) -> Result<(), Arm64AsyncFunctionPlanError> {
    let object =
        function
            .body()
            .stack(source)
            .ok_or(Arm64AsyncFunctionPlanError::MissingParameter {
                function: owner,
                parameter: source,
            })?;
    if object.size() != destination.size() || object.alignment() != destination.alignment() {
        return Err(Arm64AsyncFunctionPlanError::ParameterLayout {
            function: owner,
            parameter: source,
        });
    }
    match (transport.class(), transport.location(), object.size()) {
        (MachineValueClass::Zero, None, 0)
        | (
            MachineValueClass::Direct { .. }
            | MachineValueClass::Float32
            | MachineValueClass::Float64
            | MachineValueClass::Indirect,
            Some(_),
            _,
        ) => Ok(()),
        _ => Err(Arm64AsyncFunctionPlanError::ParameterTransport {
            function: owner,
            parameter: source,
        }),
    }
}

fn validate_result(
    owner: MachineFunctionId,
    result: MachineResultAbi,
) -> Result<u8, Arm64AsyncFunctionPlanError> {
    let MachineResultAbi::Value(result) = result else {
        return Err(Arm64AsyncFunctionPlanError::Result(owner));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64AsyncFunctionPlanError::Result(owner));
    };
    if result.class() != (MachineValueClass::Direct { words: 1 }) || registers.words() != 1 {
        return Err(Arm64AsyncFunctionPlanError::Result(owner));
    }
    Ok(registers.first())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64AsyncFunctionPlanError {
    UnknownFunction(MachineFunctionId),
    ImmediateFunction(MachineFunctionId),
    NonCallable(MachineFunctionId),
    ParameterArity(MachineFunctionId),
    InitialFields(MachineFunctionId),
    MissingParameter {
        function: MachineFunctionId,
        parameter: nocter_machine::MachineStackId,
    },
    ParameterLayout {
        function: MachineFunctionId,
        parameter: nocter_machine::MachineStackId,
    },
    ParameterTransport {
        function: MachineFunctionId,
        parameter: nocter_machine::MachineStackId,
    },
    Pack(MachineFunctionId),
    Result(MachineFunctionId),
    Frame(Arm64AsyncFrameLayoutError),
    ConstructorFrame(Arm64FrameLayoutError),
    Cancellation(crate::Arm64AsyncCancellationPlanError),
}

impl fmt::Display for Arm64AsyncFunctionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 async function planning failed: {self:?}")
    }
}

impl std::error::Error for Arm64AsyncFunctionPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::ConstructorFrame(error) => Some(error),
            Self::Cancellation(error) => Some(error),
            Self::UnknownFunction(_)
            | Self::ImmediateFunction(_)
            | Self::NonCallable(_)
            | Self::ParameterArity(_)
            | Self::InitialFields(_)
            | Self::MissingParameter { .. }
            | Self::ParameterLayout { .. }
            | Self::ParameterTransport { .. }
            | Self::Pack(_)
            | Self::Result(_) => None,
        }
    }
}

impl From<Arm64AsyncFrameLayoutError> for Arm64AsyncFunctionPlanError {
    fn from(error: Arm64AsyncFrameLayoutError) -> Self {
        Self::Frame(error)
    }
}

impl From<Arm64FrameLayoutError> for Arm64AsyncFunctionPlanError {
    fn from(error: Arm64FrameLayoutError) -> Self {
        Self::ConstructorFrame(error)
    }
}

impl From<crate::Arm64AsyncCancellationPlanError> for Arm64AsyncFunctionPlanError {
    fn from(error: crate::Arm64AsyncCancellationPlanError) -> Self {
        Self::Cancellation(error)
    }
}
