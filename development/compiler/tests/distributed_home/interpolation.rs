use super::*;

#[test]
fn distributed_std_owned_interpolation_checks() {
    let project = TempProject::new("distributed-home-interpolation-check");
    let source = project.write_source(
        "interpolation_check.nct",
        r#"func render(value: usize): String {
    return "value ${value}"
}

func main(): i32 {
    let bare: &str = "static"
    let owned: String = "${bare} ${true}"
    return 0
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_formats_decoded_text_values_and_owned_strings_in_order() {
    let project = TempProject::new("distributed-home-interpolation-runtime");
    let source = project.write_source(
        "interpolation_runtime.nct",
        r#"use std/io.print

func marked(label: &str, value: i32): i32! {
    print(label)?
    return value
}

func temporary(): String {
    return "temporary ${7}"
}

func main(): i32! {
    let existing = String "owned"
    let byte: u8 = 255
    let word: usize = 18446744073709551615
    let text = """
        escaped \"line\"\n${marked("A", -2147483648)?}/${marked("B", 0)?}/${marked("C", 2147483647)?}
        ${byte}/${word}/${false}/${existing}/${temporary()}
        """
    print(text.view())?
    if existing.view() != "owned" {
        return 1
    }
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(
        text(&output.stdout),
        "ABCescaped \"line\"\n-2147483648/0/2147483647\n255/18446744073709551615/false/owned/temporary 7"
    );
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_is_an_ordinary_owned_aggregate_value() {
    let project = TempProject::new("distributed-home-interpolation-contexts");
    let source = project.write_source(
        "interpolation_contexts.nct",
        r#"struct Holder {
    text: String
}

func consume(text: String): i32 {
    if text.view() != "argument 2" {
        return 1
    }
    return 0
}

func rendered(): String {
    return "return ${3}"
}

func main(): i32 {
    var assigned = String "initial"
    assigned = "assigned ${1}"
    if assigned.view() != "assigned 1" {
        return 2
    }

    let holder = Holder { text: "field ${4}" }
    if holder.text.view() != "field 4" {
        return 3
    }
    if rendered().view() != "return 3" {
        return 4
    }
    return consume("argument ${2}")
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_interpolation_uses_and_releases_lexical_region_context() {
    let project = TempProject::new("distributed-home-region-interpolation");
    let source = project.write_source(
        "region_interpolation.nct",
        r#"use std/mem.page_allocator

func main(): i32 {
    let arena = page_allocator()
    region temporary using arena {
        let text = "region ${42}"
        if text.view() != "region 42" {
            return 1
        }
    }
    return 0
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
