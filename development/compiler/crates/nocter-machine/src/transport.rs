use std::collections::BTreeMap;
use std::fmt;

use nocter_mir::{
    MirBody, MirCallSignature, MirCallTarget, MirFunction, MirOperationKind, MirProgram, MirRoot,
};
use nocter_model::{Arena, ArenaBuilder, ExecutableItemId, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTable};

use crate::identity::{MachineId, MachinePrimitiveAbiId, MachineTable};
use crate::{MachineLayout, MachineLayoutStore, MachineTarget};

/// Stored-value classification before a caller or callee assigns concrete ABI locations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueClass {
    Zero,
    Direct { words: u8 },
    Indirect,
}

impl MachineValueClass {
    /// Classifies one completed stored layout under the selected machine ABI.
    #[must_use]
    pub fn for_layout(layout: &MachineLayout, target: MachineTarget) -> Self {
        if layout.size() == 0 {
            Self::Zero
        } else {
            let words = layout.size().div_ceil(target.word_size());
            if words <= u64::from(target.direct_value_word_limit()) {
                u8::try_from(words).map_or(Self::Indirect, |words| Self::Direct { words })
            } else {
                Self::Indirect
            }
        }
    }
}

/// A consecutive range of integer registers in the selected ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineRegisterSpan {
    first: u8,
    words: u8,
}

impl MachineRegisterSpan {
    #[must_use]
    pub const fn first(self) -> u8 {
        self.first
    }

    #[must_use]
    pub const fn words(self) -> u8 {
        self.words
    }
}

/// One aligned caller-owned slot in the outgoing stack-argument area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineStackSlot {
    offset: u64,
    size: u64,
    alignment: u64,
}

impl MachineStackSlot {
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// Location carrying either a direct value or the pointer for an indirect argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineArgumentLocation {
    Registers(MachineRegisterSpan),
    Stack(MachineStackSlot),
}

/// One source-level argument after stored-value classification and left-to-right placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineArgumentAbi {
    ty: TypeId,
    class: MachineValueClass,
    location: Option<MachineArgumentLocation>,
}

impl MachineArgumentAbi {
    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn class(self) -> MachineValueClass {
        self.class
    }

    /// Returns no location only for a zero-sized source value.
    #[must_use]
    pub const fn location(self) -> Option<MachineArgumentLocation> {
        self.location
    }
}

/// The compiler-owned pointer lane used by an argument-pack body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachinePackAbi {
    element: TypeId,
    next: TypeId,
    next_result: MachineResultAbi,
    pointer: MachineRegisterSpan,
}

impl MachinePackAbi {
    #[must_use]
    pub const fn element(self) -> TypeId {
        self.element
    }

    #[must_use]
    pub const fn next(self) -> TypeId {
        self.next
    }

    #[must_use]
    pub const fn next_result(self) -> MachineResultAbi {
        self.next_result
    }

    #[must_use]
    pub const fn pointer(self) -> MachineRegisterSpan {
        self.pointer
    }
}

/// Where a returning stored value is delivered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineResultLocation {
    Omitted,
    Registers(MachineRegisterSpan),
    CallerStorage { pointer_register: u8 },
}

/// One concrete stored return value and its ABI transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineReturnedValue {
    ty: TypeId,
    class: MachineValueClass,
    location: MachineResultLocation,
}

impl MachineReturnedValue {
    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn class(self) -> MachineValueClass {
        self.class
    }

    #[must_use]
    pub const fn location(self) -> MachineResultLocation {
        self.location
    }
}

/// Completion, divergence, or a concrete returned value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineResultAbi {
    Completion,
    Diverging,
    Value(MachineReturnedValue),
}

/// The single caller/callee ABI contract for one concrete function signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineCallableAbi {
    arguments: Box<[MachineArgumentAbi]>,
    pack: Option<MachinePackAbi>,
    result: MachineResultAbi,
    stack_argument_size: u64,
}

impl MachineCallableAbi {
    #[must_use]
    pub const fn arguments(&self) -> &[MachineArgumentAbi] {
        &self.arguments
    }

    #[must_use]
    pub const fn pack(&self) -> Option<MachinePackAbi> {
        self.pack
    }

    #[must_use]
    pub const fn result(&self) -> MachineResultAbi {
        self.result
    }

    /// Includes final padding required to preserve call-boundary stack alignment.
    #[must_use]
    pub const fn stack_argument_size(&self) -> u64 {
        self.stack_argument_size
    }
}

/// Temporary whole-program ABI authority used while lowering MIR into machine operations.
///
/// Direct callable entries are moved into their final machine functions. Primitive signatures are
/// interned here, referenced by identity from calls, and transferred as one dense final table.
#[derive(Debug)]
pub struct MachineAbiPlan {
    target: MachineTarget,
    callables: Arena<ExecutableItemId, MachineCallableAbi>,
    primitive_signatures: MachinePrimitiveSignatureIndex,
    primitive_abis: Vec<MachineCallableAbi>,
}

/// Machine-owned lookup from a borrowed MIR signature to the canonical primitive ABI identity.
///
/// The two-level map permits lookup by `&[TypeId]` without requiring MIR signatures to implement
/// ordering or allocating a temporary owned key for repeated calls.
#[derive(Debug, Default)]
struct MachinePrimitiveSignatureIndex {
    by_parameters: BTreeMap<Box<[TypeId]>, BTreeMap<TypeId, MachinePrimitiveAbiId>>,
}

impl MachinePrimitiveSignatureIndex {
    fn get(&self, signature: &MirCallSignature) -> Option<MachinePrimitiveAbiId> {
        self.by_parameters
            .get(signature.parameters())?
            .get(&signature.result())
            .copied()
    }

    fn insert(&mut self, signature: &MirCallSignature, id: MachinePrimitiveAbiId) {
        let previous = self
            .by_parameters
            .entry(signature.parameters().into())
            .or_default()
            .insert(signature.result(), id);
        assert!(
            previous.is_none(),
            "primitive ABI signature was indexed twice"
        );
    }
}

/// Final primitive ABI authority retained after MIR signature lookup is no longer needed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MachinePrimitiveAbiTable {
    values: MachineTable<MachinePrimitiveAbiId, MachineCallableAbi>,
}

impl MachinePrimitiveAbiTable {
    pub(crate) fn get(&self, id: MachinePrimitiveAbiId) -> Option<&MachineCallableAbi> {
        self.values.get(id)
    }
}

impl MachineAbiPlan {
    /// Plans every direct function and unique primitive signature from exact MIR types.
    ///
    /// # Errors
    ///
    /// Rejects a missing parameter local, an invalid completion argument, a missing stored layout,
    /// a malformed argument-pack signature, or overflowing stack placement.
    pub fn build(
        program: &MirProgram,
        layouts: &MachineLayoutStore,
    ) -> Result<Self, MachineAbiError> {
        let target = layouts.target();
        let types = program.types();
        let mut callables = ArenaBuilder::new();
        let mut primitive_signatures = MachinePrimitiveSignatureIndex::default();
        let mut primitive_abis = Vec::new();
        for (expected, function) in program.functions().iter() {
            let actual = callables.insert(plan_function(function, types, layouts)?);
            if actual != expected {
                return Err(MachineAbiError::MismatchedFunctionIdentity { expected, actual });
            }
            collect_primitive_abis(
                function.body(),
                types,
                layouts,
                &mut primitive_signatures,
                &mut primitive_abis,
            )?;
        }
        match program.root() {
            MirRoot::Process(root) => collect_primitive_abis(
                root.body(),
                types,
                layouts,
                &mut primitive_signatures,
                &mut primitive_abis,
            )?,
            MirRoot::Tests { cases, .. } => {
                for root in cases {
                    collect_primitive_abis(
                        root.body(),
                        types,
                        layouts,
                        &mut primitive_signatures,
                        &mut primitive_abis,
                    )?;
                }
            }
        }
        Ok(Self {
            target,
            callables: callables.finish(),
            primitive_signatures,
            primitive_abis,
        })
    }

    #[must_use]
    pub const fn target(&self) -> MachineTarget {
        self.target
    }

    #[must_use]
    pub fn get(&self, item: ExecutableItemId) -> Option<&MachineCallableAbi> {
        self.callables.get(item)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (ExecutableItemId, &MachineCallableAbi)> {
        self.callables.iter()
    }

    pub(crate) fn primitive_signature_id(
        &self,
        signature: &MirCallSignature,
    ) -> Option<MachinePrimitiveAbiId> {
        self.primitive_signatures.get(signature)
    }

    pub(crate) fn finish(self) -> MachinePrimitiveAbiTable {
        MachinePrimitiveAbiTable {
            values: MachineTable::from_values(self.primitive_abis),
        }
    }
}

fn collect_primitive_abis(
    body: &MirBody,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutStore,
    signatures: &mut MachinePrimitiveSignatureIndex,
    abis: &mut Vec<MachineCallableAbi>,
) -> Result<(), MachineAbiError> {
    for (_, operation) in body.operations().iter() {
        let MirOperationKind::Call(call) = operation.kind() else {
            continue;
        };
        let MirCallTarget::StandardPrimitive { signature, .. } = call.target() else {
            continue;
        };
        if signatures.get(signature).is_some() {
            continue;
        }
        let id = MachinePrimitiveAbiId::new(abis.len());
        let abi = plan_signature(
            types,
            layouts,
            signature.parameters(),
            signature.result(),
            None,
        )?;
        signatures.insert(signature, id);
        abis.push(abi);
    }
    Ok(())
}

fn plan_function(
    function: &MirFunction,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutStore,
) -> Result<MachineCallableAbi, MachineAbiError> {
    let parameters = function
        .parameters()
        .iter()
        .copied()
        .map(|local| {
            function
                .locals()
                .get(local)
                .copied()
                .map(nocter_mir::MirLocal::ty)
                .ok_or(MachineAbiError::MissingParameterLocal(local))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pack = function.pack();
    if let Some(pack) = pack {
        require_layout(types, layouts, pack.element())?;
        require_layout(types, layouts, pack.next())?;
    }
    let pack = pack
        .map(|pack| {
            Ok(MachinePackAbi {
                element: pack.element(),
                next: pack.next(),
                next_result: plan_result(types, layouts, pack.next())?,
                pointer: MachineRegisterSpan {
                    first: layouts.target().pack_pointer_register(),
                    words: 1,
                },
            })
        })
        .transpose()?;
    plan_signature(types, layouts, &parameters, function.result(), pack)
}

pub(crate) fn plan_signature(
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutStore,
    parameters: &[TypeId],
    result: TypeId,
    pack: Option<MachinePackAbi>,
) -> Result<MachineCallableAbi, MachineAbiError> {
    let target = layouts.target();
    let mut register_window_open = true;
    let mut next_register = pack.map_or(0, |pack| pack.pointer.first + pack.pointer.words);
    let mut next_stack_offset = 0_u64;
    let mut arguments = Vec::with_capacity(parameters.len());

    for ty in parameters.iter().copied() {
        if is_completion_type(types, ty) {
            return Err(MachineAbiError::CompletionArgument(ty));
        }
        let layout = require_layout(types, layouts, ty)?;
        let class = MachineValueClass::for_layout(layout, target);
        let transport_words = match class {
            MachineValueClass::Zero => 0,
            MachineValueClass::Direct { words } => words,
            MachineValueClass::Indirect => 1,
        };
        let location = if transport_words == 0 {
            None
        } else if register_window_open
            && next_register
                .checked_add(transport_words)
                .is_some_and(|end| end <= target.argument_register_count())
        {
            let location = MachineArgumentLocation::Registers(MachineRegisterSpan {
                first: next_register,
                words: transport_words,
            });
            next_register += transport_words;
            Some(location)
        } else {
            register_window_open = false;
            let (size, alignment) = stack_transport(layout, class, target)?;
            let offset = align_up(next_stack_offset, alignment)?;
            next_stack_offset = offset
                .checked_add(size)
                .ok_or(MachineAbiError::StackOverflow)?;
            Some(MachineArgumentLocation::Stack(MachineStackSlot {
                offset,
                size,
                alignment,
            }))
        };
        arguments.push(MachineArgumentAbi {
            ty,
            class,
            location,
        });
    }

    let stack_argument_size = align_up(next_stack_offset, target.stack_alignment())?;
    Ok(MachineCallableAbi {
        arguments: arguments.into_boxed_slice(),
        pack,
        result: plan_result(types, layouts, result)?,
        stack_argument_size,
    })
}

pub(crate) fn plan_result(
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutStore,
    ty: TypeId,
) -> Result<MachineResultAbi, MachineAbiError> {
    match types.get(ty) {
        Some(RuntimeType::Primitive(RuntimePrimitive::Void)) => Ok(MachineResultAbi::Completion),
        Some(RuntimeType::Primitive(RuntimePrimitive::Never)) => Ok(MachineResultAbi::Diverging),
        Some(_) => {
            let target = layouts.target();
            let class = MachineValueClass::for_layout(require_layout(types, layouts, ty)?, target);
            let location = match class {
                MachineValueClass::Zero => MachineResultLocation::Omitted,
                MachineValueClass::Direct { words } => {
                    if words > target.direct_result_register_count() {
                        return Err(MachineAbiError::InvalidDirectResult { ty, words });
                    }
                    MachineResultLocation::Registers(MachineRegisterSpan { first: 0, words })
                }
                MachineValueClass::Indirect => MachineResultLocation::CallerStorage {
                    pointer_register: target.indirect_result_register(),
                },
            };
            Ok(MachineResultAbi::Value(MachineReturnedValue {
                ty,
                class,
                location,
            }))
        }
        None => Err(MachineAbiError::UnknownType(ty)),
    }
}

fn stack_transport(
    layout: &MachineLayout,
    class: MachineValueClass,
    target: MachineTarget,
) -> Result<(u64, u64), MachineAbiError> {
    match class {
        MachineValueClass::Zero => Err(MachineAbiError::ZeroValueHasLocation),
        MachineValueClass::Direct { words } => target
            .word_size()
            .checked_mul(u64::from(words))
            .map(|size| (size, layout.alignment().max(target.word_size())))
            .ok_or(MachineAbiError::StackOverflow),
        MachineValueClass::Indirect => Ok((target.pointer_size(), target.pointer_alignment())),
    }
}

fn require_layout<'layout>(
    types: &RuntimeTypeTable,
    layouts: &'layout MachineLayoutStore,
    ty: TypeId,
) -> Result<&'layout MachineLayout, MachineAbiError> {
    if types.get(ty).is_none() {
        return Err(MachineAbiError::UnknownType(ty));
    }
    layouts.get(ty).ok_or(MachineAbiError::MissingLayout(ty))
}

fn is_completion_type(types: &RuntimeTypeTable, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(RuntimeType::Primitive(
            RuntimePrimitive::Void | RuntimePrimitive::Never
        ))
    )
}

fn align_up(value: u64, alignment: u64) -> Result<u64, MachineAbiError> {
    if !alignment.is_power_of_two() {
        return Err(MachineAbiError::InvalidStackAlignment(alignment));
    }
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(MachineAbiError::StackOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAbiError {
    UnknownType(TypeId),
    MissingLayout(TypeId),
    MissingParameterLocal(nocter_model::MirLocalId),
    CompletionArgument(TypeId),
    MismatchedFunctionIdentity {
        expected: ExecutableItemId,
        actual: ExecutableItemId,
    },
    InvalidDirectResult {
        ty: TypeId,
        words: u8,
    },
    ZeroValueHasLocation,
    InvalidStackAlignment(u64),
    StackOverflow,
}

impl fmt::Display for MachineAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine ABI planning failed: {self:?}")
    }
}

impl std::error::Error for MachineAbiError {}
