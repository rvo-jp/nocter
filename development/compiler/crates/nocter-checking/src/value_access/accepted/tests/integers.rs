//! Target-profile usize and contextual literals use the ordinary typed/evaluator path.
use super::*;
use nocter_constant_evaluation::state::Value as Data;
fn constant(source: &str) -> nocter_model::FrozenData {
    let unit = accepted(source);
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    unit.constant_value(id).unwrap().clone()
}
#[test]
fn size_values_preserve_the_full_unsigned_profile_through_ctfe_and_frozen_data() {
    assert_eq!(
        constant("const ANSWER: usize = 18446744073709551615"),
        Data::integer(u64::MAX).freeze()
    );
    assert_eq!(
        constant(
            "const func width(): usize { var result: usize = 1\nvar index: usize = 0\nwhile index < 6 { result *= 2\nindex += 1 }\nresult }\nstatic WIDTH: usize = width()\nconst ANSWER: usize = WIDTH"
        ),
        Data::integer(64).freeze()
    );
    assert_eq!(
        constant(
            "const HIGH: usize = 9223372036854775808\nconst ANSWER: bool = 18446744073709551615 > HIGH"
        ),
        Data::boolean(true).freeze()
    );
}
#[test]
fn literal_recipes_receive_context_from_either_operand_and_selected_inputs() {
    for expression in [
        "size + 2",
        "2 + size",
        "(1 + 1) + size",
        "1 + (1 + size)",
        "receive(20 + 22)",
        "pair((20, 22))",
    ] {
        let source = format!(
            "const func receive(value: usize): usize {{ value }}\nconst func pair(value: (usize, usize)): usize {{ value.0 + value.1 }}\nconst func calculate(): usize {{ let size: usize = 40\n{expression} }}\nconst ANSWER: usize = calculate()"
        );
        assert_eq!(
            constant(&source),
            Data::integer(42).freeze(),
            "{expression}"
        );
    }
}

#[test]
fn admitted_operator_continuations_preserve_binary_meaning() {
    for (expression, ty, expected) in [
        ("40\n+ 2", "i32", Data::integer(42)),
        ("50 -\n8", "i32", Data::integer(42)),
        ("6\n* 7", "i32", Data::integer(42)),
        ("84\n/ 2", "i32", Data::integer(42)),
        ("85\n% 43", "i32", Data::integer(42)),
        ("40\n< 42", "bool", Data::boolean(true)),
        ("40\n<= 42", "bool", Data::boolean(true)),
        ("42\n> 40", "bool", Data::boolean(true)),
        ("42\n>= 40", "bool", Data::boolean(true)),
        ("42\n== 42", "bool", Data::boolean(true)),
        ("40\n!= 42", "bool", Data::boolean(true)),
        ("true\n&& true", "bool", Data::boolean(true)),
        ("false\n|| true", "bool", Data::boolean(true)),
        ("(40\n\n+ 2)", "i32", Data::integer(42)),
    ] {
        let source = format!(
            "const func calculate(): {ty} {{ {expression} }}\nconst ANSWER: {ty} = calculate()"
        );
        assert_eq!(constant(&source), expected.freeze(), "{expression}");
    }
}
#[test]
fn data_context_never_converts_typed_places_constants_or_call_results() {
    for body in [
        "let size: usize = -1",
        "let size: usize = 18446744073709551616",
        "let size = 2147483648",
        "let small = 1\nlet size: usize = 2\nlet result = small + size",
        "let size: usize = 2\nlet result = BASE + size",
        "let size: usize = 2\nlet result = signed() + size",
        "let small = 1\nreceive(small)",
    ] {
        let source = format!(
            "const BASE: i32 = 1\nfunc signed(): i32 {{ 1 }}\nfunc receive(value: usize): void {{}}\nfunc run(): void {{ {body} }}"
        );
        let (sources, tree) = input(&source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body {
                    failure: BodyFailure::Rejected(_),
                    ..
                })
            ),
            "{source}"
        );
    }
}
#[test]
fn unsigned_failures_and_operand_order_reach_required_evaluation() {
    for source in [
        "const ANSWER: usize = 18446744073709551615 + 1",
        "const ANSWER: usize = 0 - 1",
        "const ANSWER: usize = 18446744073709551615 * 2",
        "const ANSWER: usize = 1 / 0",
        "const ANSWER: usize = 1 % 0",
        "const func fail(): usize { let value: usize = 1\n-value }\nconst ANSWER: usize = fail()",
        "const func fail(): usize { let value: usize = 0\nlet _ = (18446744073709551615 + 1) + if true { return 42 } else { value }\n0 }\nconst ANSWER: usize = fail()",
    ] {
        let (sources, tree) = input(source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap());
        assert!(
            matches!(result, Err(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::ArithmeticFailure)),
            "{source}"
        );
    }
    assert_eq!(
        constant(
            "const func zero(): usize { let value: usize = 0\n-value }\nconst ANSWER: usize = zero()"
        ),
        Data::integer(0).freeze()
    );
}
