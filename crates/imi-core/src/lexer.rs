//! This is the first component of the interpreter that the main function will call during execution.
//! It was quite fun implementing this lexer

use crate::{
    error::LangError,
    token::{Spanned, Token},
};

/// walks the source character by character, keeping track of line and column as it goes
pub struct Lexer {
    /// collected into a Vec<char> so peeking by index is chill and unicode-safe (no slicing mid-character)
    /// (this is honestly why I chose to work with Rust instead of C for this project)
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    // small helpers
    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// returns '\0' when out of bounds as a sentinel
    /// real '\0's can technically exist (the '\0' escape), but every peek is paired with
    /// is_at_end() so the collision never causes trouble
    fn peek_at(&self, offset: usize) -> char {
        *self.source.get(self.pos + offset).unwrap_or(&'\0')
    }

    fn peek(&self) -> char {
        self.peek_at(0)
    }

    fn advance(&mut self) -> char {
        let c = self.peek();
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1; // we reset the column counter every time we descend a line
        } else {
            self.col += 1;
        }
        c
    }

    // this is going to be the "entry point"

    pub fn tokenize(mut self) -> Result<Vec<Spanned<Token>>, LangError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();

            let line = self.line;
            let col = self.col;

            if self.is_at_end() {
                tokens.push(Spanned {
                    node: Token::Eof,
                    line,
                    col,
                });
                break;
            }
            let token = self.next_token()?;
            tokens.push(Spanned {
                node: token,
                line,
                col,
            });
        }
        Ok(tokens)
    }

    // I might edit this later because I'm planning to implement a formatter for the language (maybe)
    // but when the lexer discards comments... then formatting a file will remove all comments, which is bad
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                '/' if self.peek_at(1) == '/' => {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                }
                '/' if self.peek_at(1) == '*' => {
                    // block comments support nesting: /* /* like this */ */ is a single comment
                    // (C says no, rust says yes, we side with rust)
                    self.advance(); // consume '/'
                    self.advance(); // consume '*'
                    let mut depth = 1;
                    while depth > 0 && !self.is_at_end() {
                        if self.peek() == '/' && self.peek_at(1) == '*' {
                            self.advance();
                            self.advance();
                            depth += 1;
                        } else if self.peek() == '*' && self.peek_at(1) == '/' {
                            self.advance();
                            self.advance();
                            depth -= 1;
                        } else {
                            self.advance();
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, LangError> {
        let c = self.advance();

        match c {
            c if c.is_ascii_digit() => self.consume_number(c),
            c if c.is_ascii_alphabetic() || c == '_' => Ok(self.consume_identifier(c)),
            '"' => self.consume_string(),

            '+' => Ok(self.consume_maybe_eq('+', Token::PlusEq, Token::Plus)),
            '-' => Ok(self.consume_maybe_eq('-', Token::MinusEq, Token::Minus)),
            '*' => Ok(self.consume_maybe_eq('*', Token::StarEq, Token::Star)),
            '/' => Ok(self.consume_maybe_eq('/', Token::SlashEq, Token::Slash)),
            '^' => Ok(self.consume_maybe_eq('^', Token::CaretEq, Token::Caret)),
            '%' => Ok(self.consume_maybe_eq('%', Token::PercentEq, Token::Percent)),

            '=' => Ok(self.consume_maybe_eq('=', Token::EqEq, Token::Eq)),
            '!' => {
                if self.peek() == '=' {
                    self.advance();
                    Ok(Token::BangEq)
                } else {
                    Err(
                        LangError::new(self.line, self.col, "Unexpected character '!'.")
                            .with_suggestion("Did you mean '!='?"),
                    )
                }
            }

            '<' => Ok(self.consume_maybe_eq('<', Token::LtEq, Token::Lt)),
            '>' => Ok(self.consume_maybe_eq('>', Token::GtEq, Token::Gt)),

            '(' => Ok(Token::LParen),
            ')' => Ok(Token::RParen),
            '{' => Ok(Token::LBrace),
            '}' => Ok(Token::RBrace),
            '[' => Ok(Token::LBracket),
            ']' => Ok(Token::RBracket),
            ',' => Ok(Token::Comma),
            ':' => Ok(Token::Colon),
            ';' => Ok(Token::Semicolon),
            '.' => Ok(Token::Dot),

            other => Err(LangError::new(
                self.line,
                self.col,
                format!("unexpected character '{}'.", other),
            )),
        }
    }

    /// will continue consuming a number until it's done, reports an error if a number was malformed
    fn consume_number(&mut self, first: char) -> Result<Token, LangError> {
        // the -1 is because next_token() already advanced past the first digit before calling
        let start_col = self.col - 1;
        let mut text = String::from(first);

        while self.peek().is_ascii_digit() {
            text.push(self.advance());
        }

        // a '.' only starts a float if a digit follows it
        // this keeps `1.` as Int then Dot instead of a broken float
        // (which is what keeps method calls like `5.foo()` alive for future additions)
        if self.peek() == '.' && self.peek_at(1).is_ascii_digit() {
            text.push(self.advance());
            while self.peek().is_ascii_digit() {
                text.push(self.advance());
            }

            if self.peek() == '.' && self.peek_at(1).is_ascii_digit() {
                return Err(LangError::new(
                    self.line,
                    start_col,
                    format!(
                        "Malformed number literal '{}' (multiple decimal points).",
                        text
                    ),
                )
                .with_suggestion("Remove the extra decimal point."));
                // any extra decimal point beyond the first one produces a lexer-time error
            }
            return Ok(Token::Float(text.parse().map_err(|_| {
                LangError::new(
                    self.line,
                    start_col,
                    format!("Invalid float literal '{}'.", text),
                )
            })?));
        }

        Ok(Token::Int(text.parse().map_err(|_| {
            LangError::new(
                self.line,
                start_col,
                format!("Invalid int literal '{}'.", text),
            )
        })?))
    }

    fn consume_identifier(&mut self, first: char) -> Token {
        let mut text = String::from(first);

        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            text.push(self.advance());
        }

        match text.as_str() {
            "let" => Token::Let,
            "var" => Token::Var,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "continue" => Token::Continue,
            "break" => Token::Break,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "true" => Token::True,
            "false" => Token::False,
            "array" => Token::Array,
            "int" => Token::KwInt,
            "float" => Token::KwFloat,
            "str" => Token::KwStr,
            "bool" => Token::KwBool,
            _ => Token::Ident(text), // if the collected word is none of the reserved keywords,
                                     // it becomes an identifier
        }
    }

    /// consumes a string literal, handling the usual escape sequences (\n, \t, \r, \\, \", \0)
    /// anything else after a '\' is an error
    fn consume_string(&mut self) -> Result<Token, LangError> {
        let start_line = self.line;
        let start_col = self.col - 1;
        let mut text = String::new();

        while self.peek() != '"' && !self.is_at_end() {
            let c = self.advance();
            if c == '\\' {
                let next = self.advance();
                match next {
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    'r' => text.push('\r'),
                    '\\' => text.push('\\'),
                    '"' => text.push('"'),
                    '0' => text.push('\0'),
                    _ => {
                        return Err(LangError::new(
                            self.line,
                            self.col - 2,
                            format!("Invalid escape sequence '\\{}'.", next),
                        ));
                    }
                }
            } else {
                text.push(c);
            }
        }

        if self.is_at_end() {
            return Err(
                LangError::new(start_line, start_col, "Unterminated string literal.")
                    .with_suggestion("Add a closing '\"'."),
            );
        }
        self.advance(); // gotta consume the closing quote '"' before exiting the function
        Ok(Token::Str(text))
    }

    /// consumes an operator that may be followed by '=' (so '+' becomes '+=', '=' becomes '==', etc.)
    ///
    /// one special case lives in here: '-' followed by '>' is the arrow '->', not a minus
    /// (this is the only place that already has '-' special-cased, so the arrow gets handled here too)
    fn consume_maybe_eq(&mut self, expected_next: char, double: Token, single: Token) -> Token {
        if expected_next == '-' && self.peek() == '>' {
            self.advance();
            return Token::Arrow;
        }

        if self.peek() == '=' {
            self.advance();
            double
        } else {
            single
        }
    }
}
