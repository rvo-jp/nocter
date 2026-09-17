use nocter_model::{
    BorrowCapability, BuiltinType, ConstantValue, FrozenValue, TypeId, TypeKind, TypeStore,
};

/// Tests one scalar compile-time value against the canonical semantic type authority.
pub(crate) fn constant_matches(types: &TypeStore, ty: TypeId, value: &ConstantValue) -> bool {
    match value {
        ConstantValue::Bool(_) => ty == types.builtin(BuiltinType::Bool),
        ConstantValue::Character(value) => {
            ty == types.builtin(BuiltinType::Char) && char::from_u32(*value).is_some()
        }
        ConstantValue::Float32(_) => ty == types.builtin(BuiltinType::F32),
        ConstantValue::Float64(_) => ty == types.builtin(BuiltinType::F64),
        ConstantValue::Integer(value) => types
            .get(ty)
            .and_then(|ty| match ty {
                TypeKind::Builtin(builtin) => Some(*builtin),
                _ => None,
            })
            .is_some_and(|builtin| integer_fits(*value, builtin)),
        ConstantValue::Text(_) => matches!(
            types.get(ty),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == types.builtin(BuiltinType::Str)
        ),
    }
}

/// Tests one recursively frozen value without reconstructing a parallel type representation.
pub(crate) fn frozen_matches(types: &TypeStore, ty: TypeId, value: &FrozenValue) -> bool {
    match (types.get(ty), value) {
        (_, FrozenValue::Scalar(value)) => constant_matches(types, ty, value),
        (Some(TypeKind::Tuple(elements)), FrozenValue::Tuple(values)) => {
            elements.iter().len() == values.len()
                && elements
                    .iter()
                    .zip(values)
                    .all(|(element, value)| frozen_matches(types, element, value))
        }
        (Some(TypeKind::FixedArray { element, length }), FrozenValue::FixedArray(values)) => {
            length
                .closed_value()
                .and_then(|length| usize::try_from(length).ok())
                == Some(values.len())
                && values
                    .iter()
                    .all(|value| frozen_matches(types, *element, value))
        }
        _ => false,
    }
}

fn integer_fits(value: i128, builtin: BuiltinType) -> bool {
    match builtin {
        BuiltinType::I8 => i8::try_from(value).is_ok(),
        BuiltinType::I16 => i16::try_from(value).is_ok(),
        BuiltinType::I32 => i32::try_from(value).is_ok(),
        BuiltinType::I64 | BuiltinType::Isize => i64::try_from(value).is_ok(),
        BuiltinType::U8 => u8::try_from(value).is_ok(),
        BuiltinType::U16 => u16::try_from(value).is_ok(),
        BuiltinType::U32 => u32::try_from(value).is_ok(),
        BuiltinType::U64 | BuiltinType::Usize => u64::try_from(value).is_ok(),
        _ => false,
    }
}
