use std::collections::HashSet;

use nocter_diagnostics::{SourceDiagnostic, syntax_diagnostics};
use nocter_source::{SourceFile, SourceMap, SourceName};
use nocter_syntax::{Keyword, NodeKind, Punctuation, SyntaxElement, SyntaxTree, Token, TokenKind};

use crate::FormatError;

pub(super) fn format(source: &SourceFile, syntax: &SyntaxTree) -> Result<String, FormatError> {
    if syntax.has_errors() {
        return Err(FormatError::Diagnostics(syntax_diagnostics(
            std::slice::from_ref(syntax),
        )));
    }
    if let Some(comment) = syntax.lexed().comments().first().copied() {
        return Err(FormatError::Diagnostics(vec![SourceDiagnostic::new(
            "E0601",
            "formatting source with comments is not supported yet",
            comment.span(),
            [],
            Some(
                "remove the comment or leave this file unformatted until comment-preserving formatting is available",
            ),
        )]
        .into_boxed_slice()));
    }
    let formatted = Formatter::new(source, syntax).run();
    validate_syntax(source, syntax, &formatted)?;
    Ok(formatted)
}

fn validate_syntax(
    source: &SourceFile,
    original: &SyntaxTree,
    formatted: &str,
) -> Result<(), FormatError> {
    let mut sources = SourceMap::new();
    let formatted_id = sources
        .add_bytes(
            SourceName::new(source.name().as_str()),
            formatted.as_bytes(),
        )
        .map_err(|_| FormatError::ChangedSyntax)?;
    let formatted_source = sources
        .get(formatted_id)
        .ok_or(FormatError::ChangedSyntax)?;
    let goal = match original.root().kind() {
        NodeKind::PackageFile => nocter_syntax::ParseGoal::PackageFile,
        NodeKind::ModuleSource => nocter_syntax::ParseGoal::ModuleSource,
        _ => return Err(FormatError::ChangedSyntax),
    };
    let formatted_tree = nocter_syntax::parse(formatted_source, goal);
    if formatted_tree.has_errors()
        || !same_tree(source, original, formatted_source, &formatted_tree)
    {
        return Err(FormatError::ChangedSyntax);
    }
    Ok(())
}

fn same_tree(
    left_source: &SourceFile,
    left: &SyntaxTree,
    right_source: &SourceFile,
    right: &SyntaxTree,
) -> bool {
    let left_nodes = left.nodes().collect::<Vec<_>>();
    let right_nodes = right.nodes().collect::<Vec<_>>();
    if left.root_id().index() != right.root_id().index() || left_nodes.len() != right_nodes.len() {
        return false;
    }
    left_nodes
        .into_iter()
        .zip(right_nodes)
        .all(|((left_id, left_node), (right_id, right_node))| {
            left_id.index() == right_id.index()
                && left_node.kind() == right_node.kind()
                && same_children(
                    left_source,
                    left.children(left_id),
                    right_source,
                    right.children(right_id),
                )
        })
}

fn same_children(
    left_source: &SourceFile,
    left: &[SyntaxElement],
    right_source: &SourceFile,
    right: &[SyntaxElement],
) -> bool {
    let left = significant_children(left).collect::<Vec<_>>();
    let right = significant_children(right).collect::<Vec<_>>();
    left.len() == right.len()
        && left
            .into_iter()
            .zip(right)
            .all(|(left, right)| same_element(left_source, left, right_source, right))
}

fn significant_children(children: &[SyntaxElement]) -> impl Iterator<Item = &SyntaxElement> {
    children.iter().filter(|child| {
        !matches!(
            child,
            SyntaxElement::Token(token) if token.kind() == TokenKind::Newline
        )
    })
}

fn same_element(
    left_source: &SourceFile,
    left: &SyntaxElement,
    right_source: &SourceFile,
    right: &SyntaxElement,
) -> bool {
    match (left, right) {
        (SyntaxElement::Node(left), SyntaxElement::Node(right)) => left.index() == right.index(),
        (SyntaxElement::Token(left), SyntaxElement::Token(right)) => {
            left.kind() == right.kind()
                && left_source.text_at(left.range()) == right_source.text_at(right.range())
        }
        (SyntaxElement::Missing(_), SyntaxElement::Missing(_)) => true,
        _ => false,
    }
}

struct Formatter<'syntax> {
    source: &'syntax SourceFile,
    syntax: &'syntax SyntaxTree,
    parent_kinds: Vec<Option<NodeKind>>,
    top_level_items: HashSet<u32>,
    output: String,
    delimiter_depth: usize,
    at_line_start: bool,
    pending_newlines: u8,
    previous: Option<Token>,
    previous_parent: Option<NodeKind>,
}

impl<'syntax> Formatter<'syntax> {
    fn new(source: &'syntax SourceFile, syntax: &'syntax SyntaxTree) -> Self {
        let mut parent_kinds = vec![None; syntax.lexed().tokens().len()];
        for (node_id, node) in syntax.nodes() {
            for child in syntax.children(node_id) {
                if let SyntaxElement::Token(token) = child {
                    parent_kinds[token.lexical().index()].get_or_insert(node.kind());
                }
            }
        }
        let top_level_items = syntax
            .children(syntax.root_id())
            .iter()
            .filter_map(|child| {
                let SyntaxElement::Node(id) = child else {
                    return None;
                };
                let node = syntax
                    .node(*id)
                    .expect("root child belongs to the same syntax tree");
                matches!(node.kind(), NodeKind::Item | NodeKind::UseDeclaration)
                    .then_some(node.range().start().get())
            })
            .collect();
        Self {
            source,
            syntax,
            parent_kinds,
            top_level_items,
            output: String::new(),
            delimiter_depth: 0,
            at_line_start: true,
            pending_newlines: 0,
            previous: None,
            previous_parent: None,
        }
    }

    fn run(mut self) -> String {
        for (index, token) in self.syntax.lexed().tokens().iter().copied().enumerate() {
            match token.kind() {
                TokenKind::Eof => break,
                TokenKind::Newline => {
                    self.pending_newlines = self.pending_newlines.saturating_add(1).min(2);
                }
                _ => self.write_token(index, token),
            }
        }
        while self.output.ends_with([' ', '\n']) {
            self.output.pop();
        }
        self.output.push('\n');
        self.output
    }

    fn write_token(&mut self, index: usize, token: Token) {
        if self.pending_newlines != 0 {
            if self.joins_previous_line(token) {
                if !self.output.is_empty()
                    && needs_space(
                        self.previous,
                        self.previous_parent,
                        token,
                        self.parent(index),
                    )
                {
                    self.output.push(' ');
                }
            } else if !self.output.is_empty() {
                let line_count = if self
                    .top_level_items
                    .contains(&token.span().range().start().get())
                {
                    2
                } else {
                    self.pending_newlines
                };
                self.output.push_str(&"\n".repeat(usize::from(line_count)));
                self.at_line_start = true;
            }
            self.pending_newlines = 0;
        } else if !self.at_line_start
            && needs_space(
                self.previous,
                self.previous_parent,
                token,
                self.parent(index),
            )
        {
            self.output.push(' ');
        }

        if self.at_line_start {
            let leading_closes = usize::from(is_closing_delimiter(token.kind()));
            let indent = self.delimiter_depth.saturating_sub(leading_closes);
            self.output.push_str(&"    ".repeat(indent));
            self.at_line_start = false;
        }

        let text = self
            .source
            .text_at(token.span().range())
            .expect("lexer token range remains in its source");
        self.output.push_str(text);
        match token.kind() {
            TokenKind::Punctuation(
                Punctuation::LeftParen | Punctuation::LeftBrace | Punctuation::LeftBracket,
            ) => self.delimiter_depth += 1,
            TokenKind::Punctuation(
                Punctuation::RightParen | Punctuation::RightBrace | Punctuation::RightBracket,
            ) => self.delimiter_depth = self.delimiter_depth.saturating_sub(1),
            _ => {}
        }
        self.previous = Some(token);
        self.previous_parent = self.parent(index);
    }

    fn joins_previous_line(&self, token: Token) -> bool {
        matches!(token.kind(), TokenKind::Punctuation(Punctuation::LeftBrace))
            || matches!(
                (self.previous.map(Token::kind), token.kind()),
                (
                    Some(TokenKind::Punctuation(Punctuation::RightBrace)),
                    TokenKind::Keyword(Keyword::Else)
                )
            )
    }

    fn parent(&self, token: usize) -> Option<NodeKind> {
        self.parent_kinds.get(token).copied().flatten()
    }
}

fn needs_space(
    previous: Option<Token>,
    previous_parent: Option<NodeKind>,
    current: Token,
    current_parent: Option<NodeKind>,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let previous_kind = previous.kind();
    let current_kind = current.kind();
    if matches!(
        current_kind,
        TokenKind::StringText | TokenKind::StringEnd(_) | TokenKind::InterpolationEnd
    ) || matches!(
        previous_kind,
        TokenKind::StringStart(_) | TokenKind::StringText | TokenKind::InterpolationStart
    ) {
        return false;
    }
    if matches!(
        previous_kind,
        TokenKind::Punctuation(Punctuation::LeftParen | Punctuation::LeftBracket)
    ) {
        return false;
    }
    if let TokenKind::Punctuation(punctuation) = current_kind {
        return space_before_punctuation(
            previous_kind,
            previous_parent,
            punctuation,
            current_parent,
        );
    }
    if let TokenKind::Punctuation(punctuation) = previous_kind {
        return space_after_punctuation(punctuation, previous_parent);
    }
    true
}

fn space_before_punctuation(
    previous: TokenKind,
    previous_parent: Option<NodeKind>,
    punctuation: Punctuation,
    parent: Option<NodeKind>,
) -> bool {
    match punctuation {
        Punctuation::LeftParen => {
            !is_attached_left_parenthesis(parent) && !is_prefix_parent(previous_parent)
        }
        Punctuation::LeftBracket => {
            parent == Some(NodeKind::SequenceBody)
                || parent != Some(NodeKind::IndexSuffix)
                    && matches!(
                        previous,
                        TokenKind::Punctuation(previous)
                            if space_after_punctuation(previous, None)
                    )
        }
        Punctuation::Dot => previous == TokenKind::Keyword(Keyword::Use),
        Punctuation::RightBrace => {
            parent != Some(NodeKind::ImportSelection)
                && previous != TokenKind::Punctuation(Punctuation::LeftBrace)
        }
        Punctuation::Less | Punctuation::Greater
            if matches!(
                parent,
                Some(
                    NodeKind::GenericParameters
                        | NodeKind::TypeArguments
                        | NodeKind::PatternArguments
                        | NodeKind::OpaqueArguments
                )
            ) =>
        {
            false
        }
        Punctuation::Slash if parent == Some(NodeKind::ModulePath) => false,
        Punctuation::ReadWrite
        | Punctuation::Star
        | Punctuation::Ampersand
        | Punctuation::Minus
        | Punctuation::Bang
            if is_prefix_parent(parent) =>
        {
            is_word(previous)
                || matches!(
                    previous,
                    TokenKind::Punctuation(previous)
                        if space_after_punctuation(previous, None)
                )
        }
        Punctuation::RightParen
        | Punctuation::RightBracket
        | Punctuation::Comma
        | Punctuation::Colon
        | Punctuation::Semicolon
        | Punctuation::Question
        | Punctuation::Range
        | Punctuation::Hash
        | Punctuation::Bang => false,
        Punctuation::LeftBrace if parent == Some(NodeKind::ImportSelection) => false,
        Punctuation::LeftBrace
        | Punctuation::Expansion
        | Punctuation::EqualEqual
        | Punctuation::BangEqual
        | Punctuation::LessEqual
        | Punctuation::GreaterEqual
        | Punctuation::LogicalAnd
        | Punctuation::LogicalOr
        | Punctuation::ShiftLeft
        | Punctuation::ShiftRight
        | Punctuation::PlusEqual
        | Punctuation::MinusEqual
        | Punctuation::StarEqual
        | Punctuation::SlashEqual
        | Punctuation::PercentEqual
        | Punctuation::Slash
        | Punctuation::Star
        | Punctuation::Less
        | Punctuation::Greater
        | Punctuation::Equal
        | Punctuation::Plus
        | Punctuation::Minus
        | Punctuation::Percent
        | Punctuation::Pipe
        | Punctuation::ReadWrite
        | Punctuation::Ampersand => true,
    }
}

const fn is_attached_left_parenthesis(parent: Option<NodeKind>) -> bool {
    matches!(
        parent,
        Some(
            NodeKind::Parameters
                | NodeKind::CallableParameters
                | NodeKind::LiteralParameters
                | NodeKind::EnumPayload
                | NodeKind::EnumPatternPayload
                | NodeKind::CallSuffix
                | NodeKind::DropDeclaration
        )
    )
}

const fn space_after_punctuation(punctuation: Punctuation, parent: Option<NodeKind>) -> bool {
    if is_prefix_parent(parent)
        && matches!(
            punctuation,
            Punctuation::ReadWrite
                | Punctuation::Star
                | Punctuation::Ampersand
                | Punctuation::Minus
                | Punctuation::Bang
                | Punctuation::Expansion
        )
    {
        return false;
    }
    if matches!(parent, Some(NodeKind::ModulePath | NodeKind::Visibility))
        && matches!(punctuation, Punctuation::Slash)
    {
        return false;
    }
    if matches!(
        parent,
        Some(
            NodeKind::GenericParameters
                | NodeKind::TypeArguments
                | NodeKind::PatternArguments
                | NodeKind::OpaqueArguments
        )
    ) && matches!(punctuation, Punctuation::Less | Punctuation::Greater)
    {
        return false;
    }
    if matches!(parent, Some(NodeKind::ImportSelection))
        && matches!(punctuation, Punctuation::LeftBrace)
    {
        return false;
    }
    matches!(
        punctuation,
        Punctuation::LeftBrace
            | Punctuation::RightParen
            | Punctuation::RightBrace
            | Punctuation::RightBracket
            | Punctuation::Question
            | Punctuation::Bang
            | Punctuation::Comma
            | Punctuation::Colon
            | Punctuation::Semicolon
            | Punctuation::EqualEqual
            | Punctuation::BangEqual
            | Punctuation::LessEqual
            | Punctuation::GreaterEqual
            | Punctuation::LogicalAnd
            | Punctuation::LogicalOr
            | Punctuation::ShiftLeft
            | Punctuation::ShiftRight
            | Punctuation::PlusEqual
            | Punctuation::MinusEqual
            | Punctuation::StarEqual
            | Punctuation::SlashEqual
            | Punctuation::PercentEqual
            | Punctuation::Slash
            | Punctuation::Star
            | Punctuation::Less
            | Punctuation::Greater
            | Punctuation::Equal
            | Punctuation::Plus
            | Punctuation::Minus
            | Punctuation::Percent
            | Punctuation::Pipe
    )
}

const fn is_word(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::Keyword(_)
            | TokenKind::IntegerLiteral
            | TokenKind::ByteLiteral
            | TokenKind::StringEnd(_)
    )
}

const fn is_prefix_parent(parent: Option<NodeKind>) -> bool {
    matches!(
        parent,
        Some(
            NodeKind::UnaryExpression
                | NodeKind::ReferenceExpression
                | NodeKind::PointerType
                | NodeKind::BorrowType
                | NodeKind::Receiver
                | NodeKind::CoercionPredicate
                | NodeKind::OperatorPredicate
                | NodeKind::ExpansionPredicate
                | NodeKind::SpreadExpression
        )
    )
}

const fn is_closing_delimiter(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Punctuation(
            Punctuation::RightParen | Punctuation::RightBrace | Punctuation::RightBracket
        )
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use nocter_source::SourceName;

    use crate::{InspectionGoal, SourceInspection};

    fn format(source: &str) -> String {
        SourceInspection::new(
            SourceName::new("test.nct"),
            source.as_bytes(),
            InspectionGoal::ModuleSource,
        )
        .unwrap()
        .format()
        .unwrap()
    }

    #[test]
    fn normalizes_spacing_indentation_braces_and_top_level_boundaries() {
        let formatted = format(
            "func first(value:i32):i32 {\nreturn value+1\n}\nfunc second():i32 {\nreturn if true { 1 }else {2}\n}\n",
        );

        assert_eq!(
            formatted,
            "func first(value: i32): i32 {\n    return value + 1\n}\n\nfunc second(): i32 {\n    return if true { 1 } else { 2 }\n}\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_index_jointness_and_distinguishes_typed_sequence_spacing() {
        assert_eq!(
            format("func f(values:&[i32]):i32 { return values[0]+Vec [1,2][0] }\n"),
            "func f(values: &[i32]): i32 { return values[0] + Vec [1, 2][0] }\n"
        );
    }

    #[test]
    fn distinguishes_attached_parentheses_from_grouping_and_closure_heads() {
        assert_eq!(
            format(
                "drop Maybe<T>(&+self) {}\nfunc f(): void { let x=(move value?)?\nlet c=(&source; value:T):bool { true }\nif(Flags { ready:true }).ready { return }\n}\n"
            ),
            "drop Maybe<T>(&+self) {}\n\nfunc f(): void { let x = (move value?)?\n    let c = (&source; value: T): bool { true }\n    if (Flags { ready: true }).ready { return }\n}\n"
        );
    }

    #[test]
    fn rejects_comments_without_rewriting_their_text() {
        let inspection = SourceInspection::new(
            SourceName::new("test.nct"),
            b"// keep me\nfunc main(): void { return }\n",
            InspectionGoal::ModuleSource,
        )
        .unwrap();

        let failure = inspection.format().unwrap_err();
        let diagnostics = failure.diagnostics().unwrap();

        assert_eq!(diagnostics[0].code(), "E0601");
        assert_eq!(diagnostics[0].primary().span().range().start().get(), 0);
    }

    #[test]
    fn runnable_examples_follow_formatter_output() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
        let mut sources = Vec::new();
        collect_sources(&repository.join("examples"), &mut sources);
        sources.sort();
        assert!(!sources.is_empty());

        for path in sources {
            let bytes = fs::read(&path).unwrap();
            let goal = if path.file_name().unwrap() == "nocter.nct" {
                InspectionGoal::PackageFile
            } else {
                InspectionGoal::ModuleSource
            };
            let inspection =
                SourceInspection::new(SourceName::new(path.to_string_lossy()), &bytes, goal)
                    .unwrap();
            let formatted = inspection.format().unwrap();
            assert_eq!(formatted.as_bytes(), bytes, "{}", path.display());
        }
    }

    #[test]
    fn complete_accepted_syntax_corpus_formats_idempotently() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/syntax");
        for (name, goal) in [
            ("g001-package.nct", InspectionGoal::PackageFile),
            ("g002-g006-module.nct", InspectionGoal::ModuleSource),
            ("g007-g012-declarations.nct", InspectionGoal::ModuleSource),
            ("g013-g018-types.nct", InspectionGoal::ModuleSource),
            ("g001-g018-semantic.nct", InspectionGoal::ModuleSource),
            ("g019-g024-executable.nct", InspectionGoal::ModuleSource),
            ("g025-g033-expressions.nct", InspectionGoal::ModuleSource),
            ("g019-g033-semantic.nct", InspectionGoal::ModuleSource),
        ] {
            let path = fixtures.join(name);
            let bytes = fs::read(&path).unwrap();
            let inspection = SourceInspection::new(SourceName::new(name), &bytes, goal).unwrap();
            assert!(inspection.ast_succeeded(), "{name}");
            let formatted = inspection.format().unwrap();
            let reparsed =
                SourceInspection::new(SourceName::new(name), formatted.as_bytes(), goal).unwrap();
            assert_eq!(reparsed.format().unwrap(), formatted, "{name}");
        }
    }

    fn collect_sources(directory: &Path, output: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_sources(&path, output);
            } else if path.extension().is_some_and(|extension| extension == "nct") {
                output.push(path);
            }
        }
    }
}
