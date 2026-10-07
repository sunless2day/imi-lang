//! ts was painful, must admit
//! I will be skipping the documentation on some functions
//! because their name and behavior is pretty trivial

mod expressions;
mod statements;
mod types;

use crate::{
    ast::StmtNode,
    error::LangError,
    token::{Spanned, Token},
};

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
    /// how deep we are inside loops, so 'break'/'continue' can be rejected at parse time
    loop_depth: usize,
    /// how deep we are inside function bodies, same trick as loop_depth but for 'return'
    fn_depth: usize,
}

impl Parser {
    /// the lexer will return a vector of tokens, and that's precisely what the parser will parse
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self {
            tokens,
            pos: 0,
            loop_depth: 0,
            fn_depth: 0,
        }
    }

    // these are helpers that I fear don't need any further explanation
    fn peek_token(&self) -> &Token {
        &self.tokens[self.pos].node
    }

    fn current_span(&self) -> &Spanned<Token> {
        &self.tokens[self.pos]
    }

    fn current_line(&self) -> usize {
        self.current_span().line
    }

    fn current_col(&self) -> usize {
        self.current_span().col
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_token(), Token::Eof)
    }

    fn check(&self, token: &Token) -> bool {
        self.peek_token() == token
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += 1;
        }
    }

    /// this function is particularly useful
    /// we give it the token we are "expecting",
    /// and if our expectations don't match reality it simply errors
    fn expect(&mut self, token: &Token, msg: &str) -> Result<(), LangError> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            Err(self
                .current_span()
                .error(format!("{} (found {:?}).", msg, self.peek_token())))
        }
    }

    fn expect_ident(&mut self) -> Result<String, LangError> {
        match self.peek_token().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(self
                .current_span()
                .error(format!("Expected identifier, found {:?}.", other))),
        }
    }

    /// same idea as `expect()`, but specifically for the ';' that ends a statement,
    /// and it errors at a different spot on purpose.
    ///
    /// a missing ';' isn't really the fault of whatever token comes after it
    /// that token could be lines away (a '}' closing an enclosing block, for
    /// example), so blaming it there just confuses whoever reads the error (me).
    /// the actual fix always goes right after the last thing we managed to
    /// parse, so that's where this points instead: at the previously consumed token, not the current one
    fn expect_semicolon(&mut self, msg: &str) -> Result<(), LangError> {
        if self.check(&Token::Semicolon) {
            self.advance();
            Ok(())
        } else {
            // self.pos is never 0 here, we always parsed something before
            // calling this, so there's always a "previous" token to blame instead
            let prev = &self.tokens[self.pos - 1];
            Err(LangError::new(prev.line, prev.col, msg.to_string())
                .with_suggestion("Add a ';' here."))
        }
    }
    /// and just like the lexer, the parser returns a vector of statements that the evaluator will run
    pub fn parse(mut self) -> Result<Vec<StmtNode>, LangError> {
        let mut program = Vec::new();
        while !self.is_at_end() {
            program.push(self.parse_statement(true)?);
        }
        Ok(program)
    }
}
