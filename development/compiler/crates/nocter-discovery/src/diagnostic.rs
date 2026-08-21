use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;
use nocter_syntax::{
    ExpectedSyntax, LexDiagnostic, LexDiagnosticKind, ParseDiagnostic, ParseDiagnosticKind,
    SyntaxTree, TokenKind,
};

use crate::syntax::use_path_node;
use crate::{DiscoveryError, ImportFailure};

pub(crate) fn discovery_diagnostics(
    error: &DiscoveryError,
    trees: &[SyntaxTree],
) -> Result<Box<[SourceDiagnostic]>, DiscoveryError> {
    let DiscoveryError::Import {
        declaration,
        path,
        failure,
    } = error
    else {
        return Ok(Box::new([]));
    };
    let tree = trees
        .iter()
        .find(|tree| tree.source() == declaration.source())
        .ok_or(DiscoveryError::InconsistentSyntax(*declaration))?;
    let path_node = use_path_node(tree, *declaration)
        .ok_or(DiscoveryError::InconsistentSyntax(*declaration))?;
    let origin = SourceOrigin::from_node(tree, path_node)
        .map_err(|_| DiscoveryError::InconsistentSyntax(*declaration))?;
    let message = format!(
        "cannot resolve module path `{path}`: {}",
        import_failure_message(failure)
    );
    Ok(vec![SourceDiagnostic::new(
        "E0263",
        message,
        origin,
        [],
        Some(import_failure_help(failure)),
    )]
    .into_boxed_slice())
}

fn import_failure_message(failure: &ImportFailure) -> Box<str> {
    match failure {
        ImportFailure::UnknownDependency { alias } => {
            format!("dependency alias `{alias}` is not declared").into()
        }
        ImportFailure::OutsidePackage => "the path escapes its package boundary".into(),
        ImportFailure::NotFound => "no source or module exists at that path".into(),
        ImportFailure::Ambiguous { .. } => {
            "both a source file and a directory module exist at that path".into()
        }
        ImportFailure::CrossesPackage { .. } => {
            "a relative source path crosses a package boundary".into()
        }
        ImportFailure::CrossesModule { .. } => {
            "a source import crosses a directory-module boundary".into()
        }
        ImportFailure::InvalidModuleDirectory => {
            "the path does not identify a valid module directory".into()
        }
        ImportFailure::SingleFileLocalImport => {
            "single-file mode cannot load a local source graph".into()
        }
    }
}

const fn import_failure_help(failure: &ImportFailure) -> &'static str {
    match failure {
        ImportFailure::UnknownDependency { .. } => {
            "declare the dependency alias in nocter.nct or correct the first path segment"
        }
        ImportFailure::Ambiguous { .. } => {
            "remove one candidate or choose a path that identifies exactly one source or module"
        }
        ImportFailure::SingleFileLocalImport => {
            "use package mode when source code spans more than one file"
        }
        ImportFailure::OutsidePackage
        | ImportFailure::NotFound
        | ImportFailure::CrossesPackage { .. }
        | ImportFailure::CrossesModule { .. }
        | ImportFailure::InvalidModuleDirectory => {
            "change the module path to one valid source or directory module within its boundary"
        }
    }
}

pub(crate) fn syntax_diagnostics(trees: &[SyntaxTree]) -> Box<[SourceDiagnostic]> {
    let mut diagnostics = trees
        .iter()
        .flat_map(|tree| {
            tree.lexed()
                .diagnostics()
                .iter()
                .copied()
                .map(lexical_diagnostic)
                .chain(tree.diagnostics().iter().copied().map(parse_diagnostic))
        })
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        let left_origin = left.primary();
        let right_origin = right.primary();
        (
            left_origin.source(),
            left_origin.span().range().start(),
            left_origin.span().range().end(),
        )
            .cmp(&(
                right_origin.source(),
                right_origin.span().range().start(),
                right_origin.span().range().end(),
            ))
            .then_with(|| left.code().cmp(right.code()))
    });
    diagnostics.into_boxed_slice()
}

fn lexical_diagnostic(diagnostic: LexDiagnostic) -> SourceDiagnostic {
    let (code, message, help) = match diagnostic.kind() {
        LexDiagnosticKind::UnexpectedCharacter => (
            "E0100",
            "unexpected character",
            Some("remove the character or replace it with valid Nocter syntax"),
        ),
        LexDiagnosticKind::UnterminatedBlockComment => (
            "E0101",
            "unterminated block comment",
            Some("close the comment with `*/`"),
        ),
        LexDiagnosticKind::InvalidIntegerLiteral => (
            "E0102",
            "invalid integer literal",
            Some("use decimal digits with separators only between digits"),
        ),
        LexDiagnosticKind::UnsupportedFloatLiteral => {
            ("E0103", "floating-point literals are not supported", None)
        }
        LexDiagnosticKind::UnterminatedString => (
            "E0104",
            "unterminated string literal",
            Some("close the string with `\"`"),
        ),
        LexDiagnosticKind::SingleLineStringNewline => (
            "E0105",
            "single-line string contains a newline",
            Some("close the string before the newline or use a multiline string"),
        ),
        LexDiagnosticKind::MultilineStringOpeningNewline => (
            "E0106",
            "multiline string content must begin on the next line",
            Some("insert a newline after the opening delimiter"),
        ),
        LexDiagnosticKind::InvalidEscape => (
            "E0107",
            "invalid escape sequence",
            Some("use a supported Nocter escape sequence"),
        ),
        LexDiagnosticKind::InvalidStringUtf8 => (
            "E0108",
            "string escape does not encode valid UTF-8",
            Some("encode a valid Unicode scalar value"),
        ),
        LexDiagnosticKind::MultilineStringIndentation => (
            "E0109",
            "multiline string indentation is inconsistent",
            Some("indent each content line at least as far as the closing delimiter"),
        ),
        LexDiagnosticKind::UnterminatedByteLiteral => (
            "E0110",
            "unterminated byte literal",
            Some("close the byte literal with `'`"),
        ),
        LexDiagnosticKind::ByteLiteralNewline => (
            "E0111",
            "byte literal contains a newline",
            Some("write exactly one byte before the closing `'`"),
        ),
        LexDiagnosticKind::InvalidByteLength => {
            ("E0112", "byte literal must contain exactly one byte", None)
        }
        LexDiagnosticKind::PlainSingleQuote => (
            "E0113",
            "plain single-quoted literals are not part of Nocter",
            Some("use a string literal or a `b'…'` byte literal"),
        ),
        LexDiagnosticKind::UnterminatedInterpolation => (
            "E0114",
            "unterminated string interpolation",
            Some("close the interpolation with `}`"),
        ),
    };
    source_diagnostic(code, message, diagnostic.span(), help)
}

fn parse_diagnostic(diagnostic: ParseDiagnostic) -> SourceDiagnostic {
    let (code, message, help) = match diagnostic.kind() {
        ParseDiagnosticKind::Expected(expected) => ("E0120", expected_message(expected), None),
        ParseDiagnosticKind::LateUseDeclaration => (
            "E0121",
            "module `use` declarations must precede items".into(),
            Some("move this `use` declaration before the first item"),
        ),
        ParseDiagnosticKind::NestingLimit => (
            "E0122",
            "source nesting exceeds the compiler limit".into(),
            Some("split the nested expression or type into named intermediate declarations"),
        ),
    };
    source_diagnostic(code, message, diagnostic.span(), help)
}

fn source_diagnostic(
    code: &'static str,
    message: impl Into<Box<str>>,
    primary: nocter_source::Span,
    help: Option<&'static str>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(code, message, primary, [], help)
}

fn expected_message(expected: ExpectedSyntax) -> Box<str> {
    match expected {
        ExpectedSyntax::Token(token) => token_expected_message(token),
        ExpectedSyntax::Keyword(keyword) => format!("expected `{}`", keyword.as_str()).into(),
        ExpectedSyntax::Punctuation(punctuation) => {
            format!("expected `{}`", punctuation.as_str()).into()
        }
        ExpectedSyntax::Contextual(spelling) => format!("expected `{spelling}`").into(),
        ExpectedSyntax::Name => "expected a name".into(),
        ExpectedSyntax::Visibility => "expected a visibility".into(),
        ExpectedSyntax::PackageDirectiveName => "expected a package directive name".into(),
        ExpectedSyntax::DirectiveValue => "expected a package directive value".into(),
        ExpectedSyntax::StringLiteral => "expected a string literal".into(),
        ExpectedSyntax::ModuleSegment => "expected a module path segment".into(),
        ExpectedSyntax::Type => "expected a type".into(),
        ExpectedSyntax::Parameter => "expected a parameter".into(),
        ExpectedSyntax::TargetableItem => "expected a targetable declaration".into(),
        ExpectedSyntax::Item => "expected a declaration".into(),
        ExpectedSyntax::DeclarationMember => "expected a declaration member".into(),
        ExpectedSyntax::DeclarationTypePattern => "expected a declaration type pattern".into(),
        ExpectedSyntax::Receiver => "expected a receiver".into(),
        ExpectedSyntax::Block => "expected a block".into(),
        ExpectedSyntax::LiteralShape => "expected a literal shape".into(),
        ExpectedSyntax::Expression => "expected an expression".into(),
        ExpectedSyntax::AssignmentTarget => "expected an assignment target".into(),
        ExpectedSyntax::EnumPattern => "expected an enum pattern".into(),
        ExpectedSyntax::ClosureHead => "expected a closure parameter list".into(),
        ExpectedSyntax::Predicate => "expected a `where` predicate".into(),
        ExpectedSyntax::Capability => "expected a borrow capability".into(),
        ExpectedSyntax::Newline => "expected a newline".into(),
    }
}

fn token_expected_message(token: TokenKind) -> Box<str> {
    match token {
        TokenKind::Identifier => "expected a name".into(),
        TokenKind::Keyword(keyword) => format!("expected `{}`", keyword.as_str()).into(),
        TokenKind::IntegerLiteral => "expected an integer literal".into(),
        TokenKind::ByteLiteral => "expected a byte literal".into(),
        TokenKind::StringStart(_) | TokenKind::StringEnd(_) => "expected a string delimiter".into(),
        TokenKind::StringText => "expected string text".into(),
        TokenKind::InterpolationStart => "expected a string interpolation".into(),
        TokenKind::InterpolationEnd => "expected the end of a string interpolation".into(),
        TokenKind::Newline => "expected a newline".into(),
        TokenKind::Punctuation(punctuation) => {
            format!("expected `{}`", punctuation.as_str()).into()
        }
        TokenKind::Eof => "expected the end of the file".into(),
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::syntax_diagnostics;

    #[test]
    fn lexer_and_parser_failures_share_the_common_envelope_in_source_order() {
        let mut sources = SourceMap::new();
        let lexical = sources
            .add_bytes(SourceName::new("lexical.nct"), b"@\n")
            .unwrap();
        let syntax = sources
            .add_bytes(SourceName::new("syntax.nct"), b"func\n")
            .unwrap();
        let trees = [
            parse(sources.get(lexical).unwrap(), ParseGoal::ModuleSource),
            parse(sources.get(syntax).unwrap(), ParseGoal::ModuleSource),
        ];

        let diagnostics = syntax_diagnostics(&trees);

        assert_eq!(diagnostics[0].code(), "E0100");
        assert_eq!(diagnostics[0].message(), "unexpected character");
        assert_eq!(diagnostics[0].primary().source(), lexical);
        assert!(diagnostics[0].primary().syntax().is_none());
        assert_eq!(diagnostics[1].code(), "E0120");
        assert_eq!(diagnostics[1].message(), "expected a name");
        assert_eq!(diagnostics[1].primary().source(), syntax);
    }
}
