use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_home_runs_first_class_outcome_round_trip() {
    let project = TempProject::new("distributed-home-first-class-outcomes");
    let source = project.write_source(
        "outcome_values.nct",
        r#"struct Holder {
    value: i32?
}

func main(): i32 {
    let initial = maybe()
    let forwarded = forward(initial)
    let holder = Holder { value: forwarded }
    let extracted = holder.value
    return extracted otherwise { 1 }
}

func maybe(): i32? {
    return 42
}

func forward(value: i32?): i32? {
    return value
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
