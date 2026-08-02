use super::*;

#[test]
fn distributed_std_sequence_bound_preserves_the_source_loan() {
    let project = TempProject::new("distributed-home-sequence-bound-loan");
    let source = project.write_source(
        "sequence_bound_loan.nct",
        r#"use std/sequence.first
use std/vec.Vec

copy struct Cell {
    value: i32
}

func read(cell: &Cell): i32 {
    return cell.value
}

func invalid(): i32 {
    var values = Vec [Cell { value: 42 }]
    let cell = first(&values) otherwise { return 0 }
    values.push(Cell { value: 1 })
    return read(cell)
}

func main(): i32 {
    return 0
}
"#,
    );

    for home in [development_root(), distributed_home()] {
        let output = Command::new(NOCTER)
            .args(["check", source.to_str().unwrap()])
            .current_dir(project.root())
            .env("NOCTER_HOME", home)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let stderr = text(&output.stderr);
        assert_eq!(stderr.matches("error[E0434]").count(), 1, "{stderr}");
        assert!(
            stderr.contains("values") && stderr.contains("cell"),
            "{stderr}"
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_sequence_bound_dispatch_runs_from_packaged_home() {
    let project = TempProject::new("distributed-home-sequence-bound-run");
    let source = project.write_source(
        "sequence_bound_run.nct",
        r#"use std/sequence.first
use std/vec.Vec

copy struct Cell {
    value: i32
}

func read(cell: &Cell): i32 {
    return cell.value
}

func main(): i32 {
    let values = Vec [Cell { value: 42 }, Cell { value: 1 }]
    let cell = first(&values) otherwise { return 1 }
    return read(cell)
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
