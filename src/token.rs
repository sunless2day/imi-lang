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
