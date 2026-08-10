//! Canonical classification of reserved type-position spellings.
//!
//! Parser pattern validation and name resolution share this boundary so a built-in type cannot
//! become a declaration binder in one phase and a reserved name in another.

pub(crate) fn is_builtin_type_name(name: &str) -> bool {
    crate::integer::IntegerType::from_name(name).is_some() || matches!(name, "bool" | "str")
}

pub(crate) fn is_reserved_type_declaration_name(name: &str) -> bool {
    is_builtin_type_name(name) || name == "error"
}
