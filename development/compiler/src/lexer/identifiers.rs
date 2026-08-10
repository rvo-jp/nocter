use super::*;

pub(in crate::lexer) fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(in crate::lexer) fn keyword(text: &str) -> Option<Keyword> {
    Some(match text {
        "use" => Keyword::Use,
        "func" => Keyword::Func,
        "pub" => Keyword::Pub,
        "type" => Keyword::Type,
        "struct" => Keyword::Struct,
        "enum" => Keyword::Enum,
        "interface" => Keyword::Interface,
        "instance" => Keyword::Instance,
        "conform" => Keyword::Conform,
        "destruct" => Keyword::Destruct,
        "method" => Keyword::Method,
        "let" => Keyword::Let,
        "var" => Keyword::Var,
        "return" => Keyword::Return,
        "if" => Keyword::If,
        "else" => Keyword::Else,
        "for" => Keyword::For,
        "in" => Keyword::In,
        "while" => Keyword::While,
        "loop" => Keyword::Loop,
        "break" => Keyword::Break,
        "continue" => Keyword::Continue,
        "match" => Keyword::Match,
        "is" => Keyword::Is,
        "otherwise" => Keyword::Otherwise,
        "catch" => Keyword::Catch,
        "none" => Keyword::None,
        "true" => Keyword::True,
        "false" => Keyword::False,
        "move" => Keyword::Move,
        "as" => Keyword::As,
        "region" => Keyword::Region,
        "using" => Keyword::Using,
        "primitive" => Keyword::Primitive,
        "literal" => Keyword::Literal,
        "construct" => Keyword::Construct,
        "coerce" => Keyword::Coerce,
        "test" => Keyword::Test,
        "void" => Keyword::Void,
        "never" => Keyword::Never,
        _ => return None,
    })
}

pub(crate) fn is_valid_identifier_name(text: &str) -> bool {
    let Some(first) = text.as_bytes().first().copied() else {
        return false;
    };

    matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'_')
        && text.bytes().all(is_identifier_continue)
        && keyword(text).is_none()
}
