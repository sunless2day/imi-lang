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
                // imi now ignores shebang lines if they're at the top of a file
                // (I can't believe I forgot about this feature)
                '#' if self.pos == 0 && self.peek_at(1) == '!' => {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, LangError> {
        // stored BEFORE advancing, so errors point at the character itself, not the one after it
        let start_col = self.col;
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
                        LangError::new(self.line, start_col, "Unexpected character '!'.")
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
                start_col,
                format!("Unexpected character '{}'.", other),
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
    /// anything else after a '\' is an error, and a '\' at the very end of the file counts as an unterminated string
    fn consume_string(&mut self) -> Result<Token, LangError> {
        let start_line = self.line;
        let start_col = self.col - 1;
        let mut text = String::new();

        while self.peek() != '"' && !self.is_at_end() {
            // saved before advancing. `self.col - 2` would underflow when the escaped
            // character was a newline (since advance() resets col to 1 on '\n')
            let (esc_line, esc_col) = (self.line, self.col);
            let c = self.advance();
            if c == '\\' {
                // a '\' as the very last character of the file is an unterminated string,
                // not an escape sequence. breaking out lets the check below report it
                if self.is_at_end() {
                    break;
                }
                let next = self.advance();
                match next {
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    'r' => text.push('\r'),
                    '\\' => text.push('\\'),
                    '"' => text.push('"'),
                    '0' => text.push('\0'),
                    _ => {
                        // {:?} is on purpose so it shows invisible characters, like a newline, as '\n'
                        // instead of printing an actual line break in the middle of the error
                        return Err(LangError::new(
                            esc_line,
                            esc_col,
                            format!("Invalid escape sequence '\\' followed by {:?}.", next),
                        )
                        // a raw string makes the source easier to read
                        .with_suggestion(r#"Valid escapes are \n, \t, \r, \\, \" and \0."#));
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

#[cfg(test)]
mod tests {
    // just bring everything into scope, bro
    use super::*;

    /// runs the lexer and only keeps the tokens
    fn tokens(src: &str) -> Vec<Token> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.node)
            .collect()
    }
    /// same as above but this one preserves the lines and columns
    fn spans(src: &str) -> Vec<(Token, usize, usize)> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| (t.node, t.line, t.col))
            .collect()
    }

    /// is supposed to fail on valid input
    /// (relevant for testing how imi handles invalid byte sequences)
    fn lex_err(src: &str) -> LangError {
        Lexer::new(src).tokenize().unwrap_err()
    }

    /// simply asserts that the lexer is able to properly produce the right token for the right input
    #[test]
    fn single_tokens() {
        let cases = [
            // arithmetic operators
            ("+", Token::Plus),
            ("+=", Token::PlusEq),
            ("-", Token::Minus),
            ("-=", Token::MinusEq),
            ("*", Token::Star),
            ("*=", Token::StarEq),
            ("/", Token::Slash),
            ("/=", Token::SlashEq),
            ("^", Token::Caret),
            ("^=", Token::CaretEq),
            ("%", Token::Percent),
            ("%=", Token::PercentEq),
            // assignment operators
            ("=", Token::Eq),
            ("==", Token::EqEq),
            ("!=", Token::BangEq),
            ("<", Token::Lt),
            ("<=", Token::LtEq),
            (">", Token::Gt),
            (">=", Token::GtEq),
            // other ops
            ("->", Token::Arrow),
            ("(", Token::LParen),
            (")", Token::RParen),
            ("{", Token::LBrace),
            ("}", Token::RBrace),
            ("[", Token::LBracket),
            ("]", Token::RBracket),
            (",", Token::Comma),
            (":", Token::Colon),
            (";", Token::Semicolon),
            (".", Token::Dot),
            // keywords
            ("let", Token::Let),
            ("var", Token::Var),
            ("fn", Token::Fn),
            ("return", Token::Return),
            ("if", Token::If),
            ("else", Token::Else),
            ("while", Token::While),
            ("continue", Token::Continue),
            ("break", Token::Break),
            ("and", Token::And),
            ("or", Token::Or),
            ("not", Token::Not),
            ("true", Token::True),
            ("false", Token::False),
            // types
            ("array", Token::Array),
            ("int", Token::KwInt),
            ("float", Token::KwFloat),
            ("str", Token::KwStr),
            ("bool", Token::KwBool),
        ];
        for (src, expected) in cases {
            assert_eq!(tokens(src), vec![expected, Token::Eof], "lexing {src:?}");
        }
    }

    #[test]
    fn keywords_only_match_whole_words() {
        assert_eq!(
            tokens(
                "toilet let integer int floating float __while__ while .\nlet imi_is_cool: bool = true;"
            ),
            vec![
                Token::Ident("toilet".into()),
                Token::Let,
                Token::Ident("integer".into()),
                Token::KwInt,
                Token::Ident("floating".into()),
                Token::KwFloat,
                Token::Ident("__while__".into()),
                Token::While,
                Token::Dot,
                Token::Let,
                Token::Ident("imi_is_cool".into()),
                Token::Colon,
                Token::KwBool,
                Token::Eq,
                Token::True,
                Token::Semicolon,
                Token::Eof,
            ]
        )
    }

    #[test]
    fn numbers() {
        assert_eq!(tokens("67"), vec![Token::Int(67), Token::Eof]);
        assert_eq!(tokens("6.7"), vec![Token::Float(6.7), Token::Eof]);
        // might seem like it belongs in `bad_numbers` but a `dot` only initiates a `float` if there
        // are more numbers after it. otherwise, it registers as two separate tokens
        assert_eq!(tokens("6."), vec![Token::Int(6), Token::Dot, Token::Eof]);
    }

    #[test]
    fn bad_numbers() {
        assert!(lex_err("4.5.6").message.contains("multiple decimal points"));
        // note for self: don't try `i64::MAX + 1`, it overflows... badly
        assert!(
            lex_err("9223372036854775808")
                .message
                .contains("Invalid int literal")
        );
    }

    #[test]
    fn escapes() {
        assert_eq!(
            tokens(r#""a\nb\t\"\\\0""#),
            vec![Token::Str("a\nb\t\"\\\0".into()), Token::Eof]
        );
    }

    #[test]
    fn string_errors() {
        let unter_str = "Unterminated string literal.";
        assert!(lex_err(r#""abc"#).message.contains(unter_str));
        // these were one of the bugs I was hunting down
        assert!(lex_err(r#""abc\"#).message.contains(unter_str));
        let err = lex_err(r#""a\q""#);
        assert_eq!((err.line, err.col), (1, 3)) // should point at the offending escape sequence
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(tokens("// hi\n1"), vec![Token::Int(1), Token::Eof]);
        assert_eq!(
            tokens("/* a /* nested comment */ b */ 2"),
            vec![Token::Int(2), Token::Eof]
        );
    }

    /// self explanatory name, heh
    #[test]
    fn shebang_only_counts_on_the_first_line() {
        assert_eq!(
            tokens("#!/usr/bin/env imi\n\"hello\""),
            vec![Token::Str("hello".into()), Token::Eof]
        );
        assert!(
            lex_err("\"world\"\n#!oopsies")
                .message
                .contains("Unexpected character '#'")
        );
    }

    #[test]
    fn positions_across_lines() {
        assert_eq!(
            spans("a\n  b"),
            vec![
                (Token::Ident("a".into()), 1, 1),
                (Token::Ident("b".into()), 2, 3),
                (Token::Eof, 2, 4),
            ]
        );
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        assert_eq!(
            spans(r#""🚀" rust"#)[1],
            (Token::Ident("rust".into()), 1, 5)
        );
    }

    #[test]
    fn a_newline_inside_a_string_still_counts_as_a_line() {
        assert_eq!(spans("\"a\nb\" c")[1], (Token::Ident("c".into()), 2, 4));
    }

    /// only checks if the lexer crashes or not, that's all
    #[test]
    fn never_panics_on_short_inputs() {
        const ALPHABET: [char; 14] = [
            '"', '\\', '\n', '/', '*', '!', '1', '.', 'a', '#', ' ', '-', '>', '=',
        ];

        fn go(depth: usize, buf: &mut String) {
            let _ = Lexer::new(buf).tokenize(); // `Ok` or `Err` are both alright, panicking isn't tho
            if depth == 0 {
                return;
            } else {
                for c in ALPHABET {
                    buf.push(c);
                    go(depth - 1, buf);
                    buf.pop();
                }
            }
        }

        go(4, &mut String::new());
    }
}
