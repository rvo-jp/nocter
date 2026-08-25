use std::collections::BTreeSet;

use nocter_model::TypeId;

use crate::{
    RuntimeAbiIdentity, RuntimeType, RuntimeTypeRepresentation, RuntimeTypeRepresentationTable,
    RuntimeTypeTable,
};

/// The complete source-independent environment required after MIR lowering.
///
/// Target construction freezes these facts once. MIR retains this value as its backend boundary;
/// machine lowering cannot recover semantic or source authority through it.
#[derive(Clone, Debug)]
pub struct RuntimeEnvironment {
    types: RuntimeTypeTable,
    type_representations: RuntimeTypeRepresentationTable,
    abi: RuntimeAbiIdentity,
}

impl RuntimeEnvironment {
    /// Freezes a mutually consistent runtime type, representation, and ABI boundary.
    ///
    /// # Errors
    ///
    /// Rejects missing, unexpected, mismatched, or internally inconsistent representations.
    pub fn new(
        types: RuntimeTypeTable,
        type_representations: RuntimeTypeRepresentationTable,
        abi: RuntimeAbiIdentity,
    ) -> Result<Self, RuntimeEnvironmentError> {
        validate_representations(&types, &type_representations)?;
        Ok(Self {
            types,
            type_representations,
            abi,
        })
    }

    #[must_use]
    pub const fn types(&self) -> &RuntimeTypeTable {
        &self.types
    }

    #[must_use]
    pub const fn type_representations(&self) -> &RuntimeTypeRepresentationTable {
        &self.type_representations
    }

    #[must_use]
    pub const fn abi(&self) -> RuntimeAbiIdentity {
        self.abi
    }
}

fn validate_representations(
    types: &RuntimeTypeTable,
    representations: &RuntimeTypeRepresentationTable,
) -> Result<(), RuntimeEnvironmentError> {
    for (ty, kind) in types.iter() {
        let representation = representations.get(ty);
        let required = matches!(
            kind,
            RuntimeType::Aggregate | RuntimeType::Closure | RuntimeType::Opaque
        );
        if required && representation.is_none() {
            return Err(RuntimeEnvironmentError::MissingRepresentation(ty));
        }
        if !required && representation.is_some() {
            return Err(RuntimeEnvironmentError::UnexpectedRepresentation(ty));
        }
    }
    for (owner, representation) in representations.iter() {
        let kind = types
            .get(owner)
            .ok_or(RuntimeEnvironmentError::UnknownRepresentationOwner(owner))?;
        let kind_matches = matches!(
            (kind, representation),
            (
                RuntimeType::Aggregate,
                RuntimeTypeRepresentation::Struct { .. } | RuntimeTypeRepresentation::Enum { .. }
            ) | (
                RuntimeType::Closure,
                RuntimeTypeRepresentation::Closure { .. }
            ) | (
                RuntimeType::Opaque,
                RuntimeTypeRepresentation::Opaque { .. }
            )
        );
        if !kind_matches {
            return Err(RuntimeEnvironmentError::RepresentationKindMismatch(owner));
        }
        validate_representation(types, owner, representation)?;
    }
    Ok(())
}

fn validate_representation(
    types: &RuntimeTypeTable,
    owner: TypeId,
    representation: &RuntimeTypeRepresentation,
) -> Result<(), RuntimeEnvironmentError> {
    match representation {
        RuntimeTypeRepresentation::Struct { fields } => {
            let mut identities = BTreeSet::new();
            for field in fields {
                if !identities.insert(field.field()) {
                    return Err(RuntimeEnvironmentError::DuplicateRepresentationMember(
                        owner,
                    ));
                }
                require_member_type(types, owner, field.ty())?;
            }
        }
        RuntimeTypeRepresentation::Enum { variants } => {
            let mut identities = BTreeSet::new();
            for variant in variants {
                if !identities.insert(variant.variant()) {
                    return Err(RuntimeEnvironmentError::DuplicateRepresentationMember(
                        owner,
                    ));
                }
                let mut payload_identities = BTreeSet::new();
                for payload in variant.payload() {
                    if !payload_identities.insert(payload.parameter()) {
                        return Err(RuntimeEnvironmentError::DuplicateRepresentationMember(
                            owner,
                        ));
                    }
                    require_member_type(types, owner, payload.ty())?;
                }
            }
        }
        RuntimeTypeRepresentation::Opaque { witness } => {
            require_member_type(types, owner, *witness)?;
        }
        RuntimeTypeRepresentation::Closure { captures } => {
            let mut identities = BTreeSet::new();
            for capture in captures {
                if !identities.insert(capture.capture()) {
                    return Err(RuntimeEnvironmentError::DuplicateRepresentationMember(
                        owner,
                    ));
                }
                require_member_type(types, owner, capture.ty())?;
            }
        }
    }
    Ok(())
}

fn require_member_type(
    types: &RuntimeTypeTable,
    owner: TypeId,
    member: TypeId,
) -> Result<(), RuntimeEnvironmentError> {
    types
        .get(member)
        .map(|_| ())
        .ok_or(RuntimeEnvironmentError::UnknownRepresentationType { owner, member })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEnvironmentError {
    MissingRepresentation(TypeId),
    UnexpectedRepresentation(TypeId),
    UnknownRepresentationOwner(TypeId),
    RepresentationKindMismatch(TypeId),
    UnknownRepresentationType { owner: TypeId, member: TypeId },
    DuplicateRepresentationMember(TypeId),
}

impl std::fmt::Display for RuntimeEnvironmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime environment: {self:?}")
    }
}

impl std::error::Error for RuntimeEnvironmentError {}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, TypeStore};

    use super::RuntimeEnvironmentError;
    use crate::{
        RuntimeAbiIdentity, RuntimeEnvironment, RuntimePrimitive, RuntimeType,
        RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    };

    #[test]
    fn aggregate_runtime_types_require_one_representation() {
        let mut types = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let ty = semantic.builtin(BuiltinType::Bool);
        types.insert(ty, RuntimeType::Aggregate).unwrap();

        assert_eq!(
            RuntimeEnvironment::new(
                types.finish().unwrap(),
                RuntimeTypeRepresentationTable::default(),
                RuntimeAbiIdentity::Arm64DarwinV1,
            )
            .unwrap_err(),
            RuntimeEnvironmentError::MissingRepresentation(ty)
        );
    }

    #[test]
    fn closed_primitive_environment_is_valid() {
        let mut types = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        types
            .insert(
                semantic.builtin(BuiltinType::Bool),
                RuntimeType::Primitive(RuntimePrimitive::Bool),
            )
            .unwrap();

        RuntimeEnvironment::new(
            types.finish().unwrap(),
            RuntimeTypeRepresentationTable::default(),
            RuntimeAbiIdentity::Arm64DarwinV1,
        )
        .unwrap();
    }
}
