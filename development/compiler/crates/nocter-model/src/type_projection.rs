use std::collections::HashMap;
use std::fmt;

use crate::{
    InvalidParameterOrigin, TypeAuthority, TypeId, TypeKind, TypeStore, TypeTransaction,
    UnknownTypeId,
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
    if let TypeKind::Builtin(builtin) = &source_kind {
        let target_id = target.builtin(*builtin);
        projected.insert(source_id, target_id);
        return Ok(target_id);
    }
    let target_kind =
        source_kind.try_map_references(|ty| project_type(source, target, projected, ty))?;
    let target_id = target.intern(target_kind)?;
    projected.insert(source_id, target_id);
    Ok(target_id)
}

#[cfg(test)]
mod tests {
    use crate::id::SemanticId;
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

    #[test]
    fn projection_remaps_type_arguments_and_preserves_constant_arguments() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let element = types.builtin(BuiltinType::U8);
        let root = types
            .intern(TypeKind::Nominal {
                definition: crate::NominalTypeId::new(0),
                arguments: crate::GenericApplication::new([
                    crate::GenericValue::Type(element),
                    crate::GenericValue::Usize(32.into()),
                ]),
            })
            .unwrap();

        let projection = types.project(root).unwrap();
        let Some(TypeKind::Nominal { arguments, .. }) = projection.types().get(projection.root())
        else {
            panic!("projected root must remain nominal")
        };
        assert_eq!(
            arguments.as_slice(),
            [
                crate::GenericValue::Type(projection.types().builtin(BuiltinType::U8)),
                crate::GenericValue::Usize(32.into()),
            ]
        );
    }
}
