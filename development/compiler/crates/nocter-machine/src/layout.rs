use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_mir::{MirBody, MirCallTarget, MirOperationKind, MirProgram, MirRoot};
use nocter_model::{CaptureId, FieldId, ParameterId, TypeId, VariantId};
use nocter_runtime_contract::{
    RuntimePrimitive, RuntimeType, RuntimeTypeRepresentation, RuntimeTypeTable,
};

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
    ty: TypeId,
    offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineTupleElementLayout {
    ty: TypeId,
    offset: u64,
}

impl MachineTupleElementLayout {
    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }
}

impl MachineFieldLayout {
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
    ty: TypeId,
    offset: u64,
}

impl MachinePayloadLayout {
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
    tag: u8,
    payload: Box<[MachinePayloadLayout]>,
}

impl MachineEnumVariantLayout {
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
    ty: TypeId,
    offset: u64,
}

impl MachineCaptureLayout {
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
    ErrorHandle,
    Struct {
        fields: Box<[MachineFieldLayout]>,
    },
    Tuple {
        elements: Box<[MachineTupleElementLayout]>,
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
    PackEntry {
        key: MachineFieldLayout,
        value: MachineFieldLayout,
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

/// Construction-time correspondence between validated semantic members and frozen layouts.
///
/// Semantic member identities are intentionally discarded by [`Self::finish`]. Downstream target
/// materializers receive only the completed physical layout store.
#[derive(Debug)]
pub(crate) struct MachineLayoutPlan {
    store: MachineLayoutStore,
    fields: BTreeMap<FieldId, MachineFieldLayout>,
    variants: BTreeMap<VariantId, MachineEnumVariantLayout>,
    payloads: BTreeMap<ParameterId, (VariantId, MachinePayloadLayout)>,
    captures: BTreeMap<CaptureId, MachineCaptureLayout>,
}

impl MachineLayoutStore {
    /// Computes the complete recursive stored-layout closure of one MIR program.
    ///
    /// # Errors
    ///
    /// Rejects symbolic, unsized, recursive-by-value, incomplete, or overflowing representation
    /// input. Such a failure is a compiler-integrity error, never a source diagnostic.
    pub fn build(program: &MirProgram) -> Result<Self, MachineLayoutError> {
        MachineLayoutPlan::build(program).map(MachineLayoutPlan::finish)
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

impl MachineLayoutPlan {
    pub(crate) fn build(program: &MirProgram) -> Result<Self, MachineLayoutError> {
        let target = MachineTarget::select(program.runtime_abi());
        let mut builder = LayoutBuilder {
            program,
            target,
            layouts: BTreeMap::new(),
            active: BTreeSet::new(),
            fields: BTreeMap::new(),
            variants: BTreeMap::new(),
            payloads: BTreeMap::new(),
            captures: BTreeMap::new(),
        };
        let mut roots = BTreeSet::new();
        // Machine-generated CFGs use these canonical control and offset carriers independently of
        // whether source MIR happens to mention them.
        roots.extend([
            primitive(program.types(), RuntimePrimitive::Bool)?,
            primitive(program.types(), RuntimePrimitive::Usize)?,
        ]);
        let byte = primitive(program.types(), RuntimePrimitive::Unsigned(8))?;
        if let Some(pointer) = program
            .types()
            .iter()
            .find_map(|(ty, kind)| (kind == &RuntimeType::Pointer(byte)).then_some(ty))
        {
            roots.insert(pointer);
        }
        for (_, function) in program.functions().iter() {
            roots.insert(function.result());
            collect_body_types(function.body(), program.types(), &mut roots);
        }
        for (_, value) in program.statics().iter() {
            roots.insert(value.ty());
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
            store: MachineLayoutStore {
                target,
                layouts: builder.layouts,
            },
            fields: builder.fields,
            variants: builder.variants,
            payloads: builder.payloads,
            captures: builder.captures,
        })
    }

    #[must_use]
    pub(crate) fn get(&self, ty: TypeId) -> Option<&MachineLayout> {
        self.store.get(ty)
    }

    #[must_use]
    pub(crate) fn field(&self, field: FieldId) -> Option<MachineFieldLayout> {
        self.fields.get(&field).copied()
    }

    #[must_use]
    pub(crate) fn variant(&self, variant: VariantId) -> Option<&MachineEnumVariantLayout> {
        self.variants.get(&variant)
    }

    #[must_use]
    pub(crate) fn payload(
        &self,
        variant: VariantId,
        parameter: ParameterId,
    ) -> Option<MachinePayloadLayout> {
        self.payloads
            .get(&parameter)
            .and_then(|(owner, layout)| (*owner == variant).then_some(*layout))
    }

    #[must_use]
    pub(crate) fn capture(&self, capture: CaptureId) -> Option<MachineCaptureLayout> {
        self.captures.get(&capture).copied()
    }

    pub(crate) fn finish(self) -> MachineLayoutStore {
        self.store
    }
}

impl std::ops::Deref for MachineLayoutPlan {
    type Target = MachineLayoutStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

struct LayoutBuilder<'program> {
    program: &'program MirProgram,
    target: MachineTarget,
    layouts: BTreeMap<TypeId, MachineLayout>,
    active: BTreeSet<TypeId>,
    fields: BTreeMap<FieldId, MachineFieldLayout>,
    variants: BTreeMap<VariantId, MachineEnumVariantLayout>,
    payloads: BTreeMap<ParameterId, (VariantId, MachinePayloadLayout)>,
    captures: BTreeMap<CaptureId, MachineCaptureLayout>,
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
        kind: &RuntimeType,
    ) -> Result<MachineLayout, MachineLayoutError> {
        match kind {
            RuntimeType::Primitive(primitive) => self.primitive(ty, *primitive),
            RuntimeType::Pointer(_) => Ok(self.pointer()),
            RuntimeType::Borrow { referent, .. } => {
                let referent = self
                    .program
                    .types()
                    .get(*referent)
                    .ok_or(MachineLayoutError::UnknownType(*referent))?;
                if matches!(
                    referent,
                    RuntimeType::Primitive(RuntimePrimitive::Text) | RuntimeType::Slice(_)
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
            RuntimeType::FixedArray { element, length } => {
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
            RuntimeType::Tuple(elements) => self.tuple(ty, elements),
            RuntimeType::PackEntry { key, value } => {
                let key_layout = self.layout(*key)?.clone();
                let value_layout = self.layout(*value)?.clone();
                let value_offset = align_up(key_layout.size, value_layout.alignment, ty)?;
                let alignment = key_layout.alignment.max(value_layout.alignment);
                let unaligned_size = value_offset
                    .checked_add(value_layout.size)
                    .ok_or(MachineLayoutError::LayoutOverflow(ty))?;
                Ok(MachineLayout {
                    size: align_up(unaligned_size, alignment, ty)?,
                    alignment,
                    kind: MachineLayoutKind::PackEntry {
                        key: MachineFieldLayout {
                            ty: *key,
                            offset: 0,
                        },
                        value: MachineFieldLayout {
                            ty: *value,
                            offset: value_offset,
                        },
                    },
                })
            }
            RuntimeType::Aggregate => self.nominal(ty),
            RuntimeType::Closure => self.closure(ty),
            RuntimeType::Optional(payload) => {
                let payload_layout = self.layout(*payload)?.clone();
                Self::outcome(
                    ty,
                    MachineOutcomeKind::Optional,
                    Some((payload_layout, *payload)),
                    None,
                )
            }
            RuntimeType::Fallible(payload) => {
                let primary = if is_void(self.program.types(), *payload) {
                    None
                } else {
                    Some((self.layout(*payload)?.clone(), *payload))
                };
                let error = primitive(self.program.types(), RuntimePrimitive::Error)?;
                let alternate = self.layout(error)?.clone();
                Self::outcome(
                    ty,
                    MachineOutcomeKind::Fallible,
                    primary,
                    Some((alternate, error)),
                )
            }
            RuntimeType::Opaque => {
                let Some(RuntimeTypeRepresentation::Opaque { witness, .. }) =
                    self.program.type_representations().get(ty)
                else {
                    return Err(MachineLayoutError::MissingRepresentation(ty));
                };
                let witness = *witness;
                let layout = self.layout(witness)?.clone();
                Ok(MachineLayout {
                    size: layout.size,
                    alignment: layout.alignment,
                    kind: MachineLayoutKind::Opaque { witness },
                })
            }
            RuntimeType::Slice(_) | RuntimeType::Callable => {
                Err(MachineLayoutError::UnsizedOrSymbolicType(ty))
            }
        }
    }

    fn primitive(
        &self,
        ty: TypeId,
        primitive: RuntimePrimitive,
    ) -> Result<MachineLayout, MachineLayoutError> {
        let (size, alignment, kind) = match primitive {
            RuntimePrimitive::Bool => (1, 1, MachineLayoutKind::Scalar(MachineScalar::Bool)),
            RuntimePrimitive::Char => integer(32, false)?,
            RuntimePrimitive::Signed(bits) => integer(bits, true)?,
            RuntimePrimitive::Unsigned(bits) => integer(bits, false)?,
            RuntimePrimitive::Isize => integer(64, true)?,
            RuntimePrimitive::Usize => integer(64, false)?,
            RuntimePrimitive::Error => (
                self.target.error().size(),
                self.target.error().alignment(),
                MachineLayoutKind::ErrorHandle,
            ),
            RuntimePrimitive::Text | RuntimePrimitive::Void | RuntimePrimitive::Never => {
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
            RuntimeTypeRepresentation::Struct { fields } => self.structure(ty, &fields),
            RuntimeTypeRepresentation::Enum { variants } => self.enumeration(ty, &variants),
            RuntimeTypeRepresentation::Opaque { .. }
            | RuntimeTypeRepresentation::Closure { .. } => {
                Err(MachineLayoutError::InvalidRepresentation(ty))
            }
        }
    }

    fn structure(
        &mut self,
        ty: TypeId,
        fields: &[nocter_runtime_contract::RuntimeFieldRepresentation],
    ) -> Result<MachineLayout, MachineLayoutError> {
        let members = fields
            .iter()
            .map(|field| (field.field(), field.ty()))
            .collect::<Vec<_>>();
        let (size, alignment, offsets) =
            self.member_offsets(ty, members.iter().map(|(_, field_type)| *field_type))?;
        let fields = members
            .into_iter()
            .zip(offsets)
            .map(|((field, field_type), offset)| {
                let layout = MachineFieldLayout {
                    ty: field_type,
                    offset,
                };
                self.fields.insert(field, layout);
                layout
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::Struct { fields },
        })
    }

    fn tuple(
        &mut self,
        ty: TypeId,
        elements: &[TypeId],
    ) -> Result<MachineLayout, MachineLayoutError> {
        let (size, alignment, offsets) = self.member_offsets(ty, elements.iter().copied())?;
        let elements = elements
            .iter()
            .copied()
            .zip(offsets)
            .map(|(ty, offset)| MachineTupleElementLayout { ty, offset })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::Tuple { elements },
        })
    }

    fn enumeration(
        &mut self,
        ty: TypeId,
        variants: &[nocter_runtime_contract::RuntimeVariantRepresentation],
    ) -> Result<MachineLayout, MachineLayoutError> {
        if variants.is_empty() || variants.len() > 256 {
            return Err(MachineLayoutError::InvalidRepresentation(ty));
        }
        let mut payload_alignment = 1;
        let mut payload_size = 0;
        let mut relative = Vec::with_capacity(variants.len());
        for variant in variants {
            let (size, alignment, offsets) =
                self.member_offsets(ty, variant.payload().iter().map(|payload| payload.ty()))?;
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
                        let layout = MachinePayloadLayout {
                            ty: payload.ty(),
                            offset: payload_offset
                                .checked_add(offset)
                                .ok_or(MachineLayoutError::LayoutOverflow(ty))?,
                        };
                        self.payloads
                            .insert(payload.parameter(), (variant.variant(), layout));
                        Ok(layout)
                    })
                    .collect::<Result<Vec<_>, MachineLayoutError>>()?
                    .into_boxed_slice();
                let layout = MachineEnumVariantLayout {
                    tag: u8::try_from(tag)
                        .map_err(|_| MachineLayoutError::InvalidRepresentation(ty))?,
                    payload,
                };
                self.variants.insert(variant.variant(), layout.clone());
                Ok(layout)
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
            .map(|((capture, capture_type), offset)| {
                let layout = MachineCaptureLayout {
                    ty: capture_type,
                    offset,
                };
                self.captures.insert(capture, layout);
                layout
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

fn collect_body_types(body: &MirBody, store: &RuntimeTypeTable, types: &mut BTreeSet<TypeId>) {
    if let Some(pack) = body.pack() {
        types.extend([pack.element(), pack.next()]);
    }
    types.extend(body.locals().iter().map(|(_, local)| local.ty()));
    types.extend(body.places().iter().filter_map(|(_, place)| {
        (!matches!(
            store.get(place.ty()),
            Some(RuntimeType::Primitive(RuntimePrimitive::Text) | RuntimeType::Slice(_))
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

fn is_completion_type(types: &RuntimeTypeTable, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(RuntimeType::Primitive(
            RuntimePrimitive::Void | RuntimePrimitive::Never
        ))
    )
}

fn is_void(types: &RuntimeTypeTable, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(RuntimeType::Primitive(RuntimePrimitive::Void))
    )
}

fn integer(bits: u16, signed: bool) -> Result<(u64, u64, MachineLayoutKind), MachineLayoutError> {
    let bits = u8::try_from(bits).map_err(|_| MachineLayoutError::InvalidIntegerWidth(bits))?;
    let bytes = u64::from(bits / 8);
    Ok((
        bytes,
        bytes,
        MachineLayoutKind::Scalar(MachineScalar::Integer { bits, signed }),
    ))
}

fn primitive(
    types: &RuntimeTypeTable,
    primitive: RuntimePrimitive,
) -> Result<TypeId, MachineLayoutError> {
    types
        .primitive(primitive)
        .ok_or(MachineLayoutError::MissingPrimitive(primitive))
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
    MissingPrimitive(RuntimePrimitive),
    InvalidIntegerWidth(u16),
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
