//! Shared checked integer failures: identical bodies in required and native execution.
/// Runtime-admissible sources whose arithmetic must fail if the body is executed.
#[must_use]
pub fn integer_failure_sources() -> Vec<(String, String)> {
    let mut sources = Vec::new();
    for (data, minimum, maximum) in [
        ("i8", -128_i128, 127_i128),
        ("i16", -32_768_i128, 32_767_i128),
        ("i32", -2_147_483_648_i128, 2_147_483_647_i128),
        (
            "i64",
            -9_223_372_036_854_775_808_i128,
            9_223_372_036_854_775_807_i128,
        ),
        ("u8", 0_i128, 255_i128),
        ("u16", 0_i128, 65_535_i128),
        ("u32", 0_i128, 4_294_967_295_i128),
        ("u64", 0_i128, 18_446_744_073_709_551_615_i128),
        (
            "isize",
            -9_223_372_036_854_775_808_i128,
            9_223_372_036_854_775_807_i128,
        ),
        ("usize", 0_i128, 18_446_744_073_709_551_615_i128),
    ] {
        let mut failures = vec![
            (
                "add",
                format!("let value: {data} = {maximum}\nlet unused = value + 1"),
            ),
            (
                "subtract",
                format!("let value: {data} = {minimum}\nlet unused = value - 1"),
            ),
            (
                "multiply",
                format!("let value: {data} = {maximum}\nlet unused = value * 2"),
            ),
            (
                "divide-zero",
                format!("let value: {data} = 1\nlet unused = value / 0"),
            ),
            (
                "remainder-zero",
                format!("let value: {data} = 1\nlet unused = value % 0"),
            ),
            (
                "negate",
                format!(
                    "let value: {data} = {}\nlet unused = -value",
                    if minimum < 0 { minimum } else { 1 }
                ),
            ),
            (
                "source-order",
                format!(
                    "let value: {data} = 0\nlet unused = ({maximum} + 1) + if true {{ return 42 }} else {{ value }}"
                ),
            ),
        ];
        if minimum < 0 {
            failures.push((
                "divide-overflow",
                format!("let value: {data} = {minimum}\nlet unused = value / -1"),
            ));
        }
        for (name, body) in failures {
            sources.push((
                format!("{data}-{name}"),
                format!(
                    "const func calculate(): i32 {{ {body}\n0 }}\nfunc run(): i32 {{ calculate() }}"
                ),
            ));
        }
    }
    sources
}
