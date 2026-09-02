use std::collections::HashMap;

use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic, syntax_diagnostics};
use nocter_source::{SourceFile, SourceMap, SourceName};
use nocter_syntax::{
    Keyword, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{FormatError, syntax_tokens};

mod equivalence;
mod layout;
mod rewrite;

use layout::LayoutPlan;

pub(super) fn format(source: &SourceFile, syntax: &SyntaxTree) -> Result<String, FormatError> {
    if syntax.has_errors() {
        return Err(FormatError::Diagnostics(syntax_diagnostics(
            std::slice::from_ref(syntax),
        )));
    }
    if let Some(comment) = syntax.lexed().comments().first().copied() {
        return Err(FormatError::Diagnostics(vec![SourceDiagnostic::new(
            DiagnosticCode::E0601,
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
    let candidate = parse_candidate(source, syntax, &formatted)?;
    if !equivalence::same_tree(source, syntax, candidate.source(), &candidate.syntax) {
        return Err(FormatError::ChangedSyntax);
    }
    let rewrites = rewrite::RewritePlan::build(&candidate.syntax);
    let Some(rewritten) = rewrites.apply(candidate.source()) else {
        return Ok(formatted);
    };
    let rewritten_candidate = parse_candidate(candidate.source(), &candidate.syntax, &rewritten)?;
    if !rewrites.preserves_tokens(
        candidate.source(),
        &candidate.syntax,
        rewritten_candidate.source(),
        &rewritten_candidate.syntax,
    ) {
        return Err(FormatError::ChangedSyntax);
    }
    Ok(rewritten)
}

struct ParsedCandidate {
    sources: SourceMap,
    syntax: SyntaxTree,
}

impl ParsedCandidate {
    fn source(&self) -> &SourceFile {
        self.sources
            .get(self.syntax.source())
            .expect("candidate source remains in its source map")
    }
}

fn parse_candidate(
    source: &SourceFile,
    original: &SyntaxTree,
    formatted: &str,
) -> Result<ParsedCandidate, FormatError> {
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
        NodeKind::SourceFile => nocter_syntax::ParseGoal::SourceFile,
        _ => return Err(FormatError::ChangedSyntax),
    };
    let formatted_tree = nocter_syntax::parse(formatted_source, goal);
    if formatted_tree.has_errors() {
        return Err(FormatError::ChangedSyntax);
    }
    Ok(ParsedCandidate {
        sources,
        syntax: formatted_tree,
    })
}

struct Formatter<'syntax> {
    source: &'syntax SourceFile,
    tokens: Vec<SyntaxToken>,
    parent_kinds: HashMap<SyntaxToken, NodeKind>,
    top_level_items: HashMap<u32, TopLevelItemKind>,
    layout: LayoutPlan,
    output: String,
    delimiter_depth: usize,
    at_line_start: bool,
    pending_newlines: u8,
    previous: Option<SyntaxToken>,
    previous_parent: Option<NodeKind>,
    previous_top_level: Option<TopLevelItemKind>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopLevelItemKind {
    Import,
    Separated,
}

impl<'syntax> Formatter<'syntax> {
    fn new(source: &'syntax SourceFile, syntax: &'syntax SyntaxTree) -> Self {
        let tokens = syntax_tokens::ordered(syntax);
        let mut parent_kinds = HashMap::new();
        for (node_id, node) in syntax.nodes() {
            for child in syntax.children(node_id) {
                if let SyntaxElement::Token(token) = child {
                    parent_kinds.entry(*token).or_insert(node.kind());
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
                let kind = match node.kind() {
                    NodeKind::UseDeclaration => TopLevelItemKind::Import,
                    NodeKind::Item | NodeKind::SourceVisibilityDeclaration => {
                        TopLevelItemKind::Separated
                    }
                    _ => return None,
                };
                Some((node.range().start().get(), kind))
            })
            .collect();
        Self {
            source,
            tokens: tokens.clone(),
            parent_kinds,
            top_level_items,
            layout: LayoutPlan::build(source, syntax, &tokens),
            output: String::new(),
            delimiter_depth: 0,
            at_line_start: true,
            pending_newlines: 0,
            previous: None,
            previous_parent: None,
            previous_top_level: None,
        }
    }

    fn run(mut self) -> String {
        let tokens = std::mem::take(&mut self.tokens);
        for token in tokens {
            if self.layout.omits(token) {
                continue;
            }
            match token.kind() {
                TokenKind::Eof => break,
                TokenKind::Newline => {
                    self.pending_newlines = self.pending_newlines.saturating_add(1).min(2);
                }
                _ => self.write_token(token),
            }
        }
        while self.output.ends_with([' ', '\n']) {
            self.output.pop();
        }
        self.output.push('\n');
        self.output
    }

    fn write_token(&mut self, token: SyntaxToken) {
        let current_top_level = self
            .top_level_items
            .get(&token.range().start().get())
            .copied();
        let forced_break = self.layout.breaks_before(token);
        if self.layout.joins_before(token) {
            self.pending_newlines = 0;
        }
        if self.layout.inserts_comma_before(token) {
            self.output.push(',');
            self.pending_newlines = 1;
        }
        if forced_break {
            self.pending_newlines = 1;
        }
        if self.pending_newlines != 0 {
            if !forced_break && self.joins_previous_line(token) {
                if !self.output.is_empty()
                    && needs_space(
                        self.previous,
                        self.previous_parent,
                        token,
                        self.parent(token),
                    )
                {
                    self.output.push(' ');
                }
            } else if !self.output.is_empty() {
                let line_count = match (self.previous_top_level, current_top_level) {
                    (Some(TopLevelItemKind::Import), Some(TopLevelItemKind::Import)) => 1,
                    (_, Some(_)) => 2,
                    _ => self.pending_newlines,
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
                self.parent(token),
            )
        {
            self.output.push(' ');
        }

        if self.at_line_start {
            let leading_closes = usize::from(is_closing_delimiter(token.kind()))
                + self.layout.structural_closes(token);
            let indent = self.delimiter_depth.saturating_sub(leading_closes);
            self.output.push_str(&"    ".repeat(indent));
            self.at_line_start = false;
        }

        let text = self
            .source
            .text_at(token.range())
            .expect("syntax token range remains in its source");
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
        self.delimiter_depth += self.layout.structural_opens(token);
        self.delimiter_depth = self
            .delimiter_depth
            .saturating_sub(self.layout.structural_closes(token));
        self.previous = Some(token);
        self.previous_parent = self.parent(token);
        if let Some(kind) = current_top_level {
            self.previous_top_level = Some(kind);
        }
    }

    fn joins_previous_line(&self, token: SyntaxToken) -> bool {
        matches!(token.kind(), TokenKind::Punctuation(Punctuation::LeftBrace))
            || matches!(
                (self.previous.map(SyntaxToken::kind), token.kind()),
                (
                    Some(TokenKind::Punctuation(Punctuation::RightBrace)),
                    TokenKind::Keyword(Keyword::Else)
                )
            )
    }

    fn parent(&self, token: SyntaxToken) -> Option<NodeKind> {
        self.parent_kinds.get(&token).copied()
    }
}

fn needs_space(
    previous: Option<SyntaxToken>,
    previous_parent: Option<NodeKind>,
    current: SyntaxToken,
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
            matches!(
                parent,
                Some(NodeKind::LiteralShape | NodeKind::SequenceBody | NodeKind::MappingBody)
            ) || parent != Some(NodeKind::IndexSuffix)
                && matches!(
                    previous,
                    TokenKind::Punctuation(previous)
                        if space_after_punctuation(previous, previous_parent)
                )
        }
        Punctuation::Dot if parent == Some(NodeKind::AssociatedTypeBinding) => true,
        Punctuation::Dot if parent == Some(NodeKind::SourceVisibilityPath) => {
            previous == TokenKind::Keyword(Keyword::See)
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
                        | NodeKind::AssociatedBindings
                )
            ) =>
        {
            false
        }
        Punctuation::Slash if parent == Some(NodeKind::ModulePath) => {
            previous == TokenKind::Keyword(Keyword::Use)
        }
        Punctuation::Slash if parent == Some(NodeKind::SourceVisibilityPath) => false,
        Punctuation::Slash if parent == Some(NodeKind::Visibility) => false,
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
                        if space_after_punctuation(previous, previous_parent)
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
                | NodeKind::Visibility
                | NodeKind::CallableParameters
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
    if matches!(
        parent,
        Some(NodeKind::SourceVisibilityPath | NodeKind::ModulePath | NodeKind::Visibility)
    ) && matches!(punctuation, Punctuation::Slash)
    {
        return false;
    }
    if matches!(
        parent,
        Some(
            NodeKind::GenericParameters
                | NodeKind::TypeArguments
                | NodeKind::PatternArguments
                | NodeKind::AssociatedBindings
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
        format_with_goal(source, InspectionGoal::SourceFile)
    }

    fn format_with_goal(source: &str, goal: InspectionGoal) -> String {
        SourceInspection::new(SourceName::new("test.nct"), source.as_bytes(), goal)
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
    fn preserves_canonical_noalloc_modifier_placement() {
        let formatted = format(
            "pub noalloc func apply(callback:noalloc &func(i32):i32):i32 { return callback(1) }\nnoalloc drop Value(&+self) {}\n",
        );
        assert_eq!(
            formatted,
            "pub noalloc func apply(callback: noalloc &func(i32): i32): i32 { return callback(1) }\n\nnoalloc drop Value(&+self) {}\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_public_path_and_directory_api_usage_canonically() {
        let formatted = format(
            "use std/fs\nuse std/path.Utf8Path\nfunc prepare(path:&Utf8Path):void! { let parent=path.parent() otherwise { return }\nfs.create_dir_all(parent)?\nreturn\n}\n",
        );
        assert_eq!(
            formatted,
            "use std/fs\nuse std/path.Utf8Path\n\nfunc prepare(path: &Utf8Path): void! { let parent = path.parent() otherwise { return }\n    fs.create_dir_all(parent)?\n    return\n}\n"
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
    fn formats_constant_declarations_and_array_length_expressions_canonically() {
        let formatted = format(
            "pub const WIDTH:usize=2+2\ntype Bytes=[u8;WIDTH*2]\nfunc f():i32 { return 1 }\n",
        );
        assert_eq!(
            formatted,
            "pub const WIDTH: usize = 2 + 2\n\ntype Bytes = [u8; WIDTH * 2]\n\nfunc f(): i32 { return 1 }\n"
        );
        assert_eq!(format(&formatted), formatted);
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
    fn keeps_scoped_visibility_and_root_module_paths_in_their_canonical_forms() {
        assert_eq!(
            format("pub (.. / .. /) use / parser.{First,Second,}\n"),
            "pub(../../) use /parser.{First, Second}\n"
        );
    }

    #[test]
    fn canonicalizes_single_and_multiline_comma_lists_from_cst_ownership() {
        assert_eq!(
            format(
                "func choose<T,U,>(\nleft:T, right:U\n):void { let values=[left,right,]\nreturn consume(left,right,)\n}\nfunc generic<\nT,U\n>():void {}\n"
            ),
            "func choose<T, U>(\n    left: T,\n    right: U,\n): void { let values = [left, right]\n    return consume(left, right)\n}\n\nfunc generic<\n    T,\n    U,\n>(): void {}\n"
        );
    }

    #[test]
    fn formats_keyed_packs_and_mapping_literals_with_the_shared_list_model() {
        let formatted = format(
            "construct Assoc<K,V>{ pub literal [:](...entries:K:V):Self { return Self {} } }\nfunc make():void { let pairs=Map [\"a\":1,\"b\":2,]\nlet empty=Map<&str,i32> [:]\nreturn\n}\n",
        );
        assert_eq!(
            formatted,
            "construct Assoc<K, V> { pub literal [:](...entries: K: V): Self { return Self {} } }\n\nfunc make(): void { let pairs = Map [\"a\": 1, \"b\": 2]\n    let empty = Map<&str, i32> [:]\n    return\n}\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_associated_bindings_as_relative_interface_members() {
        assert_eq!(
            format("instance Value { impl Source {.Item=i32,.View=&str,} }\n"),
            "instance Value { impl Source { .Item = i32, .View = &str } }\n"
        );
    }

    #[test]
    fn applies_the_same_list_model_to_package_data_imports_and_initializers() {
        assert_eq!(
            format_with_goal(
                "#dependencies: { json:\"https://example.test/json\", http:\"https://example.test/http\"\n}\n",
                InspectionGoal::SourceFile,
            ),
            "#dependencies: {\n    json: \"https://example.test/json\",\n    http: \"https://example.test/http\",\n}\n"
        );
        assert_eq!(
            format(
                "use ./values.{First,Second,}\nfunc f():void { let value=Pair { left:1, right:2, }\nreturn\n}\n"
            ),
            "use ./values.{First, Second}\n\nfunc f(): void { let value = Pair { left: 1, right: 2 }\n    return\n}\n"
        );
    }

    #[test]
    fn keeps_consecutive_module_imports_in_one_top_level_block() {
        let formatted = format(
            "use std/io\nuse std/time\nuse std/time.{Duration,Instant}\nfunc main():void { return }\n",
        );
        assert_eq!(
            formatted,
            "use std/io\nuse std/time\nuse std/time.{Duration, Instant}\n\nfunc main(): void { return }\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_practical_text_reporting_pipeline_canonically() {
        let formatted = format(
            "use std/io\nfunc report(text:&str):void! { let normalized=text.trim_ascii().replace_all(\" \",\"-\")?\nlet line=\"value: ${normalized}\"\nio.println(&line)?\nio.eprintln(\"done\")?\nreturn\n}\n",
        );
        assert_eq!(
            formatted,
            "use std/io\n\nfunc report(text: &str): void! { let normalized = text.trim_ascii().replace_all(\" \", \"-\")?\n    let line = \"value: ${normalized}\"\n    io.println(&line)?\n    io.eprintln(\"done\")?\n    return\n}\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_nested_generic_lists_through_parser_owned_split_tokens() {
        assert_eq!(
            format("func nested(value:Outer<Inner<\nT\n>>):void {}\n"),
            "func nested(\n    value: Outer<\n        Inner<\n            T,\n        >,\n    >,\n): void {}\n"
        );
    }

    #[test]
    fn removes_only_specification_owned_redundant_expression_grouping() {
        let source = "func f():i32 { let negative=-(128)\nlet optional=(move maybe)?\nlet forced=(move result)!\nlet recovered=(move result) otherwise { return 0 }\nlet nested=(move layered?)?\nreturn negative\n}\n";
        let expected = "func f(): i32 { let negative = -128\n    let optional = move maybe?\n    let forced = move result!\n    let recovered = move result otherwise { return 0 }\n    let nested = (move layered?)?\n    return negative\n}\n";
        let formatted = format(source);
        assert_eq!(formatted, expected,);
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn removes_optional_borrow_grouping_but_retains_a_prefix_over_an_outcome() {
        let formatted = format("func f<T>(optional:(&T)?, borrowed:&(T?)):void {}\n");
        assert_eq!(
            formatted,
            "func f<T>(optional: &T?, borrowed: &(T?)): void {}\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn joins_requirements_and_removes_single_line_closure_segment_commas() {
        assert_eq!(
            format(
                "func f<T>(value:T):void where copy T,\nT impl Iterator { let closure=(&value,; item:T,):bool { true }\nreturn\n}\n"
            ),
            "func f<T>(value: T): void where copy T, T impl Iterator { let closure = (&value; item: T): bool { true }\n    return\n}\n"
        );
    }

    #[test]
    fn canonicalizes_multiline_closure_segments() {
        let source = "func f():void { let callback=(\n&source,move prefix,\n;value,index\n):bool { true }\nreturn\n}\n";
        let expected = "func f(): void { let callback = (\n        &source,\n        move prefix;\n        value,\n        index,\n    ): bool { true }\n    return\n}\n";
        let formatted = format(source);

        assert_eq!(formatted, expected);
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn rejects_comments_without_rewriting_their_text() {
        let inspection = SourceInspection::new(
            SourceName::new("test.nct"),
            b"// keep me\nfunc main(): void { return }\n",
            InspectionGoal::SourceFile,
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
            let inspection = SourceInspection::new(
                SourceName::new(path.to_string_lossy()),
                &bytes,
                InspectionGoal::SourceFile,
            )
            .unwrap();
            let formatted = inspection.format().unwrap();
            assert_eq!(formatted.as_bytes(), bytes, "{}", path.display());
        }
    }

    #[test]
    fn complete_accepted_syntax_corpus_formats_idempotently() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/syntax");
        for (name, goal) in [
            ("g001-package.nct", InspectionGoal::SourceFile),
            ("g002-g006-module.nct", InspectionGoal::SourceFile),
            ("g007-g012-declarations.nct", InspectionGoal::SourceFile),
            ("g013-g018-types.nct", InspectionGoal::SourceFile),
            ("g001-g018-semantic.nct", InspectionGoal::SourceFile),
            ("g019-g024-executable.nct", InspectionGoal::SourceFile),
            ("g025-g033-expressions.nct", InspectionGoal::SourceFile),
            ("g019-g033-semantic.nct", InspectionGoal::SourceFile),
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
