use super::*;

#[test]
fn distributed_compiler_initializes_and_runs_a_fresh_package() {
    let project = TempProject::new("distributed-home-init-package");
    let initialized = Command::new(NOCTER)
        .args(["init", "--name", "fresh-app"])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();
    assert_eq!(
        initialized.status.code(),
        Some(0),
        "{}",
        text(&initialized.stderr)
    );

    for arguments in [["check"].as_slice(), ["test"].as_slice()] {
        let output = Command::new(NOCTER)
            .args(arguments)
            .current_dir(project.root())
            .env("NOCTER_HOME", distributed_home())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }

    let run = Command::new(NOCTER)
        .arg("run")
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "{}", text(&run.stderr));
    assert_eq!(run.stdout, b"Hello from fresh-app\n");
}

#[test]
fn distributed_compiler_graph_json_describes_a_fresh_package() {
    let project = TempProject::new("distributed-home-init-graph");
    let initialized = Command::new(NOCTER)
        .args(["init", "--name", "fresh-lib", "--library"])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();
    assert_eq!(initialized.status.code(), Some(0));

    let graph = Command::new(NOCTER)
        .args(["graph", "--locked", "--offline", "--format", "json"])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();
    assert_eq!(graph.status.code(), Some(0), "{}", text(&graph.stderr));
    let value: Value = serde_json::from_slice(&graph.stdout).unwrap();
    assert_eq!(value["format"], 1);
    let packages = value["packages"].as_array().expect("packages array");
    assert!(
        packages
            .iter()
            .any(|package| package["name"] == "fresh-lib")
    );
    assert!(packages.iter().any(|package| package["name"] == "std"));
}
