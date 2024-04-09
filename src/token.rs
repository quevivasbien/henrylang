use std::collections::HashMap;
use enum_iterator::Sequence;
use lazy_static::lazy_static;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash, Sequence)]
pub enum TokenType {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Pipe,
    LSquare,
    RSquare,

    Comma,
    Dot,
    Colon,
    RightArrow,

    Eq,
    NEq,
    GT,
    LT,
    GEq,
    LEq,

    Plus,
    Minus,
    Slash,
    Star,

    Assign,
    Bang,

    Ident,
    Int,
    Float,
    Str,

    And,
    Or,
    Type,
    If,
    Else,
    True,
    False,
    To,
    Some,

    Error,
    EoF,
}

lazy_static! {
    static ref SINGLE_CHAR_TOKENS: HashMap<char, TokenType> = {
        let mut map = HashMap::new();
        map.insert('(', TokenType::LParen);
        map.insert(')', TokenType::RParen);
        map.insert('{', TokenType::LBrace);
        map.insert('}', TokenType::RBrace);
        map.insert('|', TokenType::Pipe);
        map.insert('[', TokenType::LSquare);
        map.insert(']', TokenType::RSquare);
        map.insert('.', TokenType::Dot);
        map.insert(',', TokenType::Comma);
        map.insert('=', TokenType::Eq);
        map.insert('+', TokenType::Plus);
        map.insert('/', TokenType::Slash);
        map.insert('*', TokenType::Star);

        map
    };

    static ref KEYWORDS: HashMap<&'static str, TokenType> = {
        let mut map = HashMap::new();
        map.insert("and", TokenType::And);
        map.insert("or", TokenType::Or);
        map.insert("type", TokenType::Type);
        map.insert("if", TokenType::If);
        map.insert("else", TokenType::Else);
        map.insert("true", TokenType::True);
        map.insert("false", TokenType::False);
        map.insert("to", TokenType::To);
        map.insert("some", TokenType::Some);

        map
    };
}

impl TokenType {
    pub fn single_char_keyword(c: char) -> Option<TokenType> {
        SINGLE_CHAR_TOKENS.get(&c).copied()
    }

    pub fn keyword_or_ident(text: &str) -> TokenType {
        match KEYWORDS.get(text) {
            Some(ttype) => *ttype,
            None => TokenType::Ident,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub ttype: TokenType,
    pub line: usize,
    pub text: String,
}
