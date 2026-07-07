use crate::analysis::analyze_compile_unit;
use crate::backend::{BuildRequest, build_executable};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
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

pub(super) fn check_file(file: &Path) -> CheckOutput {
    let options = FrontendOptions::default();
    let output = analyze_file(file, &options);

    CheckOutput {
        root: output.root,
        root_absolute_path: output.root_absolute_path,
        diagnostics: output.diagnostics,
    }
}

pub(super) fn build_file(file: &Path) -> BuildOutput {
    let output_path = default_executable_path(file);
    build_file_to_path(file, &output_path)
}

pub(super) fn build_file_to_path(file: &Path, output_path: &Path) -> BuildOutput {
    build_file_to_path_with_options(file, output_path, &FrontendOptions::default())
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
        target: DEFAULT_TARGET,
    }) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };

    BuildOutput {
        output_path: output_path.to_path_buf(),
        diagnostics,
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
) -> (
    Option<crate::analysis::CompileUnitAnalysis>,
    Vec<Diagnostic>,
) {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return (None, diagnostics),
    };

    let analysis = analyze_compile_unit(sources, &unit);
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
            r#"program(): i32 {
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
    fn build_file_output_runs_with_program_return_code() {
        let root = make_temp_project("build-run");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("exit42.nct");
        fs::write(
            &source,
            r#"program(): i32 {
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
    fn build_file_output_runs_fallible_program_success_return_code() {
        let root = make_temp_project("build-run-fallible-success");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible.nct");
        fs::write(
            &source,
            r#"program(): i32! {
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
    fn build_file_output_runs_fallible_program_failure() {
        let root = make_temp_project("build-run-fallible-failure");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("fallible_fail.nct");
        fs::write(
            &source,
            r#"primitive make_error(code: str, message: str): error

program(): i32! {
    fail make_error("app.failed", "failed")
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
        assert_eq!(output.stderr, b"failed\n");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn build_file_output_runs_void_program_with_zero_exit_code() {
        let root = make_temp_project("build-run-void");
        let nocter_home = make_nocter_home(&root);
        let source = root.join("void.nct");
        fs::write(
            &source,
            r#"program(): void {
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
