use super::buildability::v0_buildability_diagnostics;
use crate::analysis::analyze_executable_compile_unit;
use crate::backend::{BuildRequest, build_executable};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::path::{Path, PathBuf};

struct FrontendOutput {
    root: String,
    root_absolute_path: Option<String>,
    sources: SourceMap,
    analysis: Option<crate::analysis::CompileUnitAnalysis>,
    diagnostics: Vec<Diagnostic>,
}

pub(super) struct CheckOutput {
    pub root: String,
    pub root_absolute_path: Option<String>,
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) struct BuildOutput {
    pub output_path: PathBuf,
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) fn check_file_with_target(file: &Path, target: &str) -> CheckOutput {
    let options = frontend_options_for_target(target);
    let output = analyze_file(file, &options);

    CheckOutput {
        root: output.root,
        root_absolute_path: output.root_absolute_path,
        sources: output.sources,
        diagnostics: output.diagnostics,
    }
}

pub(super) fn build_file_with_target(file: &Path, target: &str) -> BuildOutput {
    let output_path = default_executable_path(file);
    build_file_to_path_with_target(file, &output_path, target)
}

pub(super) fn build_file_to_path_with_target(
    file: &Path,
    output_path: &Path,
    target: &str,
) -> BuildOutput {
    let options = frontend_options_for_target(target);
    build_file_to_path_with_options(file, output_path, &options)
}

fn build_file_to_path_with_options(
    file: &Path,
    output_path: &Path,
    options: &FrontendOptions,
) -> BuildOutput {
    let output = analyze_file(file, options);

    if !output.diagnostics.is_empty() {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics: output.diagnostics,
        };
    }

    let Some(analysis) = output.analysis.as_ref() else {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics: vec![Diagnostic::error(
                "E0201",
                "frontend analysis completed without diagnostics but produced no analysis output",
            )],
        };
    };

    let diagnostics = v0_buildability_diagnostics(&output.sources, analysis);
    if !diagnostics.is_empty() {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            sources: output.sources,
            diagnostics,
        };
    }

    let diagnostics = match build_executable(BuildRequest {
        analysis,
        sources: &output.sources,
        output_path,
        target: options.target.as_str(),
    }) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };

    BuildOutput {
        output_path: output_path.to_path_buf(),
        sources: output.sources,
        diagnostics,
    }
}

fn frontend_options_for_target(target: &str) -> FrontendOptions {
    FrontendOptions {
        target: target.to_string(),
        ..FrontendOptions::default()
    }
}

fn analyze_file(file: &Path, options: &FrontendOptions) -> FrontendOutput {
    let mut sources = SourceMap::new();

    match sources.load_file(file) {
        Ok(source) => {
            let source_file = sources
                .get(source)
                .expect("loaded source id must resolve in source map");
            let root = source_file.display_path().to_string();
            let root_absolute_path = source_file
                .absolute_path()
                .map(|path| path.to_string_lossy().into_owned());
            let (analysis, diagnostics) = analyze_source(&mut sources, source, options);

            FrontendOutput {
                root,
                root_absolute_path,
                sources,
                analysis,
                diagnostics,
            }
        }
        Err(diagnostic) => FrontendOutput {
            root: file.to_string_lossy().into_owned(),
            root_absolute_path: canonical_absolute_string(file),
            sources,
            analysis: None,
            diagnostics: vec![diagnostic],
        },
    }
}

fn analyze_source(
    sources: &mut SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> (
    Option<crate::analysis::CompileUnitAnalysis>,
    Vec<Diagnostic>,
) {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return (None, diagnostics),
    };

    let analysis = analyze_executable_compile_unit(sources, &unit);
    let diagnostics = analysis.diagnostics();

    (Some(analysis), diagnostics)
}

fn default_executable_path(source_path: &Path) -> PathBuf {
    match source_path.file_stem() {
        Some(stem) => source_path.with_file_name(stem),
        None => PathBuf::from("a.out"),
    }
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::DEFAULT_TARGET;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn derives_default_executable_path_from_source_path() {
        assert_eq!(
            default_executable_path(Path::new("src/app.nct")),
            PathBuf::from("src/app")
        );
    }

    #[test]
    fn derives_fallback_executable_path_for_empty_source_path() {
        assert_eq!(
            default_executable_path(Path::new("")),
            PathBuf::from("a.out")
        );
    }

    #[test]
    fn build_file_writes_arm64_macho_executable() {
        let root = make_temp_project("build-macho");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("app.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return 0
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let bytes = fs::read(&executable).unwrap();
        assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
        assert_eq!(read_u32(&bytes, 4), 0x0100_000c);
        assert_eq!(read_u32(&bytes, 12), 0x2);
        assert!(bytes.len() > 0x4000);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&executable).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_main_return_code() {
        let root = make_temp_project("build-run");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("exit42.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return 42
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_same_file_function_call() {
        let root = make_temp_project("build-run-function-call");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("call.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 29
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(29));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_i32_function_call_with_arguments() {
        let root = make_temp_project("build-run-function-arguments");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("add.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_void_call_statement_before_return() {
        let root = make_temp_project("build-run-void-call-statement");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("effect.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    effect()
    return 42
}

func effect(): void {
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_usize_function_call_with_mixed_arguments() {
        let root = make_temp_project("build-run-usize-function-arguments");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("choose.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    let value: usize = choose(7, 42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, value: usize): usize {
    return value
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_usize_tail_call_with_argument() {
        let root = make_temp_project("build-run-usize-tail-call-argument");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("forward.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    let value: usize = forward(42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func forward(value: usize): usize {
    return identity(value)
}

func identity(value: usize): usize {
    return value
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_bool_function_call_with_mixed_arguments() {
        let root = make_temp_project("build-run-bool-function-arguments");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("choose_bool.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    if choose(7, true, 42) {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, flag: bool, size: usize): bool {
    return flag
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_str_function_call_argument() {
        let root = make_temp_project("build-run-str-function-argument");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("choose_str.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return choose("Nocter", 42)
}

func choose(name: &str, code: i32): i32 {
    return code
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_str_normal_call_result_as_argument() {
        let root = make_temp_project("build-run-str-normal-call-argument");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("str_call_argument.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return consume(title(), 42)
}

func title(): &str {
    return "Nocter"
}

func consume(name: &str, code: i32): i32 {
    return code
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_str_len_and_index_call_results() {
        let root = make_temp_project("build-run-str-call-result-ops");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("str_call_result_ops.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    let size: usize = identity("Nocter").len()
    let byte: u8 = identity("Nocter")[3]
    if size == 6 && byte == 116 {
        return 0
    } else {
        return 1
    }
}

func identity(text: &str): &str {
    return text
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_entry_success_return_code() {
        let root = make_temp_project("build-run-fallible-success");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible.nct");
        fs::write(
            &source,
            r#"func main(): i32! {
    return 31
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(31));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_entry_failure() {
        let root = make_temp_project("build-run-fallible-failure");
        let nocter_home = make_nocter_home(&root);
        fs::write(
            nocter_home.join("std/error.nct"),
            r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
        )
        .unwrap();
        let source = root.join("fallible_fail.nct");
        fs::write(
            &source,
            r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.failed: failed\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_propagates_fallible_void_call_failure() {
        let root = make_temp_project("build-run-fallible-void-propagation");
        let nocter_home = make_nocter_home(&root);
        fs::write(
            nocter_home.join("std/error.nct"),
            r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
        )
        .unwrap();
        let source = root.join("fallible_void_fail.nct");
        fs::write(
            &source,
            r#"use std/error.Error

func main(): void! {
    fail()?
}

func fail(): void! {
    return Error.new("app.inner", "inner failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.inner: inner failed\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_i32_call_success_propagation() {
        let root = make_temp_project("build-run-fallible-i32-success-propagation");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible_i32_success.nct");
        fs::write(
            &source,
            r#"func main(): i32! {
    let base = 2
    let value = answer()?
    return base + value
}

func answer(): i32! {
    return 40
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_propagates_fallible_i32_call_failure() {
        let root = make_temp_project("build-run-fallible-i32-failure-propagation");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        let source = root.join("fallible_i32_fail.nct");
        fs::write(
            &source,
            r#"use std/error.Error

func main(): i32! {
    return fail()?
}

func fail(): i32! {
    return Error.new("app.number", "number failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.number: number failed\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_i32_catch_success() {
        let root = make_temp_project("build-run-fallible-i32-catch-success");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible_i32_catch_success.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    return answer() catch error {
        return 7
    }
}

func answer(): i32! {
    return 42
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_i32_catch_failure() {
        let root = make_temp_project("build-run-fallible-i32-catch-failure");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        let source = root.join("fallible_i32_catch_failure.nct");
        fs::write(
            &source,
            r#"use std/error.Error

func main(): i32! {
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.answer: inner failed\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_write_text_raw_catch_failure() {
        let root = make_temp_project("build-run-write-text-raw-catch-failure");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        fs::write(
            nocter_home.join("std/io.nct"),
            r#"use std/error.Error

#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

pub func fail_write(): void! {
    write_text_raw(-1, "x") catch error {
        return Error.new("app.write", error.code)
    }
    return
}
"#,
        )
        .unwrap();
        let source = root.join("write_catch_failure.nct");
        fs::write(
            &source,
            r#"use std/io.fail_write

func main(): void! {
    fail_write()?
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.write: std.io.invalid_input\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_write_bytes_raw_catch_failure() {
        let root = make_temp_project("build-run-write-bytes-raw-catch-failure");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        fs::write(
            nocter_home.join("std/string.nct"),
            r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
        )
        .unwrap();
        fs::write(
            nocter_home.join("std/io.nct"),
            r#"use std/error.Error
use std/string.bytes

#target("arm64-darwin")
pub(nocter) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!

pub func fail_write(): void! {
    write_bytes_raw(-1, bytes("x")) catch error {
        return Error.new("app.write", error.code)
    }
    return
}
"#,
        )
        .unwrap();
        let source = root.join("write_bytes_catch_failure.nct");
        fs::write(
            &source,
            r#"use std/io.fail_write

func main(): void! {
    fail_write()?
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.write: std.io.invalid_input\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_read_bytes_raw_catch_failure() {
        let root = make_temp_project("build-run-read-bytes-raw-catch-failure");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        fs::write(
            nocter_home.join("std/ptr.nct"),
            r#"pub(nocter) primitive from_addr<T>(address: usize): *T

pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
        )
        .unwrap();
        fs::write(
            nocter_home.join("std/io.nct"),
            r#"use std/error.Error
use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

#target("arm64-darwin")
pub(nocter) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!

pub func fail_read(): void! {
    let buffer: &+[u8] = slice_from_raw_parts_mut(from_addr(1), 1)
    let count = read_bytes_raw(-1, buffer) catch error {
        return Error.new("app.read", error.code)
    }
    return
}
"#,
        )
        .unwrap();
        let source = root.join("read_bytes_catch_failure.nct");
        fs::write(
            &source,
            r#"use std/io.fail_read

func main(): void! {
    fail_read()?
    return
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.read: std.io.invalid_input\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_non_i32_catch_success_paths() {
        let root = make_temp_project("build-run-non-i32-catch-success");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("non_i32_catch_success.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    let left: i32 = success_left()
    let right: i32 = success_right()
    return left + right
}

func success_left(): i32 {
    let byte_status: i32 = success_byte()
    let size_status: i32 = success_size()
    return byte_status + size_status
}

func success_right(): i32 {
    let bool_status: i32 = success_bool()
    let str_status: i32 = success_str()
    let void_status: i32 = success_void()
    return bool_status + str_status + void_status
}

func success_byte(): i32 {
    let byte_value: u8 = make_byte() catch error {
        return 1
    }
    return byte_value as i32
}

func success_size(): i32 {
    let size_value: usize = make_size() catch error {
        return 2
    }
    if size_value == 8 {
        return 8
    } else {
        return 1
    }
}

func success_bool(): i32 {
    let flag_value: bool = make_flag() catch error {
        return 3
    }
    if flag_value {
        return 7
    } else {
        return 1
    }
}

func success_str(): i32 {
    let text: &str = make_text() catch error {
        return 4
    }
    return 6
}

func success_void(): i32 {
    effect() catch error {
        return 5
    }
    return 11
}

func make_byte(): u8! {
    return 10
}

func make_size(): usize! {
    return 8
}

func make_flag(): bool! {
    return true
}

func make_text(): &str! {
    return "abc"
}

func effect(): void! {
    return
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_non_i32_catch_failure_recovery_paths() {
        let root = make_temp_project("build-run-non-i32-catch-failure-recovery");
        let nocter_home = make_nocter_home(&root);
        write_std_error(&nocter_home);
        let source = root.join("non_i32_catch_failure_recovery.nct");
        fs::write(
            &source,
            r#"use std/error.Error

func main(): i32 {
    let left: i32 = recover_left()
    let right: i32 = recover_right()
    return left + right
}

func recover_left(): i32 {
    let byte_status: i32 = recover_byte()
    let size_status: i32 = recover_size()
    return byte_status + size_status
}

func recover_right(): i32 {
    let bool_status: i32 = recover_bool()
    let str_status: i32 = recover_str()
    let void_status: i32 = recover_void()
    return bool_status + str_status + void_status
}

func recover_byte(): i32 {
    let value: u8 = fail_byte() catch error {
        return 10
    }
    return value as i32
}

func recover_size(): i32 {
    let value: usize = fail_size() catch error {
        return 11
    }
    if value == 0 {
        return 0
    } else {
        return 1
    }
}

func recover_bool(): i32 {
    let value: bool = fail_flag() catch error {
        return 12
    }
    if value {
        return 0
    } else {
        return 1
    }
}

func recover_str(): i32 {
    let value: &str = fail_text() catch error {
        return 13
    }
    return 1
}

func recover_void(): i32 {
    fail_effect() catch error {
        return 14
    }
    return 1
}

func fail_byte(): u8! {
    return Error.new("app.byte", "byte failed")
}

func fail_size(): usize! {
    return Error.new("app.size", "size failed")
}

func fail_flag(): bool! {
    return Error.new("app.flag", "flag failed")
}

func fail_text(): &str! {
    return Error.new("app.text", "text failed")
}

func fail_effect(): void! {
    return Error.new("app.effect", "effect failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(60));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_scalar_call_success_propagation() {
        let root = make_temp_project("build-run-fallible-scalar-success-propagation");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible_scalar_success.nct");
        fs::write(
            &source,
            r#"func main(): i32! {
    let byte_value: u8 = make_byte()?
    let size_value: usize = make_size()?
    let flag_value: bool = make_flag()?
    if flag_value && size_value == 40 {
        return byte_value as i32
    } else {
        return 1
    }
}

func make_byte(): u8! {
    return 42
}

func make_size(): usize! {
    return 40
}

func make_flag(): bool! {
    return true
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_str_call_success_propagation() {
        let root = make_temp_project("build-run-fallible-str-success-propagation");
        let nocter_home = make_nocter_home(&root);
        fs::write(
            nocter_home.join("std/io.nct"),
            r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
        )
        .unwrap();
        let source = root.join("fallible_str_success.nct");
        fs::write(
            &source,
            r#"use std/io.write

func main(): i32! {
    let text: &str = message()?
    write(text)?
    return 0
}

func message(): &str! {
    return "fallible text\n"
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"fallible text\n");
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_fallible_force_unwrap_success() {
        let root = make_temp_project("build-run-fallible-force-success");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible_force_success.nct");
        fs::write(
            &source,
            r#"func main(): i32 {
    let value = answer()!
    return value
}

func answer(): i32! {
    return 42
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_void_entry_with_zero_exit_code() {
        let root = make_temp_project("build-run-void");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("void.nct");
        fs::write(
            &source,
            r#"func main(): void {
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(0));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_std_print_hello_world() {
        let root = make_temp_project("build-run-std-print");
        let nocter_home = make_nocter_home(&root);
        fs::write(
            nocter_home.join("std/io.nct"),
            r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

pub func print(text: &str): void! {
    let marker = 1
    write_text_raw(1, text)?
    return
}
"#,
        )
        .unwrap();
        let source = root.join("hello.nct");
        fs::write(
            &source,
            r#"use std/io.print

func main(): void! {
    let marker = 1
    print("Hello, world!\n")?
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output =
            build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"Hello, world!\n");
        assert!(output.stderr.is_empty());
    }

    fn make_temp_project(name: &str) -> PathBuf {
        let unique = format!(
            "nocter-pipeline-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_nocter_home(root: &Path) -> PathBuf {
        let home = root.join(".nocter");
        fs::create_dir_all(home.join("std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "").unwrap();
        home
    }

    fn write_std_error(nocter_home: &Path) {
        fs::write(
            nocter_home.join("std/error.nct"),
            r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
        )
        .unwrap();
    }

    fn frontend_options(nocter_home: PathBuf) -> FrontendOptions {
        FrontendOptions {
            nocter_home: Some(nocter_home),
            source_root: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }

    fn assert_diagnostics_empty(diagnostics: &[Diagnostic]) {
        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics, got {diagnostics:#?}"
        );
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        let mut value = [0; 4];
        value.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_le_bytes(value)
    }
}
