use super::*;

pub(super) fn checked_pair_len_index(
    first_index: usize,
    subject: &str,
) -> Result<usize, Vec<Diagnostic>> {
    first_index.checked_add(1).ok_or_else(|| {
        vec![Diagnostic::error(
            "E9005",
            format!("{subject} length word index overflows"),
        )]
    })
}

pub(super) fn return_passing_description(passing: Option<ReturnPassing>) -> &'static str {
    passing.map_or("unsupported return ABI", ReturnPassing::description)
}

pub(super) fn type_return_description(ty: &Type) -> String {
    let shape = match ty {
        Type::I32 => "i32".to_string(),
        Type::U8 => "u8".to_string(),
        Type::Usize => "usize".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Str => "&str".to_string(),
        Type::Slice { .. } => "slice".to_string(),
        Type::Aggregate { layout } => {
            format!("indirect aggregate {}", layout_description(*layout))
        }
        Type::DirectAggregate { layout, .. } => {
            format!("direct aggregate {}", layout_description(*layout))
        }
        Type::Borrow { .. } => "borrow".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::Optional(payload) => format!("optional {}", type_return_description(payload)),
        Type::Fallible(success) => format!("fallible {}", type_return_description(success)),
        Type::ComposedOutcome {
            outer,
            inner,
            payload,
        } => format!("{outer:?} {inner:?} {}", type_return_description(payload)),
    };
    format!(
        "{shape} ({})",
        return_passing_description(ty.success_return_passing())
    )
}

pub(super) fn layout_description(layout: crate::abi::ValueLayout) -> String {
    format!("{} bytes align {}", layout.size, layout.align)
}
