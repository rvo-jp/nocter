use nocter_model::{BuiltinType, TypeId, TypeKind, TypeStore};

pub(super) fn integer_type(types: &TypeStore, expected: Option<TypeId>) -> TypeId {
    contextual_integer_type(types, expected).unwrap_or_else(|| types.builtin(BuiltinType::I32))
}

pub(super) fn contextual_integer_type(
    types: &TypeStore,
    expected: Option<TypeId>,
) -> Option<TypeId> {
    expected
        .and_then(|expected| outcome_leaf(types, expected))
        .filter(|expected| is_integer_type(types, *expected))
}

pub(super) fn is_integer_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Builtin(
            BuiltinType::I8
                | BuiltinType::I16
                | BuiltinType::I32
                | BuiltinType::I64
                | BuiltinType::U8
                | BuiltinType::U16
                | BuiltinType::U32
                | BuiltinType::U64
                | BuiltinType::Usize
                | BuiltinType::Isize
        ))
    )
}

pub(super) fn parse_integer(text: &str) -> Option<u64> {
    let compact = text
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>();
    if let Some(digits) = compact.strip_prefix("0x") {
        u64::from_str_radix(digits, 16).ok()
    } else if let Some(digits) = compact.strip_prefix("0b") {
        u64::from_str_radix(digits, 2).ok()
    } else {
        compact.parse().ok()
    }
}

pub(super) fn fits_integer(types: &TypeStore, ty: TypeId, value: u64) -> bool {
    match types.get(ty) {
        Some(TypeKind::Builtin(BuiltinType::I8)) => i8::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::I16)) => i16::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::I32)) => i32::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::I64 | BuiltinType::Isize)) => {
            i64::try_from(value).is_ok()
        }
        Some(TypeKind::Builtin(BuiltinType::U8)) => u8::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::U16)) => u16::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::U32)) => u32::try_from(value).is_ok(),
        Some(TypeKind::Builtin(BuiltinType::U64 | BuiltinType::Usize)) => true,
        _ => false,
    }
}

fn outcome_leaf(types: &TypeStore, root: TypeId) -> Option<TypeId> {
    let mut current = root;
    loop {
        match types.get(current)? {
            TypeKind::Optional(payload) | TypeKind::Fallible(payload) => current = *payload,
            _ => return Some(current),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_integer;

    #[test]
    fn integer_decoder_accepts_every_lexical_radix_and_separators() {
        assert_eq!(parse_integer("1_000"), Some(1_000));
        assert_eq!(parse_integer("0xFF_FF"), Some(65_535));
        assert_eq!(parse_integer("0b1010"), Some(10));
    }
}
