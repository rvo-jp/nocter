//! Value and type lookup select independent names without changing the parse.
use super::initializers::answer;
use nocter_constant_evaluation::state::Value;

#[test]
fn a_record_and_a_callable_share_spelling_in_separate_name_spaces() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/separate-name-spaces.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    assert_eq!(answer(source, "LOCAL"), Value::integer(42));
    assert_eq!(answer(source, "width"), Value::integer(2));
}
