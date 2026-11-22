use super::{NoRemoteAcquisition, unique_test_directory, write_source};
use crate::{
    BuildCommandResult, CommandToolchain, ParsedCommand, execute_prepared_build,
    execute_prepared_check, parse_command_arguments,
};
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_diagnostics::DiagnosticCode;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::StandardPackage;
use std::{fs, path::PathBuf};

struct Fixture {
    directory: PathBuf,
    toolchain: CommandToolchain,
}

impl Fixture {
    fn new() -> Self {
        let directory = unique_test_directory("v2-commands");
        let app = directory.join("app");
        write_source(
            &app,
            "index.nct",
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\" }\n",
                "#executable: { name: \"first\" }\n",
                "#executable: { name: \"second\", module: \"./nested\" }\n",
                "see ./body.nct\nfunc main(): i32!\n",
            ),
        );
        write_source(
            &app,
            "body.nct",
            "see ./index.nct\nfunc main(): i32! { 42 }\n",
        );
        write_source(
            &app,
            "nested/index.nct",
            "see ./body.nct\nfunc main(): i32\n",
        );
        write_source(
            &app,
            "nested/body.nct",
            "see ./index.nct\nfunc main(): i32 { 17 }\n",
        );
        let standard = directory.join("std");
        write_source(
            &standard,
            "index.nct",
            "#package: { name: \"std\", version: \"2.0.0\" }\n",
        );
        let identity = PackageIdentity::new("test:standard-v2");
        let toolchain = CommandToolchain::new(
            CompilationTarget::Arm64Darwin,
            &directory,
            StandardPackage::new(identity.clone(), standard, "2.0.0"),
        )
        .with_value_access_profile(ToolchainInput::new(
            identity.clone(),
            ModuleIdentity::new(identity, Vec::<&str>::new()),
            Vec::new(),
            Vec::new(),
        ))
        .unwrap();
        Self {
            directory,
            toolchain,
        }
    }

    fn build(
        &self,
        options: &[&str],
    ) -> Result<BuildCommandResult, crate::BuildCommandExecutionError> {
        let ParsedCommand::Build(parsed) = parse("build", options) else {
            panic!("build command")
        };
        execute_prepared_build(
            parsed.prepare(&self.directory).unwrap(),
            &self.toolchain,
            &mut NoRemoteAcquisition,
        )
    }
}

fn parse(verb: &str, options: &[&str]) -> ParsedCommand {
    parse_command_arguments(
        [verb, "--root", "app"]
            .into_iter()
            .chain(options.iter().copied())
            .map(Into::into),
    )
    .unwrap()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn prepared_v2_builds_share_selected_and_package_output_planning() {
    let fixture = Fixture::new();
    let BuildCommandResult::Selected(first) = fixture.build(&["--executable", "first"]).unwrap()
    else {
        panic!("selected executable")
    };
    let app = fs::canonicalize(fixture.directory.join("app")).unwrap();
    assert_eq!(first.artifact().path(), app.join("first"));
    assert!(!app.join("second").exists());
    let first_bytes = fs::read(first.artifact().path()).unwrap();
    let BuildCommandResult::PackageSet(all) = fixture.build(&[]).unwrap() else {
        panic!("package executable set")
    };
    assert_eq!(
        all.entries()
            .iter()
            .map(|entry| entry.identity().name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(fs::read(app.join("first")).unwrap(), first_bytes);
    assert!(
        fs::read(app.join("second"))
            .unwrap()
            .starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
    );
}

#[test]
fn prepared_v2_check_preserves_source_errors_and_entry_admission_is_separate() {
    let fixture = Fixture::new();
    let app = fixture.directory.join("app");
    let check = |options: &[&str]| {
        let ParsedCommand::Check(parsed) = parse("check", options) else {
            panic!("check command")
        };
        execute_prepared_check(
            parsed.prepare(&fixture.directory).unwrap(),
            &fixture.toolchain,
            &mut NoRemoteAcquisition,
        )
    };
    let second = &["--executable", "second"];
    check(second).unwrap();
    assert_eq!(
        check(&["--executable", "second", "--target", "arm64-linux"])
            .unwrap_err()
            .diagnostic_code(),
        Some(DiagnosticCode::E0701)
    );
    fs::write(
        app.join("nested/index.nct"),
        "see ./body.nct\nfunc other(): i32\n",
    )
    .unwrap();
    fs::write(
        app.join("nested/body.nct"),
        "see ./index.nct\nfunc other(): i32 { 17 }\n",
    )
    .unwrap();
    let check_failure = check(second).unwrap_err();
    let set_failure = fixture.build(&[]).unwrap_err();
    let selected_failure = fixture.build(second).unwrap_err();
    for (diagnostics, sources) in [
        check_failure.source_diagnostics().unwrap(),
        set_failure.source_diagnostics().unwrap(),
        selected_failure.source_diagnostics().unwrap(),
    ] {
        let [diagnostic] = diagnostics else {
            panic!("one entry diagnostic")
        };
        assert_eq!(diagnostic.code(), DiagnosticCode::E0800.as_str());
        let span = diagnostic.primary().span();
        let file = sources.get(span.source()).unwrap();
        assert!(file.name().as_str().ends_with("/app/index.nct"));
        assert_eq!(file.text_at(span.range()), Some("\"second\""));
    }
    assert!(!app.join("first").exists());
    assert!(!app.join("second").exists());

    fs::write(
        app.join("nested/body.nct"),
        "see ./index.nct\nfunc other(): i32 { missing }\n",
    )
    .unwrap();
    let error = check(second).unwrap_err();
    let (diagnostics, _) = error.source_diagnostics().unwrap();
    assert!(!diagnostics.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn prepared_v2_run_launches_the_selected_native_entry() {
    let fixture = Fixture::new();
    let ParsedCommand::Run(parsed) = parse("run", &["--executable", "second"]) else {
        panic!("run command")
    };
    let executed = crate::execute_prepared_run(
        parsed.prepare(&fixture.directory).unwrap(),
        &fixture.toolchain,
        &mut NoRemoteAcquisition,
    )
    .unwrap();
    assert_eq!(executed.status().code(), Some(17));
    assert!(!fixture.directory.join("app/second").exists());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn prepared_v2_single_file_runs_its_own_main_and_allows_seen_helpers() {
    let fixture = Fixture::new();
    let file = fixture.directory.join("entry.nct");
    fs::write(&file, "see ./helper.nct\nfunc main(): i32! { answer() }\n").unwrap();
    fs::write(
        fixture.directory.join("helper.nct"),
        "func answer(): i32 { 38 }\n",
    )
    .unwrap();
    let ParsedCommand::Build(command) =
        parse_command_arguments(["build".into(), file.as_os_str().to_owned()]).unwrap()
    else {
        panic!("build command")
    };
    let BuildCommandResult::Selected(result) = execute_prepared_build(
        command.prepare(&fixture.directory).unwrap(),
        &fixture.toolchain,
        &mut NoRemoteAcquisition,
    )
    .unwrap() else {
        panic!("single-file artifact")
    };
    assert_eq!(
        result.artifact().path(),
        file.canonicalize().unwrap().with_extension("")
    );
    let run = || {
        let ParsedCommand::Run(parsed) =
            parse_command_arguments(["run".into(), file.as_os_str().to_owned()]).unwrap()
        else {
            panic!("run command")
        };
        crate::execute_prepared_run(
            parsed.prepare(&fixture.directory).unwrap(),
            &fixture.toolchain,
            &mut NoRemoteAcquisition,
        )
    };
    assert_eq!(run().unwrap().status().code(), Some(38));

    fs::write(
        &file,
        "see ./helper.nct\nfunc check_seen(): i32 { main() }\n",
    )
    .unwrap();
    fs::write(
        fixture.directory.join("helper.nct"),
        "func main(): i32 { 38 }\n",
    )
    .unwrap();
    let failure = run().unwrap_err();
    let (diagnostics, sources) = failure.source_diagnostics().unwrap();
    let [diagnostic] = diagnostics else {
        panic!("one entry diagnostic")
    };
    assert_eq!(diagnostic.code(), DiagnosticCode::E0800.as_str());
    let span = diagnostic.primary().span();
    let source = sources.get(span.source()).unwrap();
    assert_eq!(
        source.name().as_str(),
        file.canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(source.text_at(span.range()), Some(""));
    assert!(
        failure
            .to_string()
            .contains("requires main in the selected source file"),
        "{failure}"
    );
}
