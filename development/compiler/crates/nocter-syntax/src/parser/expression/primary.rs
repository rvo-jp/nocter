use super::{ExpressionMode, control, expression};
use crate::parser::{CompletedMarker, Parser, block, newline, place, root, types};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn primary(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    if mode != ExpressionMode::Header {
        if parser.at_keyword(Keyword::If) {
            return control::if_expression(parser);
        }
        if parser.at_keyword(Keyword::Match) {
            return control::match_expression(parser);
        }
    }

    match parser.current_kind() {
        TokenKind::Punctuation(Punctuation::LeftParen) => grouped_or_closure(parser, mode),
        TokenKind::Punctuation(Punctuation::LeftBracket) => array_literal(parser),
        TokenKind::StringStart(_) => string_expression(parser),
        TokenKind::IntegerLiteral
        | TokenKind::ByteLiteral
        | TokenKind::CharacterLiteral
        | TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::None) => {
            scalar_literal(parser)
        }
        TokenKind::Identifier if !parser.at_contextual(ContextualSpelling::Discard) => {
            owner_or_reference(parser, mode)
        }
        _ => {
            let marker = parser.start();
            if at_expression_boundary(parser) {
                parser.missing(ExpectedSyntax::Expression);
            } else {
                parser.error_token(ExpectedSyntax::Expression);
            }
            parser.complete(marker, NodeKind::Error)
        }
    }
}

fn grouped_or_closure(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    if mode != ExpressionMode::Header && is_closure_start(parser) {
        return closure_expression(parser);
    }
    grouped_expression(parser)
}

fn is_closure_start(parser: &Parser<'_>) -> bool {
    let mut depth = 0_usize;
    let mut distance = 0_usize;
    loop {
        match parser.nth_kind(distance) {
            TokenKind::Punctuation(Punctuation::LeftParen) => depth += 1,
            TokenKind::Punctuation(Punctuation::RightParen) => {
                depth -= 1;
                if depth == 0 {
                    return matches!(
                        parser.nth_kind(distance + 1),
                        TokenKind::Punctuation(Punctuation::LeftBrace | Punctuation::Colon)
                    );
                }
            }
            TokenKind::Eof => return false,
            _ => {}
        }
        distance += 1;
    }
}

fn grouped_expression(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        return parser.complete(marker, NodeKind::GroupedExpression);
    }
    parser.eat_newlines();
    expression(parser, ExpressionMode::Delimited);
    let tuple = parser.eat_punctuation(Punctuation::Comma);
    if tuple {
        parser.eat_newlines();
        if parser.at_punctuation(Punctuation::RightParen) || parser.at(TokenKind::Eof) {
            parser.missing(ExpectedSyntax::Expression);
        } else {
            expression(parser, ExpressionMode::Delimited);
            while parser.eat_punctuation(Punctuation::Comma) {
                parser.eat_newlines();
                if parser.at_punctuation(Punctuation::RightParen) || parser.at(TokenKind::Eof) {
                    break;
                }
                expression(parser, ExpressionMode::Delimited);
            }
        }
    }
    parser.eat_newlines();
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(
        marker,
        if tuple {
            NodeKind::TupleExpression
        } else {
            NodeKind::GroupedExpression
        },
    )
}

fn closure_expression(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        block::required(parser);
        return parser.complete(marker, NodeKind::ClosureExpression);
    }
    closure_head(parser);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    if parser.at_punctuation(Punctuation::Colon) {
        let result = parser.start();
        parser.bump();
        types::type_(parser);
        parser.complete(result, NodeKind::ClosureResult);
    }
    block::required(parser);
    parser.complete(marker, NodeKind::ClosureExpression)
}

fn closure_head(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.eat_newlines();
    if at_capture(parser) {
        let captures = parser.start();
        closure_capture_list(parser);
        parser.complete(captures, NodeKind::ClosureCaptures);
        parser.expect_punctuation(Punctuation::Semicolon);
        parser.eat_newlines();
        if !parser.at_punctuation(Punctuation::RightParen) {
            closure_parameters(parser);
        }
    } else if !parser.at_punctuation(Punctuation::RightParen) {
        closure_parameters(parser);
    }
    parser.eat_newlines();
    parser.complete(marker, NodeKind::ClosureHead);
}

fn closure_capture_list(parser: &mut Parser<'_>) {
    loop {
        let before = parser.cursor;
        closure_capture(parser);
        if parser.cursor == before {
            parser.error_token(ExpectedSyntax::ClosureHead);
        }

        if parser.eat_punctuation(Punctuation::Comma) {
            parser.eat_newlines();
            if parser.at_punctuation(Punctuation::Semicolon)
                || parser.at_punctuation(Punctuation::RightParen)
            {
                return;
            }
            continue;
        }

        parser.eat_newlines();
        if parser.at_punctuation(Punctuation::Semicolon)
            || parser.at_punctuation(Punctuation::RightParen)
            || parser.at(TokenKind::Eof)
        {
            return;
        }
        parser.missing(ExpectedSyntax::Punctuation(Punctuation::Comma));
    }
}

fn at_capture(parser: &Parser<'_>) -> bool {
    matches!(
        parser.current_kind(),
        TokenKind::Punctuation(Punctuation::Ampersand | Punctuation::ReadWrite)
            | TokenKind::Keyword(Keyword::Move)
    )
}

fn closure_capture(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if at_capture(parser) {
        parser.bump();
        parser.expect_name();
    } else {
        parser.error_token(ExpectedSyntax::ClosureHead);
    }
    parser.complete(marker, NodeKind::ClosureCapture);
}

fn closure_parameters(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.comma_list(
        Punctuation::RightParen,
        false,
        ExpectedSyntax::ClosureHead,
        closure_parameter,
    );
    parser.complete(marker, NodeKind::ClosureParameters);
}

fn closure_parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Colon) {
        let annotation = parser.start();
        parser.bump();
        types::type_(parser);
        parser.complete(annotation, NodeKind::TypeAnnotation);
    }
    parser.complete(marker, NodeKind::ClosureParameter);
}

fn owner_or_reference(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    if let Some(owner) = parser.attempt_decided_with(types::owner_reference, |parser, owner| {
        let struct_literal =
            mode != ExpressionMode::Header && parser.at_punctuation(Punctuation::LeftBrace);
        let typed_literal = has_gap_before_current(parser)
            && (parser.at_punctuation(Punctuation::LeftBracket)
                || matches!(parser.current_kind(), TokenKind::StringStart(_)));
        let generic_member =
            owner.has_type_arguments && owner.plain_selections_after_type_arguments > 0;
        struct_literal || typed_literal || generic_member
    }) {
        if mode != ExpressionMode::Header && parser.at_punctuation(Punctuation::LeftBrace) {
            return struct_literal(parser, owner.completed);
        }
        if has_gap_before_current(parser) && parser.at_punctuation(Punctuation::LeftBracket) {
            return typed_bracket_literal(parser, owner.completed);
        }
        if has_gap_before_current(parser)
            && matches!(parser.current_kind(), TokenKind::StringStart(_))
        {
            return typed_string_literal(parser, owner.completed);
        }
        let marker = parser.precede(owner.completed);
        return parser.complete(marker, NodeKind::GenericOwnerMember);
    }

    let marker = parser.start();
    parser.bump();
    parser.complete(marker, NodeKind::ReferenceExpression)
}

fn struct_literal(parser: &mut Parser<'_>, owner: CompletedMarker) -> CompletedMarker {
    let marker = parser.precede(owner);
    let initializer = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        parser.complete(initializer, NodeKind::StructInitializer);
        return parser.complete(marker, NodeKind::StructLiteral);
    }
    parser.comma_list(
        Punctuation::RightBrace,
        true,
        ExpectedSyntax::Expression,
        field_initializer,
    );
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(initializer, NodeKind::StructInitializer);
    parser.complete(marker, NodeKind::StructLiteral)
}

fn field_initializer(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    expression(parser, ExpressionMode::Delimited);
    parser.complete(marker, NodeKind::FieldInitializer);
}

fn array_literal(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
        return parser.complete(marker, NodeKind::ArrayLiteral);
    }
    parser.comma_list(
        Punctuation::RightBracket,
        true,
        ExpectedSyntax::Expression,
        |parser| {
            expression(parser, ExpressionMode::Delimited);
        },
    );
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::ArrayLiteral)
}

fn typed_bracket_literal(parser: &mut Parser<'_>, owner: CompletedMarker) -> CompletedMarker {
    let marker = parser.precede(owner);
    let body = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
        parser.complete(body, NodeKind::SequenceBody);
        return parser.complete(marker, NodeKind::TypedSequenceLiteral);
    }
    parser.eat_newlines();
    let mapping = if parser.eat_punctuation(Punctuation::Colon) {
        true
    } else if parser.at_punctuation(Punctuation::RightBracket) || parser.at(TokenKind::Eof) {
        false
    } else {
        first_bracket_element(parser)
    };
    while parser.eat_punctuation(Punctuation::Comma) {
        parser.eat_newlines();
        if parser.at_punctuation(Punctuation::RightBracket) || parser.at(TokenKind::Eof) {
            break;
        }
        if mapping {
            mapping_element(parser);
        } else {
            sequence_element(parser);
        }
    }
    parser.eat_newlines();
    if !parser.at_punctuation(Punctuation::RightBracket) && !parser.at(TokenKind::Eof) {
        parser.missing(ExpectedSyntax::Punctuation(Punctuation::Comma));
    }
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    parser.complete(
        body,
        if mapping {
            NodeKind::MappingBody
        } else {
            NodeKind::SequenceBody
        },
    );
    allocation_override(parser);
    parser.complete(
        marker,
        if mapping {
            NodeKind::TypedMappingLiteral
        } else {
            NodeKind::TypedSequenceLiteral
        },
    )
}

fn first_bracket_element(parser: &mut Parser<'_>) -> bool {
    if parser.at_punctuation(Punctuation::Expansion) {
        sequence_element(parser);
        return false;
    }
    let first = expression(parser, ExpressionMode::Delimited);
    if !parser.at_punctuation(Punctuation::Colon) {
        let marker = parser.precede(first);
        parser.complete(marker, NodeKind::SequenceElement);
        return false;
    }
    let marker = parser.precede(first);
    parser.bump();
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    expression(parser, ExpressionMode::Delimited);
    parser.complete(marker, NodeKind::MappingElement);
    true
}

fn mapping_element(parser: &mut Parser<'_>) {
    let marker = parser.start();
    expression(parser, ExpressionMode::Delimited);
    parser.expect_punctuation(Punctuation::Colon);
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    expression(parser, ExpressionMode::Delimited);
    parser.complete(marker, NodeKind::MappingElement);
}

fn sequence_element(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.eat_punctuation(Punctuation::Expansion) {
        let spread = parser.start();
        newline::after_incomplete(parser, newline::Boundary::Delimited);
        if parser.at_punctuation(Punctuation::ReadWrite) {
            parser.error_token(ExpectedSyntax::Expression);
        } else {
            expression(parser, ExpressionMode::Delimited);
        }
        parser.complete(spread, NodeKind::SpreadExpression);
    } else {
        expression(parser, ExpressionMode::Delimited);
    }
    parser.complete(marker, NodeKind::SequenceElement);
}

fn typed_string_literal(parser: &mut Parser<'_>, owner: CompletedMarker) -> CompletedMarker {
    let marker = parser.precede(owner);
    root::string_literal(parser);
    allocation_override(parser);
    parser.complete(marker, NodeKind::TypedStringLiteral)
}

fn allocation_override(parser: &mut Parser<'_>) {
    if !parser.at_keyword(Keyword::Using) {
        return;
    }
    let marker = parser.start();
    parser.bump();
    place::allocator(parser);
    parser.complete(marker, NodeKind::AllocationOverride);
}

fn string_expression(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    let TokenKind::StringStart(delimiter) = parser.current_kind() else {
        unreachable!("string expression starts at a string opener");
    };
    parser.bump();
    while !parser.at(TokenKind::StringEnd(delimiter)) && !parser.at(TokenKind::Eof) {
        let part = parser.start();
        if parser.eat(TokenKind::StringText) {
            // Text has no nested grammar.
        } else if parser.eat(TokenKind::InterpolationStart) {
            newline::after_incomplete(parser, newline::Boundary::Delimited);
            expression(parser, ExpressionMode::Delimited);
            parser.eat_newlines();
            parser.expect(TokenKind::InterpolationEnd);
        } else {
            parser.error_token(ExpectedSyntax::Expression);
        }
        parser.complete(part, NodeKind::StringPart);
    }
    parser.expect(TokenKind::StringEnd(delimiter));
    parser.complete(marker, NodeKind::StringExpression)
}

fn scalar_literal(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    parser.complete(marker, NodeKind::ScalarLiteral)
}

fn has_gap_before_current(parser: &Parser<'_>) -> bool {
    parser.cursor > 0 && !parser.tokens[parser.cursor - 1].is_joint_to_next()
}

fn at_expression_boundary(parser: &Parser<'_>) -> bool {
    matches!(
        parser.current_kind(),
        TokenKind::Newline
            | TokenKind::InterpolationEnd
            | TokenKind::Eof
            | TokenKind::Punctuation(
                Punctuation::RightParen | Punctuation::RightBracket | Punctuation::RightBrace
            )
    )
}
