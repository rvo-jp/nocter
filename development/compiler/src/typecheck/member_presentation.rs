//! Shared presentation of resolved type members for editor features.
//!
//! Callers resolve and specialize the owner and signature. This module owns the
//! stable spelling used by hover, completion, and other semantic UI surfaces.

pub(crate) fn qualified_member_name(owner: &str, member: &str) -> String {
    format!("{owner}.{member}")
}

pub(crate) fn field_member_label(owner: &str, member: &str, ty: &str) -> String {
    format!("field {}: {ty}", qualified_member_name(owner, member))
}

pub(crate) fn enum_variant_member_label(owner: &str, member: &str, payload: &[String]) -> String {
    let name = qualified_member_name(owner, member);
    if payload.is_empty() {
        format!("variant {name}")
    } else {
        format!("variant {name}({})", payload.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_labels_always_include_the_owner() {
        assert_eq!(
            field_member_label("Box<T>", "value", "T"),
            "field Box<T>.value: T"
        );
        assert_eq!(
            enum_variant_member_label("Option<T>", "some", &["value: T".to_string()]),
            "variant Option<T>.some(value: T)"
        );
        assert_eq!(
            enum_variant_member_label("Option<T>", "none", &[]),
            "variant Option<T>.none"
        );
    }
}
