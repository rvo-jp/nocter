use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn associated_type_projection_builds_and_runs_after_generic_specialization() {
    let project = TempProject::new("cli-run-associated-type-projection");
    let source = project.write_source(
        "index.nct",
        r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}

copy struct Number {
    value: i32
}

conform Source for Number {
    type Item = i32

    method &self.get(): i32 {
        return self.value
    }
}

func read<S>(source: &S): S.Item where S: Source {
    return source.get()
}

func main(): i32 {
    let number = Number { value: 42 }
    return read(&number)
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
fn opaque_result_builds_and_dispatches_through_its_interface() {
    let project = TempProject::new("cli-run-opaque-result");
    let source = project.write_source(
        "index.nct",
        r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}

struct Number {
    value: i32
}

conform Source for Number {
    type Item = i32

    method &self.get(): i32 {
        return self.value
    }
}

func make(): some Source<Item = i32> {
    return Number { value: 42 }
}

func main(): i32 {
    let source = make()
    return source.get()
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
fn generic_optional_opaque_result_builds_and_runs() {
    let project = TempProject::new("cli-run-generic-optional-opaque-result");
    let source = project.write_source(
        "index.nct",
        r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}

struct Box<T> { value: T }

conform Source for Box<T> {
    type Item = T
    method &self.get(): T { return self.value }
}

func make<T>(value: T, present: bool): some Source<Item = T>? {
    if !present { return none }
    return Box<T> { value: value }
}

func main(): i32 {
    let source = make(42, true) otherwise { return 1 }
    return source.get()
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
