use std::collections::HashMap;
use std::fmt;

use crate::{
    CallableContract, InvalidParameterOrigin, TupleElements, TypeAuthority, TypeId, TypeKind,
    TypeStore, TypeTransaction, UnknownTypeId,
};

/// Self-contained structural type authority projected from one root type.
///
/// The projection owns only the root and its transitive structural dependencies. It can cross a
/// recovery boundary without exposing the compiler store from which it was produced.
#[derive(Clone, Debug)]
pub struct TypeProjection {
    types: TypeStore,
    root: TypeId,
}

impl TypeProjection {
    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn root(&self) -> TypeId {
        self.root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeProjectionError {
    UnknownType(UnknownTypeId),
    InvalidCallable(InvalidParameterOrigin),
}

impl fmt::Display for TypeProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(error) => error.fmt(formatter),
            Self::InvalidCallable(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypeProjectionError {}

impl From<UnknownTypeId> for TypeProjectionError {
    fn from(error: UnknownTypeId) -> Self {
        Self::UnknownType(error)
    }
}

impl From<InvalidParameterOrigin> for TypeProjectionError {
    fn from(error: InvalidParameterOrigin) -> Self {
        Self::InvalidCallable(error)
    }
}

impl TypeStore {
    /// Projects one type and all of its structural dependencies into an independent store.
    ///
    /// # Errors
    ///
    /// Returns [`TypeProjectionError`] when `root` or one of its referenced type IDs is absent, or
    /// when a callable contract in the source store is internally invalid.
    pub fn project(&self, root: TypeId) -> Result<TypeProjection, TypeProjectionError> {
        let authority = TypeAuthority::new();
        let mut types = authority.transaction();
        let root = project_type(self, &mut types, &mut HashMap::new(), root)?;
        Ok(TypeProjection {
            types: types.freeze().into_store(),
            root,
        })
    }
}

fn project_type(
    source: &TypeStore,
    target: &mut TypeTransaction,
    projected: &mut HashMap<TypeId, TypeId>,
    source_id: TypeId,
) -> Result<TypeId, TypeProjectionError> {
    if let Some(target_id) = projected.get(&source_id) {
        return Ok(*target_id);
    }
    let source_kind = source
        .get(source_id)
        .ok_or_else(|| UnknownTypeId::new(source_id))?
        .clone();
    let target_kind = match source_kind {
        TypeKind::Builtin(builtin) => {
            let target_id = target.builtin(builtin);
            projected.insert(source_id, target_id);
            return Ok(target_id);
        }
        TypeKind::GenericParameter(parameter) => TypeKind::GenericParameter(parameter),
        TypeKind::InterfaceSelf(interface) => TypeKind::InterfaceSelf(interface),
        TypeKind::Nominal {
            definition,
            arguments,
        } => TypeKind::Nominal {
            definition,
            arguments: project_types(source, target, projected, &arguments)?,
        },
        TypeKind::AssociatedProjection { base, associated } => TypeKind::AssociatedProjection {
            base: project_type(source, target, projected, base)?,
            associated,
        },
        TypeKind::Opaque {
            definition,
            arguments,
        } => TypeKind::Opaque {
            definition,
            arguments: project_types(source, target, projected, &arguments)?,
        },
        TypeKind::Pointer(pointee) => {
            TypeKind::Pointer(project_type(source, target, projected, pointee)?)
        }
        TypeKind::Borrow {
            capability,
            referent,
        } => TypeKind::Borrow {
            capability,
            referent: project_type(source, target, projected, referent)?,
        },
        TypeKind::Slice(element) => {
            TypeKind::Slice(project_type(source, target, projected, element)?)
        }
        TypeKind::FixedArray { element, length } => TypeKind::FixedArray {
            element: project_type(source, target, projected, element)?,
            length,
        },
        TypeKind::Tuple(elements) => {
            let projected_elements = project_types(source, target, projected, elements.as_slice())?;
            let [first, second, remaining @ ..] = projected_elements.as_ref() else {
                unreachable!("tuple semantic identity always contains at least two elements")
            };
            TypeKind::Tuple(TupleElements::new(
                *first,
                *second,
                remaining.iter().copied(),
            ))
        }
        TypeKind::PackEntry { key, value } => TypeKind::PackEntry {
            key: project_type(source, target, projected, key)?,
            value: project_type(source, target, projected, value)?,
        },
        TypeKind::Closure {
            definition,
            arguments,
        } => TypeKind::Closure {
            definition,
            arguments: project_types(source, target, projected, &arguments)?,
        },
        TypeKind::Callable(contract) => {
            let parameters = project_types(source, target, projected, contract.parameters())?;
            let pack = contract
                .pack()
                .map(|pack| pack.try_map(|ty| project_type(source, target, projected, ty)))
                .transpose()?;
            let result = project_type(source, target, projected, contract.result())?;
            TypeKind::Callable(CallableContract::new(
                contract.capability(),
                contract.guarantees(),
                parameters,
                pack,
                result,
                contract.provenance().clone(),
            )?)
        }
        TypeKind::Optional(payload) => {
            TypeKind::Optional(project_type(source, target, projected, payload)?)
        }
        TypeKind::Fallible(payload) => {
            TypeKind::Fallible(project_type(source, target, projected, payload)?)
        }
    };
    let target_id = target.intern(target_kind)?;
    projected.insert(source_id, target_id);
    Ok(target_id)
}

fn project_types(
    source: &TypeStore,
    target: &mut TypeTransaction,
    projected: &mut HashMap<TypeId, TypeId>,
    types: &[TypeId],
) -> Result<Box<[TypeId]>, TypeProjectionError> {
    types
        .iter()
        .copied()
        .map(|ty| project_type(source, target, projected, ty))
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

#[cfg(test)]
mod tests {
    use crate::{BuiltinType, TypeAuthority, TypeKind};

    #[test]
    fn projection_owns_only_transitive_root_dependencies() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let value = types.builtin(BuiltinType::I32);
        let optional = types.intern(TypeKind::Optional(value)).unwrap();
        let fallible = types.intern(TypeKind::Fallible(optional)).unwrap();
        let _unrelated = types.intern(TypeKind::Pointer(value)).unwrap();

        let projection = types.project(fallible).unwrap();
        assert_eq!(projection.types().type_count(), BuiltinType::ALL.len() + 2);
        let Some(TypeKind::Fallible(optional)) = projection.types().get(projection.root()) else {
            panic!("projected root must remain fallible")
        };
        assert!(matches!(
            projection.types().get(*optional),
            Some(TypeKind::Optional(payload))
                if *payload == projection.types().builtin(BuiltinType::I32)
        ));
    }
}
