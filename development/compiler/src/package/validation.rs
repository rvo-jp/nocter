use super::diagnostics::package_diagnostic;
use super::model::{ExecutableId, ExecutableTarget, ModuleId, PackageId};
use crate::ast::{DirectiveField, DirectiveValue, PackageHeader};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

pub(super) struct ValidatedHeader {
    pub(super) name: Option<String>,
    pub(super) version: Option<String>,
    pub(super) executables: Vec<ExecutableTarget>,
}

pub(super) fn validate_header(
    sources: &SourceMap,
    header: &PackageHeader,
    root: &Path,
    index_path: &Path,
) -> Result<ValidatedHeader, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut name = None;
    let mut version = None;
    let mut executable_specs = Vec::new();

    for directive in &header.directives {
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
            "executable" => match executable_fields(sources, &directive.value) {
                Ok(spec) => executable_specs.push(spec),
                Err(mut errors) => diagnostics.append(&mut errors),
            },
            "depend" => diagnostics.push(package_diagnostic(
                sources,
                directive.name_span,
                "dependency declarations require the package dependency capability",
            )),
            other => diagnostics.push(package_diagnostic(
                sources,
                directive.name_span,
                format!("unknown package directive `#{other}`"),
            )),
        }
    }

    let mut names = HashSet::new();
    let mut executables = Vec::new();
    for spec in executable_specs {
        if !is_executable_name(&spec.name) {
            diagnostics.push(package_diagnostic(
                sources,
                spec.name_span,
                "executable name must start with an ASCII letter or `_` and contain only ASCII letters, digits, `_`, or `-`",
            ));
            continue;
        }
        if !names.insert(spec.name.clone()) {
            diagnostics.push(package_diagnostic(
                sources,
                spec.name_span,
                format!("duplicate executable name `{}`", spec.name),
            ));
            continue;
        }
        match resolve_package_module(root, index_path, &spec.module) {
            Ok(source_path) => executables.push(ExecutableTarget::new(
                ExecutableId::new(PackageId::ROOT, spec.name.clone()),
                spec.name,
                ModuleId::new(PackageId::ROOT, spec.module),
                source_path,
            )),
            Err(message) => {
                diagnostics.push(package_diagnostic(sources, spec.module_span, message))
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(ValidatedHeader {
            name,
            version,
            executables,
        })
    } else {
        Err(diagnostics)
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

struct ExecutableSpec {
    name: String,
    name_span: ByteSpan,
    module: String,
    module_span: ByteSpan,
}

fn executable_fields(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<ExecutableSpec, Vec<Diagnostic>> {
    let DirectiveValue::Record { fields, .. } = value else {
        return Err(vec![package_diagnostic(
            sources,
            value.span(),
            "`#executable` requires a record value",
        )]);
    };
    let mut diagnostics = Vec::new();
    let by_name = unique_fields(sources, fields, &mut diagnostics);
    for field in fields {
        if !matches!(field.name.as_str(), "name" | "module") {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("unknown executable field `{}`", field.name),
            ));
        }
    }
    let name = required_string_field(sources, &by_name, "name", value.span(), &mut diagnostics);
    let module = required_string_field(sources, &by_name, "module", value.span(), &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let (name, name_span) = name.expect("validated executable name");
    let (module, module_span) = module.expect("validated executable module");
    Ok(ExecutableSpec {
        name,
        name_span,
        module,
        module_span,
    })
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

fn required_string_field(
    sources: &SourceMap,
    fields: &HashMap<&str, &DirectiveField>,
    name: &str,
    fallback: ByteSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, ByteSpan)> {
    let Some(field) = fields.get(name) else {
        diagnostics.push(package_diagnostic(
            sources,
            fallback,
            format!("missing required executable field `{name}`"),
        ));
        return None;
    };
    let DirectiveValue::String { span, value, .. } = &field.value else {
        diagnostics.push(package_diagnostic(
            sources,
            field.value.span(),
            format!("executable field `{name}` requires a string"),
        ));
        return None;
    };
    Some((value.clone(), *span))
}

pub(crate) fn resolve_package_module(
    root: &Path,
    index_path: &Path,
    logical: &str,
) -> Result<PathBuf, String> {
    if logical == "." {
        return Ok(index_path.to_path_buf());
    }
    let Some(relative) = logical.strip_prefix("./") else {
        return Err(
            "executable module must be `.` or a package-relative path beginning with `./`"
                .to_string(),
        );
    };
    if relative.is_empty() || logical.ends_with(".nct") {
        return Err(
            "executable module must name a logical module without a `.nct` suffix".to_string(),
        );
    }
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("executable module cannot escape the package root".to_string());
    }
    let base = root.join(relative_path);
    let file = base.with_extension("nct");
    let index = base.join("index.nct");
    match (file.is_file(), index.is_file()) {
        (true, true) => Err(format!(
            "executable module `{logical}` is ambiguous because both `{}` and `{}` exist",
            file.display(),
            index.display()
        )),
        (true, false) => canonical_module_path(root, logical, file),
        (false, true) => canonical_module_path(root, logical, index),
        (false, false) => Err(format!("executable module `{logical}` does not exist")),
    }
}

fn canonical_module_path(root: &Path, logical: &str, selected: PathBuf) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(&selected).map_err(|error| {
        format!(
            "executable module `{logical}` could not be canonicalized at `{}`: {error}",
            selected.display()
        )
    })?;
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err(format!(
            "executable module `{logical}` escapes the package root through `{}`",
            selected.display()
        ))
    }
}

fn is_executable_name(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}
