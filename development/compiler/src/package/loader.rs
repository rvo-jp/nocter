use super::diagnostics::package_filesystem_diagnostic;
use super::model::SourcePackage;
use super::validation::validate_header;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
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
    let root = canonical_package_root(root);
    let index_candidate = root.join("index.nct");
    let index_path = std::fs::canonicalize(&index_candidate).unwrap_or(index_candidate);
    let mut sources = SourceMap::new();
    if !index_path.starts_with(&root) {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: vec![package_filesystem_diagnostic(format!(
                "package root `{}` has an `index.nct` that escapes the root through a symbolic link",
                root.display()
            ))],
        };
    }
    let source = match sources.load_file(&index_path) {
        Ok(source) => source,
        Err(diagnostic) => {
            return PackageLoad {
                package: None,
                sources,
                diagnostics: vec![package_filesystem_diagnostic(format!(
                    "package root `{}` must contain `index.nct`: {}",
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
    let parsed = parse(&sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: parsed.diagnostics,
        };
    }
    let Some(ast) = parsed.ast else {
        return PackageLoad {
            package: None,
            sources,
            diagnostics: vec![package_filesystem_diagnostic(
                "package parser produced no AST or diagnostic",
            )],
        };
    };
    let validated = match validate_header(&sources, &ast.package_header, &root, &index_path) {
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
            root,
            index_path,
            display_name,
            validated.version,
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
