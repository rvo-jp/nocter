use crate::analysis::analyze_compile_unit_with_entry;
use crate::backend::{BuildRequest, build_executable};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::path::{Path, PathBuf};

struct FrontendOutput {
    root: String,
    root_absolute_path: Option<String>,
    analysis: Option<crate::analysis::CompileUnitAnalysis>,
    diagnostics: Vec<Diagnostic>,
}

pub(super) struct CheckOutput {
    pub root: String,
    pub root_absolute_path: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) struct BuildOutput {
    pub output_path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub(super) fn check_file_with_entry_and_target(
    file: &Path,
    entry_name: &str,
    target: &str,
) -> CheckOutput {
    let options = frontend_options_for_target(target);
    let output = analyze_file(file, &options, entry_name);

    CheckOutput {
        root: output.root,
        root_absolute_path: output.root_absolute_path,
        diagnostics: output.diagnostics,
    }
}

pub(super) fn build_file_with_entry_and_target(
    file: &Path,
    entry_name: &str,
    target: &str,
) -> BuildOutput {
    let output_path = default_executable_path(file);
    build_file_to_path_with_entry_and_target(file, &output_path, entry_name, target)
}

pub(super) fn build_file_to_path_with_entry_and_target(
    file: &Path,
    output_path: &Path,
    entry_name: &str,
    target: &str,
) -> BuildOutput {
    let options = frontend_options_for_target(target);
    build_file_to_path_with_options(file, output_path, &options, entry_name)
}

fn build_file_to_path_with_options(
    file: &Path,
    output_path: &Path,
    options: &FrontendOptions,
    entry_name: &str,
) -> BuildOutput {
    let output = analyze_file(file, options, entry_name);

    if !output.diagnostics.is_empty() {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            diagnostics: output.diagnostics,
        };
    }

    let Some(analysis) = output.analysis.as_ref() else {
        return BuildOutput {
            output_path: output_path.to_path_buf(),
            diagnostics: vec![Diagnostic::error(
                "E0201",
                "frontend analysis completed without diagnostics but produced no analysis output",
            )],
        };
    };

    let diagnostics = match build_executable(BuildRequest {
        analysis,
        output_path,
        target: options.target.as_str(),
        entry_name,
    }) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };

    BuildOutput {
        output_path: output_path.to_path_buf(),
        diagnostics,
    }
}

fn frontend_options_for_target(target: &str) -> FrontendOptions {
    FrontendOptions {
        target: target.to_string(),
        ..FrontendOptions::default()
    }
}

fn analyze_file(file: &Path, options: &FrontendOptions, entry_name: &str) -> FrontendOutput {
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
            let (analysis, diagnostics) = analyze_source(&mut sources, source, options, entry_name);

            FrontendOutput {
                root,
                root_absolute_path,
                analysis,
                diagnostics,
            }
        }
        Err(diagnostic) => FrontendOutput {
            root: file.to_string_lossy().into_owned(),
            root_absolute_path: canonical_absolute_string(file),
            analysis: None,
            diagnostics: vec![diagnostic],
        },
    }
}

fn analyze_source(
    sources: &mut SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    entry_name: &str,
) -> (
    Option<crate::analysis::CompileUnitAnalysis>,
    Vec<Diagnostic>,
) {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return (None, diagnostics),
    };

    let analysis = analyze_compile_unit_with_entry(sources, &unit, entry_name);
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
    use crate::entry::DEFAULT_ENTRY_NAME;
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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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

    #[test]
    fn build_file_accepts_configured_entry_name() {
        let root = make_temp_project("build-custom-entry");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("app.nct");
        fs::write(
            &source,
            r#"func start(): i32 {
    return 0
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            "start",
        );

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let bytes = fs::read(&executable).unwrap();
        assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_with_entry_return_code() {
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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
        )
        .unwrap();
        let source = root.join("fallible_fail.nct");
        fs::write(
            &source,
            r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
        )
        .unwrap();
        let source = root.join("fallible_void_fail.nct");
        fs::write(
            &source,
            r#"from std/error import Error

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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

        assert_diagnostics_empty(&output.diagnostics);
        assert_eq!(output.output_path, executable);
        let output = std::process::Command::new(&executable).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"app.inner: inner failed\n");
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
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
            r#"from std/io_impl import write_text_raw

pub func print(text: &str): void! {
    let marker = 1
    write_text_raw(1, text)?
    return
}
"#,
        )
        .unwrap();
        fs::write(
            nocter_home.join("targets/arm64-darwin/std/io_impl.nct"),
            r#"pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
        )
        .unwrap();
        let source = root.join("hello.nct");
        fs::write(
            &source,
            r#"from std/io import print

func main(): void! {
    let marker = 1
    print("Hello, world!\n")?
}
"#,
        )
        .unwrap();

        let executable = default_executable_path(&source);
        let output = build_file_to_path_with_options(
            &source,
            &executable,
            &frontend_options(nocter_home),
            DEFAULT_ENTRY_NAME,
        );

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
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
        home
    }

    fn frontend_options(nocter_home: PathBuf) -> FrontendOptions {
        FrontendOptions {
            nocter_home: Some(nocter_home),
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
