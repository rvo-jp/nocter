use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::Compression;
use flate2::write::GzEncoder;
use nocter_content_integrity::{TreeHashOptions, sha256_file, sha256_regular_tree};
use nocter_diagnostics::DiagnosticCode;
use nocter_package_acquisition::PackageAcquisitionError;

use super::*;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(label: &str) -> Self {
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-cli-{label}-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
    }

    fn installation(&self, host: &str, complete_standard: bool) -> PathBuf {
        let root = self.0.join("home");
        fs::create_dir(&root).unwrap();
        let standard = root.join("std");
        if complete_standard {
            copy_directory(&nocter_test_support::standard_library_root(), &standard);
            let root_source = standard.join("index.nct");
            let source = fs::read_to_string(&root_source).unwrap().replace(
                &format!(
                    "version: \"{}\"",
                    nocter_test_support::repository_release_version()
                ),
                "version: \"0.14.0\"",
            );
            fs::write(root_source, source).unwrap();
        } else {
            fs::create_dir(&standard).unwrap();
            fs::write(
                standard.join("index.nct"),
                "#package: { name: \"std\", version: \"0.14.0\", }\n",
            )
            .unwrap();
        }
        write_installation_metadata(&root, host, "0.14.0", false);
        root
    }

    fn value_access_installation(&self) -> PathBuf {
        let root = self.0.join("home");
        fs::create_dir(&root).unwrap();
        copy_directory(
            &nocter_test_support::repository_root().join("development/stdlib"),
            &root.join("std"),
        );
        write_installation_metadata(&root, "arm64-darwin", "2.0.0", true);
        root
    }
}

fn write_installation_metadata(root: &Path, host: &str, release: &str, value_access: bool) {
    fs::write(root.join("nocter"), "compiler").unwrap();
    fs::write(root.join("LICENSE"), "license").unwrap();
    fs::write(root.join("NOTICE"), "notice").unwrap();
    fs::write(root.join("VERSION"), format!("{release}\n")).unwrap();
    let compiler_digest = sha256_file(&root.join("nocter")).unwrap();
    let standard_digest =
        sha256_regular_tree(&root.join("std"), TreeHashOptions::complete()).unwrap();
    fs::write(
        root.join("MANIFEST.json"),
        manifest(
            host,
            release,
            value_access,
            compiler_digest,
            standard_digest,
        ),
    )
    .unwrap();
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn manifest(
    host: &str,
    release: &str,
    value_access: bool,
    compiler_digest: nocter_content_integrity::ContentDigest,
    standard_digest: nocter_content_integrity::ContentDigest,
) -> String {
    let profile_fields = if value_access {
        r#""schema_version": 3, "source_profile": "value-access-v2","#
    } else {
        r#""schema_version": 2,"#
    };
    format!(
        r#"{{
            "schema": "nocter.manifest",
            {profile_fields}
            "release": "{release}",
            "host": "{host}",
            "default_target": "arm64-darwin",
            "compiler": {{
                "path": "nocter",
                "sha256": "{compiler_digest}"
            }},
            "std": {{
                "path": "std",
                "tree_sha256": "{standard_digest}"
            }},
            "license": {{
                "id": "Apache-2.0",
                "path": "LICENSE",
                "notice": "NOTICE"
            }},
            "implemented_targets": [{{
                "name": "arm64-darwin",
                "backend": "arm64",
                "executable": "macho",
                "os": "darwin"
            }}],
            "archive": {{
                "name": "nocter-v{release}-{host}.tar.gz",
                "root": ".nocter"
            }}
        }}"#
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_profile_checks_builds_and_runs_the_same_program() {
    let tree = TempTree::new("installed-value-access");
    let home = tree.value_access_installation();
    let source = tree.0.join("main.nct");
    fs::write(
        source,
        "use std/process\n\
         async func status(): i32! {\n\
             if process.arg_count() != 2 { return 3 }\n\
             let _ = process.arg(1)? otherwise { return 4 }\n\
             42\n\
         }\n\
         async func main(): i32! {\n\
             let pending = status()\n\
             await pending^\n\
         }\n",
    )
    .unwrap();

    let execute = |arguments| {
        execute_invocation(invocation(arguments, &tree.0, &home, "arm64-darwin")).unwrap()
    };
    assert!(matches!(
        execute(vec!["check", "--file", "main.nct", "--offline"]),
        InvocationOutcome::Check(_)
    ));
    assert!(matches!(
        execute(vec!["build", "--file", "main.nct", "--offline"]),
        InvocationOutcome::Build(_)
    ));
    let InvocationOutcome::Run(executed) = execute(vec![
        "run",
        "--file",
        "main.nct",
        "--offline",
        "--",
        "夜 two words",
    ]) else {
        panic!("expected native process execution");
    };
    assert_eq!(executed.status().code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_type_members_select_the_exact_owner_and_receive_ordinary_inputs() {
    let tree = TempTree::new("type-members");
    let home = tree.value_access_installation();
    let package = tree.0.join("app");
    copy_directory(
        &nocter_test_support::repository_root()
            .join("development/compiler/tests/fixtures/value-modules/type-members"),
        &package,
    );
    let source = package.join("main.nct");
    let buffer = package.join("buffer");
    let contract = fs::read_to_string(buffer.join("index.nct")).unwrap();
    let implementation = fs::read_to_string(buffer.join("body.nct")).unwrap();
    let program = fs::read_to_string(&source).unwrap();
    let execute =
        |arguments| execute_invocation(invocation(arguments, &package, &home, "arm64-darwin"));
    let executed = execute(vec!["run", "--offline"]).unwrap_or_else(|error| {
        if let InvocationErrorKind::Run(run) = error.kind()
            && let nocter_command::RunCommandExecutionError::Analysis(failure) = run.as_ref()
            && let nocter_command::CommandAnalysisError::Compilation(compilation) = failure.error()
            && compilation.primary().source_diagnostics().is_empty()
        {
            panic!("{:?}", compilation.primary());
        }
        panic!(
            "{}",
            error
                .render_source_diagnostics()
                .unwrap()
                .unwrap_or_else(|| error.to_string())
        )
    });
    let InvocationOutcome::Run(executed) = executed else {
        panic!("expected native process execution");
    };
    assert_eq!(executed.status().code(), Some(42));

    let expect_rejection = |code| {
        let failure = execute(vec!["check", "--offline", "--format", "json"]).unwrap_err();
        assert_eq!(failure.exit_code(), 1);
        let report = failure.render_json_diagnostics().unwrap().unwrap();
        assert!(report.contains(&format!("\"code\":\"{code}\"")), "{report}");
    };
    for (old, new, code) in [
        ("value^.finish()", "value.finish()", "E0371"),
        (
            "value^.finish()",
            "let _ = value^.finish()\nvalue.count()",
            "E0378",
        ),
        ("var value", "let value", "E0387"),
        ("value^.finish()", "buffer.Buffer.count()", "E0390"),
        ("value^.finish()", "value.create(0).count()", "E0390"),
    ] {
        fs::write(&source, program.replace(old, new)).unwrap();
        expect_rejection(code);
    }
    fs::write(&source, &program).unwrap();
    fs::write(
        buffer.join("body.nct"),
        implementation.replace("const method self.count", "const method edit self.count"),
    )
    .unwrap();
    expect_rejection("E0251");
    for (path, text) in [
        ("index.nct", contract.as_str()),
        ("body.nct", implementation.as_str()),
    ] {
        fs::write(buffer.join(path), text.replace(": Self", ": i32")).unwrap();
    }
    expect_rejection("E0370");
    fs::write(buffer.join("body.nct"), &implementation).unwrap();
    fs::write(
        buffer.join("index.nct"),
        contract.replace("struct Buffer", "#visibility: \"/\"\nstruct Buffer"),
    )
    .unwrap();
    let checked = execute(vec!["check", "--offline"]);
    assert!(matches!(checked, Ok(InvocationOutcome::Check(_))));
    fs::write(
        buffer.join("index.nct"),
        contract.replace(
            "method take self.finish",
            "#visibility: \"./\"\nmethod take self.finish",
        ),
    )
    .unwrap();
    expect_rejection("E0412");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_methods_edit_contextual_outcomes_in_place() {
    let tree = TempTree::new("projected-method-receiver");
    let home = tree.value_access_installation();
    let path = tree.0.join("app.nct");
    let program = fs::read_to_string(
        nocter_test_support::repository_root()
            .join("development/compiler/tests/fixtures/value-modules/type-member-receivers.nct"),
    )
    .unwrap();
    fs::write(&path, &program).unwrap();
    let execute = |command| {
        execute_invocation(invocation(
            [command, "--file", "app.nct", "--offline"],
            &tree.0,
            &home,
            "arm64-darwin",
        ))
    };
    let outcome = execute("run").unwrap_or_else(|error| {
        panic!(
            "{}",
            error
                .render_source_diagnostics()
                .unwrap()
                .unwrap_or_else(|| error.to_string())
        )
    });
    let InvocationOutcome::Run(executed) = outcome else {
        panic!("expected native process execution");
    };
    assert_eq!(executed.status().code(), Some(42));

    let expect_rejection = |source, code| {
        fs::write(&path, source).unwrap();
        let failure = execute("check").unwrap_err();
        let diagnostic = failure.render_source_diagnostics().unwrap().unwrap();
        assert!(diagnostic.contains(code), "{diagnostic}");
    };
    expect_rejection(program.replace("var grid", "let grid"), "E0387");

    let overlapping = program
        .replace(
            "    var evaluations: usize = 0",
            "    read observed = (grid!).values[index]\n    var evaluations: usize = 0",
        )
        .replace(
            "    (grid!).values[index].current()",
            "    observed.current()",
        );
    expect_rejection(overlapping, "E0397");

    let stale = program.replace(
        "    (slot^!).finish()",
        "    let _ = (slot^!).finish()\n    (slot!).current()",
    );
    expect_rejection(stale, "E0378");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_structural_literals_use_the_exact_root_field_contract() {
    let tree = TempTree::new("structural-literal-grants");
    let home = tree.value_access_installation();
    let package = tree.0.join("app");
    copy_directory(
        &nocter_test_support::repository_root()
            .join("development/compiler/tests/fixtures/value-modules/structural-literals"),
        &package,
    );
    let shapes = package.join("shapes/index.nct");
    let contract = fs::read_to_string(&shapes).unwrap();
    let execute = |command| {
        execute_invocation(invocation(
            [command, "--offline"],
            &package,
            &home,
            "arm64-darwin",
        ))
    };
    let outcome = execute("run").unwrap_or_else(|error| {
        panic!(
            "{}",
            error
                .render_source_diagnostics()
                .unwrap()
                .unwrap_or_else(|| error.to_string())
        )
    });
    let InvocationOutcome::Run(executed) = outcome else {
        panic!("expected native process execution");
    };
    assert_eq!(executed.status().code(), Some(42));

    let expect_rejection = |code, message| {
        let failure = execute("check").unwrap_err();
        let diagnostic = failure.render_source_diagnostics().unwrap().unwrap();
        assert!(
            diagnostic.contains(code) && diagnostic.contains(message),
            "{diagnostic}"
        );
    };
    for (old, new, code, message) in [
        ("literal { x, y }", "", "E0255", "no private representation"),
        (
            "literal { x, y }",
            "literal { y, x }",
            "E0256",
            "declaration order",
        ),
        (
            "literal { x, y }",
            "literal { x, x }",
            "E0256",
            "declaration order",
        ),
        (
            "literal { x, y }",
            "literal { x }",
            "E0256",
            "declaration order",
        ),
        (
            "copy struct Marker {}",
            "copy struct Marker",
            "E0256",
            "braced struct contract",
        ),
        (
            "literal { x, y }",
            "literal { x, y }\n    literal { x, y }",
            "E0256",
            "one structural literal grant",
        ),
        (
            "x: i32",
            "#visibility: \"./\"\n    x: i32",
            "E0244",
            "exposes a field",
        ),
        (
            "literal { x, y }",
            "#visibility: \"./\"\n    literal { x, y }",
            "E0370",
            "visible complete definition",
        ),
    ] {
        fs::write(&shapes, contract.replace(old, new)).unwrap();
        expect_rejection(code, message);
    }
    fs::write(&shapes, format!("see ./body.nct\n{contract}")).unwrap();
    fs::write(
        package.join("shapes/body.nct"),
        "see ./index.nct\ncopy struct Point { x: i32\ny: i32\nz: i32 }\n",
    )
    .unwrap();
    expect_rejection("E0256", "already closes this struct's representation");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_test_runs_only_authored_cases_in_source_order() {
    let tree = TempTree::new("installed-value-tests");
    let home = tree.value_access_installation();
    let package = tree.0.join("app");
    fs::create_dir_all(package.join("tests/unit")).unwrap();
    fs::create_dir_all(package.join("tests/helper")).unwrap();
    fs::write(
        package.join("index.nct"),
        "#package: { name: \"app\", version: \"2.0.0\" }\n\
         #test: { name: \"unit\", module: \"./tests/unit\" }\n",
    )
    .unwrap();
    fs::write(package.join("tests/unit/index.nct"), "see ./cases.nct\n").unwrap();
    fs::write(
        package.join("tests/helper/index.nct"),
        "see ./body.nct\nfunc answer(): i32\n",
    )
    .unwrap();
    fs::write(
        package.join("tests/helper/body.nct"),
        "see ./index.nct\nfunc answer(): i32 { 42 }\ntest unselected {}\n",
    )
    .unwrap();
    fs::write(
        package.join("tests/unit/cases.nct"),
        "use std/process as x_fails\nuse ../helper\n\
         func z_first(): i32 { helper.answer() }\n\
         test z_first { return }\n\
         test x_fails { let _ = x_fails.env(\"\")? }\n\
         test a_last { let _ = z_first() }\n",
    )
    .unwrap();
    let execute =
        |arguments| execute_invocation(invocation(arguments, &package, &home, "arm64-darwin"));

    let all = execute(vec!["test", "--offline"]).unwrap();
    assert_eq!(all.exit_code(), 1);
    let InvocationOutcome::Test(all) = all else {
        panic!("expected native tests");
    };
    let result = all
        .runs()
        .iter()
        .map(|run| {
            (
                run.test().expect("expected a case-scoped outcome").name(),
                run.outcome(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        result,
        [
            ("z_first", nocter_command::TestRunOutcome::Passed),
            ("x_fails", nocter_command::TestRunOutcome::Failed),
            ("a_last", nocter_command::TestRunOutcome::Passed),
        ],
        "{all:#?}"
    );

    let selected = execute(vec![
        "test",
        "--test",
        "unit",
        "--case",
        "a_last",
        "--offline",
    ])
    .unwrap();
    assert_eq!(selected.exit_code(), 0);
    let InvocationOutcome::Test(selected) = selected else {
        panic!("expected selected test");
    };
    assert_eq!(selected.runs().len(), 1);
    assert_eq!(selected.runs()[0].test().unwrap().name(), "a_last");
    let missing = execute(vec![
        "test",
        "--test",
        "unit",
        "--case",
        "unselected",
        "--offline",
    ])
    .unwrap_err();
    assert_eq!(missing.diagnostic_code(), Some(DiagnosticCode::E0800));

    for (source, expected) in [
        ("test one {}\ntest one {}\n", "E0241"),
        (
            "test one {}\nfunc attempts_call(): void { one() }\n",
            "E0340",
        ),
    ] {
        fs::write(package.join("tests/unit/cases.nct"), source).unwrap();
        let rejected = execute(vec!["test", "--format", "json", "--offline"]).unwrap();
        assert_eq!(rejected.exit_code(), 1);
        let report = rejected.render_json_diagnostics().unwrap().unwrap();
        assert!(
            report.contains(&format!("\"code\":\"{expected}\"")),
            "{report}"
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_tests_keep_cases_after_a_native_layout_failure() {
    let tree = TempTree::new("installed-value-test-layout");
    let home = tree.value_access_installation();
    execute_invocation(invocation(["init", "app"], &tree.0, &home, "arm64-darwin")).unwrap();
    let app = tree.0.join("app");
    fs::write(
        app.join("tests/unit/cases.nct"),
        "test before {}\n\
         test oversized {\n\
             let data: [i32; 18446744073709551615] = [1; 18446744073709551615]\n\
             let _ = data[0]\n\
         }\n\
         test after {}\n",
    )
    .unwrap();
    let outcome = execute_invocation(invocation(
        ["test", "--offline", "--format", "json"],
        &app,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    assert_eq!(outcome.exit_code(), 1);
    let report = outcome.render_json_diagnostics().unwrap().unwrap();
    assert!(report.contains("\"test\":\"oversized\""), "{report}");
    let InvocationOutcome::Test(results) = outcome else {
        panic!("expected native test report");
    };
    let [before, failed, after] = results.runs() else {
        panic!("{report}");
    };
    assert_eq!(before.test_name(), Some("before"));
    assert_eq!(before.outcome(), nocter_command::TestRunOutcome::Passed);
    assert_eq!(failed.test_name(), Some("oversized"));
    assert_eq!(
        failed.outcome(),
        nocter_command::TestRunOutcome::CompileFailed
    );
    let [diagnostic] = failed.diagnostics() else {
        panic!("{report}");
    };
    assert!(diagnostic.message().contains("Layout("), "{report}");
    assert_eq!(after.test_name(), Some("after"));
    assert_eq!(after.outcome(), nocter_command::TestRunOutcome::Passed);
}

fn invocation(
    arguments: impl IntoIterator<Item = impl Into<OsString>>,
    directory: &Path,
    home: &Path,
    host: &str,
) -> Invocation {
    Invocation::new(
        arguments.into_iter().map(Into::into),
        directory,
        Some(home.as_os_str().to_owned()),
        home.join("nocter"),
        host,
    )
}

fn release_archive(path: &Path, home: &Path) -> nocter_content_integrity::ContentDigest {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.append_dir(".nocter", home).unwrap();
    builder.append_dir(".nocter/std", home.join("std")).unwrap();
    for relative in [
        "VERSION",
        "nocter",
        "LICENSE",
        "NOTICE",
        "MANIFEST.json",
        "std/index.nct",
    ] {
        builder
            .append_path_with_name(home.join(relative), Path::new(".nocter").join(relative))
            .unwrap();
    }
    fs::write(path, builder.into_inner().unwrap().finish().unwrap()).unwrap();
    sha256_file(path).unwrap()
}

#[test]
fn argument_structure_precedes_installation_filesystem_access() {
    let tree = TempTree::new("argument-first");
    let missing = tree.0.join("missing-home");
    let error = execute_invocation(invocation(
        Vec::<&str>::new(),
        &tree.0,
        &missing,
        "arm64-darwin",
    ))
    .unwrap_err();

    assert!(matches!(error.kind(), InvocationErrorKind::Arguments(_)));
    assert_eq!(error.diagnostic_code(), Some(DiagnosticCode::E0700));
}

#[test]
fn help_does_not_select_an_installation_or_prepare_source() {
    let tree = TempTree::new("help");
    let missing_home = tree.0.join("missing-home");
    let missing_directory = tree.0.join("missing-directory");
    let overview = execute_invocation(invocation(
        ["--help"],
        &missing_directory,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();
    let selected = execute_invocation(invocation(
        ["check", "--help"],
        &missing_directory,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(overview, InvocationOutcome::Help(_)));
    assert!(
        overview
            .render_standard_output()
            .unwrap()
            .contains("Nocter compiler")
    );
    assert!(matches!(selected, InvocationOutcome::Help(_)));
    assert!(
        selected
            .render_standard_output()
            .unwrap()
            .contains("nocter check [OPTIONS] [SOURCE]")
    );
}

#[test]
fn initialization_works_without_an_installation_and_never_overwrites_source() {
    let tree = TempTree::new("init");
    let missing_home = tree.0.join("missing-home");
    let outcome = execute_invocation(invocation(
        ["init", "hello"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(outcome, InvocationOutcome::Init(_)));
    assert_eq!(outcome.exit_code(), 0);
    assert!(
        outcome
            .render_standard_output()
            .unwrap()
            .starts_with("Initialized executable package `hello` at ")
    );
    assert!(tree.0.join("hello/index.nct").is_file());
    assert!(tree.0.join("hello/tests/unit/index.nct").is_file());

    let error = execute_invocation(invocation(
        ["init", "hello"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap_err();
    assert!(matches!(error.kind(), InvocationErrorKind::Init(_)));
    assert_eq!(error.exit_code(), 2);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn installed_v2_init_generates_usable_executable_and_library_packages() {
    let tree = TempTree::new("installed-value-init");
    let home = tree.value_access_installation();
    let execute = |arguments, directory: &Path| {
        execute_invocation(invocation(arguments, directory, &home, "arm64-darwin")).unwrap()
    };
    for (directory, arguments) in [
        ("app", vec!["init", "app"]),
        ("library", vec!["init", "library", "--library"]),
    ] {
        let InvocationOutcome::Init(created) = execute(arguments, &tree.0) else {
            panic!("expected generated package");
        };
        let root = created.root();
        assert_eq!(root.file_name().unwrap(), directory);
        for source in [
            "index.nct",
            "implementation.nct",
            "tests/unit/index.nct",
            "tests/unit/cases.nct",
        ] {
            assert_eq!(execute(vec!["fmt", "--check", source], root).exit_code(), 0);
        }
        assert_eq!(execute(vec!["check", "--offline"], root).exit_code(), 0);
        let InvocationOutcome::Test(results) = execute(vec!["test", "--offline"], root) else {
            panic!("expected starter package test");
        };
        assert!(results.succeeded(), "{results:#?}");
        assert_eq!(results.runs().len(), 1);
        if directory == "app" {
            let InvocationOutcome::Run(executed) = execute(vec!["run", "--offline"], root) else {
                panic!("expected starter executable");
            };
            assert_eq!(executed.status().code(), Some(0));
        }
    }
}

#[test]
fn graph_uses_the_validated_home_and_projects_one_read_only_selection() {
    let tree = TempTree::new("graph");
    let home = tree.installation("arm64-darwin", false);
    fs::write(
        tree.0.join("index.nct"),
        concat!(
            "//! Application.\n",
            "#package: { name: \"application\", version: \"1.0.0\", }\n",
        ),
    )
    .unwrap();

    let outcome = execute_invocation(invocation(
        ["graph", "--format", "json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(outcome, InvocationOutcome::Graph(_)));
    let output = outcome.render_standard_output().unwrap();
    assert!(output.starts_with("{\"schema\":\"nocter.package_graph\",\"version\":1,"));
    assert!(output.contains("\"name\":\"application\",\"version\":\"1.0.0\""));
    assert!(output.contains("\"alias\":\"std\",\"source\":\"standard\""));
    assert!(!tree.0.join(".nocter").exists());
}

#[test]
fn both_initialized_templates_pass_public_check_and_test() {
    let tree = TempTree::new("init-interface_implementation");
    let home = tree.installation("arm64-darwin", true);
    for (name, arguments) in [
        ("application", vec!["init", "application"]),
        ("library", vec!["init", "library", "--library"]),
    ] {
        execute_invocation(invocation(
            arguments,
            &tree.0,
            &tree.0.join("unused-home"),
            "arm64-darwin",
        ))
        .unwrap();
        let package = tree.0.join(name);
        let check = execute_invocation(invocation(
            [
                OsString::from("check"),
                OsString::from("--root"),
                package.as_os_str().to_owned(),
            ],
            &tree.0,
            &home,
            "arm64-darwin",
        ))
        .unwrap();
        assert_eq!(check.exit_code(), 0, "initialized {name} must check");
        let test = execute_invocation(invocation(
            [
                OsString::from("test"),
                OsString::from("--root"),
                package.as_os_str().to_owned(),
            ],
            &tree.0,
            &home,
            "arm64-darwin",
        ))
        .unwrap();
        assert_eq!(test.exit_code(), 0, "initialized {name} tests must pass");
    }
}

#[test]
fn source_inspection_bypasses_installation_and_package_selection() {
    let tree = TempTree::new("source-inspection");
    let missing_home = tree.0.join("missing-home");
    fs::write(tree.0.join("app.nct"), "func main(): void { return }\n").unwrap();

    let tokens = execute_invocation(invocation(
        ["tokens", "app.nct", "--format", "json"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();
    let ast = execute_invocation(invocation(
        ["ast", "app.nct", "--format", "json"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();

    assert_eq!(tokens.exit_code(), 0);
    assert!(
        tokens
            .render_standard_output()
            .unwrap()
            .starts_with("{\"schema\":\"nocter.tokens\",\"version\":1,\"ok\":true")
    );
    assert_eq!(ast.exit_code(), 0);
    let ast = ast.render_standard_output().unwrap();
    assert!(ast.starts_with("{\"schema\":\"nocter.ast\",\"version\":1,\"ok\":true"));
    assert!(ast.contains("\"kind\":\"source_file\""));
}

#[test]
fn source_inspection_diagnostics_remain_in_the_inspection_envelope() {
    let tree = TempTree::new("source-inspection-diagnostic");
    let missing_home = tree.0.join("missing-home");
    fs::write(tree.0.join("bad.nct"), "@\n").unwrap();

    let outcome = execute_invocation(invocation(
        ["tokens", "bad.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();

    assert_eq!(outcome.exit_code(), 1);
    let json = outcome.render_standard_output().unwrap();
    assert!(json.starts_with("{\"schema\":\"nocter.tokens\",\"version\":1,\"ok\":false"));
    assert!(json.contains("\"code\":\"E0100\""));
}

#[test]
fn v2_standalone_tools_read_only_the_selected_grammar_metadata() {
    let tree = TempTree::new("standalone-value-access");
    let home = tree.value_access_installation();
    fs::remove_dir_all(home.join("std")).unwrap();
    let source = tree.0.join("app.nct");
    fs::write(
        &source,
        "func finish(take value:error):error {return value^}\n",
    )
    .unwrap();
    let execute = |arguments: Vec<&str>| {
        execute_invocation(invocation(arguments, &tree.0, &home, "arm64-darwin"))
    };

    let tokens = execute(vec!["tokens", "app.nct"]).unwrap();
    assert_eq!(tokens.exit_code(), 0);
    let ast = execute(vec!["ast", "app.nct"]).unwrap();
    assert_eq!(ast.exit_code(), 0);
    assert!(
        ast.render_standard_output()
            .unwrap()
            .contains("\"transfer_expression\"")
    );
    let formatted = execute(vec!["fmt", "app.nct"]).unwrap();
    assert!(matches!(
        formatted,
        InvocationOutcome::Format(nocter_command::FormatCommandResult::Rewritten)
    ));
    let expected = "func finish(take value: error): error { return value^ }\n";
    assert_eq!(fs::read_to_string(&source).unwrap(), expected);
    assert_eq!(
        execute(vec!["fmt", "--check", "app.nct"])
            .unwrap()
            .exit_code(),
        0
    );

    let manifest = home.join("MANIFEST.json");
    let invalid = fs::read_to_string(&manifest)
        .unwrap()
        .replace("value-access-v2", "unknown");
    fs::write(manifest, invalid).unwrap();
    let failure = execute(vec!["fmt", "app.nct"]).unwrap_err();
    assert!(matches!(
        failure.kind(),
        InvocationErrorKind::Installation(_)
    ));
    assert_eq!(fs::read_to_string(source).unwrap(), expected);
}

#[test]
fn format_check_and_rewrite_bypass_installation_and_publish_only_on_success() {
    let tree = TempTree::new("format");
    let missing_home = tree.0.join("missing-home");
    let source = tree.0.join("app.nct");
    fs::write(&source, "func main():void { return }\n").unwrap();

    let error = execute_invocation(invocation(
        ["fmt", "--check", "app.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap_err();

    assert_eq!(error.diagnostic_code(), Some(DiagnosticCode::E0602));
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "func main():void { return }\n"
    );

    let rewritten = execute_invocation(invocation(
        ["fmt", "app.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();
    assert!(matches!(
        rewritten,
        InvocationOutcome::Format(nocter_command::FormatCommandResult::Rewritten)
    ));
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "func main(): void { return }\n"
    );

    let unchanged = execute_invocation(invocation(
        ["fmt", "--check", "app.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();
    assert!(matches!(
        unchanged,
        InvocationOutcome::Format(nocter_command::FormatCommandResult::Unchanged)
    ));
}

#[test]
fn format_rewrites_layout_while_preserving_comments() {
    let tree = TempTree::new("format-comment");
    let missing_home = tree.0.join("missing-home");
    let source = tree.0.join("app.nct");
    let authored = "// keep exactly\nfunc main():void { return }\n";
    fs::write(&source, authored).unwrap();

    let result = execute_invocation(invocation(
        ["fmt", "app.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(
        result,
        InvocationOutcome::Format(nocter_command::FormatCommandResult::Rewritten)
    ));
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "// keep exactly\nfunc main(): void { return }\n"
    );
}

#[test]
fn compiler_host_is_checked_before_user_source_preparation() {
    let tree = TempTree::new("host");
    let home = tree.installation("arm64-darwin", false);
    let error = execute_invocation(invocation(
        ["build", "missing.nct"],
        &tree.0,
        &home,
        "x64-linux",
    ))
    .unwrap_err();

    assert!(matches!(
        error.kind(),
        InvocationErrorKind::InstallationCompatibility(_)
    ));
    assert_eq!(error.diagnostic_code(), Some(DiagnosticCode::E0703));
}

#[test]
fn version_reports_only_the_validated_installation_identity() {
    let tree = TempTree::new("version");
    let home = tree.installation("arm64-darwin", false);
    let outcome = execute_invocation(invocation(
        ["--version"],
        &tree.0.join("unused-missing-directory"),
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(outcome, InvocationOutcome::Version(_)));
    assert_eq!(
        outcome.render_standard_output().as_deref(),
        Some(concat!(
            "Nocter\n",
            "release: 0.14.0\n",
            "host: arm64-darwin\n",
            "default target: arm64-darwin\n"
        ))
    );
}

#[test]
fn language_server_launch_retains_the_validated_installation_without_rendered_output() {
    let tree = TempTree::new("lsp-launch");
    let home = tree.installation("arm64-darwin", false);
    let outcome = execute_invocation(invocation(["lsp"], &tree.0, &home, "arm64-darwin")).unwrap();
    let InvocationOutcome::LanguageServer(launch) = &outcome else {
        panic!("expected language server launch")
    };

    assert_eq!(launch.current_directory(), tree.0);
    assert_eq!(launch.installation().release(), "0.14.0");
    assert!(outcome.render_standard_output().is_none());
    assert!(outcome.render_json_diagnostics().unwrap().is_none());
}

#[test]
fn doctor_reports_the_exact_validated_home() {
    let tree = TempTree::new("doctor");
    let home = tree.installation("arm64-darwin", false);
    let outcome = execute_invocation(invocation(
        ["doctor"],
        &tree.0.join("unused-missing-directory"),
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    let InvocationOutcome::Doctor(report) = &outcome else {
        panic!("expected doctor report");
    };
    let canonical_home = fs::canonicalize(home).unwrap();

    assert_eq!(report.root(), canonical_home);
    assert_eq!(
        outcome.render_standard_output().unwrap(),
        format!(
            concat!(
                "Nocter home is valid\n",
                "root: {}\n",
                "selected by: NOCTER_HOME\n",
                "release: 0.14.0\n",
                "host: arm64-darwin\n",
                "default target: arm64-darwin\n"
            ),
            canonical_home.display()
        )
    );
}

#[test]
fn artifact_install_crosses_the_validated_process_boundary() {
    let tree = TempTree::new("artifact-install");
    let active = tree.installation("arm64-darwin", false);
    let candidate_tree = TempTree::new("artifact-candidate");
    let candidate = candidate_tree.installation("arm64-darwin", false);
    let archive = tree.0.join("nocter-v0.14.0-arm64-darwin.tar.gz");
    let digest = release_archive(&archive, &candidate).to_string();
    let outcome = execute_invocation(invocation(
        [
            OsString::from("install"),
            archive.into_os_string(),
            OsString::from("--sha256"),
            OsString::from(digest.clone()),
            OsString::from("--home"),
            OsString::from("installed"),
        ],
        &tree.0,
        &active,
        "arm64-darwin",
    ))
    .unwrap();

    let InvocationOutcome::Install(result) = &outcome else {
        panic!("expected artifact installation")
    };
    assert!(!result.replaced());
    assert_eq!(result.release(), "0.14.0");
    assert_eq!(
        fs::read_to_string(tree.0.join("installed/VERSION")).unwrap(),
        "0.14.0\n"
    );
    let rendered = outcome.render_standard_output().unwrap();
    assert!(rendered.starts_with("Installed Nocter 0.14.0\n"));
    assert!(rendered.contains(&format!("archive sha256: {digest}\n")));
    assert!(rendered.ends_with("action: created new home\n"));
}

#[test]
fn every_command_rejects_a_non_native_default_target_profile() {
    let tree = TempTree::new("default-target");
    let home = tree.installation("arm64-darwin", false);
    let manifest_path = home.join("MANIFEST.json");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace(
            "\"default_target\": \"arm64-darwin\"",
            "\"default_target\": \"x64-linux\"",
        )
        .replace("\"name\": \"arm64-darwin\"", "\"name\": \"x64-linux\"");
    fs::write(manifest_path, manifest).unwrap();
    let error =
        execute_invocation(invocation(["doctor"], &tree.0, &home, "arm64-darwin")).unwrap_err();

    assert!(matches!(
        error.kind(),
        InvocationErrorKind::InstallationCompatibility(
            nocter_installation::InstallationCompatibilityError::NativeDefaultTargetMismatch { .. }
        )
    ));
    assert_eq!(error.diagnostic_code(), Some(DiagnosticCode::E0703));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn source_failures_render_from_the_retained_invocation_snapshot() {
    let tree = TempTree::new("source-diagnostic");
    let home = tree.installation("arm64-darwin", true);
    fs::write(tree.0.join("invalid.nct"), "@\n").unwrap();
    let error = execute_invocation(invocation(
        ["build", "invalid.nct"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();

    assert_eq!(error.diagnostic_code(), None);
    let rendered = error.render_source_diagnostics().unwrap().unwrap();
    assert!(rendered.starts_with("error[E0100]: unexpected character\n"));
    assert!(rendered.contains("invalid.nct:1:1\n"));
    assert!(rendered.contains("1 | @\n  | ^\n"));
}

#[test]
fn semantic_failures_cross_the_same_process_diagnostic_boundary() {
    let tree = TempTree::new("semantic-diagnostic");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("invalid.nct"),
        "enum Empty {}\nfunc main(): i32 { 0 }\n",
    )
    .unwrap();
    let error = execute_invocation(invocation(
        ["build", "invalid.nct"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();

    let rendered = error.render_source_diagnostics().unwrap().unwrap();
    assert!(rendered.starts_with("error[E0200]: enum must declare at least one variant\n"));
    assert!(rendered.contains("invalid.nct:1:1\n"));
}

#[test]
fn public_fetch_stops_after_the_shared_package_transaction() {
    let tree = TempTree::new("fetch");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("index.nct"),
        "#package: { name: \"package\", version: \"0.0.0\", }\n",
    )
    .unwrap();

    let outcome = execute_invocation(invocation(
        ["fetch", "--locked", "--offline"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    let InvocationOutcome::Fetch(result) = outcome else {
        panic!("expected fetch outcome");
    };
    assert_eq!(result.root().as_str().get(..5), Some("path-"));
    assert_eq!(
        fs::read_to_string(tree.0.join("index.nct")).unwrap(),
        "#package: { name: \"package\", version: \"0.0.0\", }\n"
    );
}

#[test]
fn public_check_uses_the_installed_standard_library_without_emitting_a_binary() {
    let tree = TempTree::new("check");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("application.nct"),
        "func main(): i32 { return 0 }\n",
    )
    .unwrap();

    let outcome = execute_invocation(invocation(
        ["check", "application.nct"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(outcome, InvocationOutcome::Check(_)));
    assert!(!tree.0.join("application").exists());
}

#[test]
fn public_check_failure_uses_the_common_source_diagnostic_snapshot() {
    let tree = TempTree::new("check-diagnostic");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("invalid.nct"),
        "func main(): i32 { return missing }\n",
    )
    .unwrap();

    let error = execute_invocation(invocation(
        ["check", "invalid.nct"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();

    let rendered = error.render_source_diagnostics().unwrap().unwrap();
    assert!(rendered.starts_with("error[E0340]:"));
    assert!(rendered.contains("invalid.nct:1:"));
}

#[test]
fn tuple_projection_input_errors_remain_source_diagnostics() {
    let tree = TempTree::new("check-tuple-projection-diagnostic");
    let home = tree.installation("arm64-darwin", true);

    for (name, suffix) in [
        ("separator", "1_0"),
        (
            "overflow",
            "999999999999999999999999999999999999999999999999999",
        ),
    ] {
        let source_name = format!("{name}.nct");
        fs::write(
            tree.0.join(&source_name),
            format!("func main(): i32 {{ let pair = (1, 2)\n return pair.{suffix} }}\n"),
        )
        .unwrap();

        let error = execute_invocation(invocation(
            ["check", &source_name, "--format=json"],
            &tree.0,
            &home,
            "arm64-darwin",
        ))
        .unwrap_err();
        let rendered = error.render_json_diagnostics().unwrap().unwrap();

        assert_eq!(error.exit_code(), 1, "{rendered}");
        assert!(rendered.contains("\"code\":\"E0413\""), "{rendered}");
        assert!(!rendered.contains("\"code\":\"E0900\""), "{rendered}");
    }
}

#[test]
fn public_check_renders_every_declaration_validation_diagnostic() {
    let tree = TempTree::new("check-declaration-diagnostics");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("invalid.nct"),
        "primitive func first(): usize\nprimitive func second(): usize\n",
    )
    .unwrap();

    let error = execute_invocation(invocation(
        ["check", "invalid.nct"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();

    let rendered = error.render_source_diagnostics().unwrap().unwrap();
    assert_eq!(rendered.matches("error[E0208]:").count(), 2);
    assert!(rendered.contains("invalid.nct:1:"));
    assert!(rendered.contains("invalid.nct:2:"));
}

#[test]
fn json_check_success_renders_one_empty_versioned_envelope() {
    let tree = TempTree::new("check-json-success");
    let home = tree.installation("arm64-darwin", true);
    let source = tree.0.join("application.nct");
    fs::write(&source, "func main(): i32 { return 0 }\n").unwrap();

    let outcome = execute_invocation(invocation(
        ["check", "application.nct", "--format", "json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    let rendered = outcome.render_json_diagnostics().unwrap().unwrap();
    let root = fs::canonicalize(source).unwrap();

    assert_eq!(
        rendered,
        format!(
            "{{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":true,\"command\":\"check\",\"target\":\"arm64-darwin\",\"root\":\"{}\",\"root_absolute_path\":\"{}\",\"diagnostics\":[]}}\n",
            root.display(),
            root.display()
        )
    );
}

#[test]
fn json_check_failure_renders_retained_source_diagnostics_without_human_text() {
    let tree = TempTree::new("check-json-failure");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("invalid.nct"),
        "func main(): i32 { return missing }\n",
    )
    .unwrap();

    let error = execute_invocation(invocation(
        ["check", "invalid.nct", "--format=json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert!(rendered.starts_with(
        "{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":false,\"command\":\"check\",\"target\":\"arm64-darwin\""
    ));
    assert!(rendered.contains("\"code\":\"E0340\""));
    assert!(rendered.contains("\"severity\":\"error\""));
    assert!(rendered.contains("\"primary_span\":{"));
    assert!(!rendered.contains("error[E0340]"));
    assert!(rendered.ends_with("]}\n"));
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn json_argument_failure_retains_format_selected_after_the_first_error() {
    let tree = TempTree::new("check-json-argument");
    let missing = tree.0.join("missing-home");
    let error = execute_invocation(invocation(
        ["check", "--unknown", "--format=json"],
        &tree.0,
        &missing,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        rendered,
        concat!(
            "{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":false,",
            "\"command\":\"check\",\"target\":null,\"root\":\"index.nct\",",
            "\"root_absolute_path\":null,\"diagnostics\":[{\"code\":\"E0700\",",
            "\"severity\":\"error\",\"message\":\"unknown option --unknown\",",
            "\"primary_span\":null,\"notes\":[],\"help\":null}]}\n"
        )
    );
}

#[test]
fn json_installation_failure_has_partial_null_context() {
    let tree = TempTree::new("check-json-installation");
    let missing = tree.0.join("missing-home");
    let error = execute_invocation(invocation(
        ["check", "--format=json"],
        &tree.0,
        &missing,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.exit_code(), 2);
    assert!(rendered.contains("\"code\":\"E0703\""));
    assert!(rendered.contains("\"target\":null,\"root\":\"index.nct\""));
}

#[test]
fn json_input_failure_retains_the_validated_target() {
    let tree = TempTree::new("check-json-input");
    let home = tree.installation("arm64-darwin", false);
    let error = execute_invocation(invocation(
        ["check", "missing.nct", "--format=json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.exit_code(), 2);
    assert!(rendered.contains("\"code\":\"E0702\""));
    assert!(rendered.contains(
        "\"target\":\"arm64-darwin\",\"root\":\"missing.nct\",\"root_absolute_path\":null"
    ));
}

#[test]
fn internal_json_failure_keeps_code_and_status_independent() {
    let error = InvocationError::new(
        InvocationErrorKind::AcquisitionInitialization(PackageAcquisitionError::Unsupported(
            "TLS backend invariant".into(),
        )),
        Some(InvocationDiagnosticPresentation {
            command: nocter_command::DiagnosticCommand::Check,
            format: DiagnosticFormat::Json,
            target: Some("arm64-darwin"),
            root: Some("app.nct".into()),
            root_absolute_path: None,
        }),
    );
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.failure_class(), InvocationFailureClass::Internal);
    assert_eq!(error.exit_code(), 3);
    assert!(rendered.contains("\"code\":\"E0900\""));
    assert!(rendered.contains("\"ok\":false"));
}

#[test]
fn json_discovery_failure_uses_the_authored_module_path() {
    let tree = TempTree::new("check-json-discovery");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("app.nct"),
        "use ./helper\n\nfunc main(): i32 { return 0 }\n",
    )
    .unwrap();
    let error = execute_invocation(invocation(
        ["check", "app.nct", "--format=json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.failure_class(), InvocationFailureClass::Source);
    assert_eq!(error.exit_code(), 1);
    assert!(rendered.contains("\"code\":\"E0263\""));
    assert!(rendered.contains("\"start_byte\":4,\"end_byte\":12"));
    assert!(rendered.contains("single-file mode cannot import a package-local directory module"));
}

#[test]
fn public_test_reports_independent_runs_from_one_typed_result() {
    let tree = TempTree::new("test-report");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("index.nct"),
        concat!(
            "#package: { name: \"tested\", version: \"0.0.0\", }\n",
            "#test: { name: \"unit\", module: \".\" }\n",
            "test passes { return }\n",
            "test fails { return error.new(\"tested.failure\", \"failed\") }\n",
        ),
    )
    .unwrap();

    let human = execute_invocation(invocation(["test"], &tree.0, &home, "arm64-darwin")).unwrap();
    assert_eq!(human.exit_code(), 1);
    let rendered = human.render_standard_output().unwrap();
    assert!(rendered.contains("PASS unit :: passes (exit 0)"));
    assert!(rendered.contains("FAIL unit :: fails (exit 1)"));
    assert!(rendered.contains("tested.failure: failed"));
    assert!(rendered.ends_with("1 passed; 1 failed\n"));

    let json = execute_invocation(invocation(
        ["test", "--format=json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    assert_eq!(json.exit_code(), 1);
    assert_eq!(json.render_standard_output(), None);
    let rendered = json.render_json_diagnostics().unwrap().unwrap();
    assert!(rendered.starts_with(
        "{\"schema\":\"nocter.tests\",\"version\":1,\"ok\":false,\"package\":\"path-"
    ));
    assert!(rendered.contains("\"target\":\"arm64-darwin\",\"diagnostics\":[]"));
    assert!(rendered.contains(
        "\"target\":\"unit\",\"test\":\"passes\",\"outcome\":\"passed\",\"exit_code\":0"
    ));
    assert!(
        rendered.contains(
            "\"target\":\"unit\",\"test\":\"fails\",\"outcome\":\"failed\",\"exit_code\":1"
        )
    );
    assert!(
        rendered
            .contains("\"stderr\":{\"encoding\":\"utf-8\",\"text\":\"tested.failure: failed\\n\"}")
    );
    assert!(rendered.ends_with("\"summary\":{\"passed\":1,\"failed\":1}}\n"));
}

#[test]
fn json_test_argument_failure_uses_the_test_result_envelope() {
    let tree = TempTree::new("test-json-argument");
    let missing = tree.0.join("missing-home");
    let error = execute_invocation(invocation(
        ["test", "--unknown", "--format=json"],
        &tree.0,
        &missing,
        "arm64-darwin",
    ))
    .unwrap_err();
    let rendered = error.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(error.exit_code(), 2);
    assert!(rendered.starts_with(
        "{\"schema\":\"nocter.tests\",\"version\":1,\"ok\":false,\"package\":null,\"target\":null"
    ));
    assert!(rendered.contains("\"code\":\"E0700\""));
    assert!(rendered.ends_with("],\"runs\":[],\"summary\":{\"passed\":0,\"failed\":1}}\n"));
}

#[test]
fn json_test_keeps_target_local_source_failure_beside_later_runs() {
    let tree = TempTree::new("test-json-isolation");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("index.nct"),
        concat!(
            "//! Isolated package.\n",
            "#package: { name: \"isolated\", version: \"0.0.0\", }\n",
            "#test: { name: \"broken\", module: \"./broken\" }\n",
            "#test: { name: \"good\", module: \"./good\" }\n",
        ),
    )
    .unwrap();
    fs::create_dir(tree.0.join("broken")).unwrap();
    fs::write(tree.0.join("broken/index.nct"), "test incomplete {").unwrap();
    fs::create_dir(tree.0.join("good")).unwrap();
    fs::write(tree.0.join("good/index.nct"), "test passes { return }\n").unwrap();

    let outcome = execute_invocation(invocation(
        ["test", "--format=json"],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    let rendered = outcome.render_json_diagnostics().unwrap().unwrap();

    assert_eq!(outcome.exit_code(), 1);
    assert!(
        rendered.contains("\"target\":\"broken\",\"test\":null,\"outcome\":\"compile_failed\"")
    );
    assert!(rendered.contains("\"primary_span\":{"));
    assert!(rendered.contains("\"target\":\"good\",\"test\":\"passes\",\"outcome\":\"passed\""));
    assert!(rendered.ends_with("\"summary\":{\"passed\":1,\"failed\":1}}\n"));
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn public_invocation_builds_through_the_installed_standard_library() {
    let tree = TempTree::new("build");
    let home = tree.installation("arm64-darwin", true);
    fs::write(
        tree.0.join("app.nct"),
        concat!(
            "use std/string.String\n",
            "func main(): i32 {\n",
            "    let lower = \"　ΟΣ　\".trim().to_lowercase()\n",
            "    let upper = \"Straße\".to_uppercase()\n",
            "    if (&lower as &str) != \"ος\" || (&upper as &str) != \"STRASSE\" { return 1 }\n",
            "    var suffix = String.copy(\"Aλ😀\")\n",
            "    let scalar = suffix.pop() otherwise { return 2 }\n",
            "    suffix.truncate(1) catch _ { return 3 }\n",
            "    if scalar != '😀' || (&suffix as &str) != \"A\" { return 4 }\n",
            "    return 0\n",
            "}\n",
        ),
    )
    .unwrap();
    let output = tree.0.join("app-bin");
    let outcome = execute_invocation(invocation(
        [
            OsString::from("build"),
            OsString::from("app.nct"),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ],
        &tree.0,
        &home,
        "arm64-darwin",
    ))
    .unwrap();

    assert!(matches!(&outcome, InvocationOutcome::Build(_)));
    assert_eq!(outcome.exit_code(), 0);
    assert_eq!(&fs::read(&output).unwrap()[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
    assert_eq!(
        std::process::Command::new(output).status().unwrap().code(),
        Some(0)
    );
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn public_system_examples_run_through_the_installed_standard_library() {
    let tree = TempTree::new("system-examples");
    let home = tree.installation("arm64-darwin", true);
    for name in [
        "async-http",
        "http-service",
        "subprocess-status",
        "subprocess-output",
        "subprocess-configured",
        "subprocess-pipeline",
        "async-udp",
        "network-loopback",
    ] {
        let contract = nocter_test_support::PUBLIC_PACKAGE_EXAMPLES
            .iter()
            .find(|contract| contract.directory() == name)
            .unwrap();
        let run = contract.runs().first().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../examples")
            .join(name);
        let output = tree.0.join(name);
        let execution_root = tree.0.join(format!("{name}-run"));
        fs::create_dir(&execution_root).unwrap();
        nocter_test_support::materialize_public_example_fixtures(
            &execution_root,
            contract.fixtures(),
        );
        let outcome = execute_invocation(invocation(
            [
                OsString::from("build"),
                OsString::from("-o"),
                output.as_os_str().to_owned(),
            ],
            &example,
            &home,
            "arm64-darwin",
        ))
        .unwrap();

        assert!(matches!(&outcome, InvocationOutcome::Build(_)));
        let mut command = std::process::Command::new(&output);
        command.current_dir(&execution_root);
        for argument in run.arguments() {
            match argument {
                nocter_test_support::PublicExampleArgument::FixturePath(path) => {
                    let _ = nocter_test_support::public_example_fixture_path(&execution_root, path);
                    command.arg(path);
                }
                nocter_test_support::PublicExampleArgument::Text(value) => {
                    command.arg(value);
                }
            }
        }
        for variable in run.environment() {
            command.env(variable.name(), variable.value());
        }
        let executed = command.output().unwrap();
        assert_eq!(
            executed.status.code(),
            Some(run.status()),
            "example {name} exited unexpectedly; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&executed.stdout),
            String::from_utf8_lossy(&executed.stderr),
        );
        assert_eq!(executed.stdout, run.stdout(), "example {name} stdout");
        assert_eq!(executed.stderr, run.stderr(), "example {name} stderr");
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
struct OperationalTestProcess(Option<std::process::Child>);

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
impl OperationalTestProcess {
    fn new(child: std::process::Child) -> Self {
        Self(Some(child))
    }

    fn id(&self) -> u32 {
        self.0.as_ref().unwrap().id()
    }

    fn wait_with_output(mut self) -> std::io::Result<std::process::Output> {
        self.0.take().unwrap().wait_with_output()
    }

    fn stop_with_output(&mut self) -> std::io::Result<std::process::Output> {
        let mut child = self.0.take().unwrap();
        if child.try_wait()?.is_none() {
            child.kill()?;
        }
        child.wait_with_output()
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
impl Drop for OperationalTestProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn operational_http_request(address: &std::net::SocketAddr) -> String {
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::time::Duration;

    let mut stream = std::net::TcpStream::connect_timeout(address, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(
            b"GET /users/alice?view=full HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = Vec::new();
    let mut scratch = [0_u8; 1024];
    loop {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&scratch[..count]),
            Err(error)
                if !response.is_empty()
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                break;
            }
            Err(error) => panic!("HTTP response read failed: {error}"),
        }
    }
    String::from_utf8(response).unwrap()
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn send_operational_signal(process: &OperationalTestProcess, signal: &str) {
    assert!(
        std::process::Command::new("/bin/kill")
            .arg(signal)
            .arg(process.id().to_string())
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn wait_for_operational_response(address: &std::net::SocketAddr, expected: &str) {
    use std::thread;
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(5);
    while !operational_http_request(address).contains(expected) {
        assert!(
            Instant::now() < deadline,
            "expected reload was not published"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn spawn_operational_service(
    executable: &Path,
    root: &Path,
    config: &Path,
    state: &Path,
) -> OperationalTestProcess {
    use std::process::{Command, Stdio};

    OperationalTestProcess::new(
        Command::new(executable)
            .current_dir(root)
            .arg("--operational")
            .arg("--config")
            .arg(config)
            .arg("--listen")
            .arg("127.0.0.1:0")
            .arg("--state")
            .arg(state)
            .env("NOCTER_HTTP_TOKEN", "environment-secret")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn wait_for_operational_readiness(
    root: &Path,
    process: &mut OperationalTestProcess,
) -> std::net::SocketAddr {
    use std::thread;
    use std::time::{Duration, Instant};

    let ready = root.join(".http-service-ready");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.is_file() {
        if Instant::now() >= deadline {
            let output = process.stop_with_output().unwrap();
            panic!(
                "service did not publish readiness; status: {:?}; stdout: {}; stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
    fs::read_to_string(ready).unwrap().parse().unwrap()
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn installed_http_service_reloads_valid_configuration_and_shuts_down_cleanly() {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    let tree = TempTree::new("installed-operational-http-service");
    let home = tree.installation("arm64-darwin", true);
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/http-service");
    let executable = tree.0.join("http-service-operational");
    let outcome = execute_invocation(invocation(
        [
            OsString::from("build"),
            OsString::from("-o"),
            executable.as_os_str().to_owned(),
        ],
        &example,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    assert!(matches!(&outcome, InvocationOutcome::Build(_)));

    let config = tree.0.join("service.json");
    let state = tree.0.join("service.state");
    fs::write(
        &config,
        b"{\"response\":\"initial-response\",\"token\":\"file-secret\"}",
    )
    .unwrap();

    let mut child = spawn_operational_service(&executable, &tree.0, &config, &state);

    let ready = tree.0.join(".http-service-ready");
    let address = wait_for_operational_readiness(&tree.0, &mut child);

    assert!(operational_http_request(&address).contains("initial-response"));

    let competing = Command::new(&executable)
        .current_dir(&tree.0)
        .arg("--operational")
        .arg("--config")
        .arg(&config)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert_eq!(competing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&competing.stderr).contains("std.store.already_open"));

    fs::write(
        &config,
        b"{\"response\":\"reloaded-response\",\"token\":\"reloaded-secret\"}",
    )
    .unwrap();
    send_operational_signal(&child, "-HUP");
    wait_for_operational_response(&address, "reloaded-response");

    fs::write(&config, b"{\"response\":\"\",\"token\":\"never-log-this\"}").unwrap();
    send_operational_signal(&child, "-HUP");
    thread::sleep(Duration::from_millis(100));
    assert!(operational_http_request(&address).contains("reloaded-response"));

    fs::write(
        &config,
        b"{\"response\":\"second-reload\",\"token\":\"second-secret\"}",
    )
    .unwrap();
    send_operational_signal(&child, "-HUP");
    wait_for_operational_response(&address, "second-reload");

    send_operational_signal(&child, "-TERM");
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("secret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret"));
    assert!(!ready.exists());
    assert!(!state.exists());

    let idle_state = tree.0.join("idle-service.state");
    let mut idle = spawn_operational_service(&executable, &tree.0, &config, &idle_state);
    let _idle_address = wait_for_operational_readiness(&tree.0, &mut idle);
    send_operational_signal(&idle, "-TERM");
    let idle_output = idle.wait_with_output().unwrap();
    assert_eq!(idle_output.status.code(), Some(0));
    assert!(!ready.exists());
    assert!(!idle_state.exists());
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn public_http_example_runs_through_the_installed_standard_library() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let tree = TempTree::new("installed-http-example");
    let home = tree.installation("arm64-darwin", true);
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/http-get");
    let output = tree.0.join("http-get");
    let outcome = execute_invocation(invocation(
        [
            OsString::from("build"),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ],
        &example,
        &home,
        "arm64-darwin",
    ))
    .unwrap();
    assert!(matches!(&outcome, InvocationOutcome::Build(_)));

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        while !request.ends_with(b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0);
            request.extend_from_slice(&scratch[..received]);
        }
        assert_eq!(
            request,
            format!(
                "GET /installed?q=1 HTTP/1.1\r\nhost: localhost:{port}\r\nconnection: close\r\ncontent-length: 0\r\n\r\n"
            )
            .into_bytes()
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\ninstalled\n")
            .unwrap();
    });

    let executed = std::process::Command::new(&output)
        .arg(format!("http://localhost:{port}/installed?q=1"))
        .output()
        .unwrap();
    server.join().unwrap();
    assert_eq!(executed.status.code(), Some(0));
    assert_eq!(executed.stdout, b"HTTP 200\ninstalled\n");
    assert!(executed.stderr.is_empty());
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&source, &destination);
        } else {
            fs::copy(source, destination).unwrap();
        }
    }
}
