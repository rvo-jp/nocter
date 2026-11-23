use super::super::{CompletedMarker, Marker, Parser, newline};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) struct Expression {
    pub(super) completed: CompletedMarker,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ClosingAngle {
    Operator,
    Delimiter,
}

pub(super) fn expression(parser: &mut Parser<'_>, boundary: newline::Boundary) -> Expression {
    parse_expression(parser, boundary, true, ClosingAngle::Operator)
}

pub(super) fn header_expression(parser: &mut Parser<'_>) -> Expression {
    parse_expression(
        parser,
        newline::Boundary::Statement,
        false,
        ClosingAngle::Operator,
    )
}

pub(super) fn generic_argument(parser: &mut Parser<'_>) {
    parse_expression(
        parser,
        newline::Boundary::Delimited,
        true,
        ClosingAngle::Delimiter,
    );
}

fn parse_expression(
    parser: &mut Parser<'_>,
    boundary: newline::Boundary,
    allow_braces: bool,
    closing_angle: ClosingAngle,
) -> Expression {
    let marker = parser.start();
    let mut left = binary(parser, boundary, 0, allow_braces, closing_angle);
    loop {
        newline::before(parser, boundary, |kind| {
            matches!(
                kind,
                TokenKind::Keyword(Keyword::Otherwise | Keyword::Catch)
            )
        });
        if !matches!(
            parser.current_kind(),
            TokenKind::Keyword(Keyword::Otherwise | Keyword::Catch)
        ) {
            break;
        }
        let parent = parser.precede(left.completed);
        let clause = parser.start();
        let catch = parser.at_keyword(Keyword::Catch);
        parser.bump();
        newline::after_incomplete(parser, boundary);
        if catch {
            parser.expect_name_or_discard();
            newline::after_incomplete(parser, boundary);
        }
        super::required_block(parser);
        parser.complete(clause, NodeKind::RecoveryClause);
        left = Expression {
            completed: parser.complete(parent, NodeKind::RecoveryExpression),
        };
    }
    Expression {
        completed: parser.complete(marker, NodeKind::Expression),
    }
}

const OPERATORS: &[(&[Punctuation], NodeKind)] = &[
    (&[Punctuation::LogicalOr], NodeKind::LogicalOrExpression),
    (&[Punctuation::LogicalAnd], NodeKind::LogicalAndExpression),
    (
        &[Punctuation::EqualEqual, Punctuation::BangEqual],
        NodeKind::EqualityExpression,
    ),
    (
        &[
            Punctuation::Less,
            Punctuation::LessEqual,
            Punctuation::Greater,
            Punctuation::GreaterEqual,
        ],
        NodeKind::OrderingExpression,
    ),
    (
        &[Punctuation::ShiftLeft, Punctuation::ShiftRight],
        NodeKind::ShiftExpression,
    ),
    (
        &[Punctuation::Plus, Punctuation::Minus],
        NodeKind::AdditiveExpression,
    ),
    (
        &[Punctuation::Star, Punctuation::Slash, Punctuation::Percent],
        NodeKind::MultiplicativeExpression,
    ),
];

fn binary(
    parser: &mut Parser<'_>,
    boundary: newline::Boundary,
    level: usize,
    allow_braces: bool,
    closing_angle: ClosingAngle,
) -> Expression {
    let Some((operators, kind)) = OPERATORS.get(level) else {
        return conversion(parser, boundary, allow_braces);
    };
    let mut left = binary(parser, boundary, level + 1, allow_braces, closing_angle);
    loop {
        newline::before(parser, boundary, |token| {
            operators.iter().any(|operator| {
                *operator != Punctuation::Minus && token == TokenKind::Punctuation(*operator)
            })
        });
        if closing_angle == ClosingAngle::Delimiter
            && matches!(
                parser.current_kind(),
                TokenKind::Punctuation(Punctuation::Greater | Punctuation::ShiftRight)
            )
        {
            return left;
        }
        if !operators
            .iter()
            .any(|operator| parser.at_punctuation(*operator))
        {
            return left;
        }
        let marker = parser.precede(left.completed);
        parser.bump();
        newline::after_incomplete(parser, boundary);
        binary(parser, boundary, level + 1, allow_braces, closing_angle);
        left = Expression {
            completed: parser.complete(marker, *kind),
        };
        if matches!(
            kind,
            NodeKind::EqualityExpression | NodeKind::OrderingExpression
        ) {
            return left;
        }
    }
}

fn conversion(
    parser: &mut Parser<'_>,
    boundary: newline::Boundary,
    allow_braces: bool,
) -> Expression {
    let mut source = prefix(parser, boundary, allow_braces);
    loop {
        newline::before(parser, boundary, |token| {
            token == TokenKind::Keyword(Keyword::As)
        });
        if !parser.at_keyword(Keyword::As) {
            return source;
        }
        let conversion = parser.precede(source.completed);
        parser.bump();
        newline::after_incomplete(parser, boundary);
        super::type_(parser);
        source = Expression {
            completed: parser.complete(conversion, NodeKind::ConversionExpression),
        };
    }
}

fn prefix(parser: &mut Parser<'_>, boundary: newline::Boundary, allow_braces: bool) -> Expression {
    let mut wrappers = Vec::new();
    while matches!(
        parser.current_kind(),
        TokenKind::Punctuation(Punctuation::Bang | Punctuation::Minus)
    ) {
        let marker = parser.start();
        parser.bump();
        newline::after_incomplete(parser, boundary);
        wrappers.push(marker);
    }
    let mut awaits = Vec::new();
    while parser.at_keyword(Keyword::Await) {
        let marker = parser.start();
        parser.bump();
        newline::after_incomplete(parser, boundary);
        awaits.push(marker);
    }
    let mut inner = postfix_in(parser, allow_braces);
    for marker in awaits.into_iter().rev() {
        inner = Expression {
            completed: parser.complete(marker, NodeKind::AwaitExpression),
        };
    }
    if matches!(
        parser.current_kind(),
        TokenKind::Punctuation(Punctuation::Bang | Punctuation::Question)
    ) {
        let marker = parser.precede(inner.completed);
        parser.bump();
        inner = Expression {
            completed: parser.complete(marker, NodeKind::OutcomeExpression),
        };
    }
    for marker in wrappers.into_iter().rev() {
        inner = Expression {
            completed: parser.complete(marker, NodeKind::UnaryExpression),
        };
    }
    inner
}

pub(super) fn postfix(parser: &mut Parser<'_>) -> Expression {
    postfix_in(parser, true)
}

fn postfix_in(parser: &mut Parser<'_>, allow_braces: bool) -> Expression {
    let mut name_path = parser.at(TokenKind::Identifier);
    let mut named_callee = name_path;
    let mut left = primary(parser, allow_braces);
    loop {
        if named_callee
            && parser.at_punctuation(Punctuation::Less)
            && previous_is_joint(parser)
            && let Some(call) = generic_call(parser, left.completed)
        {
            left = call;
            name_path = false;
            named_callee = false;
            continue;
        }
        if allow_braces && name_path && parser.at_punctuation(Punctuation::LeftBrace) {
            left = record_value(parser, left.completed);
            name_path = false;
            named_callee = false;
            continue;
        }
        if !parser.at_punctuation(Punctuation::LeftParen)
            && !parser.at_punctuation(Punctuation::Dot)
            && !((parser.at_punctuation(Punctuation::Caret)
                || parser.at_punctuation(Punctuation::LeftBracket))
                && previous_is_joint(parser))
        {
            return left;
        }
        let marker = parser.precede(left.completed);
        if parser.eat_punctuation(Punctuation::Caret) {
            name_path = false;
            named_callee = false;
            left = Expression {
                completed: parser.complete(marker, NodeKind::TransferExpression),
            };
            continue;
        }
        let suffix = parser.start();
        if parser.eat_punctuation(Punctuation::Dot) {
            named_callee = parser.at(TokenKind::Identifier);
            name_path &= named_callee;
            if parser.at(TokenKind::IntegerLiteral) {
                parser.bump();
            } else {
                parser.expect_name();
            }
            parser.complete(suffix, NodeKind::MemberSuffix);
        } else if parser.eat_punctuation(Punctuation::LeftBracket) {
            name_path = false;
            named_callee = false;
            if parser.enter_nesting() {
                parser.eat_newlines();
                expression(parser, newline::Boundary::Delimited);
                parser.eat_newlines();
                parser.expect_punctuation(Punctuation::RightBracket);
                parser.leave_nesting();
            } else {
                parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
            }
            parser.complete(suffix, NodeKind::IndexSuffix);
        } else {
            name_path = false;
            named_callee = false;
            call_suffix(parser, suffix);
        }
        left = Expression {
            completed: parser.complete(marker, NodeKind::PostfixExpression),
        };
    }
}

fn generic_call(parser: &mut Parser<'_>, callee: CompletedMarker) -> Option<Expression> {
    let position = (parser.cursor, parser.nesting);
    let possible = parser
        .explicit_call_angles
        .get_or_insert_with(|| explicit_call_angles(parser.tokens));
    if !possible[parser.cursor] || parser.failed_explicit_calls.contains(&position) {
        return None;
    }
    let Some((parent, suffix)) = parser.attempt_with(
        |parser| {
            let parent = parser.start();
            let suffix = parser.start();
            super::types::generic_arguments(parser);
            (parent, suffix)
        },
        |parser, _| parser.at_punctuation(Punctuation::LeftParen),
    ) else {
        parser.failed_explicit_calls.insert(position);
        return None;
    };
    parser.attach_parent(callee, parent);
    call_suffix(parser, suffix);
    Some(Expression {
        completed: parser.complete(parent, NodeKind::PostfixExpression),
    })
}

/// A necessary token boundary for each candidate `<`. Ignoring angle nesting here preserves
/// the possibility that a less-than token belongs to a constant argument. The parser still
/// decides the entire argument tree before attaching a call.
fn explicit_call_angles(tokens: &[crate::Token]) -> Box<[bool]> {
    use Punctuation::{
        Greater, LeftBrace, LeftBracket, LeftParen, Less, RightBrace, RightBracket, RightParen,
        ShiftRight,
    };
    let mut candidates = vec![false; tokens.len()];
    let mut groups = Vec::new();
    let mut possible = false;
    let mut next = TokenKind::Eof;
    for (index, token) in tokens.iter().enumerate().rev() {
        let kind = token.kind();
        let opening = match kind {
            TokenKind::Punctuation(RightParen) => Some(TokenKind::Punctuation(LeftParen)),
            TokenKind::Punctuation(RightBracket) => Some(TokenKind::Punctuation(LeftBracket)),
            TokenKind::Punctuation(RightBrace) => Some(TokenKind::Punctuation(LeftBrace)),
            TokenKind::InterpolationEnd => Some(TokenKind::InterpolationStart),
            TokenKind::StringEnd(delimiter) => Some(TokenKind::StringStart(delimiter)),
            _ => None,
        };
        if let Some(opening) = opening {
            groups.push((opening, possible));
            possible = false;
        } else if matches!(
            kind,
            TokenKind::Punctuation(LeftParen | LeftBracket | LeftBrace)
                | TokenKind::InterpolationStart
                | TokenKind::StringStart(_)
        ) {
            // A malformed group is left to the real parser's recovery.
            possible = groups
                .pop()
                .is_none_or(|(expected, outside)| kind != expected || outside);
        } else if matches!(kind, TokenKind::Punctuation(Greater | ShiftRight))
            && next == TokenKind::Punctuation(LeftParen)
        {
            possible = true;
        } else if kind == TokenKind::Punctuation(Less) {
            candidates[index] = possible;
        }
        next = kind;
    }
    candidates.into_boxed_slice()
}

fn call_suffix(parser: &mut Parser<'_>, suffix: Marker) {
    parser.bump();
    if parser.enter_nesting() {
        parser.comma_list(
            Punctuation::RightParen,
            true,
            ExpectedSyntax::Expression,
            argument,
        );
        parser.expect_punctuation(Punctuation::RightParen);
        parser.leave_nesting();
    } else {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
    }
    parser.complete(suffix, NodeKind::CallSuffix);
}

fn argument(parser: &mut Parser<'_>) {
    expression(parser, newline::Boundary::Delimited);
}

fn primary(parser: &mut Parser<'_>, allow_braces: bool) -> Expression {
    if parser.at_punctuation(Punctuation::LeftBracket) {
        return array(parser);
    }
    if parser.at_punctuation(Punctuation::LeftParen) {
        if allow_braces
            && let Some((completed, _)) =
                parser.attempt_decided_with(closure, |_, (_, decided)| *decided)
        {
            return Expression { completed };
        }
        return grouped(parser);
    }
    if allow_braces && parser.at_keyword(Keyword::If) {
        return if_(parser);
    }
    if allow_braces && parser.at_keyword(Keyword::Match) {
        return Expression {
            completed: super::patterns::match_expression(parser),
        };
    }
    let marker = parser.start();
    let kind = match parser.current_kind() {
        TokenKind::Identifier => {
            if parser.at_contextual(crate::ContextualSpelling::UpperSelf)
                && (parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Dot)
                    || (allow_braces
                        && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::LeftBrace)))
            {
                parser.bump();
            } else {
                parser.expect_name();
            }
            NodeKind::ReferenceExpression
        }
        TokenKind::IntegerLiteral
        | TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::None) => {
            parser.bump();
            NodeKind::ScalarLiteral
        }
        TokenKind::StringStart(_) => {
            super::super::root::string_literal(parser);
            NodeKind::StringExpression
        }
        _ => {
            // Leave statement/delimiter boundaries to their owner, including a missing operand.
            if matches!(
                parser.current_kind(),
                TokenKind::Eof
                    | TokenKind::Newline
                    | TokenKind::Punctuation(
                        Punctuation::RightParen
                            | Punctuation::RightBrace
                            | Punctuation::RightBracket
                            | Punctuation::Comma
                            | Punctuation::Semicolon
                    )
            ) {
                parser.missing(ExpectedSyntax::Expression);
            } else {
                parser.error_token(ExpectedSyntax::Expression);
            }
            NodeKind::Error
        }
    };
    let completed = parser.complete(marker, kind);
    Expression { completed }
}

fn array(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
        return Expression {
            completed: parser.complete(marker, NodeKind::ArrayLiteral),
        };
    }
    parser.eat_newlines();
    let mut kind = NodeKind::ArrayLiteral;
    if !parser.at_punctuation(Punctuation::RightBracket) {
        expression(parser, newline::Boundary::Delimited);
        parser.eat_newlines();
        if parser.eat_punctuation(Punctuation::Semicolon) {
            kind = NodeKind::ArrayRepeatLiteral;
            parser.eat_newlines();
            expression(parser, newline::Boundary::Delimited);
            parser.eat_newlines();
        } else {
            while parser.eat_punctuation(Punctuation::Comma) {
                parser.eat_newlines();
                if parser.at_punctuation(Punctuation::RightBracket) || parser.at(TokenKind::Eof) {
                    break;
                }
                expression(parser, newline::Boundary::Delimited);
                parser.eat_newlines();
            }
        }
    }
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, kind),
    }
}

fn closure(parser: &mut Parser<'_>) -> (CompletedMarker, bool) {
    let marker = parser.start();
    let head = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        return (parser.complete(head, NodeKind::Error), false);
    }
    parser.eat_newlines();
    parser.attempt(|parser| {
        let captures = parser.start();
        loop {
            let capture = parser.start();
            parser.expect_name();
            if parser.at_punctuation(Punctuation::Caret) && previous_is_joint(parser) {
                parser.bump();
            }
            parser.complete(capture, NodeKind::ClosureCapture);
            parser.eat_newlines();
            if !parser.eat_punctuation(Punctuation::Comma) {
                break;
            }
            parser.eat_newlines();
            if parser.at_punctuation(Punctuation::Semicolon) {
                break;
            }
        }
        parser.expect_punctuation(Punctuation::Semicolon);
        parser.complete(captures, NodeKind::ClosureCaptures);
    });
    let parameters = parser.start();
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        |parser| {
            let parameter = parser.start();
            super::input_binding(parser);
            if parser.eat_punctuation(Punctuation::Colon) {
                super::type_(parser);
            }
            parser.complete(parameter, NodeKind::ClosureParameter);
        },
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.complete(parameters, NodeKind::ClosureParameters);
    parser.complete(head, NodeKind::ClosureHead);
    if parser.eat_punctuation(Punctuation::Colon) {
        let result = parser.start();
        super::type_(parser);
        parser.complete(result, NodeKind::ClosureResult);
    }
    let decided = parser.at_punctuation(Punctuation::LeftBrace);
    if decided {
        super::required_block(parser);
    }
    parser.leave_nesting();
    (
        parser.complete(marker, NodeKind::ClosureExpression),
        decided,
    )
}

fn record_value(parser: &mut Parser<'_>, owner: CompletedMarker) -> Expression {
    let marker = parser.precede(owner);
    let initializer = parser.start();
    parser.bump();
    if parser.enter_nesting() {
        parser.comma_list(
            Punctuation::RightBrace,
            true,
            ExpectedSyntax::Expression,
            field,
        );
        parser.expect_punctuation(Punctuation::RightBrace);
        parser.leave_nesting();
    } else {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
    }
    parser.complete(initializer, NodeKind::StructInitializer);
    Expression {
        completed: parser.complete(marker, NodeKind::StructLiteral),
    }
}

fn field(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    expression(parser, newline::Boundary::Delimited);
    parser.complete(marker, NodeKind::FieldInitializer);
}

fn grouped(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        return Expression {
            completed: parser.complete(marker, NodeKind::GroupedExpression),
        };
    }
    parser.eat_newlines();
    if parser.eat_punctuation(Punctuation::RightParen) {
        parser.leave_nesting();
        return Expression {
            completed: parser.complete(marker, NodeKind::UnitExpression),
        };
    }
    expression(parser, newline::Boundary::Delimited);
    parser.eat_newlines();
    let tuple = parser.eat_punctuation(Punctuation::Comma);
    if tuple {
        parser.eat_newlines();
        expression(parser, newline::Boundary::Delimited);
        parser.eat_newlines();
        while parser.eat_punctuation(Punctuation::Comma) {
            parser.eat_newlines();
            if parser.at_punctuation(Punctuation::RightParen) {
                break;
            }
            expression(parser, newline::Boundary::Delimited);
            parser.eat_newlines();
        }
    }
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(
            marker,
            if tuple {
                NodeKind::TupleExpression
            } else {
                NodeKind::GroupedExpression
            },
        ),
    }
}

fn if_(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    if !parser.enter_nesting() {
        parser.bump();
        return Expression {
            completed: parser.complete(marker, NodeKind::Error),
        };
    }
    parser.bump();
    let condition = parser.start();
    super::patterns::if_condition(parser);
    parser.complete(condition, NodeKind::IfCondition);
    super::required_block(parser);
    if parser.at_keyword(Keyword::Else) {
        let alternative = parser.start();
        parser.bump();
        if parser.at_keyword(Keyword::If) {
            if_(parser);
        } else {
            super::required_block(parser);
        }
        parser.complete(alternative, NodeKind::ElseClause);
    }
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, NodeKind::IfExpression),
    }
}

fn previous_is_joint(parser: &Parser<'_>) -> bool {
    parser.cursor > 0 && parser.tokens[parser.cursor - 1].is_joint_to_next()
}
