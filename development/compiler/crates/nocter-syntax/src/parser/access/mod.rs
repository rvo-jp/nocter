//! Recognition of the future bounded private-function grammar, using the shared lossless parser.
//! No released-language production is an acceptance fallback for this parse goal.
mod expression;
#[cfg(test)]
mod tests;

use super::{CompletedMarker, Parser, block, newline, statement};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn unit(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.eat_newlines();
    while !parser.at(TokenKind::Eof) {
        if parser.at_keyword(Keyword::Func) {
            function(parser);
        } else if parser.at_keyword(Keyword::Struct)
            || (parser.at_contextual(ContextualSpelling::Copy)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Struct))
        {
            record(parser);
        } else {
            parser.error_token(ExpectedSyntax::Keyword(Keyword::Func));
            parser.recover_to_line_end();
        }
        parser.require_line_end();
    }
    parser.bump();
    parser.complete(marker, NodeKind::V2CoreUnit);
}

fn record(parser: &mut Parser<'_>) {
    let item = parser.start();
    let marker = parser.start();
    if parser.at_contextual(ContextualSpelling::Copy) {
        parser.bump();
    }
    parser.expect_keyword(Keyword::Struct);
    parser.expect_name();
    parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, |parser| {
        let field = parser.start();
        parser.expect_name();
        parser.expect_punctuation(Punctuation::Colon);
        type_(parser);
        parser.complete(field, NodeKind::StructField);
    });
    parser.complete(marker, NodeKind::StructDeclaration);
    parser.complete(item, NodeKind::Item);
}

fn function(parser: &mut Parser<'_>) {
    let item = parser.start();
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    let tail = parser.start();
    let parameters = parser.start();
    if parser.expect_punctuation(Punctuation::LeftParen) {
        parser.comma_list(
            Punctuation::RightParen,
            true,
            ExpectedSyntax::Parameter,
            parameter,
        );
        parser.expect_punctuation(Punctuation::RightParen);
    }
    parser.complete(parameters, NodeKind::Parameters);
    parser.expect_punctuation(Punctuation::Colon);
    annotation_type(parser);
    parser.complete(tail, NodeKind::CallableTail);
    required_block(parser);
    parser.complete(marker, NodeKind::FunctionDeclaration);
    parser.complete(item, NodeKind::Item);
}

fn parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if (parser.at_contextual(ContextualSpelling::Edit)
        || parser.at_contextual(ContextualSpelling::Take))
        && parser.nth_kind(1) == TokenKind::Identifier
    {
        let operation = parser.start();
        parser.bump();
        parser.complete(operation, NodeKind::InputOperation);
    }
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    annotation_type(parser);
    parser.complete(marker, NodeKind::Parameter);
}

fn type_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let named = parser.start();
    if matches!(
        parser.current_kind(),
        TokenKind::Identifier | TokenKind::Keyword(Keyword::Void)
    ) {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::Type);
    }
    parser.complete(named, NodeKind::NamedType);
    parser.complete(marker, NodeKind::Type);
}

fn annotation_type(parser: &mut Parser<'_>) {
    let callable = parser.at_keyword(Keyword::Func)
        || ((parser.at_contextual(ContextualSpelling::Mut)
            || parser.at_contextual(ContextualSpelling::Once))
            && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::LeftParen));
    if !callable {
        type_(parser);
        return;
    }
    let ty = parser.start();
    let contract = parser.start();
    parser.bump();
    let parameters = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if parser.enter_nesting() {
        parser.comma_list(
            Punctuation::RightParen,
            true,
            ExpectedSyntax::Parameter,
            |parser| {
                let parameter = parser.start();
                if (parser.at_contextual(ContextualSpelling::Edit)
                    || parser.at_contextual(ContextualSpelling::Take))
                    && matches!(
                        parser.nth_kind(1),
                        TokenKind::Identifier | TokenKind::Keyword(Keyword::Void)
                    )
                {
                    let operation = parser.start();
                    parser.bump();
                    parser.complete(operation, NodeKind::InputOperation);
                }
                type_(parser);
                parser.complete(parameter, NodeKind::CallableParameter);
            },
        );
        parser.expect_punctuation(Punctuation::RightParen);
        parser.leave_nesting();
    } else {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
    }
    parser.complete(parameters, NodeKind::CallableParameters);
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    parser.complete(contract, NodeKind::CallableType);
    parser.complete(ty, NodeKind::Type);
}

fn required_block(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if !parser.at_punctuation(Punctuation::LeftBrace) {
        parser.missing(ExpectedSyntax::Block);
        parser.complete(marker, NodeKind::Block);
        return;
    }
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        parser.complete(marker, NodeKind::Block);
        return;
    }
    parser.eat_newlines();
    if !parser.at_punctuation(Punctuation::RightBrace) && !parser.at(TokenKind::Eof) {
        let sequence = parser.start();
        parser.line_sequence(Punctuation::RightBrace, ExpectedSyntax::Expression, entry);
        parser.complete(sequence, NodeKind::ExecutableSequence);
    }
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::Block);
}

fn entry(parser: &mut Parser<'_>) {
    match parser.current_kind() {
        TokenKind::Keyword(Keyword::Let | Keyword::Var) => binding(parser),
        TokenKind::Identifier
            if (parser.at_contextual(ContextualSpelling::Read)
                || parser.at_contextual(ContextualSpelling::Edit))
                && parser.nth_kind(1) == TokenKind::Identifier =>
        {
            binding(parser);
        }
        TokenKind::Keyword(Keyword::Return) => return_(parser),
        TokenKind::Keyword(Keyword::Break | Keyword::Continue) => {
            let marker = parser.start();
            let kind = if parser.at_keyword(Keyword::Break) {
                NodeKind::BreakStatement
            } else {
                NodeKind::ContinueStatement
            };
            parser.bump();
            parser.complete(marker, kind);
        }
        TokenKind::Keyword(Keyword::While | Keyword::Loop) => loop_(parser),
        TokenKind::Identifier if at_drop(parser) => {
            let marker = parser.start();
            parser.bump();
            parser.expect_name();
            parser.complete(marker, NodeKind::DropStatement);
        }
        _ => expression_entry(parser),
    }
}

fn binding(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let operation = parser.start();
    parser.bump();
    parser.complete(operation, NodeKind::BindingOperation);
    let pattern = parser.start();
    parser.expect_name_or_discard();
    parser.complete(pattern, NodeKind::BindingPattern);
    if parser.at_punctuation(Punctuation::Colon) {
        let annotation = parser.start();
        parser.bump();
        annotation_type(parser);
        parser.complete(annotation, NodeKind::TypeAnnotation);
    }
    parser.expect_punctuation(Punctuation::Equal);
    newline::after_incomplete(parser, newline::Boundary::Statement);
    expression::expression(parser, newline::Boundary::Statement);
    parser.complete(marker, NodeKind::BindingStatement);
}

fn return_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !matches!(
        parser.current_kind(),
        TokenKind::Newline | TokenKind::Eof | TokenKind::Punctuation(Punctuation::RightBrace)
    ) {
        expression::expression(parser, newline::Boundary::Statement);
    }
    parser.complete(marker, NodeKind::ReturnStatement);
}

fn loop_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let is_while = parser.at_keyword(Keyword::While);
    parser.bump();
    if is_while {
        expression::header_expression(parser);
    }
    required_block(parser);
    parser.complete(
        marker,
        if is_while {
            NodeKind::WhileStatement
        } else {
            NodeKind::LoopStatement
        },
    );
}

fn at_drop(parser: &Parser<'_>) -> bool {
    parser.at_contextual(ContextualSpelling::Drop)
        && parser.nth_kind(1) == TokenKind::Identifier
        && matches!(
            parser.nth_kind(2),
            TokenKind::Newline | TokenKind::Eof | TokenKind::Punctuation(Punctuation::RightBrace)
        )
}

fn expression_entry(parser: &mut Parser<'_>) {
    if statement::has_possible_assignment(parser)
        && parser
            .attempt_decided_with(assignment, |_, (_, has_operator)| *has_operator)
            .is_some()
    {
        return;
    }
    let expression = expression::expression(parser, newline::Boundary::Statement);
    let wrapper = parser.precede(expression.completed);
    parser.complete(
        wrapper,
        if block::only_newlines_before_close(parser) {
            NodeKind::BodyResult
        } else {
            NodeKind::ExpressionStatement
        },
    );
}

fn assignment(parser: &mut Parser<'_>) -> (CompletedMarker, bool) {
    let marker = parser.start();
    let target = parser.start();
    expression::postfix(parser);
    parser.complete(target, NodeKind::AssignmentTarget);
    newline::before(parser, newline::Boundary::Statement, assignment_operator);
    let has_operator = assignment_operator(parser.current_kind());
    if has_operator {
        parser.bump();
        newline::after_incomplete(parser, newline::Boundary::Statement);
        expression::expression(parser, newline::Boundary::Statement);
    }
    (
        parser.complete(marker, NodeKind::AssignmentStatement),
        has_operator,
    )
}

fn assignment_operator(kind: TokenKind) -> bool {
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
