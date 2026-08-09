use super::diagnostics::package_diagnostic;
use super::targets::{TargetDeclaration, parse_executable_declaration};
use super::test_targets::parse_test_declaration;
use super::{DependencyDeclaration, DependencyLock, DependencySource, LockedDependency};
use crate::ast::{DirectiveField, DirectiveValue, PackageManifest};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};

pub(super) struct PackageDefinition {
    pub(super) name: Option<String>,
    pub(super) version: Option<String>,
    pub(super) dependencies: Vec<DependencyDeclaration>,
    pub(super) locks: Vec<LockedDependency>,
    pub(super) executables: Vec<TargetDeclaration>,
    pub(super) tests: Vec<TargetDeclaration>,
}

pub(super) fn validate_manifest(
    sources: &SourceMap,
    manifest: &PackageManifest,
) -> Result<PackageDefinition, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut name = None;
    let mut version = None;
    let mut dependencies = None;
    let mut locks = None;
    let mut executable_specs = Vec::new();
    let mut test_specs = Vec::new();

    for directive in &manifest.directives {
        match directive.name.as_str() {
            "name" => validate_unique_string(
                sources,
                directive.name_span,
                &directive.value,
                "name",
                &mut name,
                &mut diagnostics,
            ),
            "version" => validate_unique_string(
                sources,
                directive.name_span,
                &directive.value,
                "version",
                &mut version,
                &mut diagnostics,
            ),
            "executable" => match parse_executable_declaration(sources, &directive.value) {
                Ok(spec) => executable_specs.push(spec),
                Err(mut errors) => diagnostics.append(&mut errors),
            },
            "test" => match parse_test_declaration(sources, &directive.value) {
                Ok(spec) => test_specs.push(spec),
                Err(mut errors) => diagnostics.append(&mut errors),
            },
            "dependencies" => {
                if dependencies.is_some() {
                    diagnostics.push(package_diagnostic(
                        sources,
                        directive.name_span,
                        "`#dependencies` may appear only once",
                    ));
                } else {
                    match dependency_declarations(sources, &directive.value) {
                        Ok(value) => dependencies = Some(value),
                        Err(mut errors) => diagnostics.append(&mut errors),
                    }
                }
            }
            "lock" => {
                if locks.is_some() {
                    diagnostics.push(package_diagnostic(
                        sources,
                        directive.name_span,
                        "`#lock` may appear only once",
                    ));
                } else {
                    match locked_dependencies(sources, &directive.value) {
                        Ok(value) => locks = Some(value),
                        Err(mut errors) => diagnostics.append(&mut errors),
                    }
                }
            }
            other => diagnostics.push(package_diagnostic(
                sources,
                directive.name_span,
                format!("unknown package directive `#{other}`"),
            )),
        }
    }

    if diagnostics.is_empty() {
        let dependencies = dependencies.unwrap_or_default();
        let locks = locks.unwrap_or_default();
        validate_lock_names(sources, &dependencies, &locks, &mut diagnostics);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(PackageDefinition {
            name,
            version,
            dependencies,
            locks,
            executables: executable_specs,
            tests: test_specs,
        })
    } else {
        Err(diagnostics)
    }
}

fn dependency_declarations(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<Vec<DependencyDeclaration>, Vec<Diagnostic>> {
    let DirectiveValue::Record { fields, .. } = value else {
        return Err(vec![package_diagnostic(
            sources,
            value.span(),
            "`#dependencies` requires a record value",
        )]);
    };
    let mut diagnostics = Vec::new();
    let mut names = HashSet::new();
    let mut dependencies = Vec::new();
    for field in fields {
        if field.name == super::standard_library::STANDARD_LIBRARY_ALIAS {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                "dependency name `std` is reserved for the standard library",
            ));
            continue;
        }
        if !names.insert(field.name.as_str()) {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("duplicate dependency name `{}`", field.name),
            ));
            continue;
        }
        match dependency_source(sources, &field.value) {
            Ok(source) => dependencies.push(DependencyDeclaration::new(
                field.name.clone(),
                field.span,
                source,
            )),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(dependencies)
    } else {
        Err(diagnostics)
    }
}

fn dependency_source(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<DependencySource, Vec<Diagnostic>> {
    let DirectiveValue::Record { fields, .. } = value else {
        return Err(vec![package_diagnostic(
            sources,
            value.span(),
            "dependency declarations require a source record",
        )]);
    };
    let mut diagnostics = Vec::new();
    let by_name = unique_fields(sources, fields, &mut diagnostics);
    for field in fields {
        if !matches!(field.name.as_str(), "git" | "revision" | "archive" | "path") {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("unknown dependency source field `{}`", field.name),
            ));
        }
    }
    let source_kinds = ["git", "archive", "path"]
        .into_iter()
        .filter(|name| by_name.contains_key(name))
        .collect::<Vec<_>>();
    if source_kinds.len() != 1 {
        diagnostics.push(package_diagnostic(
            sources,
            value.span(),
            "dependency source requires exactly one of `git`, `archive`, or `path`",
        ));
        return Err(diagnostics);
    }
    let kind = source_kinds[0];
    let source =
        required_dependency_string(sources, &by_name, kind, value.span(), &mut diagnostics);
    let revision = optional_dependency_string(sources, &by_name, "revision", &mut diagnostics);
    if kind != "git" && revision.is_some() {
        diagnostics.push(package_diagnostic(
            sources,
            by_name["revision"].name_span,
            "`revision` is valid only for Git dependencies",
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let source = source.expect("validated dependency source");
    Ok(match kind {
        "git" => DependencySource::Git {
            url: source,
            revision: revision.unwrap_or_else(|| "HEAD".to_string()),
        },
        "archive" => DependencySource::Archive { url: source },
        "path" => DependencySource::Path { path: source },
        _ => unreachable!(),
    })
}

fn required_dependency_string(
    sources: &SourceMap,
    fields: &HashMap<&str, &DirectiveField>,
    name: &str,
    fallback: ByteSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let Some(field) = fields.get(name) else {
        diagnostics.push(package_diagnostic(
            sources,
            fallback,
            format!("missing dependency source field `{name}`"),
        ));
        return None;
    };
    let Some((value, _)) = field.value.string_value() else {
        diagnostics.push(package_diagnostic(
            sources,
            field.value.span(),
            format!("dependency source field `{name}` requires a string"),
        ));
        return None;
    };
    if value.is_empty() {
        diagnostics.push(package_diagnostic(
            sources,
            field.value.span(),
            format!("dependency source field `{name}` cannot be empty"),
        ));
        return None;
    }
    Some(value.to_string())
}

fn optional_dependency_string(
    sources: &SourceMap,
    fields: &HashMap<&str, &DirectiveField>,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let field = fields.get(name)?;
    let Some((value, _)) = field.value.string_value() else {
        diagnostics.push(package_diagnostic(
            sources,
            field.value.span(),
            format!("dependency source field `{name}` requires a string"),
        ));
        return None;
    };
    Some(value.to_string())
}

fn locked_dependencies(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<Vec<LockedDependency>, Vec<Diagnostic>> {
    let DirectiveValue::Record { fields, .. } = value else {
        return Err(vec![package_diagnostic(
            sources,
            value.span(),
            "`#lock` requires a record value",
        )]);
    };
    let mut diagnostics = Vec::new();
    let by_name = unique_fields(sources, fields, &mut diagnostics);
    for field in fields {
        if !matches!(field.name.as_str(), "format" | "dependencies") {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("unknown lock field `{}`", field.name),
            ));
        }
    }
    match by_name.get("format").map(|field| &field.value) {
        Some(DirectiveValue::Integer { value: 1, .. }) => {}
        Some(value) => diagnostics.push(package_diagnostic(
            sources,
            value.span(),
            "lock format must be integer `1`",
        )),
        None => diagnostics.push(package_diagnostic(
            sources,
            value.span(),
            "missing required lock field `format`",
        )),
    }
    let Some(dependencies) = by_name.get("dependencies") else {
        diagnostics.push(package_diagnostic(
            sources,
            value.span(),
            "missing required lock field `dependencies`",
        ));
        return Err(diagnostics);
    };
    let DirectiveValue::Record { fields: locked, .. } = &dependencies.value else {
        diagnostics.push(package_diagnostic(
            sources,
            dependencies.value.span(),
            "lock field `dependencies` requires a record",
        ));
        return Err(diagnostics);
    };
    let mut names = HashSet::new();
    let mut result = Vec::new();
    for field in locked {
        if !names.insert(field.name.as_str()) {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("duplicate locked dependency `{}`", field.name),
            ));
            continue;
        }
        let Some((locked, _)) = field.value.string_value() else {
            diagnostics.push(package_diagnostic(
                sources,
                field.value.span(),
                "locked dependency requires a string",
            ));
            continue;
        };
        let resolution = if let Some(commit) = locked.strip_prefix("git:") {
            ((commit.len() == 40 || commit.len() == 64)
                && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| DependencyLock::GitCommit(commit.to_ascii_lowercase()))
        } else if let Some(digest) = locked.strip_prefix("sha256:") {
            (digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| DependencyLock::ArchiveSha256(digest.to_ascii_lowercase()))
        } else {
            None
        };
        match resolution {
            Some(resolution) => result.push(LockedDependency::new(
                field.name.clone(),
                field.span,
                resolution,
            )),
            None => diagnostics.push(package_diagnostic(
                sources,
                field.value.span(),
                "locked dependency must be `git:<commit>` or `sha256:<64 hexadecimal digits>`",
            )),
        }
    }
    if diagnostics.is_empty() {
        Ok(result)
    } else {
        Err(diagnostics)
    }
}

fn validate_lock_names(
    sources: &SourceMap,
    dependencies: &[DependencyDeclaration],
    locks: &[LockedDependency],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let declarations = dependencies
        .iter()
        .map(DependencyDeclaration::name)
        .collect::<HashSet<_>>();
    for lock in locks {
        if !declarations.contains(lock.name()) {
            diagnostics.push(package_diagnostic(
                sources,
                lock.span(),
                format!("lock contains undeclared dependency `{}`", lock.name()),
            ));
        }
    }
}

fn validate_unique_string(
    sources: &SourceMap,
    name_span: ByteSpan,
    value: &DirectiveValue,
    directive_name: &str,
    destination: &mut Option<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let DirectiveValue::String { span, value, .. } = value else {
        diagnostics.push(package_diagnostic(
            sources,
            value.span(),
            format!("`#{directive_name}` requires a string value"),
        ));
        return;
    };
    if destination.is_some() {
        diagnostics.push(package_diagnostic(
            sources,
            name_span,
            format!("`#{directive_name}` may appear only once"),
        ));
        return;
    }
    if value.is_empty() {
        diagnostics.push(package_diagnostic(
            sources,
            *span,
            format!("`#{directive_name}` cannot be empty"),
        ));
        return;
    }
    *destination = Some(value.clone());
}

fn unique_fields<'a>(
    sources: &SourceMap,
    fields: &'a [DirectiveField],
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a DirectiveField> {
    let mut by_name = HashMap::new();
    for field in fields {
        if by_name.insert(field.name.as_str(), field).is_some() {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("duplicate directive field `{}`", field.name),
            ));
        }
    }
    by_name
}
