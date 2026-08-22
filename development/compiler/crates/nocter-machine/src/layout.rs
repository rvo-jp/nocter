use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_mir::{MirBody, MirCallTarget, MirOperationKind, MirProgram, MirRoot};
use nocter_model::{
    BuiltinType, CaptureId, FieldId, ParameterId, TypeId, TypeKind, TypeStore, VariantId,
};
use nocter_runtime_contract::RuntimeTypeRepresentation;

use crate::MachineTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineScalar {
    Bool,
    Integer { bits: u8, signed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineOutcomeKind {
    Optional,
    Fallible,
}

impl MachineOutcomeKind {
    #[must_use]
    pub const fn primary_tag(self) -> u8 {
        0
    }

    #[must_use]
    pub const fn alternate_tag(self) -> u8 {
        1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineFieldLayout {
    field: FieldId,
    ty: TypeId,
    offset: u64,
}

impl MachineFieldLayout {
    #[must_use]
    pub const fn field(self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachinePayloadLayout {
    parameter: ParameterId,
    ty: TypeId,
    offset: u64,
}

impl MachinePayloadLayout {
    #[must_use]
    pub const fn parameter(self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineEnumVariantLayout {
    variant: VariantId,
    tag: u8,
    payload: Box<[MachinePayloadLayout]>,
}

impl MachineEnumVariantLayout {
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn tag(&self) -> u8 {
        self.tag
    }

    #[must_use]
    pub const fn payload(&self) -> &[MachinePayloadLayout] {
        &self.payload
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineCaptureLayout {
    capture: CaptureId,
    ty: TypeId,
    offset: u64,
}

impl MachineCaptureLayout {
    #[must_use]
    pub const fn capture(self) -> CaptureId {
        self.capture
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineLayoutKind {
    Scalar(MachineScalar),
    Pointer,
    View {
        pointer_offset: u64,
        length_offset: u64,
    },
    Error {
        code_offset: u64,
        message_offset: u64,
    },
    Struct {
        fields: Box<[MachineFieldLayout]>,
    },
    Enum {
        tag_offset: u64,
        payload_offset: u64,
        variants: Box<[MachineEnumVariantLayout]>,
    },
    FixedArray {
        element: TypeId,
        length: u64,
        stride: u64,
    },
    Closure {
        captures: Box<[MachineCaptureLayout]>,
    },
    Outcome {
        kind: MachineOutcomeKind,
        tag_offset: u64,
        payload_offset: u64,
        primary: Option<TypeId>,
        alternate: Option<TypeId>,
    },
    Opaque {
        witness: TypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineLayout {
    size: u64,
    alignment: u64,
    kind: MachineLayoutKind,
}

impl MachineLayout {
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn kind(&self) -> &MachineLayoutKind {
        &self.kind
    }
}

/// One immutable layout per concrete runtime type used by a validated MIR program.
#[derive(Debug)]
pub struct MachineLayoutStore {
    target: MachineTarget,
    layouts: BTreeMap<TypeId, MachineLayout>,
}

impl MachineLayoutStore {
    /// Computes the complete recursive stored-layout closure of one MIR program.
    ///
    /// # Errors
    ///
    /// Rejects symbolic, unsized, recursive-by-value, incomplete, or overflowing representation
    /// input. Such a failure is a compiler-integrity error, never a source diagnostic.
    pub fn build(program: &MirProgram) -> Result<Self, MachineLayoutError> {
        let target = MachineTarget::select(program.runtime_abi());
        let mut builder = LayoutBuilder {
            program,
            target,
            layouts: BTreeMap::new(),
            active: BTreeSet::new(),
        };
        let mut roots = BTreeSet::new();
        // Machine-generated CFGs use these canonical control and offset carriers independently of
        // whether source MIR happens to mention them.
        roots.extend([
            program.types().builtin(BuiltinType::Bool),
            program.types().builtin(BuiltinType::Usize),
        ]);
        let byte = program.types().builtin(BuiltinType::U8);
        if let Some(pointer) = program
            .types()
            .iter()
            .find_map(|(ty, kind)| (kind == &TypeKind::Pointer(byte)).then_some(ty))
        {
            roots.insert(pointer);
        }
        for (_, function) in program.functions().iter() {
            roots.insert(function.result());
            collect_body_types(function.body(), program.types(), &mut roots);
        }
        match program.root() {
            MirRoot::Process(root) => {
                collect_body_types(root.body(), program.types(), &mut roots);
            }
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_body_types(case.body(), program.types(), &mut roots);
                }
            }
        }
        for ty in roots {
            if !is_completion_type(program.types(), ty) {
                builder.layout(ty)?;
            }
        }
        Ok(Self {
            target,
            layouts: builder.layouts,
        })
    }

    #[must_use]
    pub const fn target(&self) -> MachineTarget {
        self.target
    }

    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<&MachineLayout> {
        self.layouts.get(&ty)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &MachineLayout)> {
        self.layouts.iter().map(|(ty, layout)| (*ty, layout))
    }
}

struct LayoutBuilder<'program> {
    program: &'program MirProgram,
    target: MachineTarget,
    layouts: BTreeMap<TypeId, MachineLayout>,
    active: BTreeSet<TypeId>,
}

impl LayoutBuilder<'_> {
    fn layout(&mut self, ty: TypeId) -> Result<&MachineLayout, MachineLayoutError> {
        if self.layouts.contains_key(&ty) {
            return Ok(&self.layouts[&ty]);
        }
        if !self.active.insert(ty) {
            return Err(MachineLayoutError::RecursiveValue(ty));
        }
        let kind = self
            .program
            .types()
            .get(ty)
            .cloned()
            .ok_or(MachineLayoutError::UnknownType(ty))?;
        let layout = self.compute(ty, &kind)?;
        self.active.remove(&ty);
        self.layouts.insert(ty, layout);
        Ok(&self.layouts[&ty])
    }

    #[allow(clippy::too_many_lines)]
    fn compute(
        &mut self,
        ty: TypeId,
        kind: &TypeKind,
    ) -> Result<MachineLayout, MachineLayoutError> {
        match kind {
            TypeKind::Builtin(builtin) => self.builtin(ty, *builtin),
            TypeKind::Pointer(_) => Ok(self.pointer()),
            TypeKind::Borrow { referent, .. } => {
                let referent = self
                    .program
                    .types()
                    .get(*referent)
                    .ok_or(MachineLayoutError::UnknownType(*referent))?;
                if matches!(
                    referent,
                    TypeKind::Builtin(BuiltinType::Str) | TypeKind::Slice(_)
                ) {
                    Ok(MachineLayout {
                        size: self.target.word_size() * 2,
                        alignment: self.target.word_size(),
                        kind: MachineLayoutKind::View {
                            pointer_offset: 0,
                            length_offset: self.target.word_size(),
                        },
                    })
                } else {
                    Ok(self.pointer())
                }
            }
            TypeKind::FixedArray { element, length } => {
                let element_layout = self.layout(*element)?.clone();
                let stride = align_up(element_layout.size, element_layout.alignment, ty)?;
                let size = stride
                    .checked_mul(*length)
                    .ok_or(MachineLayoutError::LayoutOverflow(ty))?;
                Ok(MachineLayout {
                    size,
                    alignment: element_layout.alignment,
                    kind: MachineLayoutKind::FixedArray {
                        element: *element,
                        length: *length,
                        stride,
                    },
                })
            }
            TypeKind::Nominal { .. } => self.nominal(ty),
            TypeKind::Closure { .. } => self.closure(ty),
            TypeKind::Optional(payload) => {
                let payload_layout = self.layout(*payload)?.clone();
                Self::outcome(
                    ty,
                    MachineOutcomeKind::Optional,
                    Some((payload_layout, *payload)),
                    None,
                )
            }
            TypeKind::Fallible(payload) => {
                let primary = if is_void(self.program.types(), *payload) {
                    None
                } else {
                    Some((self.layout(*payload)?.clone(), *payload))
                };
                let error = self.program.types().builtin(BuiltinType::Error);
                let alternate = self.layout(error)?.clone();
                Self::outcome(
                    ty,
                    MachineOutcomeKind::Fallible,
                    primary,
                    Some((alternate, error)),
                )
            }
            TypeKind::Opaque { definition, .. } => {
                let Some(RuntimeTypeRepresentation::Opaque {
                    definition: actual,
                    witness,
                }) = self.program.type_representations().get(ty)
                else {
                    return Err(MachineLayoutError::MissingRepresentation(ty));
                };
                if actual != definition {
                    return Err(MachineLayoutError::InvalidRepresentation(ty));
                }
                let witness = *witness;
                let layout = self.layout(witness)?.clone();
                Ok(MachineLayout {
                    size: layout.size,
                    alignment: layout.alignment,
                    kind: MachineLayoutKind::Opaque { witness },
                })
            }
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. }
            | TypeKind::Slice(_)
            | TypeKind::Callable(_) => Err(MachineLayoutError::UnsizedOrSymbolicType(ty)),
        }
    }

    fn builtin(
        &self,
        ty: TypeId,
        builtin: BuiltinType,
    ) -> Result<MachineLayout, MachineLayoutError> {
        let (size, alignment, kind) = match builtin {
            BuiltinType::Bool => (1, 1, MachineLayoutKind::Scalar(MachineScalar::Bool)),
            BuiltinType::I8 => integer(8, true),
            BuiltinType::I16 => integer(16, true),
            BuiltinType::I32 => integer(32, true),
            BuiltinType::I64 | BuiltinType::Isize => integer(64, true),
            BuiltinType::U8 => integer(8, false),
            BuiltinType::U16 => integer(16, false),
            BuiltinType::U32 => integer(32, false),
            BuiltinType::U64 | BuiltinType::Usize => integer(64, false),
            BuiltinType::Error => (
                self.target.word_size() * 4,
                self.target.word_size(),
                MachineLayoutKind::Error {
                    code_offset: 0,
                    message_offset: self.target.word_size() * 2,
                },
            ),
            BuiltinType::Str | BuiltinType::Void | BuiltinType::Never => {
                return Err(MachineLayoutError::UnsizedOrSymbolicType(ty));
            }
        };
        Ok(MachineLayout {
            size,
            alignment,
            kind,
        })
    }

    fn pointer(&self) -> MachineLayout {
        MachineLayout {
            size: self.target.pointer_size(),
            alignment: self.target.pointer_alignment(),
            kind: MachineLayoutKind::Pointer,
        }
    }

    fn nominal(&mut self, ty: TypeId) -> Result<MachineLayout, MachineLayoutError> {
        let representation = self
            .program
            .type_representations()
            .get(ty)
            .cloned()
            .ok_or(MachineLayoutError::MissingRepresentation(ty))?;
        match representation {
            RuntimeTypeRepresentation::Struct { fields } => {
                let members = fields
                    .iter()
                    .map(|field| (field.field(), field.ty()))
                    .collect::<Vec<_>>();
                let (size, alignment, offsets) =
                    self.member_offsets(ty, members.iter().map(|(_, field_type)| *field_type))?;
                let fields = members
                    .into_iter()
                    .zip(offsets)
                    .map(|((field, field_type), offset)| MachineFieldLayout {
                        field,
                        ty: field_type,
                        offset,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Ok(MachineLayout {
                    size,
                    alignment,
                    kind: MachineLayoutKind::Struct { fields },
                })
            }
            RuntimeTypeRepresentation::Enum { variants } => {
                if variants.is_empty() || variants.len() > 256 {
                    return Err(MachineLayoutError::InvalidRepresentation(ty));
                }
                let mut payload_alignment = 1;
                let mut payload_size = 0;
                let mut relative = Vec::with_capacity(variants.len());
                for variant in &variants {
                    let (size, alignment, offsets) = self
                        .member_offsets(ty, variant.payload().iter().map(|payload| payload.ty()))?;
                    payload_alignment = payload_alignment.max(alignment);
                    payload_size = payload_size.max(size);
                    relative.push(offsets);
                }
                let payload_offset = align_up(1, payload_alignment, ty)?;
                let size = align_up(
                    payload_offset
                        .checked_add(payload_size)
                        .ok_or(MachineLayoutError::LayoutOverflow(ty))?,
                    payload_alignment,
                    ty,
                )?;
                let variants = variants
                    .iter()
                    .enumerate()
                    .zip(relative)
                    .map(|((tag, variant), offsets)| {
                        let payload = variant
                            .payload()
                            .iter()
                            .zip(offsets)
                            .map(|(payload, offset)| {
                                Ok(MachinePayloadLayout {
                                    parameter: payload.parameter(),
                                    ty: payload.ty(),
                                    offset: payload_offset
                                        .checked_add(offset)
                                        .ok_or(MachineLayoutError::LayoutOverflow(ty))?,
                                })
                            })
                            .collect::<Result<Vec<_>, MachineLayoutError>>()?
                            .into_boxed_slice();
                        Ok(MachineEnumVariantLayout {
                            variant: variant.variant(),
                            tag: u8::try_from(tag)
                                .map_err(|_| MachineLayoutError::InvalidRepresentation(ty))?,
                            payload,
                        })
                    })
                    .collect::<Result<Vec<_>, MachineLayoutError>>()?
                    .into_boxed_slice();
                Ok(MachineLayout {
                    size,
                    alignment: payload_alignment,
                    kind: MachineLayoutKind::Enum {
                        tag_offset: 0,
                        payload_offset,
                        variants,
                    },
                })
            }
            RuntimeTypeRepresentation::Opaque { .. }
            | RuntimeTypeRepresentation::Closure { .. } => {
                Err(MachineLayoutError::InvalidRepresentation(ty))
            }
        }
    }

    fn closure(&mut self, ty: TypeId) -> Result<MachineLayout, MachineLayoutError> {
        let Some(RuntimeTypeRepresentation::Closure { captures }) =
            self.program.type_representations().get(ty)
        else {
            return Err(MachineLayoutError::MissingRepresentation(ty));
        };
        let captures = captures
            .iter()
            .map(|capture| (capture.capture(), capture.ty()))
            .collect::<Vec<_>>();
        let (size, alignment, offsets) =
            self.member_offsets(ty, captures.iter().map(|(_, ty)| *ty))?;
        let captures = captures
            .into_iter()
            .zip(offsets)
            .map(|((capture, capture_type), offset)| MachineCaptureLayout {
                capture,
                ty: capture_type,
                offset,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::Closure { captures },
        })
    }

    fn outcome(
        ty: TypeId,
        kind: MachineOutcomeKind,
        primary: Option<(MachineLayout, TypeId)>,
        alternate: Option<(MachineLayout, TypeId)>,
    ) -> Result<MachineLayout, MachineLayoutError> {
        let alignment = primary
            .as_ref()
            .map_or(1, |(layout, _)| layout.alignment)
            .max(alternate.as_ref().map_or(1, |(layout, _)| layout.alignment));
        let payload_size = primary
            .as_ref()
            .map_or(0, |(layout, _)| layout.size)
            .max(alternate.as_ref().map_or(0, |(layout, _)| layout.size));
        let payload_offset = align_up(1, alignment, ty)?;
        let size = align_up(
            payload_offset
                .checked_add(payload_size)
                .ok_or(MachineLayoutError::LayoutOverflow(ty))?,
            alignment,
            ty,
        )?;
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::Outcome {
                kind,
                tag_offset: 0,
                payload_offset,
                primary: primary.map(|(_, ty)| ty),
                alternate: alternate.map(|(_, ty)| ty),
            },
        })
    }

    fn member_offsets(
        &mut self,
        owner: TypeId,
        members: impl IntoIterator<Item = TypeId>,
    ) -> Result<(u64, u64, Vec<u64>), MachineLayoutError> {
        let mut size = 0_u64;
        let mut alignment = 1_u64;
        let mut offsets = Vec::new();
        for member in members {
            let layout = self.layout(member)?.clone();
            size = align_up(size, layout.alignment, owner)?;
            offsets.push(size);
            size = size
                .checked_add(layout.size)
                .ok_or(MachineLayoutError::LayoutOverflow(owner))?;
            alignment = alignment.max(layout.alignment);
        }
        size = align_up(size, alignment, owner)?;
        Ok((size, alignment, offsets))
    }
}

fn collect_body_types(body: &MirBody, store: &TypeStore, types: &mut BTreeSet<TypeId>) {
    if let Some(pack) = body.pack() {
        types.extend([pack.element(), pack.next()]);
    }
    types.extend(body.locals().iter().map(|(_, local)| local.ty()));
    types.extend(body.places().iter().filter_map(|(_, place)| {
        (!matches!(
            store.get(place.ty()),
            Some(TypeKind::Builtin(BuiltinType::Str) | TypeKind::Slice(_))
        ))
        .then_some(place.ty())
    }));
    types.extend(body.values().iter().map(|(_, value)| value.ty()));
    for (_, operation) in body.operations().iter() {
        if let MirOperationKind::Call(call) = operation.kind()
            && let MirCallTarget::StandardPrimitive { type_arguments, .. } = call.target()
        {
            types.extend(type_arguments.iter().copied());
        }
    }
}

fn is_completion_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
    )
}

fn is_void(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.get(ty), Some(TypeKind::Builtin(BuiltinType::Void)))
}

fn integer(bits: u8, signed: bool) -> (u64, u64, MachineLayoutKind) {
    let bytes = u64::from(bits / 8);
    (
        bytes,
        bytes,
        MachineLayoutKind::Scalar(MachineScalar::Integer { bits, signed }),
    )
}

fn align_up(value: u64, alignment: u64, ty: TypeId) -> Result<u64, MachineLayoutError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or(MachineLayoutError::InvalidAlignment { ty, alignment })?;
    if !alignment.is_power_of_two() {
        return Err(MachineLayoutError::InvalidAlignment { ty, alignment });
    }
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(MachineLayoutError::LayoutOverflow(ty))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineLayoutError {
    UnknownType(TypeId),
    UnsizedOrSymbolicType(TypeId),
    MissingRepresentation(TypeId),
    InvalidRepresentation(TypeId),
    RecursiveValue(TypeId),
    LayoutOverflow(TypeId),
    InvalidAlignment { ty: TypeId, alignment: u64 },
}

impl fmt::Display for MachineLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine layout construction failed: {self:?}")
    }
}

impl std::error::Error for MachineLayoutError {}
