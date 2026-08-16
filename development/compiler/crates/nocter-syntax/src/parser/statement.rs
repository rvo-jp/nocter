use super::{CompletedMarker, Parser, block, expression, newline, place, types};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) struct Executable {
    pub(super) completed: CompletedMarker,
    pub(super) is_expression: bool,
}

pub(super) fn executable(parser: &mut Parser<'_>) -> Executable {
    let statement = match parser.current_kind() {
        TokenKind::Keyword(Keyword::Let | Keyword::Var) => Some(binding(parser)),
        TokenKind::Keyword(Keyword::Return) => Some(return_statement(parser)),
        TokenKind::Keyword(Keyword::Break) => Some(simple_statement(
            parser,
            Keyword::Break,
            NodeKind::BreakStatement,
        )),
        TokenKind::Keyword(Keyword::Continue) => Some(simple_statement(
            parser,
            Keyword::Continue,
            NodeKind::ContinueStatement,
        )),
        TokenKind::Keyword(Keyword::While) => Some(while_statement(parser)),
        TokenKind::Keyword(Keyword::Loop) => Some(loop_statement(parser)),
        TokenKind::Keyword(Keyword::For) => Some(for_statement(parser)),
        TokenKind::Keyword(Keyword::Region) => Some(region_statement(parser)),
        TokenKind::Identifier if at_drop_statement(parser) => Some(drop_statement(parser)),
        _ => None,
    };

    if let Some(completed) = statement {
        return Executable {
            completed,
            is_expression: false,
        };
    }

    if has_possible_assignment(parser)
        && let Some(attempt) =
            parser.attempt_decided_with(assignment, |_, attempt| attempt.has_operator)
    {
        return Executable {
            completed: attempt.completed,
            is_expression: false,
        };
    }

    Executable {
        completed: expression::expression(parser, expression::ExpressionMode::Ordinary),
        is_expression: true,
    }
}

fn binding(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    let target = parser.start();
    parser.expect_name_or_discard();
    parser.complete(target, NodeKind::BindingTarget);
    if parser.at_punctuation(Punctuation::Colon) {
        let annotation = parser.start();
        parser.bump();
        types::type_(parser);
        parser.complete(annotation, NodeKind::TypeAnnotation);
    }
    parser.expect_punctuation(Punctuation::Equal);
    newline::after_incomplete(parser, newline::Boundary::Statement);
    expression::expression(parser, expression::ExpressionMode::Ordinary);
    parser.complete(marker, NodeKind::BindingStatement)
}

fn assignment(parser: &mut Parser<'_>) -> AssignmentAttempt {
    let marker = parser.start();
    let target = parser.start();
    expression::postfix(parser, expression::ExpressionMode::Ordinary);
    parser.complete(target, NodeKind::AssignmentTarget);
    newline::before(parser, newline::Boundary::Statement, |kind| {
        matches!(
            kind,
            TokenKind::Punctuation(
                Punctuation::Equal
                    | Punctuation::PlusEqual
                    | Punctuation::MinusEqual
                    | Punctuation::StarEqual
                    | Punctuation::SlashEqual
                    | Punctuation::PercentEqual
            )
        )
    });
    if !at_assignment_operator(parser) {
        parser.missing(ExpectedSyntax::AssignmentTarget);
        return AssignmentAttempt {
            completed: parser.complete(marker, NodeKind::AssignmentStatement),
            has_operator: false,
        };
    }
    parser.bump();
    newline::after_incomplete(parser, newline::Boundary::Statement);
    expression::expression(parser, expression::ExpressionMode::Ordinary);
    AssignmentAttempt {
        completed: parser.complete(marker, NodeKind::AssignmentStatement),
        has_operator: true,
    }
}

fn at_assignment_operator(parser: &Parser<'_>) -> bool {
    is_assignment_operator(parser.current_kind())
}

fn has_possible_assignment(parser: &Parser<'_>) -> bool {
    let mut distance = 0_usize;
    let mut delimiter_depth = 0_usize;
    let mut interpolation_depth = 0_usize;
    loop {
        match parser.nth_kind(distance) {
            TokenKind::Punctuation(
                Punctuation::LeftParen | Punctuation::LeftBracket | Punctuation::LeftBrace,
            ) => delimiter_depth += 1,
            TokenKind::Punctuation(
                Punctuation::RightParen | Punctuation::RightBracket | Punctuation::RightBrace,
            ) if delimiter_depth == 0 => {
                return false;
            }
            TokenKind::Punctuation(
                Punctuation::RightParen | Punctuation::RightBracket | Punctuation::RightBrace,
            ) => {
                delimiter_depth -= 1;
            }
            TokenKind::InterpolationStart => interpolation_depth += 1,
            TokenKind::InterpolationEnd => {
                interpolation_depth = interpolation_depth.saturating_sub(1);
            }
            kind if delimiter_depth == 0
                && interpolation_depth == 0
                && is_assignment_operator(kind) =>
            {
                return true;
            }
            TokenKind::Newline if delimiter_depth == 0 && interpolation_depth == 0 => {
                return is_assignment_operator(parser.nth_kind(distance + 1));
            }
            TokenKind::Eof => return false,
            _ => {}
        }
        distance += 1;
    }
}

fn is_assignment_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Punctuation(
            Punctuation::Equal
                | Punctuation::PlusEqual
                | Punctuation::MinusEqual
                | Punctuation::StarEqual
                | Punctuation::SlashEqual
                | Punctuation::PercentEqual
        )
    )
}

fn return_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    if !at_line_end(parser) {
        expression::expression(parser, expression::ExpressionMode::Ordinary);
    }
    parser.complete(marker, NodeKind::ReturnStatement)
}

fn simple_statement(parser: &mut Parser<'_>, keyword: Keyword, kind: NodeKind) -> CompletedMarker {
    let marker = parser.start();
    parser.expect_keyword(keyword);
    parser.complete(marker, kind)
}

fn drop_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    parser.complete(marker, NodeKind::DropStatement)
}

fn while_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    expression::expression(parser, expression::ExpressionMode::Header);
    block::required(parser);
    parser.complete(marker, NodeKind::WhileStatement)
}

fn loop_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    block::required(parser);
    parser.complete(marker, NodeKind::LoopStatement)
}

fn for_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    parser.expect_keyword(Keyword::In);
    let source = parser.start();
    expression::expression(parser, expression::ExpressionMode::Header);
    newline::before_token(
        parser,
        newline::Boundary::Statement,
        TokenKind::Punctuation(Punctuation::Range),
    );
    if parser.eat_punctuation(Punctuation::Range) {
        newline::after_incomplete(parser, newline::Boundary::Statement);
        expression::expression(parser, expression::ExpressionMode::Header);
    }
    parser.complete(source, NodeKind::ForSource);
    block::required(parser);
    parser.complete(marker, NodeKind::ForStatement)
}

fn region_statement(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    parser.expect_keyword(Keyword::Using);
    place::allocator(parser);
    block::required(parser);
    parser.complete(marker, NodeKind::RegionStatement)
}

fn at_drop_statement(parser: &Parser<'_>) -> bool {
    parser.current_text() == "drop"
        && parser.nth_kind(1) == TokenKind::Identifier
        && matches!(
            parser.nth_kind(2),
            TokenKind::Newline | TokenKind::Eof | TokenKind::Punctuation(Punctuation::RightBrace)
        )
}

fn at_line_end(parser: &Parser<'_>) -> bool {
    matches!(
        parser.current_kind(),
        TokenKind::Newline | TokenKind::Eof | TokenKind::Punctuation(Punctuation::RightBrace)
    )
}

struct AssignmentAttempt {
    completed: CompletedMarker,
    has_operator: bool,
}
