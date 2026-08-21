use nocter_mir::{MirBody, MirPlace, MirPlaceRoot, MirProjectionKind};
use nocter_model::{
    BuiltinField, BuiltinType, FieldIdentity, MirPlaceId, TypeId, TypeKind, TypeStore,
};

use super::body::BodyIdentities;
use super::{MachineAddressError, MachineProgramError};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineIndex, MachineIndexBound,
    MachineLayoutKind, MachineLayoutStore, MachineOutcomeKind,
};

pub(super) fn lower_addresses(
    body: &MirBody,
    types: &TypeStore,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<Vec<MachineAddress>, MachineProgramError> {
    body.places()
        .iter()
        .map(|(place, value)| lower_address(place, value, body, types, layouts, ids))
        .collect()
}

fn lower_address(
    place: MirPlaceId,
    value: &MirPlace,
    body: &MirBody,
    types: &TypeStore,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<MachineAddress, MachineProgramError> {
    let (root, mut current, mut current_view) =
        lower_root(place, value.root(), body, types, layouts, ids)?;
    let context = ProjectionContext {
        place,
        types,
        layouts,
        ids,
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
    if current != value.ty() || current_view {
        return Err(address_error(
            ids,
            place,
            MachineAddressError::InvalidProjection,
        ));
    }
    let layout = layouts
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
    body: &MirBody,
    types: &TypeStore,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<(MachineAddressRoot, TypeId, bool), MachineProgramError> {
    match root {
        MirPlaceRoot::Local(local) => {
            let source = local;
            let local = body
                .locals()
                .get(source)
                .copied()
                .ok_or_else(|| address_error(ids, place, MachineAddressError::InvalidRoot))?;
            Ok((
                MachineAddressRoot::Stack(ids.stack(source)?),
                local.ty(),
                false,
            ))
        }
        MirPlaceRoot::Dereference { value, .. } => {
            let source = body
                .values()
                .get(value)
                .copied()
                .ok_or_else(|| address_error(ids, place, MachineAddressError::InvalidRoot))?;
            let Some(TypeKind::Borrow { referent, .. }) = types.get(source.ty()) else {
                return Err(address_error(ids, place, MachineAddressError::InvalidRoot));
            };
            let layout = layouts
                .get(source.ty())
                .ok_or(MachineProgramError::MissingStoredLayout(source.ty()))?;
            match layout.kind() {
                MachineLayoutKind::Pointer => Ok((
                    MachineAddressRoot::Pointer {
                        value: ids.value(value)?,
                    },
                    *referent,
                    false,
                )),
                MachineLayoutKind::View {
                    pointer_offset,
                    length_offset,
                } => Ok((
                    MachineAddressRoot::View {
                        value: ids.value(value)?,
                        pointer_offset: *pointer_offset,
                        length_offset: *length_offset,
                    },
                    *referent,
                    true,
                )),
                _ => Err(address_error(ids, place, MachineAddressError::InvalidRoot)),
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProjectionContext<'a> {
    place: MirPlaceId,
    types: &'a TypeStore,
    layouts: &'a MachineLayoutStore,
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
        | MirProjectionKind::ClosureCapture(_)
        | MirProjectionKind::VariantPayload { .. } => push_offset(
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
        (MirProjectionKind::Field(field), MachineLayoutKind::Struct { fields }) => fields
            .iter()
            .find(|member| field == FieldIdentity::Declared(member.field()))
            .map(|member| member.offset()),
        (
            MirProjectionKind::Field(FieldIdentity::Builtin(field)),
            MachineLayoutKind::Error {
                code_offset,
                message_offset,
            },
        ) => Some(match field {
            BuiltinField::ErrorCode => *code_offset,
            BuiltinField::ErrorMessage => *message_offset,
        }),
        (MirProjectionKind::ClosureCapture(capture), MachineLayoutKind::Closure { captures }) => {
            captures
                .iter()
                .find(|member| member.capture() == capture)
                .map(|member| member.offset())
        }
        (
            MirProjectionKind::VariantPayload { variant, parameter },
            MachineLayoutKind::Enum { variants, .. },
        ) => variants
            .iter()
            .find(|candidate| candidate.variant() == variant)
            .and_then(|candidate| {
                candidate
                    .payload()
                    .iter()
                    .find(|payload| payload.parameter() == parameter)
            })
            .map(|payload| payload.offset()),
        _ => None,
    };
    offset.ok_or_else(|| invalid_projection(context.ids, context.place))
}

fn lower_dereference(
    place: MirPlaceId,
    source: TypeId,
    layouts: &MachineLayoutStore,
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
        Some(TypeKind::FixedArray { .. }) => {
            let MachineLayoutKind::FixedArray { length, stride, .. } =
                layout_kind(context.layouts, source)?
            else {
                return Err(invalid_projection(context.ids, context.place));
            };
            (*stride, MachineIndexBound::Fixed(*length))
        }
        Some(TypeKind::Slice(element)) if state.current_view => {
            let layout = context
                .layouts
                .get(*element)
                .ok_or(MachineProgramError::MissingStoredLayout(*element))?;
            (layout.size(), MachineIndexBound::CurrentView)
        }
        Some(TypeKind::Builtin(BuiltinType::Str)) if state.current_view => {
            let byte = context.types.builtin(BuiltinType::U8);
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
    layouts: &MachineLayoutStore,
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
    layouts: &MachineLayoutStore,
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
