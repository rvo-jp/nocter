use super::diagnostics::package_filesystem_diagnostic;
use super::model::SourcePackage;
use super::validation::validate_header;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse_package_file;
use crate::source::SourceMap;
use std::path::{Path, PathBuf};

pub struct PackageLoad {
    pub package: Option<SourcePackage>,
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

impl PackageLoad {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn load_package(root: &Path) -> PackageLoad {
    load_package_with_id(root, None)
}

pub(super) fn load_package_with_id(root: &Path, id: Option<super::PackageId>) -> PackageLoad {
    let root = canonical_package_root(root);
    let manifest_candidate = root.join("nocter.nct");
    let manifest_path = std::fs::canonicalize(&manifest_candidate).unwrap_or(manifest_candidate);
    let mut sources = SourceMap::new();
    if !manifest_path.starts_with(&root) {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: vec![package_filesystem_diagnostic(format!(
                "package root `{}` has a `nocter.nct` that escapes the root through a symbolic link",
                root.display()
            ))],
        };
    }
    let source = match sources.load_file(&manifest_path) {
        Ok(source) => source,
        Err(diagnostic) => {
            return PackageLoad {
                package: None,
                sources,
                diagnostics: vec![package_filesystem_diagnostic(format!(
                    "package root `{}` must contain `nocter.nct`: {}",
                    root.display(),
                    diagnostic.message
                ))],
            };
        }
    };
    let lexed = lex(&sources, source);
    if !lexed.diagnostics.is_empty() {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: lexed.diagnostics,
        };
    }
    let parsed = parse_package_file(&sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: parsed.diagnostics,
        };
    }
    let Some(package_file) = parsed.package_file else {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: vec![package_filesystem_diagnostic(
                "package parser produced no AST or diagnostic",
            )],
        };
    };
    let validated = match validate_header(&sources, &package_file.manifest, &root, &manifest_path) {
        Ok(validated) => validated,
        Err(diagnostics) => {
            return PackageLoad {
                package: None,
                sources,
                diagnostics,
            };
        }
    };
    let display_name = validated
        .name
        .unwrap_or_else(|| default_display_name(&root));
    PackageLoad {
        package: Some(SourcePackage::new(
            id.unwrap_or_else(|| super::model::PackageId::root(&root)),
            root,
            manifest_path,
            display_name,
            validated.version,
            validated.dependencies,
            validated.locks,
            validated.executables,
        )),
        sources,
        diagnostics: Vec::new(),
    }
}

fn canonical_package_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

fn default_display_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("package")
        .to_string()
}
