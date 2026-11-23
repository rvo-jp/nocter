//! Recognition of the future bounded private core, using the shared lossless parser.
//! No released-language production is an acceptance fallback for this parse goal.
mod behavior;
mod directives;
mod expression;
mod nominal;
mod patterns;
mod types;
use types::type_;
#[cfg(test)]
mod tests;

use super::{CompletedMarker, Parser, block, newline, root, source_visibility, statement};
use crate::{
    ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, ParseDiagnosticKind, Punctuation,
    TokenKind,
};

pub(super) fn unit(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.eat_newlines();
    super::package::prefix(parser);
    let mut declarations_started = false;
    while !parser.at(TokenKind::Eof) {
        if parser.at_keyword(Keyword::See) {
            if declarations_started {
                parser.diagnostic(ParseDiagnosticKind::LateDependencyDeclaration);
            }
            source_visibility::declaration(parser);
            parser.require_line_end();
            continue;
        }
        let entry = parser.start();
        let target = directives::prefix(parser, true);
        if parser.at_keyword(Keyword::Use) {
            if declarations_started {
                parser.diagnostic(ParseDiagnosticKind::LateDependencyDeclaration);
            }
            if target {
                parser.diagnostic(ParseDiagnosticKind::Expected(
                    ExpectedSyntax::TargetableItem,
                ));
            }
            parser.bump();
            root::use_tree(parser);
            parser.complete(entry, NodeKind::UseDeclaration);
            parser.require_line_end();
            continue;
        }
        declarations_started = true;
        if parser.at_keyword(Keyword::Func)
            || (parser.at_keyword(Keyword::Async)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func))
            || (parser.at_keyword(Keyword::Primitive)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func))
            || (parser.at_keyword(Keyword::Const)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func))
        {
            function(parser, NodeKind::FunctionDeclaration);
        } else if parser.at_keyword(Keyword::Primitive)
            && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Type)
        {
            let declaration = parser.start();
            parser.bump();
            parser.bump();
            if matches!(
                parser.current_kind(),
                TokenKind::Keyword(Keyword::Void | Keyword::Never)
            ) {
                parser.bump();
            } else {
                parser.expect_name();
            }
            parser.complete(declaration, NodeKind::PrimitiveTypeDeclaration);
        } else if parser.at_keyword(Keyword::Const)
            || parser.at_contextual(ContextualSpelling::Static)
        {
            initializer(parser);
        } else if parser.at_keyword(Keyword::Type) {
            type_alias(parser);
        } else if parser.at_keyword(Keyword::Struct)
            || (parser.at_contextual(ContextualSpelling::Copy)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Struct))
        {
            nominal::record(parser);
        } else if parser.at_keyword(Keyword::Enum)
            || (parser.at_contextual(ContextualSpelling::Copy)
                && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Enum))
        {
            nominal::enumeration(parser);
        } else if parser.at_keyword(Keyword::Impl) {
            behavior::declaration(parser);
        } else if parser.at_keyword(Keyword::Test) {
            super::declaration::test_declaration(parser, required_block);
        } else {
            parser.error_token(ExpectedSyntax::Keyword(Keyword::Func));
            parser.recover_to_line_end();
        }
        parser.complete(entry, NodeKind::Item);
        parser.require_line_end();
    }
    parser.bump();
    parser.complete(marker, NodeKind::V2CoreUnit);
}

fn type_alias(parser: &mut Parser<'_>) {
    let declaration = parser.start();
    parser.expect_keyword(Keyword::Type);
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Equal);
    type_(parser);
    parser.complete(declaration, NodeKind::TypeAliasDeclaration);
}

fn function(parser: &mut Parser<'_>, kind: NodeKind) {
    let marker = parser.start();
    function_with_prefix(parser, marker, kind);
}

fn function_with_prefix(parser: &mut Parser<'_>, marker: super::Marker, kind: NodeKind) {
    callable_modifier(parser);
    let primitive = parser.eat_keyword(Keyword::Primitive);
    parser.expect_keyword(Keyword::Func);
    parser.expect_name();
    callable_tail(parser);
    if primitive {
        if parser.at_punctuation(Punctuation::LeftBrace) {
            parser.error_token(ExpectedSyntax::Newline);
        }
    } else if parser.at_punctuation(Punctuation::LeftBrace) {
        required_block(parser);
    }
    parser.complete(marker, kind);
}

fn callable_modifier(parser: &mut Parser<'_>) {
    let modifier_kind = match parser.current_kind() {
        TokenKind::Keyword(Keyword::Const) => Some(NodeKind::CompileTimeModifier),
        TokenKind::Keyword(Keyword::Async) => Some(NodeKind::AsyncModifier),
        _ => None,
    };
    if let Some(kind) = modifier_kind {
        let modifier = parser.start();
        parser.bump();
        parser.complete(modifier, kind);
    }
}

fn callable_tail(parser: &mut Parser<'_>) {
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
    type_(parser);
    parser.complete(tail, NodeKind::CallableTail);
}

fn initializer(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let kind = if parser.at_contextual(ContextualSpelling::Static) {
        NodeKind::StaticDeclaration
    } else {
        NodeKind::ConstantDeclaration
    };
    parser.bump();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    if parser.eat_punctuation(Punctuation::Equal) {
        expression::expression(parser, newline::Boundary::Statement);
    }
    parser.complete(marker, kind);
}

fn parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    input_binding(parser);
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    parser.complete(marker, NodeKind::Parameter);
}

fn input_binding(parser: &mut Parser<'_>) {
    input_operation(parser, |next| {
        matches!(
            next,
            TokenKind::Identifier | TokenKind::Punctuation(Punctuation::LeftParen)
        )
    });
    if parser.at_punctuation(Punctuation::LeftParen)
        || parser.at_contextual(ContextualSpelling::Discard)
    {
        super::statement::binding_pattern(parser);
    } else {
        parser.expect_name();
    }
}

fn input_operation(parser: &mut Parser<'_>, starts_input: impl FnOnce(TokenKind) -> bool) {
    if at_input_operation(parser) && starts_input(parser.nth_kind(1)) {
        consume_input_operation(parser);
    }
}

fn at_input_operation(parser: &Parser<'_>) -> bool {
    parser.at_contextual(ContextualSpelling::Edit) || parser.at_contextual(ContextualSpelling::Take)
}

fn consume_input_operation(parser: &mut Parser<'_>) {
    let operation = parser.start();
    parser.bump();
    parser.complete(operation, NodeKind::InputOperation);
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
    super::block::parse_use_prefix(parser);
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
        TokenKind::Keyword(Keyword::Use) => {
            parser.diagnostic(ParseDiagnosticKind::LateDependencyDeclaration);
            super::block::block_use(parser);
        }
        TokenKind::Keyword(Keyword::Let | Keyword::Var) => binding(parser),
        TokenKind::Identifier if at_scoped_binding(parser) => binding(parser),
        TokenKind::Keyword(Keyword::Return) => return_(parser),
        TokenKind::Keyword(Keyword::Throw) => {
            let marker = parser.start();
            parser.bump();
            newline::after_incomplete(parser, newline::Boundary::Statement);
            expression::expression(parser, newline::Boundary::Statement);
            parser.complete(marker, NodeKind::ThrowStatement);
        }
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
    statement::binding_pattern(parser);
    if parser.at_punctuation(Punctuation::Colon) {
        let annotation = parser.start();
        parser.bump();
        type_(parser);
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

fn at_scoped_binding(parser: &Parser<'_>) -> bool {
    if !parser.at_contextual(ContextualSpelling::Read)
        && !parser.at_contextual(ContextualSpelling::Edit)
    {
        return false;
    }
    match parser.nth_kind(1) {
        TokenKind::Identifier => return true,
        TokenKind::Punctuation(Punctuation::LeftParen) => {}
        _ => return false,
    }
    // A parenthesized pattern has a binding tail. Ordinary calls keep their contextual name.
    let mut depth = 1;
    let mut distance = 2;
    loop {
        match parser.nth_kind(distance) {
            TokenKind::Punctuation(Punctuation::LeftParen) => depth += 1,
            TokenKind::Punctuation(Punctuation::RightParen) if depth == 1 => {
                return matches!(
                    parser.nth_kind(distance + 1),
                    TokenKind::Punctuation(Punctuation::Colon | Punctuation::Equal)
                );
            }
            TokenKind::Punctuation(Punctuation::RightParen) => depth -= 1,
            TokenKind::Identifier
            | TokenKind::Newline
            | TokenKind::Punctuation(Punctuation::Comma) => {}
            _ => return false,
        }
        distance += 1;
    }
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
