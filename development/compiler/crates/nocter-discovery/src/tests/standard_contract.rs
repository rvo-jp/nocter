use std::collections::BTreeMap;

use nocter_compile_input::ModuleSourceKind;
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::{NodeKind, SyntaxElement, declaration_name_token};

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
