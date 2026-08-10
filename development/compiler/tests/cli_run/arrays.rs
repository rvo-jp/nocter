use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_nonlegacy_integer_fixed_array_elements() {
    let project = TempProject::new("cli-run-nonlegacy-integer-fixed-array");
    let source = project.write_source(
        "nonlegacy_integer_fixed_array.nct",
        r#"func add(left: i16, right: i16): i16 {
    return left + right
}

func main(): i32 {
    var values: [i16; 3] = [-3 as i16, -2 as i16, -1 as i16]
    let index: usize = 1
    values[index] = add(values[0], 0)
    values[index] += 1
    if values[index] == -2 && values[2] == -1 {
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
fn run_command_returns_generic_fixed_array_literal_value_argument_exit_code() {
    let project = TempProject::new("cli-run-generic-fixed-array-literal-value-argument");
    let source = project.write_source(
        "generic_fixed_array_literal_value_argument.nct",
        r#"func main(): i32 {
    return first([42, 1])
}

func first<T>(values: [T; 2]): T {
    return values[0]
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
fn run_command_returns_generic_fixed_array_aggregate_field_exit_code() {
    let project = TempProject::new("cli-run-generic-fixed-array-aggregate-fields");
    let source = project.write_source(
        "generic_fixed_array_aggregate_fields.nct",
        r#"copy struct Box<T> {
    values: [T; 2]
}

func main(): i32 {
    var box = Box<i32> { values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Box<i32> { values: [20, 22] }
    box.values = [5, 6]
    box.values = replacement
    box.values = make_pair()
    box.values = other.values
    return box.values[0] + box.values[1]
}

func make_pair(): [i32; 2] {
    return [7, 8]
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
fn run_command_reads_fixed_array_literal_constant_indices() {
    let project = TempProject::new("cli-run-fixed-array-literal-constant-index");
    let source = project.write_source(
        "fixed_array_literal_constant_index.nct",
        r#"func main(): i32 {
    let scores: [i32; 3] = [10, 20, 12]
    let bytes: [u8; 2] = [3, 4]
    let sizes: [usize; 2] = [5, 6]
    let flags: [bool; 2] = [false, true]
    let words: [&str; 2] = ["bad", "Nocter"]
    let copied_scores: [i32; 3] = scores
    let copied_words: [&str; 2] = words
    var assigned_scores: [i32; 3] = [0, 0, 0]
    var assigned_bytes: [u8; 2] = [0, 0]
    var assigned_words: [&str; 2] = ["bad", "bad"]
    assigned_scores = copied_scores
    assigned_bytes = [3, 4]
    assigned_words = copied_words
    let score: i32 = assigned_scores[0] + assigned_scores[1] + assigned_scores[2]
    let byte: u8 = assigned_bytes[1]
    let size: usize = sizes[0] + sizes[1]
    let flag: bool = flags[1]
    let word: &str = assigned_words[1]
    if score == 42 {
        if byte == 4 {
            if size == 11 {
                if flag {
                    if word.len() == 6 {
                        return 42
                    }
                }
            }
        }
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
fn run_command_reads_fixed_array_variable_indices() {
    let project = TempProject::new("cli-run-fixed-array-variable-indices");
    let source = project.write_source(
        "fixed_array_variable_indices.nct",
        r#"func main(): i32 {
    let scores: [i32; 3] = [10, 20, 12]
    let bytes: [u8; 2] = [3, 4]
    let sizes: [usize; 2] = [5, 6]
    let flags: [bool; 2] = [false, true]
    let words: [&str; 3] = ["bad", "Nocter", "lang"]
    let a: usize = 0
    let b: usize = 1
    let c: usize = 2
    let score: i32 = scores[a] + scores[b] + scores[c]
    let byte: u8 = bytes[b]
    let size: usize = sizes[a] + sizes[b]
    let flag: bool = flags[b]
    let word: &str = words[b]
    if score == 42 {
        if byte == 4 {
            if size == 11 {
                if flag {
                    if word.len() == 6 {
                        return 42
                    }
                }
            }
        }
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
fn run_command_passes_and_returns_fixed_arrays_by_value() {
    let project = TempProject::new("cli-run-fixed-array-value-parameters-returns");
    let source = project.write_source(
        "fixed_array_value_parameters_returns.nct",
        r#"func main(): i32 {
    let pair: [i32; 2] = [20, 22]
    let copied_pair: [i32; 2] = identity_pair(pair)
    let words: [&str; 3] = ["bad", "Nocter", "lang"]
    let copied_words: [&str; 3] = identity_words(words)
    let word: &str = copied_words[1]
    if sum_pair(copied_pair) == 42 {
        if word.len() == 6 {
            return 42
        }
    }
    return 1
}

func identity_pair(values: [i32; 2]): [i32; 2] {
    return values
}

func sum_pair(values: [i32; 2]): i32 {
    return values[0] + values[1]
}

func identity_words(values: [&str; 3]): [&str; 3] {
    return values
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
fn run_command_passes_fixed_array_literals_by_value() {
    let project = TempProject::new("cli-run-fixed-array-literal-value-arguments");
    let source = project.write_source(
        "fixed_array_literal_value_arguments.nct",
        r#"func main(): i32 {
    return consume([20, 22], ["bad", "Nocter", "lang"], [])
}

func consume(pair: [i32; 2], words: [&str; 3], empty: [u8; 0]): i32 {
    let word: &str = words[1]
    if word.len() == 6 {
        return pair[0] + pair[1]
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
fn run_command_returns_fixed_array_literals() {
    let project = TempProject::new("cli-run-fixed-array-literal-returns");
    let source = project.write_source(
        "fixed_array_literal_returns.nct",
        r#"func main(): i32 {
    let pair: [i32; 2] = make_pair()
    let words: [&str; 3] = make_words()
    let total: i32 = pair[0] + pair[1]
    let word: &str = words[1]
    if total == 42 {
        if word.len() == 6 {
            return 42
        }
    }
    return 1
}

func make_pair(): [i32; 2] {
    return [20, 22]
}

func make_words(): [&str; 3] {
    return ["bad", "Nocter", "lang"]
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
fn run_command_assigns_fixed_array_call_results() {
    let project = TempProject::new("cli-run-fixed-array-call-result-assignments");
    let source = project.write_source(
        "fixed_array_call_result_assignments.nct",
        r#"func main(): i32 {
    var pair: [i32; 2] = [0, 0]
    var words: [&str; 2] = ["bad", "bad"]
    var empty: [u8; 0] = []
    pair = make_pair()
    pair = make_fallible_pair()!
    words = make_words()
    empty = make_empty()
    empty = make_fallible_empty()!
    let total: i32 = pair[0] + pair[1]
    let word: &str = words[1]
    if total == 42 {
        if word.len() == 6 {
            return 42
        }
    }
    return 1
}

func make_pair(): [i32; 2] {
    return [1, 2]
}

func make_fallible_pair(): [i32; 2]! {
    return [20, 22]
}

func make_words(): [&str; 2] {
    return ["lang", "Nocter"]
}

func make_empty(): [u8; 0] {
    return []
}

func make_fallible_empty(): [u8; 0]! {
    return []
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
fn run_command_writes_fixed_array_constant_indices() {
    let project = TempProject::new("cli-run-fixed-array-constant-index-writes");
    let source = project.write_source(
        "fixed_array_constant_index_writes.nct",
        r#"func main(): i32 {
    var scores: [i32; 3] = [0, 0, 0]
    var bytes: [u8; 2] = [0, 0]
    var sizes: [usize; 2] = [0, 0]
    var flags: [bool; 2] = [false, false]
    var words: [&str; 2] = ["bad", "bad"]
    scores[0] = 10
    scores[1] = 20
    scores[2] = 12
    bytes[0] = 3
    bytes[1] = 4
    sizes[0] = 5
    sizes[1] = 6
    flags[1] = true
    words[1] = "Nocter"
    let score: i32 = scores[0] + scores[1] + scores[2]
    let byte: u8 = bytes[1]
    let size: usize = sizes[0] + sizes[1]
    let flag: bool = flags[1]
    let word: &str = words[1]
    if score == 42 {
        if byte == 4 {
            if size == 11 {
                if flag {
                    if word.len() == 6 {
                        return 42
                    }
                }
            }
        }
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
fn run_command_binds_zero_length_fixed_array_literal() {
    let project = TempProject::new("cli-run-zero-length-fixed-array-literal-binding");
    let source = project.write_source(
        "zero_length_fixed_array_literal_binding.nct",
        r#"func main(): i32 {
    let empty: [u8; 0] = []
    return 42
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
fn run_command_copies_and_assigns_zero_length_fixed_arrays() {
    let project = TempProject::new("cli-run-zero-length-fixed-array-copy-assignment");
    let source = project.write_source(
        "zero_length_fixed_array_copy_assignment.nct",
        r#"func main(): i32 {
    var empty: [u8; 0] = []
    let copied: [u8; 0] = empty
    empty = []
    empty = copied
    return 42
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
fn run_command_passes_and_returns_zero_length_fixed_arrays() {
    let project = TempProject::new("cli-run-zero-length-fixed-array-parameters-calls-returns");
    let source = project.write_source(
        "zero_length_fixed_array_parameters_calls_returns.nct",
        r#"func main(): i32 {
    let empty: [u8; 0] = []
    let copied: [u8; 0] = identity(empty)
    let made: [u8; 0] = make_empty()
    return consume(copied, made)
}

func identity(values: [u8; 0]): [u8; 0] {
    return values
}

func make_empty(): [u8; 0] {
    return []
}

func consume(left: [u8; 0], right: [u8; 0]): i32 {
    return 42
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
fn run_command_writes_fixed_array_variable_indices() {
    let project = TempProject::new("cli-run-fixed-array-variable-index-writes");
    let source = project.write_source(
        "fixed_array_variable_index_writes.nct",
        r#"func main(): i32 {
    var scores: [i32; 3] = [0, 0, 0]
    var bytes: [u8; 2] = [0, 0]
    var sizes: [usize; 2] = [0, 0]
    var flags: [bool; 2] = [false, false]
    var words: [&str; 2] = ["bad", "bad"]
    let a: usize = 0
    let b: usize = 1
    let c: usize = 2
    scores[a] = 10
    scores[b] = 20
    scores[c] = 12
    bytes[a] = 3
    bytes[b] = 4
    sizes[a] = 5
    sizes[b] = 6
    flags[b] = true
    words[b] = "Nocter"
    let score: i32 = scores[a] + scores[b] + scores[c]
    let byte: u8 = bytes[b]
    let size: usize = sizes[a] + sizes[b]
    let flag: bool = flags[b]
    let word: &str = words[b]
    if score == 42 {
        if byte == 4 {
            if size == 11 {
                if flag {
                    if word.len() == 6 {
                        return 42
                    }
                }
            }
        }
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
fn run_command_applies_fixed_array_constant_index_compound_assignments() {
    let project = TempProject::new("cli-run-fixed-array-constant-index-compound");
    let source = project.write_source(
        "fixed_array_constant_index_compound.nct",
        r#"func main(): i32 {
    var scores: [i32; 3] = [10, 10, 10]
    var bytes: [u8; 2] = [8, 2]
    var sizes: [usize; 2] = [9, 7]
    scores[0] += 5
    scores[1] *= 2
    scores[2] -= 3
    bytes[0] -= 4
    bytes[1] *= 2
    sizes[0] /= 3
    sizes[1] %= 5
    let score: i32 = scores[0] + scores[1] + scores[2]
    let byte: u8 = bytes[1]
    let size: usize = sizes[0] + sizes[1]
    if score == 42 {
        if byte == 4 {
            if size == 5 {
                return 42
            }
        }
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
fn run_command_applies_fixed_array_variable_index_compound_assignments() {
    let project = TempProject::new("cli-run-fixed-array-variable-index-compound");
    let source = project.write_source(
        "fixed_array_variable_index_compound.nct",
        r#"func main(): i32 {
    var scores: [i32; 3] = [10, 10, 10]
    var bytes: [u8; 2] = [8, 2]
    var sizes: [usize; 2] = [9, 7]
    let a: usize = 0
    let b: usize = 1
    let c: usize = 2
    scores[a] += 5
    scores[b] *= 2
    scores[c] -= 3
    bytes[a] -= 4
    bytes[b] *= 2
    sizes[a] /= 3
    sizes[b] %= 5
    let score: i32 = scores[a] + scores[b] + scores[c]
    let byte: u8 = bytes[b]
    let size: usize = sizes[a] + sizes[b]
    if score == 42 {
        if byte == 4 {
            if size == 5 {
                return 42
            }
        }
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
fn run_command_indexes_fixed_array_aggregate_fields() {
    let project = TempProject::new("cli-run-fixed-array-aggregate-field-indexing");
    let source = project.write_source(
        "fixed_array_aggregate_field_indexing.nct",
        r#"struct Bag {
    values: [i32; 3]
    flags: [bool; 1]
    words: [&str; 2]
}

func main(): i32 {
    var bag = Bag {
        values: [1, 2, 3],
        flags: [false],
        words: ["bad", "bad"]
    }
    let index: usize = 1
    bag.values[0] = 20
    bag.values[index] += 20
    bag.flags[0] = true
    bag.words[index] = "Nocter"
    let total: i32 = bag.values[0] + bag.values[index]
    let flag: bool = bag.flags[0]
    let word: &str = bag.words[index]
    if total == 42 {
        if flag {
            if word.len() == 6 {
                return 42
            }
        }
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
fn run_command_uses_fixed_array_aggregate_field_values() {
    let project = TempProject::new("cli-run-fixed-array-aggregate-field-values");
    let source = project.write_source(
        "fixed_array_aggregate_field_values.nct",
        r#"copy struct Bag {
    values: [i32; 3]
    flags: [bool; 1]
    words: [&str; 2]
}

func main(): i32 {
    var bag = Bag {
        values: [1, 2, 3],
        flags: [true],
        words: ["lang", "Nocter"]
    }
    let copied: [i32; 3] = bag.values
    var assigned: [i32; 3] = [0, 0, 0]
    assigned = bag.values
    let clone = Bag {
        values: bag.values,
        flags: bag.flags,
        words: bag.words
    }
    let made = Bag {
        values: make_values(),
        flags: [true],
        words: make_words()
    }
    let extracted: [i32; 3] = extract_values(clone)
    let word: &str = made.words[1]
    let made_value: i32 = made.values[1]
    if take(bag.values) == 6 {
        if take(copied) == 6 {
            if take(assigned) == 6 {
                if take(extracted) == 6 {
                    if made_value == 8 {
                        if word.len() == 6 {
                            return 42
                        }
                    }
                }
            }
        }
    }
    return 1
}

func take(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}

func extract_values(bag: Bag): [i32; 3] {
    return bag.values
}

func make_values(): [i32; 3] {
    return [7, 8, 9]
}

func make_words(): [&str; 2] {
    return ["lang", "Nocter"]
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
fn run_command_uses_fixed_array_aggregate_field_assignments() {
    let project = TempProject::new("cli-run-fixed-array-aggregate-field-assignments");
    let source = project.write_source(
        "fixed_array_aggregate_field_assignments.nct",
        r#"copy struct Bag {
    values: [i32; 3]
    words: [&str; 2]
}

func main(): i32 {
    var bag = Bag { values: [0, 0, 0], words: ["bad", "bad"] }
    let replacement: [i32; 3] = [4, 5, 6]
    let other = Bag { values: [20, 21, 1], words: ["lang", "Nocter"] }
    bag.values = [1, 2, 3]
    bag.values = replacement
    bag.values = make_values()
    bag.values = make_fallible_values()!
    bag.values = other.values
    bag.words = ["bad", "still"]
    bag.words = other.words
    bag.words = make_words()
    let word: &str = bag.words[1]
    if word.len() == 6 {
        return take(bag.values)
    }
    return 1
}

func take(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}

func make_values(): [i32; 3] {
    return [7, 8, 9]
}

func make_fallible_values(): [i32; 3]! {
    return [10, 11, 12]
}

func make_words(): [&str; 2] {
    return ["lang", "Nocter"]
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
fn run_command_drops_recursive_drop_fixed_array_literal_elements_in_reverse_order() {
    let project = TempProject::new("cli-run-recursive-drop-fixed-array-literal");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "recursive_drop_fixed_array_literal.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func main(): i32 {
    let files: [File; 3] = [
        File { name: "a" },
        File { name: "b" },
        File { name: "c" }
    ]
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"cba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_explicitly_drops_moved_fixed_array_elements_once() {
    let project = TempProject::new("cli-run-explicit-drop-moved-fixed-array-elements");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "explicit_drop_moved_fixed_array_elements.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func main(): i32 {
    let first = File { name: "a" }
    let second = File { name: "b" }
    let files: [File; 2] = [move first, move second]
    drop files
    write("x")!
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"bax");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_replaces_recursive_drop_fixed_array_literal_after_old_cleanup() {
    let project = TempProject::new("cli-run-replace-recursive-drop-fixed-array-literal");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replace_recursive_drop_fixed_array_literal.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func main(): i32 {
    var files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    files = [File { name: "c" }, File { name: "d" }]
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_recursively_drops_fixed_array_call_initialized_elements() {
    let project = TempProject::new("cli-run-recursive-drop-fixed-array-call-elements");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "recursive_drop_fixed_array_call_elements.nct",
        r#"use std/log.write

struct Handle {
    name: &str
}

instance Handle {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Wrapper {
    name: &str
    handle: Handle
}

instance Wrapper {
    drop &+self {
        write(self.name)!
        return
    }
}

func make(name: &str, handle_name: &str): Wrapper {
    return Wrapper { name: name, handle: Handle { name: handle_name } }
}

func make_fallible(name: &str, handle_name: &str): Wrapper! {
    return Wrapper { name: name, handle: Handle { name: handle_name } }
}

func main(): i32 {
    let wrappers: [Wrapper; 2] = [make("A", "a"), make_fallible("B", "b")!]
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"BbAa");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_and_reinitializes_move_only_fixed_array_locals() {
    let project = TempProject::new("cli-run-transfer-reinitialize-move-only-fixed-arrays");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "transfer_reinitialize_move_only_fixed_arrays.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func main(): i32 {
    var first: [File; 2] = [File { name: "a" }, File { name: "b" }]
    let second = move first
    first = [File { name: "c" }, File { name: "d" }]
    var third: [File; 2] = [File { name: "e" }, File { name: "f" }]
    third = move second
    drop third
    third = [File { name: "g" }, File { name: "h" }]
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"febahgdc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_keeps_move_only_fixed_array_cleanup_on_terminal_branch_owner() {
    let project = TempProject::new("cli-run-move-only-fixed-array-terminal-branch-owner");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "move_only_fixed_array_terminal_branch_owner.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func run(move_files: bool): void {
    let files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    if move_files {
        let moved = move files
        return
    }
    return
}

func main(): i32 {
    run(true)
    run(false)
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"baba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_move_only_fixed_arrays_across_return_boundaries() {
    let project = TempProject::new("cli-run-move-only-fixed-array-returns");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "move_only_fixed_array_returns.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func make(first: &str, second: &str): [File; 2] {
    return [File { name: first }, File { name: second }]
}

func main(): i32 {
    var files: [File; 2] = make("a", "b")
    files = make("c", "d")
    make("e", "f")
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"bafedc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_move_only_fixed_arrays_to_owned_parameters() {
    let project = TempProject::new("cli-run-move-only-fixed-array-owned-parameters");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "move_only_fixed_array_owned_parameters.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func consume(files: [File; 2]): void {
    return
}

func forward(files: [File; 2]): void {
    consume(move files)
    return
}

func main(): i32 {
    let files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    consume(move files)
    forward([File { name: "c" }, File { name: "d" }])
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_direct_move_only_fixed_array_call_arguments() {
    let project = TempProject::new("cli-run-direct-move-only-fixed-array-call-argument");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "direct_move_only_fixed_array_call_argument.nct",
        r#"use std/log.write

struct Token {
    tag: u8
}

instance Token {
    drop &+self {
        if self.tag == 1 {
            write("a")!
        } else {
            write("b")!
        }
        return
    }
}

func make(): [Token; 2] {
    return [Token { tag: 1 }, Token { tag: 2 }]
}

func consume(tokens: [Token; 2]): void {
    return
}

func main(): i32 {
    consume(make())
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_successful_fallible_move_only_fixed_array_results() {
    let project = TempProject::new("cli-run-fallible-move-only-fixed-array-success");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "fallible_move_only_fixed_array_success.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func make(): [File; 2]! {
    return [File { name: "a" }, File { name: "b" }]
}

func main(): i32! {
    var files: [File; 2] = make()?
    files = make()?
    make()?
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"bababa");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_move_only_fixed_array_target_on_propagated_failure() {
    let project = TempProject::new("cli-run-fallible-move-only-fixed-array-failure");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "fallible_move_only_fixed_array_failure.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func replace(): void! {
    var files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    files = fail()?
}

func fail(): [File; 2]! {
    return Error.new("app.failed", "failed")
}

func main(): void! {
    replace()?
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ba");
    assert_eq!(output.stderr, b"app.failed: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_forces_direct_fallible_move_only_fixed_array_argument() {
    let project = TempProject::new("cli-run-direct-fallible-move-only-fixed-array-force");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "direct_fallible_move_only_fixed_array_force.nct",
        r#"use std/log.write

struct Token {
    tag: u8
}

instance Token {
    drop &+self {
        if self.tag == 1 {
            write("a")!
        } else {
            write("b")!
        }
        return
    }
}

func make(): [Token; 2]! {
    return [Token { tag: 1 }, Token { tag: 2 }]
}

func consume(tokens: [Token; 2]): void {
    return
}

func main(): i32 {
    consume(make()!)
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_does_not_drop_uninitialized_move_only_fixed_array_catch_binding() {
    let project = TempProject::new("cli-run-move-only-fixed-array-catch-failure");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "move_only_fixed_array_catch_failure.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func fail(): [File; 2]! {
    return Error.new("app.failed", "failed")
}

func main(): i32! {
    let files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    let unused: [File; 2] = fail() catch error {
        write("x")!
        return 0
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"xba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_owns_optional_move_only_fixed_array_success_and_fallback_values() {
    let project = TempProject::new("cli-run-optional-move-only-fixed-array-values");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "optional_move_only_fixed_array_values.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

func maybe_files(present: bool): [File; 2]? {
    if present {
        return [File { name: "a" }, File { name: "b" }]
    }
    return none
}

func main(): i32 {
    let success: [File; 2] = maybe_files(true) otherwise {
        [File { name: "x" }, File { name: "y" }]
    }
    let fallback: [File; 2] = maybe_files(false) otherwise {
        [File { name: "c" }, File { name: "d" }]
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"dcba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_recursively_drops_move_only_fixed_array_struct_fields() {
    let project = TempProject::new("cli-run-move-only-fixed-array-struct-field-drop");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "move_only_fixed_array_struct_field_drop.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

func make(): Bundle {
    let files: [File; 2] = [File { name: "a" }, File { name: "b" }]
    return Bundle { code: 42, files: move files }
}

func consume(bundle: Bundle): void {
    return
}

func main(): i32 {
    consume(make())
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"Xba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_replaces_move_only_fixed_array_struct_fields() {
    let project = TempProject::new("cli-run-replace-move-only-fixed-array-struct-fields");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replace_move_only_fixed_array_struct_fields.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

func make(first: &str, second: &str): [File; 2] {
    return [File { name: first }, File { name: second }]
}

func main(): i32 {
    var bundle = Bundle { code: 42, files: [File { name: "a" }, File { name: "b" }] }
    bundle.files = [File { name: "c" }, File { name: "d" }]
    bundle.files = make("e", "f")
    let replacement: [File; 2] = [File { name: "g" }, File { name: "h" }]
    bundle.files = move replacement
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badcfeXhg");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_replaces_borrowed_move_only_fixed_array_struct_fields() {
    let project = TempProject::new("cli-run-replace-borrowed-move-only-array-field");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replace_borrowed_move_only_array_field.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

func make(): [File; 2]! {
    return [File { name: "c" }, File { name: "d" }]
}

func maybe_files(): [File; 2]? {
    return none
}

func replace(bundle: &+Bundle): void! {
    bundle.files = make()?
    bundle.files = maybe_files() otherwise {
        [File { name: "e" }, File { name: "f" }]
    }
    return
}

func main(): i32! {
    var bundle = Bundle {
        code: 42,
        files: [File { name: "a" }, File { name: "b" }],
    }
    replace(&+bundle)?
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badcXfe");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_move_only_fixed_array_field_on_call_failure() {
    let project = TempProject::new("cli-run-preserve-move-only-array-field-on-failure");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "preserve_move_only_array_field_on_failure.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

func fail(): [File; 2]! {
    return Error.new("app.failed", "failed")
}

func replace(bundle: &+Bundle): void! {
    bundle.files = fail()?
}

func main(): void! {
    var bundle = Bundle {
        code: 42,
        files: [File { name: "a" }, File { name: "b" }],
    }
    replace(&+bundle)?
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"Xba");
    assert_eq!(output.stderr, b"app.failed: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_replaces_direct_move_only_fixed_array_borrowed_fields() {
    let project = TempProject::new("cli-run-replace-direct-move-only-array-field");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replace_direct_move_only_array_field.nct",
        r#"use std/log.write

struct Token {
    tag: u8
}

instance Token {
    drop &+self {
        if self.tag == 1 {
            write("a")!
            return
        }
        if self.tag == 2 {
            write("b")!
            return
        }
        if self.tag == 3 {
            write("c")!
            return
        }
        write("d")!
        return
    }
}

struct Bundle {
    code: i32
    tokens: [Token; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

func replace(bundle: &+Bundle): void {
    bundle.tokens = [Token { tag: 3 }, Token { tag: 4 }]
    return
}

func main(): i32 {
    var bundle = Bundle {
        code: 42,
        tokens: [Token { tag: 1 }, Token { tag: 2 }],
    }
    replace(&+bundle)
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"baXdc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_recursively_drops_replaced_structs_with_move_only_array_fields() {
    let project = TempProject::new("cli-run-replace-struct-with-move-only-array-field");
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replace_struct_with_move_only_array_field.nct",
        r#"use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

struct Bundle {
    files: [File; 2]
}

instance Bundle {
    drop &+self {
        write("X")!
        return
    }
}

struct Container {
    code: i32
    bundle: Bundle
}

func main(): i32 {
    var container = Container {
        code: 42,
        bundle: Bundle { files: [File { name: "a" }, File { name: "b" }] },
    }
    container.bundle = Bundle { files: [File { name: "c" }, File { name: "d" }] }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"XbaXdc");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_partial_payload_in_current_fixed_array_element() {
    let project = TempProject::new("cli-run-current-array-element-payload-drop");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log/index.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "current_array_element_payload_drop.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    name: &str
}

instance File {
    drop &+self {
        write(self.name)!
        return
    }
}

enum Result {
    ok(first: File, second: File)
    failed
}

struct Wrapper {
    result: Result
}

func fail_file(): File! {
    return Error.new("app.failed", "failed")
}

func main(): void! {
    let wrappers: [Wrapper; 2] = [
        Wrapper { result: Result.ok(File { name: "a" }, File { name: "b" }) },
        Wrapper { result: Result.ok(File { name: "c" }, fail_file()?) },
    ]
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"cba");
    assert_eq!(output.stderr, b"app.failed: failed\n");
}
