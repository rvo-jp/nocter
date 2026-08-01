use super::*;

#[test]
fn build_command_lowers_generic_fixed_array_literal_value_argument() {
    let project = TempProject::new("cli-build-generic-fixed-array-literal-value-argument");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_generic_fixed_array_aggregate_fields() {
    let project = TempProject::new("cli-build-generic-fixed-array-aggregate-fields");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_variable_index_reads() {
    let project = TempProject::new("cli-build-fixed-array-variable-index-reads");
    let source = project.write_source(
        "fixed_array_variable_index_reads.nct",
        r#"func main(): i32 {
    let values: [i32; 2] = [1, 2]
    let index: usize = 1
    return values[index]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_value_branch_fixed_array_literal() {
    let project = TempProject::new("cli-build-value-branch-fixed-array-literal");
    let source = project.write_source(
        "value_branch_fixed_array_literal.nct",
        r#"func main(): i32 {
    let answer = if true {
        let values: [i32; 1] = [42]
        values[0]
    } else {
        1
    }
    return answer
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_copy_binding() {
    let project = TempProject::new("cli-build-fixed-array-copy-binding");
    let source = project.write_source(
        "fixed_array_copy_binding.nct",
        r#"func main(): i32 {
    let values: [i32; 2] = [1, 2]
    let copy: [i32; 2] = values
    return copy[0]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_value_parameters_and_returns() {
    let project = TempProject::new("cli-build-fixed-array-value-parameters-returns");
    let source = project.write_source(
        "fixed_array_value_parameters_returns.nct",
        r#"func main(): i32 {
    let values: [i32; 3] = [10, 20, 12]
    let copied: [i32; 3] = identity(values)
    return copied[0]
}

func identity(values: [i32; 3]): [i32; 3] {
    return values
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_literal_value_arguments() {
    let project = TempProject::new("cli-build-fixed-array-literal-value-arguments");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_literal_returns() {
    let project = TempProject::new("cli-build-fixed-array-literal-returns");
    let source = project.write_source(
        "fixed_array_literal_returns.nct",
        r#"func main(): i32 {
    let values: [i32; 2] = make_pair()
    return values[0]
}

func make_pair(): [i32; 2] {
    return [42, 1]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_whole_assignments() {
    let project = TempProject::new("cli-build-fixed-array-whole-assignments");
    let source = project.write_source(
        "fixed_array_whole_assignments.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    let replacement: [i32; 2] = [3, 4]
    values = [5, 6]
    values = replacement
    values = make_pair()
    values = make_fallible_pair()!
    return values[0]
}

func make_pair(): [i32; 2] {
    return [7, 8]
}

func make_fallible_pair(): [i32; 2]! {
    return [9, 10]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_constant_index_assignment() {
    let project = TempProject::new("cli-build-fixed-array-constant-index-assignment");
    let source = project.write_source(
        "fixed_array_constant_index_assignment.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    values[0] = 7
    return values[0]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_zero_length_fixed_array_literal_binding() {
    let project = TempProject::new("cli-build-zero-length-fixed-array-literal-binding");
    let source = project.write_source(
        "zero_length_fixed_array_literal_binding.nct",
        r#"func main(): i32 {
    let empty: [u8; 0] = []
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_zero_length_fixed_array_copy_and_assignment() {
    let project = TempProject::new("cli-build-zero-length-fixed-array-copy-assignment");
    let source = project.write_source(
        "zero_length_fixed_array_copy_assignment.nct",
        r#"func main(): i32 {
    var empty: [u8; 0] = []
    let copied: [u8; 0] = empty
    empty = []
    empty = copied
    empty = make_empty()
    empty = make_fallible_empty()!
    return 0
}

func make_empty(): [u8; 0] {
    return []
}

func make_fallible_empty(): [u8; 0]! {
    return []
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_zero_length_fixed_array_parameters_calls_and_returns() {
    let project = TempProject::new("cli-build-zero-length-fixed-array-parameters-calls-returns");
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
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_constant_index_compound_assignment() {
    let project = TempProject::new("cli-build-fixed-array-constant-index-compound-assignment");
    let source = project.write_source(
        "fixed_array_constant_index_compound_assignment.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    var bytes: [u8; 1] = [5]
    var sizes: [usize; 1] = [9]
    values[0] += 6
    bytes[0] -= 1
    sizes[0] %= 5
    return values[0]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_variable_index_assignment() {
    let project = TempProject::new("cli-build-fixed-array-variable-index-assignment");
    let source = project.write_source(
        "fixed_array_variable_index_assignment.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    let index: usize = 0
    values[index] = 7
    return values[0]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_variable_index_compound_assignment() {
    let project = TempProject::new("cli-build-fixed-array-variable-index-compound-assignment");
    let source = project.write_source(
        "fixed_array_variable_index_compound_assignment.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    let index: usize = 0
    values[index] += 7
    return values[0]
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_aggregate_field_indexing() {
    let project = TempProject::new("cli-build-fixed-array-aggregate-field-indexing");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_aggregate_field_values() {
    let project = TempProject::new("cli-build-fixed-array-aggregate-field-values");
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
    return take(bag.values) + take(copied) + take(assigned) + take(extracted) + made.values[1]
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_aggregate_field_assignments() {
    let project = TempProject::new("cli-build-fixed-array-aggregate-field-assignments");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_tracks_partial_move_only_array_struct_fields() {
    let project = TempProject::new("cli-build-partial-move-only-array-struct-fields");
    let source = project.write_source(
        "partial_move_only_array_struct_fields.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

func make_file(): File! {
    return File { fd: 2 }
}

func main(): i32! {
    let bundle = Bundle {
        code: 42,
        files: [File { fd: 1 }, make_file()?]
    }
    return bundle.code
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}
