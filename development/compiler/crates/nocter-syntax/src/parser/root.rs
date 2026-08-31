use super::{Parser, declaration, source_visibility};
use crate::{ExpectedSyntax, Keyword, NodeKind, ParseDiagnosticKind, Punctuation, TokenKind};

pub(super) fn source_file(parser: &mut Parser<'_>) {
    let root = parser.start();
    parser.eat_newlines();

    while at_package_directive_start(parser) {
        package_directive(parser);
        parser.require_line_end();
    }

    while at_dependency_start(parser) {
        dependency_declaration(parser);
        parser.require_line_end();
    }

    while !parser.at(TokenKind::Eof) {
        if at_dependency_start(parser) {
            parser.diagnostic(ParseDiagnosticKind::LateDependencyDeclaration);
            dependency_declaration(parser);
        } else {
            declaration::item(parser);
        }
        parser.require_line_end();
    }

    parser.bump();
    parser.complete(root, NodeKind::SourceFile);
}

fn at_dependency_start(parser: &Parser<'_>) -> bool {
    parser.at_keyword(Keyword::See) || at_use_start(parser)
}

fn dependency_declaration(parser: &mut Parser<'_>) {
    if parser.at_keyword(Keyword::See) {
        source_visibility::declaration(parser);
    } else {
        use_declaration(parser);
    }
}

fn package_directive(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();

    if is_package_directive_name(parser) {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::PackageDirectiveName);
    }
    parser.expect_punctuation(Punctuation::Colon);
    directive_value(parser);
    parser.complete(marker, NodeKind::PackageDirective);
}

fn at_package_directive_start(parser: &Parser<'_>) -> bool {
    if !parser.at_punctuation(Punctuation::Hash) {
        return false;
    }
    let Some(next) = parser.tokens.get(parser.cursor + 1) else {
        return false;
    };
    matches!(
        next.kind(),
        TokenKind::Identifier | TokenKind::Keyword(Keyword::Test)
    ) && matches!(
        parser.source.text_at(next.span().range()),
        Some("package" | "dependencies" | "lock" | "executable" | "test")
    )
}

fn is_package_directive_name(parser: &Parser<'_>) -> bool {
    matches!(
        parser.current_text(),
        "package" | "dependencies" | "lock" | "executable"
    ) && parser.at(TokenKind::Identifier)
        || parser.at_keyword(Keyword::Test)
}

fn directive_value(parser: &mut Parser<'_>) {
    let marker = parser.start();
    match parser.current_kind() {
        TokenKind::StringStart(_) => string_literal(parser),
        TokenKind::IntegerLiteral => parser.bump(),
        TokenKind::Punctuation(Punctuation::LeftBrace) => directive_record(parser),
        _ => parser.error_token(ExpectedSyntax::DirectiveValue),
    }
    parser.complete(marker, NodeKind::DirectiveValue);
}

pub(super) fn string_literal(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let TokenKind::StringStart(delimiter) = parser.current_kind() else {
        parser.missing(ExpectedSyntax::StringLiteral);
        parser.complete(marker, NodeKind::StringLiteral);
        return;
    };
    parser.bump();
    parser.eat(TokenKind::StringText);
    if parser.at(TokenKind::InterpolationStart) {
        while !parser.at(TokenKind::StringEnd(delimiter)) && !parser.at(TokenKind::Eof) {
            parser.error_token(ExpectedSyntax::StringLiteral);
        }
    }
    parser.expect(TokenKind::StringEnd(delimiter));
    parser.complete(marker, NodeKind::StringLiteral);
}

fn directive_record(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        parser.complete(marker, NodeKind::DirectiveRecord);
        return;
    }

    parser.comma_list(
        Punctuation::RightBrace,
        true,
        ExpectedSyntax::Name,
        directive_field,
    );
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::DirectiveRecord);
}

fn directive_field(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    directive_value(parser);
    parser.complete(marker, NodeKind::DirectiveField);
}

fn at_use_start(parser: &Parser<'_>) -> bool {
    let mut cursor = parser.cursor;
    if parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::Pub) {
        cursor += 1;
        if parser.tokens[cursor].kind() == TokenKind::Punctuation(Punctuation::LeftParen) {
            cursor += 1;
            while !matches!(
                parser.tokens[cursor].kind(),
                TokenKind::Punctuation(Punctuation::RightParen)
                    | TokenKind::Eof
                    | TokenKind::Newline
            ) {
                cursor += 1;
            }
            if parser.tokens[cursor].kind() == TokenKind::Punctuation(Punctuation::RightParen) {
                cursor += 1;
            }
        }
    }
    parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::Use)
}

fn use_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_keyword(Keyword::Pub) {
        visibility(parser);
    }
    parser.expect_keyword(Keyword::Use);
    use_tree(parser);
    parser.complete(marker, NodeKind::UseDeclaration);
}

pub(super) fn visibility(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if parser.eat_punctuation(Punctuation::LeftParen) {
        if parser.eat_punctuation(Punctuation::Slash) {
            // Package visibility.
        } else if parser.eat_punctuation(Punctuation::Dot) {
            if parser.eat_punctuation(Punctuation::Slash) {
                // Child-module visibility.
            } else {
                parser.expect_punctuation(Punctuation::Dot);
                parser.expect_punctuation(Punctuation::Slash);
                while parser.at_punctuation(Punctuation::Dot) {
                    parser.bump();
                    parser.expect_punctuation(Punctuation::Dot);
                    parser.expect_punctuation(Punctuation::Slash);
                }
            }
        } else {
            parser.missing(ExpectedSyntax::Punctuation(Punctuation::Slash));
        }
        parser.expect_punctuation(Punctuation::RightParen);
    }
    parser.complete(marker, NodeKind::Visibility);
}

pub(super) fn use_tree(parser: &mut Parser<'_>) {
    let path = parser.start();
    module_path(parser);
    parser.complete(path, NodeKind::ModulePath);

    if parser.eat_punctuation(Punctuation::Dot) {
        let selection = parser.start();
        if parser.eat_punctuation(Punctuation::LeftBrace) {
            parser.comma_list(
                Punctuation::RightBrace,
                false,
                ExpectedSyntax::Name,
                selected_name,
            );
            parser.expect_punctuation(Punctuation::RightBrace);
        } else {
            selected_name(parser);
        }
        parser.complete(selection, NodeKind::ImportSelection);
    } else if parser.at_keyword(Keyword::As) {
        let alias = parser.start();
        parser.bump();
        expect_module_segment(parser);
        parser.complete(alias, NodeKind::ModuleAlias);
    }
}

fn selected_name(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    if parser.eat_keyword(Keyword::As) {
        parser.expect_name();
    }
    parser.complete(marker, NodeKind::SelectedName);
}

fn module_path(parser: &mut Parser<'_>) {
    let package_absolute = if parser.eat_punctuation(Punctuation::Slash) {
        // Package-absolute prefix.
        true
    } else if parser.eat_punctuation(Punctuation::Dot) {
        if parser.eat_punctuation(Punctuation::Slash) {
            // Current-module relative prefix.
        } else {
            parser.expect_punctuation(Punctuation::Dot);
            parser.expect_punctuation(Punctuation::Slash);
            while parser.at_punctuation(Punctuation::Dot) {
                parser.bump();
                parser.expect_punctuation(Punctuation::Dot);
                parser.expect_punctuation(Punctuation::Slash);
            }
        }
        false
    } else {
        false
    };

    if package_absolute
        && (parser.at_punctuation(Punctuation::Dot) || parser.at_keyword(Keyword::As))
    {
        return;
    }

    if !expect_module_segment(parser) {
        return;
    }
    while parser.eat_punctuation(Punctuation::Slash) {
        expect_module_segment(parser);
    }
}

fn expect_module_segment(parser: &mut Parser<'_>) -> bool {
    if parser.at(TokenKind::Identifier) && is_module_segment(parser.current_text()) {
        parser.bump();
        true
    } else {
        parser.error_token(ExpectedSyntax::ModuleSegment);
        false
    }
}

fn is_module_segment(text: &str) -> bool {
    text != "_"
        && text.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
