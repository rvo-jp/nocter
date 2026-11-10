//! One checking-owned disposition, independent of later Machine constant propagation.
use super::*;
use nocter_model::IndexBoundsCheck;

#[test]
fn in_bounds_singletons_are_frozen_without_classifying_unclosed_trap_continuations() {
    for (source, expected) in [
        (
            "func work(values: [i32; 2]): i32 { values[1] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "const INDEX: usize = 1\nfunc work(values: [i32; 2]): i32 { values[INDEX] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [i32; 2], flag: bool): i32 { values[if flag { 1 } else { 1 }] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [void; 18446744073709551615]): void { values[18446744073709551614] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [i32; 2], index: usize): i32 { values[index] }",
            IndexBoundsCheck::Required,
        ),
        (
            "func work(values: [i32; 2]): i32 { values[2] }",
            IndexBoundsCheck::Required,
        ),
        (
            "func work(values: [void; 0]): void { values[0] }",
            IndexBoundsCheck::Required,
        ),
        (
            "func work(): void { let values = [0]\nlet _ = values[if true { return } else { 0 }] }",
            IndexBoundsCheck::Required,
        ),
    ] {
        let unit = accepted(source);
        let query = named(&unit, "work");
        let checks: Vec<_> = query
            .typed()
            .body()
            .expressions()
            .iter()
            .filter_map(|(_, value)| match value.kind {
                crate::value_access::ExpressionKind::Index { bounds, .. } => Some(bounds),
                _ => None,
            })
            .collect();
        assert_eq!(checks, [expected], "{source}");
    }
}
