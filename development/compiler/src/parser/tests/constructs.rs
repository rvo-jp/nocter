use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{ConstructMemberDecl, Item, LiteralShape, TypeExpr};

#[test]
fn parses_type_owned_construction_members_and_contextual_default() {
    let (sources, output) = parse_text_with_sources(
        r#"construct Vec<T> {
    pub default literal [](...items: T): Self {
        return Self.empty()
    }

    pub func new(): Self {
        return make()
    }

    pub func from_iter<I: Iterator<T>>(iterator: I): Self {
        return Self.new()
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Construct(construct) = &ast.items[0] else {
        panic!("expected construct declaration");
    };
    assert!(matches!(construct.target, TypeExpr::Generic(_)));
    assert_eq!(construct.members.len(), 3);
    assert!(construct.members[0].is_default());
    let ConstructMemberDecl::Literal(literal) = &construct.members[0].declaration else {
        panic!("expected literal member");
    };
    assert_eq!(literal.shape, LiteralShape::Sequence);
    let ConstructMemberDecl::Function(function) = &construct.members[2].declaration else {
        panic!("expected function member");
    };
    assert_eq!(function.name, "Vec.from_iter");
    assert_eq!(function.member_name, "from_iter");
    assert_eq!(
        function
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        vec!["T", "I"]
    );

    let json = ast.to_json(&sources);
    assert!(find_json_node(&json, "construct_decl").is_some());
    assert!(find_json_node(&json, "default_modifier").is_some());
    assert!(find_json_node(&json, "construct_literal_member").is_some());
    assert!(find_json_node(&json, "construct_function_member").is_some());
}

#[test]
fn rejects_removed_result_allocation_in_construct_members() {
    let (_sources, output) = parse_text_with_sources(
        r#"construct Text {
    pub default alloc func new(): Self { return make() }
    pub alloc literal ""(text: &str): Self { return make() }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("result `alloc` modifiers have been removed")
    }));
}

#[test]
fn requires_explicit_public_construct_members() {
    let output = parse_text(
        r#"construct Value {
    func new(): Self { return make() }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("construct members must be explicitly marked `pub`")
    }));
}

#[test]
fn rejects_qualified_construct_function_names() {
    let output = parse_text(
        r#"construct Value {
    pub func Value.new(): Self { return make() }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("construct function names are unqualified")
    }));
}

#[test]
fn diagnoses_non_parameter_construct_target_arguments() {
    let output = parse_text(
        r#"construct Value<Vec<T>> {
    pub func new(): Self { return make() }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("construct target arguments must be generic parameter names")
    }));
}
