pub(super) fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "str"
    )
}

pub(super) fn is_reserved_type_declaration_name(name: &str) -> bool {
    is_builtin_type_name(name) || name == "error"
}
