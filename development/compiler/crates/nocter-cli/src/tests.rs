use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
            let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            copy_directory(&compiler.join("../std"), &standard);
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
        fs::write(root.join("nocter"), "compiler").unwrap();
        fs::write(root.join("LICENSE"), "license").unwrap();
        fs::write(root.join("NOTICE"), "notice").unwrap();
        fs::write(root.join("VERSION"), "0.14.0\n").unwrap();
        let compiler_digest = sha256_file(&root.join("nocter")).unwrap();
        let standard_digest = sha256_regular_tree(&standard, TreeHashOptions::complete()).unwrap();
        fs::write(
            root.join("MANIFEST.json"),
            manifest(host, compiler_digest, standard_digest),
        )
        .unwrap();
        root
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn manifest(
    host: &str,
    compiler_digest: nocter_content_integrity::ContentDigest,
    standard_digest: nocter_content_integrity::ContentDigest,
) -> String {
    format!(
        r#"{{
            "schema": "nocter.manifest",
            "schema_version": 2,
            "release": "0.14.0",
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
                "name": "nocter-v0.14.0-{host}.tar.gz",
                "root": ".nocter"
            }}
        }}"#
    )
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
fn initialization_does_not_select_an_installation_and_never_overwrites_source() {
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
fn format_rejection_keeps_commented_source_unchanged() {
    let tree = TempTree::new("format-comment");
    let missing_home = tree.0.join("missing-home");
    let source = tree.0.join("app.nct");
    let authored = "// keep exactly\nfunc main():void { return }\n";
    fs::write(&source, authored).unwrap();

    let error = execute_invocation(invocation(
        ["fmt", "app.nct"],
        &tree.0,
        &missing_home,
        "arm64-darwin",
    ))
    .unwrap_err();

    assert_eq!(error.exit_code(), 1);
    assert_eq!(fs::read_to_string(&source).unwrap(), authored);
    let rendered = error.render_source_diagnostics().unwrap().unwrap();
    assert!(rendered.contains("error[E0601]"));
    assert!(rendered.contains("// keep exactly"));
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
            command: "check",
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
            "func main(): i32 {\n",
            "    let text: &str = \"Aλ😀\"\n",
            "    var count: usize = 0\n",
            "    for scalar in text.chars() {\n",
            "        if scalar == '\\u{1F600}' && scalar.utf8_len() != 4 { return 1 }\n",
            "        count += 1\n",
            "    }\n",
            "    if count != 3 { return 2 }\n",
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
fn public_subprocess_examples_run_through_the_installed_standard_library() {
    let tree = TempTree::new("subprocess-examples");
    let home = tree.installation("arm64-darwin", true);
    for name in [
        "subprocess-status",
        "subprocess-output",
        "subprocess-configured",
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
        let executed = std::process::Command::new(&output)
            .current_dir(&example)
            .output()
            .unwrap();
        assert_eq!(executed.status.code(), Some(run.status()));
        assert_eq!(executed.stdout, run.stdout());
        assert_eq!(executed.stderr, run.stderr());
    }
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
