//! Fallible source construction and exits preserve explicit branch and ownership decisions.
use super::*;

#[test]
fn fallible_sources_forward_construct_success_and_lower_failure_to_ordinary_returns() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/fallible-values.nct"
    ));
    let unit = accepted(source);
    for (name, primary) in [("success_error", true), ("failure_error", false)] {
        let query = named(&unit, name).typed();
        let tags: Vec<_> = query
            .body()
            .expressions()
            .iter()
            .filter_map(|(_, value)| {
                let crate::value_access::ExpressionKind::Aggregate { fields } = &value.kind else {
                    return None;
                };
                matches!(
                    unit.program().data_schema(value.ty.access()?.data()),
                    Some(nocter_declarations::value_access::PayloadSchema::Fallible(
                        _
                    ))
                )
                .then(|| fields.iter().map(|field| field.field).collect::<Vec<_>>())
            })
            .collect();
        assert_eq!(
            tags,
            [vec![nocter_model::ValueField::Variant(primary)]],
            "{name}"
        );
    }
}
#[test]
fn fallible_exits_cannot_invent_error_ownership_convert_absence_or_retire_a_read_carrier() {
    for source in [
        "func run(take value: error): i32 { throw value^ }",
        "func run(): i32! { throw 42 }",
        "func run(take value: error): i32! { throw value }",
        "func run(value: error): i32! { throw value^ }",
        "func run(): void { let value: i32! = 42\nlet copied = value }",
        "func run(value: i32!): i32 { value! }",
        "func run(): i32 { var value: i32! = 42\nread payload = value!\nvalue = 1\npayload }",
        "func run(): i32 { let value: i32! = 42\nlet first = value^!\nfirst + value^! }",
        "func run(take value: i32!): i32? { value^? }",
        "func run(): i32! { let value: i32? = none\nvalue? }",
        "func set(edit value: i32): void {}\nfunc run(value: i32!): void { set(value!) }",
        "func run(value: i32!): i32! { read payload = value?\npayload }",
        "func run(take value: i32!): i32 { value^ otherwise { 42 } }",
        "func run(): void { let value: (i32!)! = 42 }",
        "func run(value: (i32!)!): void {}",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        assert!(
            matches!(
                SourceUnit::prepare(fixture.syntax()),
                Err(SourceFailure::Checking(
                    Failure::Body {
                        failure: BodyFailure::Rejected(_),
                        ..
                    } | Failure::Flow { .. }
                ) | SourceFailure::Declarations(
                    nocter_declaration_lowering::access_declarations::Failure::Headers(_)
                ))
            ),
            "{source}"
        );
    }
}
