use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reads_process_environment_count() {
    let project = TempProject::new("cli-run-process-environment-count");
    project.write_nocter_home_file(
        "std/process.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive env_count_raw(): usize

pub func environment_count(): usize {
    return env_count_raw()
}

"#,
    );
    let source = project.write_source(
        "process_environment_count.nct",
        r#"use std/process.environment_count

func main(): i32 {
    if environment_count() == 0 { return 1 }
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
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reads_indexed_process_environment_views() {
    let project = TempProject::new("cli-run-process-environment-views");
    project.write_nocter_home_file(
        "std/process.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive env_count_raw(): usize
#target: "arm64-darwin"
pub(nocter) primitive env_name_raw(index: usize): &str
#target: "arm64-darwin"
pub(nocter) primitive env_value_raw(index: usize): &str

pub func environment_probe(name: &str, value: &str): i32 {
    let count = env_count_raw()
    var index: usize = 0
    while index < count {
        let entry_name = env_name_raw(index)
        let entry_value = env_value_raw(index)
        if entry_name == name {
            if entry_value == value { return 0 }
            return 2
        }
        index = index + 1
    }
    return 1
}
"#,
    );
    let source = project.write_source(
        "process_environment_views.nct",
        r#"use std/process.environment_probe

func main(): i32 {
    return environment_probe("NOCTER_PHASE5_RAW", "raw-value")
}
"#,
    );

    let output = Command::new(NOCTER)
        .args(["run", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .env("NOCTER_PHASE5_RAW", "raw-value")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
