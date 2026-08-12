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
fn source_strict_order_drives_every_derived_comparison() {
    run_source(
        "source-strict-order",
        r#"struct Rank { value: i32 }

instance Rank {
    operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}

func main(): i32 {
    let low = Rank { value: 4 }
    let same = Rank { value: 4 }
    let high = Rank { value: 9 }
    if !(low < high) { return 1 }
    if !(high > low) { return 2 }
    if !(low <= same) { return 3 }
    if !(same >= low) { return 4 }
    if high <= low { return 5 }
    if low >= high { return 6 }
    return 0
}
"#,
    );
}

#[test]
fn reversed_strict_order_preserves_source_evaluation_order() {
    run_source(
        "strict-order-evaluation-order",
        r#"struct Rank { value: i32 }
struct Counter { value: i32 }

instance Rank {
    operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}

func observed(sequence: &+Counter, marker: i32, value: i32): Rank {
    sequence.value = sequence.value * 10 + marker
    return Rank { value: value }
}

func main(): i32 {
    var sequence = Counter { value: 0 }
    if observed(&+sequence, 1, 9) > observed(&+sequence, 2, 4) {
    } else {
        return 1
    }
    if sequence.value != 12 { return 2 }
    return 0
}
"#,
    );
}

#[test]
fn generic_strict_order_specializes_through_readonly_coercion() {
    run_source(
        "generic-coerced-strict-order",
        r#"struct View { value: i32 }

instance View {
    operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}

struct Owner { view: View }

instance Owner {
    coerce &self as &View { return &self.view }
}

func less<T>(left: &T, right: &T): bool where (&T < &T): bool {
    return left < right
}

func main(): i32 {
    let low = Owner { view: View { value: 2 } }
    let high = Owner { view: View { value: 8 } }
    if !less(&low, &high) { return 1 }
    let low_ref = &low
    let high_ref = &high
    if !(high_ref > low_ref) { return 2 }
    return 0
}
"#,
    );
}

#[test]
fn imported_public_strict_order_executes_by_declaration_identity() {
    let project = TempProject::new("imported-strict-order");
    project.write_source(
        "rank/index.nct",
        r#"pub struct Rank { pub value: i32 }
instance Rank {
    pub operator (&self < other: &Self): bool { return self.value < other.value }
}
"#,
    );
    project.write_source(
        "main.nct",
        r#"use ./rank.Rank
func main(): i32 {
    let low = Rank { value: 1 }
    let high = Rank { value: 9 }
    if high > low { return 0 }
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
fn imported_private_strict_order_is_unavailable() {
    let project = TempProject::new("private-imported-strict-order");
    project.write_source(
        "rank/index.nct",
        r#"pub struct Rank { pub value: i32 }
instance Rank {
    operator (&self < other: &Self): bool { return self.value < other.value }
}
"#,
    );
    project.write_source(
        "main.nct",
        r#"use ./rank.Rank
func main(): i32 {
    let low = Rank { value: 1 }
    let high = Rank { value: 9 }
    if low < high { return 0 }
    return 1
}
"#,
    );

    let output = nocter(&project, ["check", "main.nct"]);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0348]"), "{stderr}");
    assert!(stderr.contains("strict ordering"), "{stderr}");
}
