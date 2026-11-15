mod data_compatibility;
pub use data_compatibility::ValueRepresentationRelations;
use std::collections::BTreeSet;

use crate::RuntimeDataId;

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
        for (ty, kind) in types.iter() {
            if let RuntimeType::Storage(role) = kind
                && role.layout(abi).is_none()
            {
                return Err(RuntimeEnvironmentError::UnsupportedStorage { ty, role: *role });
            }
        }
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

    /// Select one stored value-model component. This grants no initialization or access proof.
    #[must_use]
    pub fn value_field(
        &self,
        data: RuntimeDataId,
        field: nocter_model::ValueField,
    ) -> Option<RuntimeDataId> {
        match field {
            nocter_model::ValueField::Variant(primary) => self.types.outcome_payload(data, primary),
            nocter_model::ValueField::Array(index) => {
                let (element, length) = self.types.get(data)?.array()?;
                (index < length).then_some(element)
            }
            _ => {
                let RuntimeTypeRepresentation::ValueAggregate { fields } =
                    self.type_representations.get(data)?
                else {
                    return None;
                };
                fields
                    .iter()
                    .find(|component| component.field() == field)
                    .map(|component| component.ty())
            }
        }
    }
    /// Project frozen value-model selectors through runtime identities, without semantic lookup.
    /// The caller retains the source proof for each saved index and its bound disposition.
    #[must_use]
    pub fn value_path<I>(
        &self,
        mut data: RuntimeDataId,
        path: &[nocter_model::ValueProjection<I>],
        array_bound: impl Fn(&I, u64) -> bool,
    ) -> Option<RuntimeDataId> {
        use nocter_model::{ValueField, ValueProjection};
        for projection in path {
            data = match projection {
                ValueProjection::Record(field) => {
                    self.value_field(data, ValueField::Record(*field))?
                }
                ValueProjection::Tuple(index) => {
                    self.value_field(data, ValueField::Tuple(*index))?
                }
                ValueProjection::Variant(primary) => {
                    self.value_field(data, ValueField::Variant(*primary))?
                }
                ValueProjection::Array(index) => {
                    let (element, length) = self.types.get(data)?.array()?;
                    if !array_bound(index, length) {
                        return None;
                    }
                    element
                }
            };
        }
        Some(data)
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
        let text_value = matches!(
            (kind, representation),
            (
                RuntimeType::Primitive(crate::RuntimePrimitive::Text),
                Some(RuntimeTypeRepresentation::ValueText)
            )
        );
        if !required && representation.is_some() && !text_value {
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
                RuntimeType::Primitive(crate::RuntimePrimitive::Text),
                RuntimeTypeRepresentation::ValueText
            ) | (
                RuntimeType::Aggregate,
                RuntimeTypeRepresentation::Struct { .. }
                    | RuntimeTypeRepresentation::ValueAggregate { .. }
                    | RuntimeTypeRepresentation::Enum { .. }
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
    owner: RuntimeDataId,
    representation: &RuntimeTypeRepresentation,
) -> Result<(), RuntimeEnvironmentError> {
    match representation {
        RuntimeTypeRepresentation::ValueText => {}
        RuntimeTypeRepresentation::ValueAggregate { fields } => {
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
    owner: RuntimeDataId,
    member: RuntimeDataId,
) -> Result<(), RuntimeEnvironmentError> {
    types
        .get(member)
        .map(|_| ())
        .ok_or(RuntimeEnvironmentError::UnknownRepresentationType { owner, member })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEnvironmentError {
    MissingRepresentation(RuntimeDataId),
    UnexpectedRepresentation(RuntimeDataId),
    UnknownRepresentationOwner(RuntimeDataId),
    RepresentationKindMismatch(RuntimeDataId),
    UnknownRepresentationType {
        owner: RuntimeDataId,
        member: RuntimeDataId,
    },
    DuplicateRepresentationMember(RuntimeDataId),
    UnsupportedStorage {
        ty: RuntimeDataId,
        role: crate::RuntimeStorageRole,
    },
}

impl std::fmt::Display for RuntimeEnvironmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime environment: {self:?}")
    }
}

impl std::error::Error for RuntimeEnvironmentError {}

#[cfg(test)]
mod tests {
    use nocter_model::{AccessFieldId, ArenaBuilder, BuiltinType, TypeStore};

    use super::RuntimeEnvironmentError;
    use crate::{
        RuntimeAbiIdentity, RuntimeEnvironment, RuntimePrimitive, RuntimeType,
        RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
        RuntimeValueFieldRepresentation,
    };

    #[test]
    fn value_transport_ignores_empty_element_metadata_but_preserves_static_callable_identity() {
        use crate::RuntimeDataId;
        use nocter_model::ValueField;
        let [first, second, empty_a, empty_b, tuple_a, tuple_b, wrong] =
            [0, 1, 2, 3, 4, 5, 6].map(RuntimeDataId::Instance);
        let mut types = RuntimeTypeTableBuilder::new();
        for id in [first, second] {
            types
                .insert(id, RuntimeType::StaticCallable { environment: None })
                .unwrap();
        }
        for (id, element) in [(empty_a, 100), (empty_b, 101)] {
            types
                .insert(
                    id,
                    RuntimeType::EmptyArray {
                        element: RuntimeDataId::Instance(element),
                    },
                )
                .unwrap();
        }
        let mut representations = std::collections::BTreeMap::new();
        for (id, empty, callable) in [
            (tuple_a, empty_a, first),
            (tuple_b, empty_b, first),
            (wrong, empty_b, second),
        ] {
            types.insert(id, RuntimeType::Aggregate).unwrap();
            representations.insert(
                id,
                RuntimeTypeRepresentation::ValueAggregate {
                    fields: [empty, callable]
                        .into_iter()
                        .enumerate()
                        .map(|(index, data)| {
                            RuntimeValueFieldRepresentation::new(ValueField::Tuple(index), data)
                        })
                        .collect(),
                },
            );
        }
        let runtime = RuntimeEnvironment::new(
            types.finish().unwrap(),
            RuntimeTypeRepresentationTable::new(representations),
            RuntimeAbiIdentity::Arm64DarwinV1,
        )
        .unwrap();
        assert!(runtime.has_same_value_representation(empty_a, empty_b));
        assert!(runtime.has_same_value_representation(tuple_a, tuple_b));
        assert!(runtime.has_same_value_representation(tuple_b, tuple_a));
        assert!(!runtime.has_same_value_representation(first, second));
        assert!(!runtime.has_same_value_representation(tuple_a, wrong));
        assert!(!runtime.has_same_value_representation(
            RuntimeDataId::Instance(100),
            RuntimeDataId::Instance(100)
        ));
    }

    #[test]
    fn stored_text_representation_requires_the_exact_text_identity() {
        for primitive in [RuntimePrimitive::Text, RuntimePrimitive::Bool] {
            let semantic = TypeStore::new();
            let ty: crate::RuntimeDataId = semantic.builtin(BuiltinType::Str).into();
            let mut types = RuntimeTypeTableBuilder::new();
            types.insert(ty, RuntimeType::Primitive(primitive)).unwrap();
            let types = types.finish().unwrap();
            assert!(
                RuntimeEnvironment::new(
                    types.clone(),
                    RuntimeTypeRepresentationTable::default(),
                    RuntimeAbiIdentity::Arm64DarwinV1
                )
                .is_ok()
            );
            let result = RuntimeEnvironment::new(
                types,
                RuntimeTypeRepresentationTable::new(std::collections::BTreeMap::from([(
                    ty,
                    RuntimeTypeRepresentation::ValueText,
                )])),
                RuntimeAbiIdentity::Arm64DarwinV1,
            );
            if primitive == RuntimePrimitive::Text {
                assert!(result.is_ok());
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    RuntimeEnvironmentError::UnexpectedRepresentation(ty)
                );
            }
        }
    }

    #[test]
    fn aggregate_runtime_types_require_one_representation() {
        let mut types = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let ty: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
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

    #[test]
    fn value_record_members_validate_identity_and_closed_type_references() {
        let semantic = TypeStore::new();
        let owner: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        let member: crate::RuntimeDataId = semantic.builtin(BuiltinType::I8).into();
        let mut ids = ArenaBuilder::<AccessFieldId, ()>::new();
        let field = ids.insert(());
        for (fields, expected) in [
            (
                vec![RuntimeValueFieldRepresentation::new(field.into(), member)],
                RuntimeEnvironmentError::UnknownRepresentationType { owner, member },
            ),
            (
                vec![RuntimeValueFieldRepresentation::new(field.into(), owner); 2],
                RuntimeEnvironmentError::DuplicateRepresentationMember(owner),
            ),
        ] {
            let mut types = RuntimeTypeTableBuilder::new();
            types.insert(owner, RuntimeType::Aggregate).unwrap();
            let representations = RuntimeTypeRepresentationTable::new(
                [(
                    owner,
                    RuntimeTypeRepresentation::ValueAggregate {
                        fields: fields.into_boxed_slice(),
                    },
                )]
                .into(),
            );
            assert_eq!(
                RuntimeEnvironment::new(
                    types.finish().unwrap(),
                    representations,
                    RuntimeAbiIdentity::Arm64DarwinV1
                )
                .unwrap_err(),
                expected
            );
        }
    }
}
