use super::diagnostics::package_diagnostic;
use super::modules::resolve_explicit_module;
use super::targets::{TargetDeclaration, is_target_name, parse_target_declaration};
use super::{PackageId, TestTarget, TestTargetId};
use crate::ast::DirectiveValue;
use crate::diagnostics::Diagnostic;
use crate::source::SourceMap;
use std::collections::HashSet;
use std::path::Path;

pub(super) fn parse_test_declaration(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<TargetDeclaration, Vec<Diagnostic>> {
    parse_target_declaration(sources, value, "test", true)
}

pub(super) fn resolve_test_targets(
    sources: &SourceMap,
    root: &Path,
    package: &PackageId,
    declarations: Vec<TargetDeclaration>,
) -> Result<Vec<TestTarget>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut names = HashSet::new();
    let mut targets = Vec::new();
    for declaration in declarations {
        if !is_target_name(&declaration.name) {
            diagnostics.push(package_diagnostic(
                sources,
                declaration.name_span,
                "test name must start with an ASCII letter or `_` and contain only ASCII letters, digits, `_`, or `-`",
            ));
            continue;
        }
        if !names.insert(declaration.name.clone()) {
            diagnostics.push(package_diagnostic(
                sources,
                declaration.name_span,
                format!("duplicate test name `{}`", declaration.name),
            ));
            continue;
        }
        let Some((logical, span)) = declaration.entry else {
            unreachable!("validated test declaration must have an entry")
        };
        let entry = match resolve_explicit_module(root, package.clone(), &logical) {
            Ok(entry) => entry,
            Err(message) => {
                diagnostics.push(package_diagnostic(sources, span, message));
                continue;
            }
        };
        targets.push(TestTarget::new(
            TestTargetId::new(package.clone(), declaration.name),
            entry,
        ));
    }
    if diagnostics.is_empty() {
        Ok(targets)
    } else {
        Err(diagnostics)
    }
}
