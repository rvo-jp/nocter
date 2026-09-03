use std::collections::HashMap;

use nocter_source::SourceFile;

use crate::{
    ExpectedSyntax, LexDiagnosticKind, NodeId, NodeKind, ParseDiagnosticKind, SyntaxElement,
    SyntaxOrigin, SyntaxToken, SyntaxTree, TokenKind,
};

/// Canonical, source-identity-independent syntax that can affect declaration semantics.
///
/// Function and method block contents are represented only by a block marker. Comments,
/// documentation, whitespace, and source coordinates are deliberately absent. This product is an
/// invalidation boundary, not a second parser or a declaration model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationSyntaxSurface {
    canonical: Box<[u8]>,
}

impl DeclarationSyntaxSurface {
    /// Returns deterministic bytes suitable for a revision-local semantic fingerprint.
    #[must_use]
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}

/// Source-identity-independent address of one node or token on a declaration syntax surface.
///
/// Locators are valid only with a projection whose semantic surface is equal to the projection
/// that produced them. Body descendants deliberately have no locator.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationSyntaxLocator {
    Node(u32),
    Token(u32),
}

/// One current syntax tree's binding to its source-neutral declaration surface.
///
/// This is the sole syntax-owned bridge used to turn a reusable semantic projection recipe back
/// into generation-local syntax identities. It does not contain semantic entities.
#[derive(Clone, Debug)]
pub struct DeclarationSyntaxProjection {
    surface: DeclarationSyntaxSurface,
    bodies: Box<[crate::BodySyntaxSurface]>,
    nodes: Box<[NodeId]>,
    tokens: Box<[SyntaxToken]>,
    node_locators: HashMap<NodeId, u32>,
    token_locators: HashMap<SyntaxToken, u32>,
}

impl DeclarationSyntaxProjection {
    /// Returns the source-neutral semantic surface owned by this projection.
    #[must_use]
    pub const fn surface(&self) -> &DeclarationSyntaxSurface {
        &self.surface
    }

    /// Consumes the binding and returns its source-neutral semantic surface.
    #[must_use]
    pub fn into_surface(self) -> DeclarationSyntaxSurface {
        self.surface
    }

    /// Returns executable bodies keyed by their stable declaration-surface block locator.
    #[must_use]
    pub const fn body_surfaces(&self) -> &[crate::BodySyntaxSurface] {
        &self.bodies
    }

    /// Converts a current-generation syntax identity into a stable surface locator.
    #[must_use]
    pub fn locate(&self, origin: SyntaxOrigin) -> Option<DeclarationSyntaxLocator> {
        match origin {
            SyntaxOrigin::Node(node) => self
                .node_locators
                .get(&node)
                .copied()
                .map(DeclarationSyntaxLocator::Node),
            SyntaxOrigin::Token(token) => self
                .token_locators
                .get(&token)
                .copied()
                .map(DeclarationSyntaxLocator::Token),
        }
    }

    /// Resolves a stable locator into this projection's current-generation syntax identity.
    #[must_use]
    pub fn resolve(&self, locator: DeclarationSyntaxLocator) -> Option<SyntaxOrigin> {
        match locator {
            DeclarationSyntaxLocator::Node(index) => self
                .nodes
                .get(usize::try_from(index).ok()?)
                .copied()
                .map(SyntaxOrigin::Node),
            DeclarationSyntaxLocator::Token(index) => self
                .tokens
                .get(usize::try_from(index).ok()?)
                .copied()
                .map(SyntaxOrigin::Token),
        }
    }
}

/// Binds one current syntax tree to its source-neutral declaration surface.
///
/// Returns `None` when the source does not own the tree. The caller therefore cannot accidentally
/// create locators from text in a different source-identity domain.
#[must_use]
pub fn project_declaration_syntax(
    tree: &SyntaxTree,
    source: &SourceFile,
) -> Option<DeclarationSyntaxProjection> {
    (tree.source() == source.id()).then(|| declaration_projection(tree, source.text()))
}

pub(crate) fn declaration_projection(
    tree: &SyntaxTree,
    normalized_text: &str,
) -> DeclarationSyntaxProjection {
    enum Visit {
        Element(SyntaxElement),
        CloseNode,
    }

    let mut canonical = Vec::new();
    let mut nodes = Vec::new();
    let mut tokens = Vec::new();
    let mut bodies = Vec::new();
    let mut node_locators = HashMap::new();
    let mut token_locators = HashMap::new();
    let mut pending = vec![Visit::Element(SyntaxElement::Node(tree.root_id()))];
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Element(SyntaxElement::Node(node)) => {
                let index = u32::try_from(nodes.len())
                    .expect("declaration surface node count fits stable locator domain");
                nodes.push(node);
                node_locators.insert(node, index);
                let syntax = tree
                    .node(node)
                    .expect("surface traversal retains one syntax-tree owner");
                encode(0, syntax.kind().as_str().as_bytes(), &mut canonical);
                pending.push(Visit::CloseNode);
                if syntax.kind() == NodeKind::Block {
                    bodies.push(crate::body_surface::body_surface(
                        DeclarationSyntaxLocator::Node(index),
                        tree,
                        node,
                        normalized_text,
                    ));
                } else {
                    pending.extend(
                        tree.children(node)
                            .iter()
                            .rev()
                            .copied()
                            .map(Visit::Element),
                    );
                }
            }
            Visit::Element(SyntaxElement::Token(token)) => {
                if matches!(token.kind(), TokenKind::Newline | TokenKind::Eof) {
                    continue;
                }
                let index = u32::try_from(tokens.len())
                    .expect("declaration surface token count fits stable locator domain");
                tokens.push(token);
                token_locators.insert(token, index);
                encode(1, token.kind().as_str().as_bytes(), &mut canonical);
                let text = text_at(normalized_text, token.range());
                encode(2, text.as_bytes(), &mut canonical);
            }
            Visit::Element(SyntaxElement::Missing(missing)) => {
                encode_expected(missing.expected(), &mut canonical);
            }
            Visit::CloseNode => canonical.push(4),
        }
    }
    encode_declaration_diagnostics(tree, &mut canonical);
    DeclarationSyntaxProjection {
        surface: DeclarationSyntaxSurface {
            canonical: canonical.into_boxed_slice(),
        },
        bodies: bodies.into_boxed_slice(),
        nodes: nodes.into_boxed_slice(),
        tokens: tokens.into_boxed_slice(),
        node_locators,
        token_locators,
    }
}

fn encode_declaration_diagnostics(tree: &SyntaxTree, canonical: &mut Vec<u8>) {
    let body_ranges = tree
        .nodes()
        .filter(|(_, node)| node.kind() == NodeKind::Block)
        .map(|(_, node)| node.range())
        .collect::<Vec<_>>();
    for diagnostic in tree.lexed().diagnostics() {
        if body_ranges
            .iter()
            .any(|body| body.contains_range(diagnostic.span().range()))
        {
            continue;
        }
        encode(
            6,
            lex_diagnostic_name(diagnostic.kind()).as_bytes(),
            canonical,
        );
    }
    for diagnostic in tree.diagnostics() {
        if body_ranges
            .iter()
            .any(|body| body.contains_range(diagnostic.span().range()))
        {
            continue;
        }
        match diagnostic.kind() {
            ParseDiagnosticKind::Expected(expected) => {
                encode(7, b"expected", canonical);
                encode_expected(expected, canonical);
            }
            ParseDiagnosticKind::LateDependencyDeclaration => {
                encode(7, b"late_dependency_declaration", canonical);
            }
            ParseDiagnosticKind::NestingLimit => {
                encode(7, b"nesting_limit", canonical);
            }
        }
    }
}

const fn lex_diagnostic_name(kind: LexDiagnosticKind) -> &'static str {
    match kind {
        LexDiagnosticKind::UnexpectedCharacter => "unexpected_character",
        LexDiagnosticKind::UnterminatedBlockComment => "unterminated_block_comment",
        LexDiagnosticKind::InvalidIntegerLiteral => "invalid_integer_literal",
        LexDiagnosticKind::UnsupportedFloatLiteral => "unsupported_float_literal",
        LexDiagnosticKind::UnterminatedString => "unterminated_string",
        LexDiagnosticKind::SingleLineStringNewline => "single_line_string_newline",
        LexDiagnosticKind::MultilineStringOpeningNewline => "multiline_string_opening_newline",
        LexDiagnosticKind::InvalidEscape => "invalid_escape",
        LexDiagnosticKind::InvalidStringUtf8 => "invalid_string_utf8",
        LexDiagnosticKind::MultilineStringIndentation => "multiline_string_indentation",
        LexDiagnosticKind::UnterminatedByteLiteral => "unterminated_byte_literal",
        LexDiagnosticKind::ByteLiteralNewline => "byte_literal_newline",
        LexDiagnosticKind::InvalidByteLength => "invalid_byte_length",
        LexDiagnosticKind::UnterminatedInterpolation => "unterminated_interpolation",
        LexDiagnosticKind::UnterminatedCharacterLiteral => "unterminated_character_literal",
        LexDiagnosticKind::CharacterLiteralNewline => "character_literal_newline",
        LexDiagnosticKind::InvalidCharacterLength => "invalid_character_length",
    }
}

fn encode_expected(expected: ExpectedSyntax, output: &mut Vec<u8>) {
    let (category, detail) = match expected {
        ExpectedSyntax::Token(kind) => ("token", kind.as_str()),
        ExpectedSyntax::Keyword(keyword) => ("keyword", keyword.as_str()),
        ExpectedSyntax::Punctuation(punctuation) => ("punctuation", punctuation.as_str()),
        ExpectedSyntax::Contextual(name) => ("contextual", name),
        ExpectedSyntax::Name => ("name", ""),
        ExpectedSyntax::Visibility => ("visibility", ""),
        ExpectedSyntax::PackageDirectiveName => ("package_directive_name", ""),
        ExpectedSyntax::DirectiveValue => ("directive_value", ""),
        ExpectedSyntax::StringLiteral => ("string_literal", ""),
        ExpectedSyntax::ModuleSegment => ("module_segment", ""),
        ExpectedSyntax::Type => ("type", ""),
        ExpectedSyntax::Parameter => ("parameter", ""),
        ExpectedSyntax::TargetableItem => ("targetable_item", ""),
        ExpectedSyntax::Item => ("item", ""),
        ExpectedSyntax::DeclarationMember => ("declaration_member", ""),
        ExpectedSyntax::AssociatedTypeBinding => ("associated_type_binding", ""),
        ExpectedSyntax::DeclarationTypePattern => ("declaration_type_pattern", ""),
        ExpectedSyntax::Receiver => ("receiver", ""),
        ExpectedSyntax::Block => ("block", ""),
        ExpectedSyntax::LiteralShape => ("literal_shape", ""),
        ExpectedSyntax::Expression => ("expression", ""),
        ExpectedSyntax::BindingPattern => ("binding_pattern", ""),
        ExpectedSyntax::AssignmentTarget => ("assignment_target", ""),
        ExpectedSyntax::EnumPattern => ("enum_pattern", ""),
        ExpectedSyntax::ClosureHead => ("closure_head", ""),
        ExpectedSyntax::Predicate => ("predicate", ""),
        ExpectedSyntax::Interface => ("interface", ""),
        ExpectedSyntax::Newline => ("newline", ""),
    };
    encode(3, category.as_bytes(), output);
    encode(5, detail.as_bytes(), output);
}

fn encode(tag: u8, bytes: &[u8], output: &mut Vec<u8>) {
    output.push(tag);
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn text_at(text: &str, range: nocter_source::TextRange) -> &str {
    let start = usize::try_from(range.start().get()).expect("source offsets fit usize");
    let end = usize::try_from(range.end().get()).expect("source offsets fit usize");
    text.get(start..end)
        .expect("syntax token ranges address normalized source text")
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};

    use crate::{
        NodeKind, ParseGoal, SyntaxOrigin, declaration_name_token, parse, parse_reusable,
        project_declaration_syntax,
    };

    fn surface(text: &str) -> super::DeclarationSyntaxSurface {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("surface.nct"), text.as_bytes())
            .unwrap();
        parse_reusable(sources.get(source).unwrap(), ParseGoal::SourceFile).declaration_surface()
    }

    #[test]
    fn body_edits_preserve_the_declaration_surface() {
        assert_eq!(
            surface("func answer(): i32 { return 1 }\n"),
            surface("func answer(): i32 { let value = 40\n return value + 2 }\n")
        );
    }

    #[test]
    fn body_surfaces_change_independently_under_stable_declaration_locators() {
        let (first_tree, first_source) = tree(concat!(
            "func first(): i32 { return 1 }\n",
            "func second(): i32 { return 2 }\n",
        ));
        let (second_tree, second_source) = tree(concat!(
            "func first(): i32 { return 10 }\n",
            "func second(): i32 { return 2 }\n",
        ));
        let first = project_declaration_syntax(&first_tree, &first_source).unwrap();
        let second = project_declaration_syntax(&second_tree, &second_source).unwrap();

        assert_eq!(first.surface(), second.surface());
        assert_eq!(first.body_surfaces().len(), 2);
        assert_eq!(second.body_surfaces().len(), 2);
        assert_eq!(
            first.body_surfaces()[0].locator(),
            second.body_surfaces()[0].locator()
        );
        assert_ne!(first.body_surfaces()[0], second.body_surfaces()[0]);
        assert_eq!(first.body_surfaces()[1], second.body_surfaces()[1]);
    }

    #[test]
    fn declaration_edits_change_the_declaration_surface() {
        assert_ne!(
            surface("func answer(): i32 { return 1 }\n"),
            surface("func answer(): usize { return 1 }\n")
        );
        assert_ne!(
            surface("func answer(): i32\n"),
            surface("func answer(): i32 { return 1 }\n")
        );
    }

    #[test]
    fn formatting_and_documentation_do_not_change_semantic_surface() {
        assert_eq!(
            surface("/// Computes.\nfunc answer(): i32 { return 1 }\n"),
            surface("func answer(  ): i32 { return 1 }\n")
        );
    }

    #[test]
    fn declarations_after_a_body_remain_part_of_the_surface() {
        assert_ne!(
            surface("func first(): i32 { return 1 }\nfunc second(): i32\n"),
            surface("func first(): i32 { return 2 }\nfunc second(): usize\n")
        );
    }

    #[test]
    fn body_diagnostics_do_not_poison_the_declaration_boundary() {
        assert_eq!(
            surface("func answer(): i32 { return 1 }\n"),
            surface("func answer(): i32 { @ }\n")
        );
        assert_ne!(
            surface("func answer(): i32 { return 1 }\n"),
            surface("@ func answer(): i32 { return 1 }\n")
        );
    }

    #[test]
    fn stable_locators_rebind_declarations_across_body_edits() {
        let (first_tree, first_source) = tree("func answer(): i32 { return 1 }\n");
        let (second_tree, second_source) =
            tree("/// Updated.\nfunc answer( ): i32 { let value = 2\n return value }\n");
        let first = project_declaration_syntax(&first_tree, &first_source).unwrap();
        let second = project_declaration_syntax(&second_tree, &second_source).unwrap();
        assert_eq!(first.surface(), second.surface());

        let first_function = node(&first_tree, NodeKind::FunctionDeclaration);
        let second_function = node(&second_tree, NodeKind::FunctionDeclaration);
        let first_name = declaration_name_token(&first_tree, first_function).unwrap();
        let second_name = declaration_name_token(&second_tree, second_function).unwrap();
        let locator = first.locate(SyntaxOrigin::Token(first_name)).unwrap();
        assert_eq!(
            second.resolve(locator),
            Some(SyntaxOrigin::Token(second_name))
        );
    }

    #[test]
    fn body_descendants_cannot_escape_through_surface_locators() {
        let (tree, source) = tree("func answer(): i32 { return 1 }\n");
        let projection = project_declaration_syntax(&tree, &source).unwrap();
        let body = node(&tree, NodeKind::Block);
        let statement = node(&tree, NodeKind::ReturnStatement);
        assert!(projection.locate(SyntaxOrigin::Node(body)).is_some());
        assert_eq!(projection.locate(SyntaxOrigin::Node(statement)), None);
    }

    fn tree(text: &str) -> (crate::SyntaxTree, nocter_source::SourceFile) {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("surface.nct"), text.as_bytes())
            .unwrap();
        let source = sources.get(source).unwrap().clone();
        let tree = parse(&source, ParseGoal::SourceFile);
        (tree, source)
    }

    fn node(tree: &crate::SyntaxTree, kind: NodeKind) -> crate::NodeId {
        tree.nodes()
            .find_map(|(id, node)| (node.kind() == kind).then_some(id))
            .unwrap()
    }
}
