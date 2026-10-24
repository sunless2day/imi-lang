//! Tokens that define my language's vocabulary

use crate::error::LangError;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),

    // Keywords in the language
    Let,
    Var,
    Fn,
    Return,
    If,
    Else,
    While,
    Continue,
    Break,
    And,
    Or,
    Not,
    True,
    False,
    Array,
    KwInt,
    KwFloat,
    KwStr,
    KwBool,

    // Relevant symbols
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    Eq,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    CaretEq,
    PercentEq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Arrow,
    Dot,

    /// sentinel so peeking past the end is always valid,
    /// the parser checks for this instead of dealing with Option<Token> everywhere
    /// much nicer imo
    Eof,
}

/// real pain to write.
/// before this existed, I relied on rust's {:?} and leaked names for things,
/// like "found RBrace" or "found Ident("x")". now it's "found '}'" and "found identifier 'x'"
impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Token::Int(n) => return write!(f, "'{}'", n),
            Token::Float(x) => return write!(f, "'{}'", x),
            Token::Str(s) => return write!(f, "string \"{}\"", s),
            Token::Ident(name) => return write!(f, "identifier '{}'", name),
            Token::Eof => return write!(f, "end of file"),
            Token::Let => "let",
            Token::Var => "var",
            Token::Fn => "fn",
            Token::Return => "return",
            Token::If => "if",
            Token::Else => "else",
            Token::While => "while",
            Token::Continue => "continue",
            Token::Break => "break",
            Token::And => "and",
            Token::Or => "or",
            Token::Not => "not",
            Token::True => "true",
            Token::False => "false",
            Token::Array => "array",
            Token::KwInt => "int",
            Token::KwFloat => "float",
            Token::KwStr => "str",
            Token::KwBool => "bool",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Caret => "^",
            Token::Percent => "%",
            Token::Eq => "=",
            Token::EqEq => "==",
            Token::BangEq => "!=",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::LtEq => "<=",
            Token::GtEq => ">=",
            Token::PlusEq => "+=",
            Token::MinusEq => "-=",
            Token::StarEq => "*=",
            Token::SlashEq => "/=",
            Token::CaretEq => "^=",
            Token::PercentEq => "%=",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::Comma => ",",
            Token::Colon => ":",
            Token::Semicolon => ";",
            Token::Arrow => "->",
            Token::Dot => ".",
        };
        write!(f, "'{}'", s)
    }
}

/// The interpreter must keep track of which line and column it is in the code to notify errors clearly
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    /// we use generics because Spanned will wrap around multiple types
    /// to avoid writing multiple structs of the same kind for each distinct type that will use it, we resort to generics
    /// this also avoids writing multiple implementations of practically the same error method
    pub node: T,
    /// 1-based, so it matches what editors show
    pub line: usize,
    /// also 1-based, resets to 1 on every newline
    pub col: usize,
}

impl<T> Spanned<T> {
    /// I'm adding this now to make a good use of the error module
    pub fn error(&self, msg: impl Into<String>) -> LangError {
        LangError::new(self.line, self.col, msg)
        // look how elegant and fancy, way better than panic!("x and y");
    }
}
