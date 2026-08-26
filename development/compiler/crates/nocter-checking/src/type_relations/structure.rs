use nocter_model::{CallableContract, TypeId, TypeKind};

use super::SubstitutionError;

/// Visits every direct child type in the structural order owned by [`TypeKind`].
pub(crate) fn visit_type_children(kind: &TypeKind, mut visit: impl FnMut(TypeId)) {
    match kind {
        TypeKind::Builtin(_) | TypeKind::GenericParameter(_) | TypeKind::InterfaceSelf(_) => {}
        TypeKind::Nominal { arguments, .. }
        | TypeKind::Opaque { arguments, .. }
        | TypeKind::Closure { arguments, .. } => arguments.iter().copied().for_each(&mut visit),
        TypeKind::AssociatedProjection { base, .. }
        | TypeKind::Pointer(base)
        | TypeKind::Borrow { referent: base, .. }
        | TypeKind::Slice(base)
        | TypeKind::FixedArray { element: base, .. }
        | TypeKind::Optional(base)
        | TypeKind::Fallible(base) => visit(*base),
        TypeKind::Callable(contract) => {
            contract.parameters().iter().copied().for_each(&mut visit);
            contract.pack().into_iter().for_each(&mut visit);
            visit(contract.result());
        }
    }
}

/// Rebuilds one type kind after mapping every direct child through one semantic authority.
pub(crate) fn map_type_children<E>(
    kind: TypeKind,
    mut map: impl FnMut(TypeId) -> Result<TypeId, E>,
) -> Result<TypeKind, E>
where
    E: From<SubstitutionError>,
{
    let mapped = |types: &[TypeId], map: &mut dyn FnMut(TypeId) -> Result<TypeId, E>| {
        types
            .iter()
            .copied()
            .map(map)
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    };
    Ok(match kind {
        TypeKind::Builtin(builtin) => TypeKind::Builtin(builtin),
        TypeKind::GenericParameter(parameter) => TypeKind::GenericParameter(parameter),
        TypeKind::InterfaceSelf(interface) => TypeKind::InterfaceSelf(interface),
        TypeKind::Closure {
            definition,
            arguments,
        } => TypeKind::Closure {
            definition,
            arguments: mapped(&arguments, &mut map)?,
        },
        TypeKind::Nominal {
            definition,
            arguments,
        } => TypeKind::Nominal {
            definition,
            arguments: mapped(&arguments, &mut map)?,
        },
        TypeKind::AssociatedProjection { base, associated } => TypeKind::AssociatedProjection {
            base: map(base)?,
            associated,
        },
        TypeKind::Opaque {
            definition,
            arguments,
        } => TypeKind::Opaque {
            definition,
            arguments: mapped(&arguments, &mut map)?,
        },
        TypeKind::Pointer(base) => TypeKind::Pointer(map(base)?),
        TypeKind::Borrow {
            capability,
            referent,
        } => TypeKind::Borrow {
            capability,
            referent: map(referent)?,
        },
        TypeKind::Slice(element) => TypeKind::Slice(map(element)?),
        TypeKind::FixedArray { element, length } => TypeKind::FixedArray {
            element: map(element)?,
            length,
        },
        TypeKind::Callable(contract) => TypeKind::Callable(
            CallableContract::new(
                contract.capability(),
                contract
                    .parameters()
                    .iter()
                    .copied()
                    .map(&mut map)
                    .collect::<Result<Vec<_>, _>>()?,
                contract.pack().map(&mut map).transpose()?,
                map(contract.result())?,
                contract.provenance().clone(),
            )
            .map_err(|_| E::from(SubstitutionError::InvalidStore))?,
        ),
        TypeKind::Optional(payload) => TypeKind::Optional(map(payload)?),
        TypeKind::Fallible(payload) => TypeKind::Fallible(map(payload)?),
    })
}
