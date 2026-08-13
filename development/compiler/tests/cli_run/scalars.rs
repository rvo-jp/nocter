use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_computes_narrow_and_wide_integer_values() {
    let project = TempProject::new("cli-run-all-integer-values");
    let source = project.write_source(
        "all_integer_values.nct",
        r#"copy struct IntegerPair {
    narrow: i16
    wide: u64
}

func add_u16(left: u16, right: u16): u16 {
    return left + right
}

func add_i64(left: i64, right: i64): i64 {
    return left + right
}

func negate_i64(value: i64): i64 {
    return 0 - value
}

func add_i16(left: i16, right: i16): i16 {
    return left + right
}

func compute_i8(value: i8): i8 {
    return (value + 2) * 2
}

func compute_isize(value: isize): isize {
    return value << 1
}

func compute_u32(value: u32): u32 {
    return value / 2
}

func compute_u64(value: u64): u64 {
    return value % 100
}

func shift_i64(value: i64): i64 {
    return value >> 2
}

func negate_i16(value: i16): i16 {
    return -value
}

func maybe_i16(value: i16): i16? {
    return value
}

func fallible_i64(value: i64): i64! {
    return value
}

func widen_u16(value: u16): u32 {
    return value as u32
}

func widen_i16(value: i16): i64 {
    return value as i64
}

func make_pair(value: i16): IntegerPair {
    return IntegerPair { narrow: value, wide: 42 as u64 }
}

func main(): i32! {
    let narrow: u16 = add_u16(20, 22)
    let wide: i64 = add_i64(-2, 44)
    var pair = make_pair(-2)
    pair.narrow = add_i16(-3, 1)
    let optional: i16 = maybe_i16(-4) otherwise { return 7 }
    let fallible: i64 = fallible_i64(42)?
    if narrow == 42 && wide == 42 && fallible == 42 && compute_i8(19) == 42 && compute_isize(21) == 42 && compute_u32(84) == 42 && compute_u64(142) == 42 && shift_i64(-8) == -2 && add_i64(-3, 0) < add_i64(-2, 0) && negate_i64(-17) == 17 && widen_u16(narrow) == 42 && widen_i16(-2) == -2 && negate_i16(-2) == 2 && optional == -4 && pair.narrow == -2 && pair.wide == 42 {
        return 42
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_propagates_fallible_calls_with_wide_integer_parameters() {
    let project = TempProject::new("cli-run-fallible-wide-integer-parameter");
    let source = project.write_source(
        "fallible_wide_integer_parameter.nct",
        r#"copy struct Cell {
    value: i64
}

func digit(cell: &+Cell, value: i64): void! {
    cell.value = value
    return
}

func accept(cell: &+Cell, value: i64): void! {
    if value >= 10 {
        accept(cell, value / 10)?
    }
    digit(cell, value % 10)?
    return
}

func forward(cell: &+Cell, value: i32): void! {
    accept(cell, value as i64)?
    return
}

func main(): i32! {
    var cell = Cell { value: 0 as i64 }
    forward(&+cell, 42)?
    if cell.value == 2 {
        return 42
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_usize_entry_exit_code() {
    let project = TempProject::new("cli-run-usize-entry-return");
    let source = project.write_source(
        "usize_entry_return.nct",
        r#"func main(): usize {
    return 23
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_scalar_var_assignment_exit_code() {
    let project = TempProject::new("cli-run-scalar-var-assignment");
    let source = project.write_source(
        "scalar_var_assignment.nct",
        r#"func main(): i32 {
    var count = 1
    count = count + 39
    var byte: u8 = 1
    byte = 2
    var size: usize = 0
    size = 40
    var flag: bool = false
    flag = ready()
    if flag && size == 40 {
        return count + (byte as i32)
    } else {
        return 1
    }
}

func ready(): bool {
    return true
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_identity_conversions_of_computed_integer_values() {
    let project = TempProject::new("cli-run-computed-integer-identity-conversions");
    let source = project.write_source(
        "computed_integer_identity_conversions.nct",
        r#"func main(): i32 {
    let size = (size_value() + 1) as usize
    let byte = (byte_value() + 1) as u8
    let number = (number_value() + 1) as i32
    if size == 2 && byte == 2 {
        return number + 40
    }
    return 1
}

func size_value(): usize {
    return 1
}

func byte_value(): u8 {
    return 1
}

func number_value(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_scalar_compound_assignment_exit_code() {
    let project = TempProject::new("cli-run-scalar-compound-assignment");
    let source = project.write_source(
        "scalar_compound_assignment.nct",
        r#"func main(): i32 {
    var count = 40
    count += one()
    count *= 2
    count -= 40
    count /= 2
    var size: usize = 47
    size %= 5
    var byte: u8 = 6
    byte += 3
    byte *= 2
    byte -= 6
    byte /= 3
    byte %= 4
    if count == 21 && size == 2 && byte == 0 {
        return 23
    } else {
        return 1
    }
}

func one(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_unary_negate_value_exit_code() {
    let project = TempProject::new("cli-run-i32-unary-negate-value");
    let source = project.write_source(
        "i32_unary_negate_value.nct",
        r#"func main(): i32 {
    return 42 + negative(7)
}

func negative(value: i32): i32 {
    return -value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(35),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_usize_arithmetic_and_shift_exit_code() {
    let project = TempProject::new("cli-run-usize-arithmetic-shift");
    let source = project.write_source(
        "usize_arithmetic_shift.nct",
        r#"func main(): i32 {
    if combined(20, size()) == 23 {
        return 42
    } else {
        return 1
    }
}

func combined(left: usize, right: usize): usize {
    return arithmetic(left, right) + shifted_left() + shifted_right()
}

func arithmetic(left: usize, right: usize): usize {
    let doubled: usize = right * 2
    let adjusted: usize = left + doubled - 4
    let quotient: usize = adjusted / 2
    let remainder: usize = quotient % 9
    return remainder
}

func shifted_left(): usize {
    return one() << left_count()
}

func shifted_right(): usize {
    return sixty_four() >> right_count()
}

func size(): usize {
    return 6
}

func one(): usize {
    return 1
}

func sixty_four(): usize {
    return 64
}

func left_count(): usize {
    return 4
}

func right_count(): usize {
    return 5
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_short_circuits_boolean_rhs() {
    let project = TempProject::new("cli-run-short-circuit-boolean-rhs");
    let source = project.write_source(
        "short_circuit_boolean_rhs.nct",
        r#"func zero(): i32 {
    return 0
}

func skipped_by_and(): bool {
    return false && (1 / zero() == 0)
}

func skipped_by_or(): bool {
    return true || (1 / zero() == 0)
}

func main(): i32 {
    if skipped_by_and() {
        return 1
    }
    if skipped_by_or() {
        return 42
    }
    return 2
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_executes_u8_calls_through_scalar_storage() {
    let project = TempProject::new("cli-run-u8-scalar-storage");
    let source = project.write_source(
        "u8_scalar_storage.nct",
        r#"func increment(value: u8): u8 {
    return value + 1
}

func main(): i32 {
    if increment(40) == 41 {
        return 42
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_executes_lossless_integer_casts_through_mir() {
    let project = TempProject::new("cli-run-lossless-integer-casts");
    let source = project.write_source(
        "lossless_integer_casts.nct",
        r#"func widen_byte(value: u8): i32 {
    return value as i32
}

func widen_number(value: i32): i64 {
    return value as i64
}

func main(): i32 {
    if widen_byte(40) == 40 && widen_number(-7) == -7 {
        return 42
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
