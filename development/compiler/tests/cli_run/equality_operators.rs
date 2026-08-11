use super::*;

fn run_source(name: &str, source: &str) {
    let project = TempProject::new(name);
    project.write_source("main.nct", source);
    let output = nocter(&project, ["run", "main.nct"]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn instance_equality_operator_executes_as_a_static_borrowing_call() {
    run_source(
        "instance-equality-operator",
        r#"struct Text {
    value: i32,
}

instance Text {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

func same(left: &Text, right: &Text): bool {
    return left == right
}

func main(): i32 {
    let left = Text { value: 7 }
    let right = Text { value: 7 }
    if same(&left, &right) {
        return 0
    }
    return 1
}
"#,
    );
}

#[test]
fn instance_equality_calls_compose_with_short_circuit_logic() {
    run_source(
        "instance-equality-short-circuit",
        r#"struct Point {
    value: i32,
}

instance Point {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

func main(): i32 {
    let first = Point { value: 7 }
    let same = Point { value: 7 }
    let other = Point { value: 9 }
    if first != same || first == other { return 1 }
    if first == same && first != other { return 0 }
    return 2
}
"#,
    );
}

#[test]
fn equality_uses_one_readonly_coercion_per_operand() {
    run_source(
        "coerced-equality-operator",
        r#"struct TextView {
    value: i32,
}

instance TextView {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

struct Text {
    view: TextView,
}

coerce Text {
    &self as &TextView {
        return &self.view
    }
}

func text_text(left: &Text, right: &Text): bool {
    return left == right
}

func text_view(left: &Text, right: &TextView): bool {
    return left == right
}

func view_text(left: &TextView, right: &Text): bool {
    return left == right
}

func view_view(left: &TextView, right: &TextView): bool {
    return left == right
}

func main(): i32 {
    let left = Text { view: TextView { value: 7 } }
    let right = Text { view: TextView { value: 7 } }
    let view = TextView { value: 7 }
    if text_text(&left, &right) {
    } else {
        return 1
    }
    if text_view(&left, &view) {
    } else {
        return 2
    }
    if view_text(&view, &right) {
    } else {
        return 3
    }
    if view_view(&view, &view) {
    } else {
        return 4
    }
    return 0
}
"#,
    );
}

#[test]
fn generic_equality_requirement_specializes_direct_and_coerced_operators() {
    run_source(
        "generic-equality-operator",
        r#"struct View {
    value: i32,
}

instance View {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

struct Owner {
    view: View,
}

coerce Owner {
    &self as &View {
        return &self.view
    }
}

func equal<T>(left: &T, right: &T): bool where (&T == &T): bool {
    return left == right
}

func main(): i32 {
    let left = Owner { view: View { value: 9 } }
    let right = Owner { view: View { value: 9 } }
    if !equal(&left, &right) { return 1 }
    let first_number = 9
    let second_number = 9
    if !equal(&first_number, &second_number) { return 2 }
    return 0
}
"#,
    );
}

#[test]
fn imported_public_equality_operator_executes_by_declaration_identity() {
    let project = TempProject::new("imported-equality-operator");
    project.write_source(
        "text/index.nct",
        r#"pub struct Text {
    pub value: i32,
}

instance Text {
    pub operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}
"#,
    );
    project.write_source(
        "main.nct",
        r#"use ./text.Text

func main(): i32 {
    let left = Text { value: 21 }
    let right = Text { value: 21 }
    if left == right { return 0 }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", "main.nct"]);
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn imported_private_equality_operator_is_unavailable() {
    let project = TempProject::new("private-imported-equality-operator");
    project.write_source(
        "text/index.nct",
        r#"pub struct Text {
    pub value: i32,
}

instance Text {
    operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}
"#,
    );
    project.write_source(
        "main.nct",
        r#"use ./text.Text

func main(): i32 {
    let left = Text { value: 21 }
    let right = Text { value: 21 }
    if left == right { return 0 }
    return 1
}
"#,
    );

    let output = nocter(&project, ["check", "main.nct"]);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0347]"), "{stderr}");
    assert!(
        stderr.contains("no accessible equality operation"),
        "{stderr}"
    );
}
