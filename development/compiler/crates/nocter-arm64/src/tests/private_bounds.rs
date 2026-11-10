//! Generated instructions qualify the benefit without a timing-dependent benchmark.
#[test]
fn source_bounds_proof_removes_only_the_qualified_native_check() {
    let bounds_trap = crate::Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::Bounds.immediate(),
    }
    .encode()
    .unwrap();
    for (source, expected) in [
        ("func run(): i32 { [20, 42][1] }", 0),
        (
            "func run(): i32 { let index: usize = 1\n[20, 42][index] }",
            1,
        ),
        ("func run(): i32 { [20, 42][2] }", 1),
        ("func run(): i32 { let _ = [(); 0][0]\n42 }", 1),
        ("func run(): i32 { let _ = [(), ()][1]\n42 }", 0),
        (
            "func run(): i32 { var count = 40\nlet _ = [(), ()][if true { count += 2\n1 } else { 1 }]\ncount }",
            0,
        ),
    ] {
        let machine = crate::test_support::lower_value_machine(source);
        let native = crate::Arm64Program::lower_machine(&machine).unwrap();
        let checks = native
            .text()
            .chunks_exact(4)
            .filter(|instruction| *instruction == bounds_trap)
            .count();
        assert_eq!(checks, expected, "{source}");
    }
}
