use nocter_mir::{MirBody, MirPlace, MirPlaceRoot, MirProjectionKind, MirStatic};
use nocter_model::{Arena, ExecutableStaticId, MirPlaceId, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTable};

use super::body::BodyIdentities;
use super::{MachineAddressError, MachineProgramError};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineIndex, MachineIndexBound,
    MachineLayoutKind, MachineLayoutPlan, MachineOutcomeKind,
};

pub(super) fn lower_addresses(
    body: &MirBody,
    statics: &Arena<ExecutableStaticId, MirStatic>,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutPlan,
    data: &crate::data::MachineDataPlan,
    ids: &BodyIdentities,
) -> Result<Vec<MachineAddress>, MachineProgramError> {
    let context = AddressLoweringContext {
        body,
        statics,
        types,
        layouts,
        data,
        ids,
    };
    body.places()
        .iter()
        .map(|(place, value)| lower_address(place, value, context))
        .collect()
}

#[derive(Clone, Copy)]
struct AddressLoweringContext<'a> {
    body: &'a MirBody,
    statics: &'a Arena<ExecutableStaticId, MirStatic>,
    types: &'a RuntimeTypeTable,
    layouts: &'a MachineLayoutPlan,
    data: &'a crate::data::MachineDataPlan,
    ids: &'a BodyIdentities,
}

fn lower_address(
    place: MirPlaceId,
    value: &MirPlace,
    lowering: AddressLoweringContext<'_>,
) -> Result<MachineAddress, MachineProgramError> {
    let (root, mut current, mut current_view) = lower_root(place, value.root(), lowering)?;
    let context = ProjectionContext {
        place,
        types: lowering.types,
        layouts: lowering.layouts,
        ids: lowering.ids,
    };
    let mut state = AddressState {
        steps: Vec::with_capacity(value.projections().len()),
        current_view,
    };
    for projection in value.projections().iter().copied() {
        lower_projection(context, &mut state, current, projection.kind())?;
        current = projection.ty();
    }
    current_view = state.current_view;
    let steps = state.steps;
    if current != value.ty() {
        return Err(address_error(
            lowering.ids,
            place,
            MachineAddressError::InvalidProjection,
        ));
    }
    if current_view {
        if !matches!(
            lowering.types.get(value.ty()),
            Some(RuntimeType::Primitive(RuntimePrimitive::Text) | RuntimeType::Slice(_))
        ) {
            return Err(address_error(
                lowering.ids,
                place,
                MachineAddressError::InvalidProjection,
            ));
        }
        return Ok(MachineAddress::new_view(value.ty(), root, steps));
    }
    let layout = lowering
        .layouts
        .get(value.ty())
        .ok_or(MachineProgramError::MissingStoredLayout(value.ty()))?;
    Ok(MachineAddress::new(
        value.ty(),
        layout.size(),
        layout.alignment(),
        root,
        steps,
    ))
}

fn lower_root(
    place: MirPlaceId,
    root: MirPlaceRoot,
    context: AddressLoweringContext<'_>,
) -> Result<(MachineAddressRoot, TypeId, bool), MachineProgramError> {
    match root {
        MirPlaceRoot::Local(local) => {
            let source = local;
            let local = context.body.locals().get(source).copied().ok_or_else(|| {
                address_error(context.ids, place, MachineAddressError::InvalidRoot)
            })?;
            Ok((
                MachineAddressRoot::Stack(context.ids.stack(source)?),
                local.ty(),
                false,
            ))
        }
        MirPlaceRoot::Static(id) => {
            let definition = context.data.static_value(id).ok_or_else(|| {
                address_error(context.ids, place, MachineAddressError::InvalidRoot)
            })?;
            let ty = context.statics.get(id).map(MirStatic::ty).ok_or_else(|| {
                address_error(context.ids, place, MachineAddressError::InvalidRoot)
            })?;
            Ok((MachineAddressRoot::Data(definition), ty, false))
        }
        MirPlaceRoot::Dereference { value, .. } => {
            let source = context.body.values().get(value).copied().ok_or_else(|| {
                address_error(context.ids, place, MachineAddressError::InvalidRoot)
            })?;
            let Some(RuntimeType::Borrow { referent, .. }) = context.types.get(source.ty()) else {
                return Err(address_error(
                    context.ids,
                    place,
                    MachineAddressError::InvalidRoot,
                ));
            };
            let layout = context
                .layouts
                .get(source.ty())
                .ok_or(MachineProgramError::MissingStoredLayout(source.ty()))?;
            match layout.kind() {
                MachineLayoutKind::Pointer => Ok((
                    MachineAddressRoot::Pointer {
                        value: context.ids.value(value)?,
                    },
                    *referent,
                    false,
                )),
                MachineLayoutKind::View {
                    pointer_offset,
                    length_offset,
                } => Ok((
                    MachineAddressRoot::View {
                        value: context.ids.value(value)?,
                        pointer_offset: *pointer_offset,
                        length_offset: *length_offset,
                    },
                    *referent,
                    true,
                )),
                _ => Err(address_error(
                    context.ids,
                    place,
                    MachineAddressError::InvalidRoot,
                )),
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProjectionContext<'a> {
    place: MirPlaceId,
    types: &'a RuntimeTypeTable,
    layouts: &'a MachineLayoutPlan,
    ids: &'a BodyIdentities,
}

struct AddressState {
    steps: Vec<MachineAddressStep>,
    current_view: bool,
}

fn lower_projection(
    context: ProjectionContext<'_>,
    state: &mut AddressState,
    source: TypeId,
    projection: MirProjectionKind,
) -> Result<(), MachineProgramError> {
    if state.current_view
        && !matches!(
            projection,
            MirProjectionKind::FixedIndex(_) | MirProjectionKind::DynamicIndex(_)
        )
    {
        return Err(address_error(
            context.ids,
            context.place,
            MachineAddressError::InvalidProjection,
        ));
    }
    match projection {
        MirProjectionKind::Field(_)
        | MirProjectionKind::TupleElement(_)
        | MirProjectionKind::ClosureCapture(_)
        | MirProjectionKind::VariantPayload { .. }
        | MirProjectionKind::PackEntryKey
        | MirProjectionKind::PackEntryValue => push_offset(
            context.place,
            context.ids,
            &mut state.steps,
            static_projection_offset(context, source, projection)?,
        )?,
        MirProjectionKind::BorrowDereference(_) => {
            lower_dereference(
                context.place,
                source,
                context.layouts,
                context.ids,
                &mut state.steps,
                &mut state.current_view,
            )?;
        }
        MirProjectionKind::FixedIndex(index) => {
            lower_index(context, state, source, MachineIndex::Constant(index))?;
        }
        MirProjectionKind::DynamicIndex(index) => {
            lower_index(
                context,
                state,
                source,
                MachineIndex::Value(context.ids.value(index)?),
            )?;
        }
        MirProjectionKind::OptionalPayload => {
            push_outcome_offset(
                context.place,
                source,
                MachineOutcomeKind::Optional,
                context.layouts,
                context.ids,
                &mut state.steps,
            )?;
        }
        MirProjectionKind::FallibleSuccess | MirProjectionKind::FallibleFailure => {
            push_outcome_offset(
                context.place,
                source,
                MachineOutcomeKind::Fallible,
                context.layouts,
                context.ids,
                &mut state.steps,
            )?;
        }
        MirProjectionKind::OpaqueWitness(_) => {
            if !matches!(
                layout_kind(context.layouts, source)?,
                MachineLayoutKind::Opaque { .. }
            ) {
                return Err(invalid_projection(context.ids, context.place));
            }
        }
    }
    Ok(())
}

fn static_projection_offset(
    context: ProjectionContext<'_>,
    source: TypeId,
    projection: MirProjectionKind,
) -> Result<u64, MachineProgramError> {
    let offset = match (projection, layout_kind(context.layouts, source)?) {
        (MirProjectionKind::Field(field), MachineLayoutKind::Struct { .. }) => context
            .layouts
            .field(field)
            .map(crate::MachineFieldLayout::offset),
        (MirProjectionKind::TupleElement(index), MachineLayoutKind::Tuple { elements }) => {
            elements.get(index).map(|element| element.offset())
        }
        (MirProjectionKind::ClosureCapture(capture), MachineLayoutKind::Closure { .. }) => context
            .layouts
            .capture(capture)
            .map(crate::MachineCaptureLayout::offset),
        (
            MirProjectionKind::VariantPayload { variant, parameter },
            MachineLayoutKind::Enum { .. },
        ) => context
            .layouts
            .variant(variant)
            .and_then(|_| context.layouts.payload(variant, parameter))
            .map(crate::MachinePayloadLayout::offset),
        (MirProjectionKind::PackEntryKey, MachineLayoutKind::PackEntry { key, .. }) => {
            Some(key.offset())
        }
        (MirProjectionKind::PackEntryValue, MachineLayoutKind::PackEntry { value, .. }) => {
            Some(value.offset())
        }
        _ => None,
    };
    offset.ok_or_else(|| invalid_projection(context.ids, context.place))
}

fn lower_dereference(
    place: MirPlaceId,
    source: TypeId,
    layouts: &MachineLayoutPlan,
    ids: &BodyIdentities,
    steps: &mut Vec<MachineAddressStep>,
    current_view: &mut bool,
) -> Result<(), MachineProgramError> {
    match layout_kind(layouts, source)? {
        MachineLayoutKind::Pointer => {
            steps.push(MachineAddressStep::Dereference);
            *current_view = false;
        }
        MachineLayoutKind::View {
            pointer_offset,
            length_offset,
        } => {
            steps.push(MachineAddressStep::ViewDereference {
                pointer_offset: *pointer_offset,
                length_offset: *length_offset,
            });
            *current_view = true;
        }
        _ => return Err(invalid_projection(ids, place)),
    }
    Ok(())
}

fn lower_index(
    context: ProjectionContext<'_>,
    state: &mut AddressState,
    source: TypeId,
    index: MachineIndex,
) -> Result<(), MachineProgramError> {
    let (stride, bound) = match context.types.get(source) {
        Some(RuntimeType::FixedArray { .. }) => {
            let MachineLayoutKind::FixedArray { length, stride, .. } =
                layout_kind(context.layouts, source)?
            else {
                return Err(invalid_projection(context.ids, context.place));
            };
            (*stride, MachineIndexBound::Fixed(*length))
        }
        Some(RuntimeType::Slice(element)) if state.current_view => {
            let layout = context
                .layouts
                .get(*element)
                .ok_or(MachineProgramError::MissingStoredLayout(*element))?;
            (layout.size(), MachineIndexBound::CurrentView)
        }
        Some(RuntimeType::Primitive(RuntimePrimitive::Text)) if state.current_view => {
            let byte = context
                .types
                .primitive(RuntimePrimitive::Unsigned(8))
                .ok_or(MachineProgramError::MissingRuntimePrimitive(
                    RuntimePrimitive::Unsigned(8),
                ))?;
            let layout = context
                .layouts
                .get(byte)
                .ok_or(MachineProgramError::MissingStoredLayout(byte))?;
            (layout.size(), MachineIndexBound::CurrentView)
        }
        _ => return Err(invalid_projection(context.ids, context.place)),
    };
    state.steps.push(MachineAddressStep::Index {
        index,
        stride,
        bound,
    });
    state.current_view = false;
    Ok(())
}

fn push_outcome_offset(
    place: MirPlaceId,
    source: TypeId,
    expected: MachineOutcomeKind,
    layouts: &MachineLayoutPlan,
    ids: &BodyIdentities,
    steps: &mut Vec<MachineAddressStep>,
) -> Result<(), MachineProgramError> {
    let MachineLayoutKind::Outcome {
        kind,
        payload_offset,
        ..
    } = layout_kind(layouts, source)?
    else {
        return Err(invalid_projection(ids, place));
    };
    if *kind != expected {
        return Err(invalid_projection(ids, place));
    }
    push_offset(place, ids, steps, *payload_offset)
}

fn layout_kind(
    layouts: &MachineLayoutPlan,
    ty: TypeId,
) -> Result<&MachineLayoutKind, MachineProgramError> {
    layouts
        .get(ty)
        .map(crate::MachineLayout::kind)
        .ok_or(MachineProgramError::MissingStoredLayout(ty))
}

fn push_offset(
    place: MirPlaceId,
    ids: &BodyIdentities,
    steps: &mut Vec<MachineAddressStep>,
    offset: u64,
) -> Result<(), MachineProgramError> {
    if offset == 0 {
        return Ok(());
    }
    if let Some(MachineAddressStep::Offset(previous)) = steps.last_mut() {
        *previous = previous
            .checked_add(offset)
            .ok_or_else(|| address_error(ids, place, MachineAddressError::OffsetOverflow))?;
    } else {
        steps.push(MachineAddressStep::Offset(offset));
    }
    Ok(())
}

fn invalid_projection(ids: &BodyIdentities, place: MirPlaceId) -> MachineProgramError {
    address_error(ids, place, MachineAddressError::InvalidProjection)
}

const fn address_error(
    ids: &BodyIdentities,
    place: MirPlaceId,
    error: MachineAddressError,
) -> MachineProgramError {
    MachineProgramError::Address {
        owner: ids.owner(),
        place,
        error,
    }
}
