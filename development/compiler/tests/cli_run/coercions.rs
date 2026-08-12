use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generic_coercion_requirement_specializes_to_the_concrete_instance_body() {
    let project = TempProject::new("cli-run-generic-coercion-requirement");
    let source = project.write_source(
        "index.nct",
        r#"struct Payload { code: i32 }
struct Box { selected: Payload }

instance Box {
    pub coerce &self as &Payload from self { return &self.selected }
}

func project<T>(value: &T): &Payload from value where &T as &Payload {
    return value
}

func read_code(value: &Payload): i32 { return value.code }

func main(): i32 {
    let box = Box { selected: Payload { code: 47 } }
    let projected = project(&box)
    return read_code(projected)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(47),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn explicit_generic_coercion_invokes_the_selected_body_once() {
    let project = TempProject::new("cli-run-explicit-generic-coercion-once");
    let source = project.write_source(
        "index.nct",
        r#"copy struct Payload {
    code: i32
}

struct Box<T> {
    selected: Payload
    marker: T
}

struct Counter {
    calls: i32
}

instance Box<T> {
    pub coerce &self as &Payload from self {
        return &self.selected
    }
}

func counted<T>(value: &Box<T>, counter: &+Counter): &Box<T> from value {
    counter.calls = counter.calls + 1
    return value
}

func read_code(value: &Payload): i32 {
    return value.code
}

func main(): i32 {
    let box = Box<i32> { selected: Payload { code: 42 }, marker: 7 }
    var counter = Counter { calls: 0 }
    let view = counted(&box, &+counter) as &Payload
    if read_code(view) != 42 { return 1 }
    if box.marker != 7 { return 2 }
    if counter.calls != 1 { return 3 }
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn contextual_coercion_lowers_each_borrow_valued_control_flow_branch() {
    let project = TempProject::new("cli-run-contextual-coercion-control-flow");
    let source = project.write_source(
        "index.nct",
        r#"struct Box {
    selected: i32
}

instance Box {
    pub coerce &self as &i32 from self {
        return &self.selected
    }
}

func choose(use_first: bool, first: &Box, second: &Box): &i32 from first | second {
    return if use_first { first } else { second }
}

func consume(value: &i32): i32 {
    return 42
}

func main(): i32 {
    let first = Box { selected: 41 }
    let second = Box { selected: 42 }
    let selected = choose(false, &first, &second)
    return consume(selected)
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
fn explicit_coercion_lowers_optional_projection_once() {
    let project = TempProject::new("cli-run-explicit-coercion-projection");
    let source = project.write_source(
        "index.nct",
        r#"copy struct Payload {
    code: i32
}

struct Box {
    selected: Payload
}

instance Box {
    pub coerce &self as &Payload from self {
        return &self.selected
    }
}

func maybe(value: &Box): &Box? from value {
    return value
}

func project(value: &Box): &Payload? from value {
    return maybe(value)? as &Payload
}

func read_code(value: &Payload): i32 {
    return value.code
}

func main(): i32 {
    let box = Box { selected: Payload { code: 43 } }
    let selected = project(&box)!
    return read_code(selected)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(43),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
