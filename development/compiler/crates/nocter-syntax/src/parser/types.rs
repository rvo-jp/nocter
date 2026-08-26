use super::{CompletedMarker, Parser, expression};
use crate::{BuiltinType, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn type_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    prefix_type(parser);
    outcome_suffix(parser);
    parser.complete(marker, NodeKind::Type);
}

pub(super) fn callable_result(parser: &mut Parser<'_>) {
    if parser.at_identifier_text("some") {
        opaque_result(parser);
        outcome_suffix(parser);
    } else {
        type_(parser);
    }
}

pub(super) fn generic_parameters(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    type_delimited_list(parser, false, ExpectedSyntax::Name, |parser| {
        expect_pattern_binder(parser);
    });
    parser.expect_type_greater();
    parser.complete(marker, NodeKind::GenericParameters);
}

pub(super) fn declaration_type_pattern(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.eat_punctuation(Punctuation::LeftBracket) {
        expect_pattern_binder(parser);
        parser.expect_punctuation(Punctuation::RightBracket);
    } else if at_builtin_pattern(parser) {
        named_type(parser);
    } else if parser.at(TokenKind::Identifier) && !matches!(parser.current_text(), "_" | "Self") {
        parser.bump();
        if parser.at_punctuation(Punctuation::Less) {
            pattern_arguments(parser);
        }
    } else {
        parser.error_token(ExpectedSyntax::DeclarationTypePattern);
    }
    parser.complete(marker, NodeKind::DeclarationTypePattern);
}

pub(super) fn parameters(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(marker, NodeKind::Parameters);
        return;
    }
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        parameter,
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::Parameters);
}

pub(super) fn provenance_clause(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    while parser.eat_punctuation(Punctuation::Pipe) {
        parser.expect_name();
    }
    parser.complete(marker, NodeKind::ProvenanceClause);
}

pub(super) fn interface_application(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at(TokenKind::Identifier) && !at_builtin_type(parser) {
        named_type(parser);
    } else {
        parser.error_token(ExpectedSyntax::Interface);
    }
    if at_associated_bindings(parser) {
        associated_bindings(parser);
    }
    parser.complete(marker, NodeKind::InterfaceApplication);
}

fn at_associated_bindings(parser: &Parser<'_>) -> bool {
    if !parser.at_punctuation(Punctuation::LeftBrace) {
        return false;
    }
    let mut offset = 1;
    while parser.nth_kind(offset) == TokenKind::Newline {
        offset += 1;
    }
    parser.nth_kind(offset) == TokenKind::Punctuation(Punctuation::Dot)
}

fn associated_bindings(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightBrace)]);
        parser.expect_punctuation(Punctuation::RightBrace);
        parser.complete(marker, NodeKind::AssociatedBindings);
        return;
    }
    parser.comma_list(
        Punctuation::RightBrace,
        false,
        ExpectedSyntax::DeclarationMember,
        associated_type_binding,
    );
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::AssociatedBindings);
}

fn associated_type_binding(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Equal);
    type_(parser);
    parser.complete(marker, NodeKind::AssociatedTypeBinding);
}

pub(super) fn parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    argument_pack_modifier(parser);
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    parser.complete(marker, NodeKind::Parameter);
}

fn argument_pack_modifier(parser: &mut Parser<'_>) {
    if !parser.at_punctuation(Punctuation::Expansion) {
        return;
    }
    let marker = parser.start();
    parser.bump();
    parser.complete(marker, NodeKind::ArgumentPackModifier);
}

fn prefix_type(parser: &mut Parser<'_>) {
    prefix_type_with_callable(parser, true);
}

pub(super) fn borrow_type(parser: &mut Parser<'_>) {
    if at_readonly_borrow(parser) || parser.at_punctuation(Punctuation::ReadWrite) {
        prefix_type_with_callable(parser, false);
    } else {
        parser.error_token(ExpectedSyntax::Type);
    }
}

fn prefix_type_with_callable(parser: &mut Parser<'_>, mut callable_allowed: bool) {
    let mut wrappers = Vec::new();

    loop {
        if callable_allowed && at_callable_type(parser) {
            callable_type(parser);
            break;
        }
        if parser.at_punctuation(Punctuation::Star) {
            let marker = parser.start();
            parser.bump();
            wrappers.push((marker, NodeKind::PointerType));
            callable_allowed = true;
            continue;
        }
        if !at_readonly_borrow(parser) && !parser.at_punctuation(Punctuation::ReadWrite) {
            type_atom(parser);
            break;
        }
        let marker = parser.start();
        if parser.at_punctuation(Punctuation::LogicalAnd) {
            parser.split_current(
                TokenKind::Punctuation(Punctuation::Ampersand),
                TokenKind::Punctuation(Punctuation::Ampersand),
            );
        } else {
            parser.bump();
        }
        wrappers.push((marker, NodeKind::BorrowType));
        callable_allowed = false;
    }

    for (marker, kind) in wrappers.into_iter().rev() {
        parser.complete(marker, kind);
    }
}

fn at_readonly_borrow(parser: &Parser<'_>) -> bool {
    parser.at_punctuation(Punctuation::Ampersand) || parser.at_punctuation(Punctuation::LogicalAnd)
}

fn at_callable_type(parser: &Parser<'_>) -> bool {
    parser.at_keyword(Keyword::Func)
        || parser.at_punctuation(Punctuation::Ampersand)
            && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
        || parser.at_punctuation(Punctuation::ReadWrite)
            && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
}

fn callable_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_punctuation(Punctuation::Ampersand)
        || parser.at_punctuation(Punctuation::ReadWrite)
    {
        parser.bump();
    }
    parser.expect_keyword(Keyword::Func);
    let parameters = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(parameters, NodeKind::CallableParameters);
        parser.complete(marker, NodeKind::CallableType);
        return;
    }
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        callable_parameter,
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(parameters, NodeKind::CallableParameters);
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    if parser.at_identifier_text("from") {
        provenance_clause(parser);
    }
    parser.complete(marker, NodeKind::CallableType);
}

fn callable_parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    argument_pack_modifier(parser);
    if parser.at(TokenKind::Identifier)
        && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Colon)
    {
        parser.bump();
        parser.bump();
    }
    type_(parser);
    parser.complete(marker, NodeKind::CallableParameter);
}

fn type_atom(parser: &mut Parser<'_>) {
    match parser.current_kind() {
        TokenKind::Identifier | TokenKind::Keyword(Keyword::Void | Keyword::Never) => {
            named_type(parser);
        }
        TokenKind::Punctuation(Punctuation::LeftBracket) => bracket_type(parser),
        TokenKind::Punctuation(Punctuation::LeftParen) => grouped_type(parser),
        _ => parser.error_token(ExpectedSyntax::Type),
    }
}

fn named_type(parser: &mut Parser<'_>) -> CompletedMarker {
    named_type_with_facts(parser).0
}

pub(super) fn owner_reference(parser: &mut Parser<'_>) -> OwnerFacts {
    if parser.at(TokenKind::Identifier) {
        let (completed, has_type_arguments, plain_selections_after_type_arguments) =
            named_type_with_facts(parser);
        OwnerFacts {
            completed,
            has_type_arguments,
            plain_selections_after_type_arguments,
        }
    } else {
        let marker = parser.start();
        parser.error_token(ExpectedSyntax::Type);
        OwnerFacts {
            completed: parser.complete(marker, NodeKind::Error),
            has_type_arguments: false,
            plain_selections_after_type_arguments: 0,
        }
    }
}

fn named_type_with_facts(parser: &mut Parser<'_>) -> (CompletedMarker, bool, usize) {
    let marker = parser.start();
    let mut has_type_arguments = false;
    let mut plain_selections_after_type_arguments = 0;
    if parser.at_identifier_text("Self") {
        let self_type = parser.start();
        parser.bump();
        parser.complete(self_type, NodeKind::SelfType);
    } else {
        parser.bump();
        if parser.at_punctuation(Punctuation::Less) {
            has_type_arguments = parser.attempt(type_arguments);
        }
    }
    while parser.eat_punctuation(Punctuation::Dot) {
        parser.expect_name();
        if parser.at_punctuation(Punctuation::Less) {
            if parser.attempt(type_arguments) {
                has_type_arguments = true;
                plain_selections_after_type_arguments = 0;
            }
        } else if has_type_arguments {
            plain_selections_after_type_arguments += 1;
        }
    }
    (
        parser.complete(marker, NodeKind::NamedType),
        has_type_arguments,
        plain_selections_after_type_arguments,
    )
}

fn pattern_arguments(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    type_delimited_list(parser, false, ExpectedSyntax::Name, expect_pattern_binder);
    parser.expect_type_greater();
    parser.complete(marker, NodeKind::PatternArguments);
}

fn expect_pattern_binder(parser: &mut Parser<'_>) {
    if parser.at(TokenKind::Identifier)
        && !matches!(parser.current_text(), "_" | "Self")
        && !at_builtin_pattern(parser)
    {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::Name);
    }
}

fn at_builtin_pattern(parser: &Parser<'_>) -> bool {
    parser.at(TokenKind::Identifier)
        && BuiltinType::from_spelling(parser.current_text())
            .is_some_and(BuiltinType::is_declaration_pattern)
}

fn at_builtin_type(parser: &Parser<'_>) -> bool {
    BuiltinType::from_spelling(parser.current_text()).is_some()
}

fn type_arguments(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[
            TokenKind::Punctuation(Punctuation::Greater),
            TokenKind::Punctuation(Punctuation::ShiftRight),
        ]);
        parser.expect_type_greater();
        parser.complete(marker, NodeKind::TypeArguments);
        return;
    }
    type_delimited_list(parser, false, ExpectedSyntax::Type, type_);
    parser.expect_type_greater();
    parser.leave_nesting();
    parser.complete(marker, NodeKind::TypeArguments);
}

fn bracket_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightBracket)]);
        parser.expect_punctuation(Punctuation::RightBracket);
        parser.complete(marker, NodeKind::SliceType);
        return;
    }
    type_(parser);
    let kind = if parser.eat_punctuation(Punctuation::Semicolon) {
        expression::expression(parser, expression::ExpressionMode::Delimited);
        NodeKind::FixedArrayType
    } else {
        NodeKind::SliceType
    };
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    parser.complete(marker, kind);
}

fn grouped_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(marker, NodeKind::GroupedType);
        return;
    }
    type_(parser);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::GroupedType);
}

fn outcome_suffix(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::Question) {
        parser.bump();
        parser.eat_punctuation(Punctuation::Bang);
    } else if parser.at_punctuation(Punctuation::Bang) {
        parser.bump();
    }
}

fn opaque_result(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    interface_application(parser);
    parser.complete(marker, NodeKind::OpaqueResult);
}

fn type_delimited_list(
    parser: &mut Parser<'_>,
    allow_empty: bool,
    expected_element: ExpectedSyntax,
    parse_element: fn(&mut Parser<'_>),
) {
    parser.eat_newlines();
    if at_type_list_end(parser) {
        if !allow_empty {
            parser.missing(expected_element);
        }
        return;
    }

    loop {
        let before = parser.cursor;
        parse_element(parser);
        if parser.cursor == before && parser.split.is_none() {
            parser.error_token(expected_element);
        }
        if parser.eat_punctuation(Punctuation::Comma) {
            parser.eat_newlines();
            if at_type_list_end(parser) {
                return;
            }
            continue;
        }
        parser.eat_newlines();
        if at_type_list_end(parser) {
            return;
        }
        parser.missing(ExpectedSyntax::Punctuation(Punctuation::Comma));
    }
}

fn at_type_list_end(parser: &Parser<'_>) -> bool {
    parser.at_punctuation(Punctuation::Greater)
        || parser.at_punctuation(Punctuation::ShiftRight)
        || parser.at(TokenKind::Eof)
}

pub(super) struct OwnerFacts {
    pub(super) completed: CompletedMarker,
    pub(super) has_type_arguments: bool,
    pub(super) plain_selections_after_type_arguments: usize,
}
