use super::diagnostics::package_diagnostic;
use super::modules::resolve_explicit_module;
use super::{ExecutableId, ExecutableTarget, PackageId, ResolvedModule};
use crate::ast::{DirectiveField, DirectiveValue, PackageManifest};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct ExecutableDeclaration {
    name: String,
    name_span: ByteSpan,
    entry: Option<(String, ByteSpan)>,
}

pub(super) fn parse_executable_declaration(
    sources: &SourceMap,
    value: &DirectiveValue,
) -> Result<ExecutableDeclaration, Vec<Diagnostic>> {
    let DirectiveValue::Record { fields, .. } = value else {
        return Err(vec![package_diagnostic(
            sources,
            value.span(),
            "`#executable` requires a record value",
        )]);
    };
    let mut diagnostics = Vec::new();
    let mut by_name = HashMap::new();
    for field in fields {
        if by_name.insert(field.name.as_str(), field).is_some() {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("duplicate executable field `{}`", field.name),
            ));
        }
        if !matches!(field.name.as_str(), "name" | "entry") {
            diagnostics.push(package_diagnostic(
                sources,
                field.name_span,
                format!("unknown executable field `{}`", field.name),
            ));
        }
    }
    let name = string_field(
        sources,
        &by_name,
        "name",
        true,
        value.span(),
        &mut diagnostics,
    );
    let entry = string_field(
        sources,
        &by_name,
        "entry",
        false,
        value.span(),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let (name, name_span) = name.expect("validated executable name");
    Ok(ExecutableDeclaration {
        name,
        name_span,
        entry,
    })
}

pub(super) fn resolve_executable_targets(
    sources: &SourceMap,
    root: &Path,
    package: &PackageId,
    root_module: &ResolvedModule,
    declarations: Vec<ExecutableDeclaration>,
) -> Result<Vec<ExecutableTarget>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut names = HashSet::new();
    let mut targets = Vec::new();
    for declaration in declarations {
        if !is_executable_name(&declaration.name) {
            diagnostics.push(package_diagnostic(
                sources,
                declaration.name_span,
                "executable name must start with an ASCII letter or `_` and contain only ASCII letters, digits, `_`, or `-`",
            ));
            continue;
        }
        if !names.insert(declaration.name.clone()) {
            diagnostics.push(package_diagnostic(
                sources,
                declaration.name_span,
                format!("duplicate executable name `{}`", declaration.name),
            ));
            continue;
        }
        let entry = match declaration.entry {
            None => root_module.clone(),
            Some((logical, span)) => match resolve_explicit_module(root, package.clone(), &logical)
            {
                Ok(entry) => entry,
                Err(message) => {
                    diagnostics.push(package_diagnostic(sources, span, message));
                    continue;
                }
            },
        };
        targets.push(ExecutableTarget::new(
            ExecutableId::new(package.clone(), declaration.name),
            entry,
        ));
    }
    if diagnostics.is_empty() {
        Ok(targets)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn executable_entry_at_offset(
    manifest: &PackageManifest,
    offset: usize,
) -> Option<(&str, ByteSpan)> {
    manifest
        .directives
        .iter()
        .filter(|directive| directive.name == "executable")
        .filter_map(|directive| match &directive.value {
            DirectiveValue::Record { fields, .. } => Some(fields),
            _ => None,
        })
        .flatten()
        .filter(|field| field.name == "entry")
        .filter_map(|field| field.value.string_value())
        .find(|(_, span)| span.start <= offset && offset <= span.end)
}

fn string_field(
    sources: &SourceMap,
    fields: &HashMap<&str, &DirectiveField>,
    name: &str,
    required: bool,
    fallback: ByteSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, ByteSpan)> {
    let Some(field) = fields.get(name) else {
        if required {
            diagnostics.push(package_diagnostic(
                sources,
                fallback,
                format!("missing required executable field `{name}`"),
            ));
        }
        return None;
    };
    let Some((value, span)) = field.value.string_value() else {
        diagnostics.push(package_diagnostic(
            sources,
            field.value.span(),
            format!("executable field `{name}` requires a string"),
        ));
        return None;
    };
    if value.is_empty() {
        diagnostics.push(package_diagnostic(
            sources,
            span,
            format!("executable field `{name}` cannot be empty"),
        ));
        return None;
    }
    Some((value.to_string(), span))
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
