use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_compile_input::ModuleIdentity;
use nocter_declarations::{DeclarationGraph, Module, ModulePath};
use nocter_discovery::DiscoveredUnit;
use nocter_model::{ModuleId, PackageId, PackageIdentity, Symbol};
use nocter_source::{ByteOffset, SourceFile, SourceId, TextRange};
use nocter_source_index::SemanticEntity;
use nocter_syntax::{CommentKind, NodeKind, SyntaxElement, SyntaxTree};

use super::{Candidate, SemanticCompletion, SemanticCompletionEdit, exported_candidate};
use crate::AnalysisSnapshot;
use crate::query::SemanticQueryContext;

/// Inconsistency while deriving an importable name and its source edit.
#[derive(Debug)]
pub enum AutomaticImportError {
    Presentation(crate::query::presentation::PresentationError),
    UnknownSource(SourceId),
    SyntaxUnavailable(SourceId),
    UnknownModule(ModuleId),
    UnknownPackage(PackageId),
    UnknownSymbol(Symbol),
    SemanticModuleUnavailable(Box<str>),
}

impl fmt::Display for AutomaticImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Presentation(error) => error.fmt(formatter),
            Self::UnknownSource(source) => {
                write!(formatter, "automatic-import source {source:?} is absent")
            }
            Self::SyntaxUnavailable(source) => {
                write!(
                    formatter,
                    "automatic-import syntax for {source:?} is absent"
                )
            }
            Self::UnknownModule(module) => {
                write!(formatter, "automatic-import module {module:?} is absent")
            }
            Self::UnknownPackage(package) => {
                write!(formatter, "automatic-import package {package:?} is absent")
            }
            Self::UnknownSymbol(symbol) => {
                write!(formatter, "automatic-import symbol {symbol:?} is absent")
            }
            Self::SemanticModuleUnavailable(module) => {
                write!(
                    formatter,
                    "discovered module {module} has no semantic identity"
                )
            }
        }
    }
}

impl std::error::Error for AutomaticImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Presentation(error) => Some(error),
            Self::UnknownSource(_)
            | Self::SyntaxUnavailable(_)
            | Self::UnknownModule(_)
            | Self::UnknownPackage(_)
            | Self::UnknownSymbol(_)
            | Self::SemanticModuleUnavailable(_) => None,
        }
    }
}

impl From<crate::query::presentation::PresentationError> for AutomaticImportError {
    fn from(error: crate::query::presentation::PresentationError) -> Self {
        Self::Presentation(error)
    }
}

pub(super) fn completions(
    snapshot: &AnalysisSnapshot,
    program: SemanticQueryContext<'_>,
    source: SourceId,
    module: ModuleId,
    visible: &BTreeMap<Symbol, Candidate>,
) -> Result<Box<[SemanticCompletion]>, AutomaticImportError> {
    let graph = program.graph();
    let current = graph
        .modules()
        .get(module)
        .ok_or(AutomaticImportError::UnknownModule(module))?;
    let package = graph
        .packages()
        .get(current.package())
        .ok_or(AutomaticImportError::UnknownPackage(current.package()))?;
    let unit = snapshot
        .current_unit()
        .ok_or(AutomaticImportError::UnknownSource(source))?;
    let source_file = snapshot
        .sources()
        .get(source)
        .ok_or(AutomaticImportError::UnknownSource(source))?;
    if !unit.is_root_package_source(source_file.name().as_str()) {
        return Ok(Box::new([]));
    }
    let dependencies = unit
        .package_dependencies(package.identity())
        .ok_or(AutomaticImportError::UnknownPackage(current.package()))?;
    let routes = dependency_routes(dependencies);
    let syntax = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == source)
        .ok_or(AutomaticImportError::SyntaxUnavailable(source))?;
    let insertion = ImportInsertion::new(source_file, syntax);
    let spellings = snapshot.queries.module_spellings(graph, module);
    let cycle_sources = modules_reaching(graph, unit, module)?;
    let mut emitted = BTreeSet::new();
    let mut completions = Vec::new();

    for (candidate_module, namespace) in graph.module_namespaces().iter() {
        if cycle_sources.contains(&candidate_module) {
            continue;
        }
        let declaration = graph
            .modules()
            .get(candidate_module)
            .ok_or(AutomaticImportError::UnknownModule(candidate_module))?;
        let paths = import_paths(graph, current, declaration, &routes)?;
        if paths.is_empty() {
            continue;
        }
        for entry in namespace.authored() {
            if !graph.is_visible_from(entry.visibility(), module, candidate_module) {
                continue;
            }
            let Some(candidate) = exported_candidate(graph, entry.target()) else {
                continue;
            };
            let member = graph
                .symbols()
                .spelling(entry.name())
                .ok_or(AutomaticImportError::UnknownSymbol(entry.name()))?;
            for path in &paths {
                if !emitted.insert((entry.name(), path.clone())) {
                    continue;
                }
                if entry.target().is_selectable_type() {
                    if visible.contains_key(&entry.name()) {
                        continue;
                    }
                    let import = format!("{path}.{member}");
                    completions.push(
                        SemanticCompletion::new(
                            member,
                            candidate.kind,
                            program.completion_detail(candidate.entity, &spellings)?,
                        )
                        .with_entity(candidate.entity)
                        .with_additional_edit(insertion.edit(&import))
                        .with_automatic_import(import),
                    );
                    continue;
                }

                let (namespace, import) = value_namespace(graph, visible, candidate_module, path)?;
                let label = format!("{namespace}.{member}");
                let automatic_import = import.as_deref().unwrap_or(path);
                let completion = SemanticCompletion::new(
                    label,
                    candidate.kind,
                    program.completion_detail(candidate.entity, &spellings)?,
                )
                .with_entity(candidate.entity)
                .with_automatic_import(automatic_import);
                completions.push(if let Some(import) = import {
                    completion.with_additional_edit(insertion.edit(&import))
                } else {
                    completion
                });
            }
        }
    }
    Ok(completions.into_boxed_slice())
}

fn value_namespace(
    graph: &DeclarationGraph,
    visible: &BTreeMap<Symbol, Candidate>,
    module: ModuleId,
    path: &str,
) -> Result<(Box<str>, Option<Box<str>>), AutomaticImportError> {
    let default = path
        .split('/')
        .rev()
        .find(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .unwrap_or("root");
    if let Some(existing) = visible_candidate(graph, visible, default) {
        if existing.entity == SemanticEntity::Module(module) {
            return Ok((default.into(), None));
        }
    }
    if let Some(namespace) = visible_module_namespace(graph, visible, module)? {
        return Ok((namespace, None));
    }
    if visible_candidate(graph, visible, default).is_none() {
        return Ok((default.into(), Some(path.into())));
    }

    let stem = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .collect::<Vec<_>>()
        .join("_");
    let stem = if stem.is_empty() { "root" } else { &stem };
    for suffix in ["", "_module"] {
        let alias = format!("{stem}{suffix}");
        if visible_candidate(graph, visible, &alias).is_none() {
            return Ok((
                alias.clone().into(),
                Some(format!("{path} as {alias}").into()),
            ));
        }
    }
    let mut index = 2_u32;
    loop {
        let alias = format!("{stem}_module_{index}");
        if visible_candidate(graph, visible, &alias).is_none() {
            return Ok((
                alias.clone().into(),
                Some(format!("{path} as {alias}").into()),
            ));
        }
        index = index
            .checked_add(1)
            .ok_or(AutomaticImportError::UnknownModule(module))?;
    }
}

fn visible_module_namespace(
    graph: &DeclarationGraph,
    visible: &BTreeMap<Symbol, Candidate>,
    module: ModuleId,
) -> Result<Option<Box<str>>, AutomaticImportError> {
    let mut namespace: Option<&str> = None;
    for (name, candidate) in visible {
        if candidate.entity != SemanticEntity::Module(module) {
            continue;
        }
        let spelling = graph
            .symbols()
            .spelling(*name)
            .ok_or(AutomaticImportError::UnknownSymbol(*name))?;
        if namespace.is_none() || namespace.is_some_and(|current| spelling < current) {
            namespace = Some(spelling);
        }
    }
    Ok(namespace.map(Into::into))
}

fn visible_candidate(
    graph: &DeclarationGraph,
    visible: &BTreeMap<Symbol, Candidate>,
    spelling: &str,
) -> Option<Candidate> {
    graph
        .symbols()
        .get(spelling)
        .and_then(|symbol| visible.get(&symbol))
        .copied()
}

fn modules_reaching(
    graph: &DeclarationGraph,
    unit: &DiscoveredUnit,
    destination: ModuleId,
) -> Result<BTreeSet<ModuleId>, AutomaticImportError> {
    let edges = unit
        .module_dependencies()
        .iter()
        .map(|dependency| {
            Ok((
                semantic_module(graph, dependency.source())?,
                semantic_module(graph, dependency.target())?,
            ))
        })
        .collect::<Result<Vec<_>, AutomaticImportError>>()?;
    let mut reaching = BTreeSet::from([destination]);
    loop {
        let mut changed = false;
        for (source, target) in &edges {
            if reaching.contains(target) && reaching.insert(*source) {
                changed = true;
            }
        }
        if !changed {
            return Ok(reaching);
        }
    }
}

fn semantic_module(
    graph: &DeclarationGraph,
    discovered: &ModuleIdentity,
) -> Result<ModuleId, AutomaticImportError> {
    let resolve = || {
        let package = graph.package_by_identity(discovered.package())?;
        let path = discovered
            .path()
            .iter()
            .map(|segment| graph.symbols().get(segment))
            .collect::<Option<Vec<_>>>()?;
        graph.module_by_path(package, &ModulePath::from_segments(path))
    };
    resolve().ok_or_else(|| {
        AutomaticImportError::SemanticModuleUnavailable(format!("{discovered:?}").into())
    })
}

fn dependency_routes(
    dependencies: &BTreeMap<Box<str>, PackageIdentity>,
) -> BTreeMap<PackageIdentity, Vec<Box<str>>> {
    let mut routes = BTreeMap::<_, Vec<_>>::new();
    for (alias, dependency) in dependencies {
        routes
            .entry(dependency.clone())
            .or_default()
            .push(alias.clone());
    }
    routes
}

fn import_paths(
    graph: &DeclarationGraph,
    current: &Module,
    target: &Module,
    dependencies: &BTreeMap<PackageIdentity, Vec<Box<str>>>,
) -> Result<Vec<Box<str>>, AutomaticImportError> {
    if target.package() == current.package() {
        return Ok(vec![relative_module_path(
            graph,
            current.path(),
            target.path(),
        )?]);
    }
    let package = graph
        .packages()
        .get(target.package())
        .ok_or(AutomaticImportError::UnknownPackage(target.package()))?;
    let Some(aliases) = dependencies.get(package.identity()) else {
        return Ok(Vec::new());
    };
    aliases
        .iter()
        .map(|alias| absolute_dependency_path(graph, alias, target.path()))
        .collect()
}

fn relative_module_path(
    graph: &DeclarationGraph,
    current: &ModulePath,
    target: &ModulePath,
) -> Result<Box<str>, AutomaticImportError> {
    let shared = current
        .segments()
        .iter()
        .zip(target.segments())
        .take_while(|(left, right)| left == right)
        .count();
    let upward = current.segments().len() - shared;
    let remaining = module_segments(graph, &target.segments()[shared..])?;
    if upward == 0 {
        return Ok(format!("./{}", remaining.join("/")).into());
    }
    if remaining.is_empty() {
        let absolute = module_segments(graph, target.segments())?;
        return Ok(format!("/{}", absolute.join("/")).into());
    }
    Ok(format!("{}{}", "../".repeat(upward), remaining.join("/")).into())
}

fn absolute_dependency_path(
    graph: &DeclarationGraph,
    alias: &str,
    target: &ModulePath,
) -> Result<Box<str>, AutomaticImportError> {
    let segments = module_segments(graph, target.segments())?;
    if segments.is_empty() {
        Ok(alias.into())
    } else {
        Ok(format!("{alias}/{}", segments.join("/")).into())
    }
}

fn module_segments<'graph>(
    graph: &'graph DeclarationGraph,
    segments: &[Symbol],
) -> Result<Vec<&'graph str>, AutomaticImportError> {
    segments
        .iter()
        .map(|segment| {
            graph
                .symbols()
                .spelling(*segment)
                .ok_or(AutomaticImportError::UnknownSymbol(*segment))
        })
        .collect()
}

struct ImportInsertion {
    offset: ByteOffset,
    prefix: &'static str,
    suffix: &'static str,
}

impl ImportInsertion {
    fn new(source: &SourceFile, syntax: &SyntaxTree) -> Self {
        let top_level = syntax
            .children(syntax.root_id())
            .iter()
            .filter_map(|element| {
                let SyntaxElement::Node(node) = element else {
                    return None;
                };
                syntax.node(*node).map(|syntax| (*node, syntax))
            });
        let mut uses = Vec::new();
        let mut first_item = None;
        for (node, child) in top_level {
            match child.kind() {
                NodeKind::UseDeclaration => uses.push(child.range()),
                NodeKind::Item if first_item.is_none() => first_item = Some((node, child.range())),
                _ => {}
            }
        }
        if let Some(last) = uses.last() {
            return Self {
                offset: import_line_end(source, syntax, last.end()),
                prefix: "\n",
                suffix: "",
            };
        }
        if let Some((_, item)) = first_item {
            return Self {
                offset: attached_documentation_start(source, syntax, item.start()),
                prefix: "",
                suffix: "\n\n",
            };
        }
        let text = source.text();
        let (prefix, suffix) = if text.is_empty() || text.ends_with("\n\n") {
            ("", "\n")
        } else if text.ends_with('\n') {
            ("\n", "\n")
        } else {
            ("\n\n", "\n")
        };
        Self {
            offset: source.len(),
            prefix,
            suffix,
        }
    }

    fn edit(&self, import: &str) -> SemanticCompletionEdit {
        SemanticCompletionEdit::new(
            TextRange::new(self.offset, self.offset),
            format!("{}use {import}{}", self.prefix, self.suffix),
        )
    }
}

fn import_line_end(
    source: &SourceFile,
    syntax: &SyntaxTree,
    declaration_end: ByteOffset,
) -> ByteOffset {
    let mut content_end = declaration_end;
    for comment in syntax.lexed().comments().iter().copied() {
        let range = comment.span().range();
        if range.start() < content_end {
            continue;
        }
        let Some(gap) = source.text_at(TextRange::new(content_end, range.start())) else {
            break;
        };
        if !gap.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
            break;
        }
        content_end = range.end();
    }
    let Some(remainder) = source.text_at(TextRange::new(content_end, source.len())) else {
        return content_end;
    };
    let Some(newline) = remainder.find('\n') else {
        return source.len();
    };
    let newline = u32::try_from(newline).expect("source length is bounded by ByteOffset");
    ByteOffset::new(content_end.get() + newline)
}

fn attached_documentation_start(
    source: &SourceFile,
    syntax: &SyntaxTree,
    item_start: ByteOffset,
) -> ByteOffset {
    let mut anchor = item_start;
    for comment in syntax.lexed().comments().iter().rev().copied() {
        if comment.kind() != CommentKind::ItemDocumentation || comment.span().range().end() > anchor
        {
            continue;
        }
        let range = comment.span().range();
        if !is_attachment_gap(source, range.end(), anchor) {
            break;
        }
        anchor = range.start();
    }
    anchor
}

fn is_attachment_gap(source: &SourceFile, start: ByteOffset, end: ByteOffset) -> bool {
    let Some(gap) = source.text_at(TextRange::new(start, end)) else {
        return false;
    };
    gap.bytes().all(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        && !gap.as_bytes().windows(2).any(|window| window == b"\n\n")
        && !gap
            .split('\n')
            .any(|line| !line.is_empty() && line.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::ImportInsertion;

    fn apply_import(source_text: &str, import: &str) -> String {
        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), source_text.as_bytes())
            .unwrap();
        let source = sources.get(source_id).unwrap();
        let syntax = parse(source, ParseGoal::SourceFile);
        assert!(!syntax.has_errors());
        let edit = ImportInsertion::new(source, &syntax).edit(import);
        let offset = usize::try_from(edit.range().start().get()).unwrap();
        format!(
            "{}{}{}",
            &source_text[..offset],
            edit.new_text(),
            &source_text[offset..]
        )
    }

    #[test]
    fn insertion_preserves_file_and_first_item_documentation() {
        let source = concat!(
            "//! Package API.\n",
            "\n",
            "/// Runs the application.\n",
            "func main(): void { return }\n",
        );
        assert_eq!(
            apply_import(source, "std/io"),
            concat!(
                "//! Package API.\n",
                "\n",
                "use std/io\n",
                "\n",
                "/// Runs the application.\n",
                "func main(): void { return }\n",
            )
        );
    }

    #[test]
    fn insertion_extends_the_last_existing_import_group_without_rewriting_it() {
        let source = concat!(
            "use ./first\n",
            "\n",
            "use ./second\n",
            "\n",
            "func main(): void { return }\n",
        );
        assert_eq!(
            apply_import(source, "std/io"),
            concat!(
                "use ./first\n",
                "\n",
                "use ./second\n",
                "use std/io\n",
                "\n",
                "func main(): void { return }\n",
            )
        );
    }

    #[test]
    fn insertion_keeps_a_trailing_import_comment_on_its_original_line() {
        let source = concat!(
            "use ./first // establishes the namespace\n",
            "\n",
            "func main(): void { return }\n",
        );
        assert_eq!(
            apply_import(source, "std/io"),
            concat!(
                "use ./first // establishes the namespace\n",
                "use std/io\n",
                "\n",
                "func main(): void { return }\n",
            )
        );
    }
}
