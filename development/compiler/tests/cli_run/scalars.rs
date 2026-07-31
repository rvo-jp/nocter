use super::*;

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
