//! Program-extent decoded text is ordinary data, separate from its access operation.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;

#[test]
fn text_decoding_publication_and_value_handoffs_use_the_ordinary_required_path() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/text-values.nct"
    ));
    assert_eq!(answer(source, "TEXT"), Datum::text("夜\0\n\"\\"));
    assert_eq!(answer(source, "MULTILINE"), Datum::text("first\nsecond"));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn text_storage_does_not_grant_copy_component_access_or_source_comparison() {
    for source in [
        "func run(): void { let first = \"text\"\nlet next = first }",
        "func run(value: str): str { value }",
        "func run(value: str): str { value^ }",
        "func run(): void { read value = \"text\" }",
        "func run(): void { let text = \"text\"\nlet values = [text; 2] }",
        "func run(): void { let text = \"text\"\nlet next = text^\nlet reused = text^ }",
        "func run(): bool { \"text\" == \"text\" }",
        "func run(value: str): void { let byte = value[0] }",
        "func forward(take value: str): str { value^ }\nfunc observe(value: str, ignored: i32): void {}\nfunc run(): void { let value = \"text\"\nloop { observe(forward(value^), if true { break } else { 0 }) }\nobserve(value, 0) }",
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
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn primitive_syntax_needs_authority_while_ordinary_similar_names_remain_ordinary() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "primitive func new_error(code: str, message: str): error",
    );
    assert!(
        matches!(SourceUnit::prepare(fixture.syntax()), Err(SourceFailure::Declarations(declarations::Failure::Primitive(diagnostic))) if diagnostic.rule() == declarations::PrimitiveRule::Unauthorized)
    );
    let unit = accepted("func new_error(): i32 { 42 }\nfunc run(): i32 { new_error() }");
    assert!(matches!(
        named(&unit, "new_error").typed().code(),
        nocter_model::ValueCode::Declaration(_)
    ));
}
