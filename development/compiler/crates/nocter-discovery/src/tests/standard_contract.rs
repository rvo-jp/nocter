use std::collections::{BTreeMap, BTreeSet};

use nocter_compile_input::ModuleSourceKind;
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::{Keyword, NodeKind, SyntaxElement, TokenKind, declaration_name_token};

pub(super) fn assert_standard_root_visibility_boundaries(unit: &crate::DiscoveredUnit) {
    let mut restricted_public_roots = Vec::new();
    let mut globally_visible_internal_roots = Vec::new();

    for module in unit.modules() {
        let path = module.identity().path();
        let internal = path
            .first()
            .is_some_and(|segment| segment.as_ref() == "internal");
        let representation_bound_mem = path.len() == 1 && path[0].as_ref() == "mem";

        for source in module
            .sources()
            .iter()
            .filter(|source| source.kind() == ModuleSourceKind::Root)
        {
            let tree = &unit.syntax_trees()[source.syntax_index()];
            let source_file = unit.sources().get(tree.source()).unwrap();
            for element in tree.children(tree.root_id()) {
                let SyntaxElement::Node(declaration) = element else {
                    continue;
                };
                let Some(visibility) = declaration_visibility(tree, source_file, *declaration)
                else {
                    continue;
                };
                let name = declaration_name_token(tree, *declaration)
                    .and_then(|token| source_file.text_at(token.range()))
                    .unwrap_or("<anonymous>");
                if internal && visibility == "pub" {
                    globally_visible_internal_roots
                        .push(format!("{}:{name}", source.canonical_path()));
                }
                if !internal
                    && !representation_bound_mem
                    && matches!(visibility.as_str(), "pub(/)" | "pub(./)")
                {
                    restricted_public_roots.push(format!("{}:{name}", source.canonical_path()));
                }
            }
        }
    }

    assert!(
        restricted_public_roots.is_empty(),
        "user-facing standard roots must not contain package plumbing; move independent contracts below std/internal: {restricted_public_roots:#?}"
    );
    assert!(
        globally_visible_internal_roots.is_empty(),
        "std/internal roots must not expose globally visible declarations: {globally_visible_internal_roots:#?}"
    );
}

/// Exact cross-module dependencies accepted by the standard-library ownership review.
const REVIEWED_STANDARD_DEPENDENCIES: &[(&str, &str)] = &[
    ("fmt", "internal/mem"),
    ("fmt", "internal/ptr"),
    ("fmt", "ptr"),
    ("fmt", "string"),
    ("fs", "internal/io"),
    ("fs", "internal/os"),
    ("fs", "internal/os/darwin"),
    ("fs", "internal/path"),
    ("fs", "internal/ptr"),
    ("fs", "io"),
    ("fs", "mem"),
    ("fs", "path"),
    ("fs", "ptr"),
    ("fs", "string"),
    ("fs", "vec"),
    ("hash", "internal/hash"),
    ("hash", "internal/ptr"),
    ("hash", "num"),
    ("hash", "ptr"),
    ("hash", "string"),
    ("hash", "vec"),
    ("internal/hash", "internal/os/darwin"),
    ("internal/hash", "ptr"),
    ("internal/io", "internal/os"),
    ("internal/os/darwin", "internal/os"),
    ("internal/os/darwin", "ptr"),
    ("internal/path", "internal/mem"),
    ("internal/path", "internal/ptr"),
    ("internal/path", "mem"),
    ("internal/path", "path"),
    ("internal/path", "ptr"),
    ("internal/ptr", "ptr"),
    ("internal/safety", "internal/os/darwin"),
    ("internal/table", "hash"),
    ("internal/table", "internal/mem"),
    ("internal/table", "internal/ptr"),
    ("internal/table", "mem"),
    ("internal/table", "vec"),
    ("internal/utf8", "internal/ptr"),
    ("internal/utf8", "ptr"),
    ("io", "internal/io"),
    ("io", "internal/os/darwin"),
    ("io", "internal/path"),
    ("io", "ptr"),
    ("io", "string"),
    ("io", "vec"),
    ("io/buffer", "io"),
    ("io/buffer", "internal/io"),
    ("io/buffer", "string"),
    ("io/buffer", "vec"),
    ("iter", "internal/ptr"),
    ("iter", "ptr"),
    ("iter/collect", "iter"),
    ("iter/collect", "vec"),
    ("json", "fmt"),
    ("json", "internal/mem"),
    ("json", "internal/utf8"),
    ("json", "map"),
    ("json", "mem"),
    ("json", "string"),
    ("json", "vec"),
    ("map", "hash"),
    ("map", "internal/safety"),
    ("map", "internal/table"),
    ("map", "iter"),
    ("map", "mem"),
    ("mem", "internal/mem"),
    ("mem", "internal/os/darwin"),
    ("mem", "internal/ptr"),
    ("mem", "ptr"),
    ("num", "fmt"),
    ("num", "internal/mem"),
    ("num", "mem"),
    ("num", "string"),
    ("path", "string"),
    ("prelude", "fmt"),
    ("prelude", "iter"),
    ("prelude", "map"),
    ("prelude", "set"),
    ("prelude", "string"),
    ("prelude", "vec"),
    ("process", "internal/os/darwin"),
    ("process", "internal/path"),
    ("process", "internal/ptr"),
    ("process", "mem"),
    ("process", "ptr"),
    ("process", "string"),
    ("process", "vec"),
    ("set", "hash"),
    ("set", "internal/table"),
    ("set", "iter"),
    ("set", "mem"),
    ("slice", "internal/ptr"),
    ("str", "internal/ptr"),
    ("str", "iter"),
    ("str", "string"),
    ("str", "vec"),
    ("string", "internal/mem"),
    ("string", "internal/ptr"),
    ("string", "internal/utf8"),
    ("string", "mem"),
    ("vec", "internal/mem"),
    ("vec", "internal/ptr"),
    ("vec", "iter"),
    ("vec", "mem"),
    ("vec", "ptr"),
];

pub(super) fn assert_reviewed_standard_dependencies(
    input: &nocter_compile_input::CompileUnitInput<'_>,
) {
    let source_modules = input
        .modules()
        .iter()
        .flat_map(|module| {
            module
                .sources()
                .iter()
                .map(move |source| (source.syntax().source(), module.identity()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeSet::new();
    for resolution in input.use_resolutions() {
        let Some(source) = source_modules.get(&resolution.declaration().source()) else {
            continue;
        };
        if source.package() != resolution.target_module().package() {
            continue;
        }
        let source_path = module_path(source.path());
        let target_path = module_path(resolution.target_module().path());
        if source_path == target_path {
            continue;
        }
        actual.insert((source_path, target_path));
    }
    let expected = REVIEWED_STANDARD_DEPENDENCIES
        .iter()
        .map(|(source, target)| ((*source).to_owned(), (*target).to_owned()))
        .collect::<BTreeSet<_>>();
    let unexpected = actual.difference(&expected).collect::<Vec<_>>();
    let stale = expected.difference(&actual).collect::<Vec<_>>();
    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "standard module dependency review must exactly match the authored graph; unexpected: {unexpected:#?}; stale: {stale:#?}"
    );
}

pub(super) fn assert_private_implementation_functions_are_referenced(
    unit: &crate::DiscoveredUnit,
    checked: &nocter_checking::CheckedProgramOutput,
) {
    let root_callables = unit
        .modules()
        .iter()
        .flat_map(crate::DiscoveredModule::sources)
        .filter(|source| source.kind() == ModuleSourceKind::Root)
        .flat_map(|source| {
            let tree = &unit.syntax_trees()[source.syntax_index()];
            tree.nodes()
                .filter(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
                .filter_map(|(node, _)| declaration_callable(tree, node, checked))
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    let mut unused = Vec::new();
    for module in unit.modules() {
        for source in module
            .sources()
            .iter()
            .filter(|source| source.kind() != ModuleSourceKind::Root)
        {
            let tree = &unit.syntax_trees()[source.syntax_index()];
            let source_file = unit.sources().get(tree.source()).unwrap();
            for (node, _) in tree
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            {
                if contains_direct_keyword(tree, node, Keyword::Primitive) {
                    continue;
                }
                let Some(entity) = declaration_callable(tree, node, checked) else {
                    continue;
                };
                if root_callables.contains(&entity)
                    || checked
                        .source_index()
                        .bindings_for(entity)
                        .iter()
                        .any(|binding| binding.role() == SourceRole::Reference)
                {
                    continue;
                }
                let name = declaration_name_token(tree, node)
                    .and_then(|token| source_file.text_at(token.range()))
                    .unwrap_or("<anonymous>");
                unused.push(format!("{}:{name}", source.canonical_path()));
            }
        }
    }
    assert!(
        unused.is_empty(),
        "private standard implementation functions must have a semantic reference: {unused:#?}"
    );
}

fn contains_direct_keyword(
    tree: &nocter_syntax::SyntaxTree,
    node: nocter_syntax::NodeId,
    keyword: Keyword,
) -> bool {
    tree.children(node).iter().any(|child| {
        matches!(child, SyntaxElement::Token(token) if token.kind() == TokenKind::Keyword(keyword))
    })
}

fn declaration_callable(
    tree: &nocter_syntax::SyntaxTree,
    declaration: nocter_syntax::NodeId,
    checked: &nocter_checking::CheckedProgramOutput,
) -> Option<SemanticEntity> {
    let name = declaration_name_token(tree, declaration)?;
    checked
        .source_index()
        .bindings_at(tree.source(), name.range().start())
        .find_map(|binding| match (binding.role(), binding.entity()) {
            (SourceRole::Declaration, entity @ SemanticEntity::Callable(_)) => Some(entity),
            _ => None,
        })
}

pub(super) fn assert_standard_self_uses_are_package_absolute(
    input: &nocter_compile_input::CompileUnitInput<'_>,
) {
    let source_modules = input
        .modules()
        .iter()
        .flat_map(|module| {
            module
                .sources()
                .iter()
                .map(move |source| (source.syntax().source(), module.identity()))
        })
        .collect::<BTreeMap<_, _>>();
    let source_trees = input
        .modules()
        .iter()
        .flat_map(nocter_compile_input::ModuleInput::sources)
        .map(|source| (source.syntax().source(), source.syntax()))
        .collect::<BTreeMap<_, _>>();
    let mut non_absolute = Vec::new();
    for resolution in input.use_resolutions() {
        let declaration = resolution.declaration();
        let Some(source_module) = source_modules.get(&declaration.source()) else {
            continue;
        };
        if source_module.package() != resolution.target_module().package() {
            continue;
        }
        let tree = source_trees
            .get(&declaration.source())
            .expect("resolved standard use has no syntax tree");
        let node = tree
            .node(declaration)
            .expect("resolved standard use has no syntax node");
        let source = input
            .sources()
            .get(declaration.source())
            .expect("resolved standard use has no source file");
        let spelling = source
            .text_at(node.range())
            .expect("resolved standard use range is outside its source");
        let path = spelling
            .find("use")
            .map(|offset| spelling[offset + "use".len()..].trim_start());
        if !path.is_some_and(|path| path.starts_with('/')) {
            non_absolute.push(format!("{}: {spelling}", source.name()));
        }
    }
    assert!(
        non_absolute.is_empty(),
        "standard self imports must use package-absolute / paths so the authored tree remains analyzable as an ordinary package: {non_absolute:#?}"
    );
}

fn module_path(path: &[Box<str>]) -> String {
    path.iter()
        .map(AsRef::as_ref)
        .collect::<Vec<&str>>()
        .join("/")
}

pub(super) fn assert_package_visible_functions_have_cross_module_references(
    unit: &crate::DiscoveredUnit,
    checked: &nocter_checking::CheckedProgramOutput,
) {
    let source_modules = unit
        .modules()
        .iter()
        .flat_map(|module| {
            module.sources().iter().map(move |source| {
                (
                    unit.syntax_trees()[source.syntax_index()].source(),
                    module.identity(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut unnecessary = Vec::new();
    for module in unit.modules() {
        for source in module
            .sources()
            .iter()
            .filter(|source| source.kind() == ModuleSourceKind::Root)
        {
            let tree = &unit.syntax_trees()[source.syntax_index()];
            let source_file = unit.sources().get(tree.source()).unwrap();
            for (node, syntax) in tree
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            {
                if !is_package_visible_function(tree, source_file, node) {
                    continue;
                }
                let Some(name) = declaration_name_token(tree, node) else {
                    panic!("package-visible function has no declaration name: {syntax:?}");
                };
                let entity = checked
                    .source_index()
                    .bindings_at(tree.source(), name.range().start())
                    .find_map(|binding| match (binding.role(), binding.entity()) {
                        (SourceRole::Declaration, entity @ SemanticEntity::Callable(_)) => {
                            Some(entity)
                        }
                        _ => None,
                    })
                    .expect("package-visible function has no semantic callable binding");
                let cross_module_reference = checked
                    .source_index()
                    .bindings_for(entity)
                    .iter()
                    .filter(|binding| binding.role() == SourceRole::Reference)
                    .any(|binding| {
                        source_modules
                            .get(&binding.origin().source())
                            .is_some_and(|referencing| *referencing != module.identity())
                    });
                if !cross_module_reference {
                    unnecessary.push(format!(
                        "{}:{}",
                        source.canonical_path(),
                        source_file.text_at(name.range()).unwrap_or("<unknown>")
                    ));
                }
            }
        }
    }
    assert!(
        unnecessary.is_empty(),
        "package-visible standard functions need a cross-module reference; keep module-local helpers private: {unnecessary:#?}"
    );
}

fn is_package_visible_function(
    tree: &nocter_syntax::SyntaxTree,
    source: &nocter_source::SourceFile,
    declaration: nocter_syntax::NodeId,
) -> bool {
    tree.children(declaration).iter().any(|element| {
        let SyntaxElement::Node(visibility) = element else {
            return false;
        };
        tree.node(*visibility).is_some_and(|visibility| {
            visibility.kind() == NodeKind::Visibility
                && source
                    .text_at(visibility.range())
                    .is_some_and(|text| text.split_whitespace().collect::<String>() == "pub(/)")
        })
    })
}

fn declaration_visibility(
    tree: &nocter_syntax::SyntaxTree,
    source: &nocter_source::SourceFile,
    declaration: nocter_syntax::NodeId,
) -> Option<String> {
    tree.children(declaration).iter().find_map(|element| {
        let SyntaxElement::Node(visibility) = element else {
            return None;
        };
        tree.node(*visibility)
            .filter(|visibility| visibility.kind() == NodeKind::Visibility)
            .and_then(|visibility| source.text_at(visibility.range()))
            .map(|text| text.split_whitespace().collect())
    })
}
