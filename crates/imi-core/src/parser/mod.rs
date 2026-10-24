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
                // the messages passed in end with a '.', which would otherwise read "...expression. (found ';')."
                .error(format!(
                    "{} (found {}).",
                    msg.trim_end_matches('.'),
                    self.peek_token()
                )))
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
                .error(format!("Expected identifier, found {}.", other))),
        }
    }

    /// same idea as `expect()`, but specifically for the ';' that ends a statement,
    /// and it errors at a different spot on purpose.
    ///
    /// a missing ';' isn't really the fault of whatever token comes after it
    /// that token could be lines away (a '}' closing an enclosing block, for
    /// example), so blaming it there just confuses whoever reads the error (me).
    /// the actual fix always goes right after the last thing we managed to parse,
    /// so that's where this points instead. at the previously consumed token, not the current one.
    /// (spans only store where a token STARTS, hence "after this" in the suggestion
    /// rather than pretending the caret sits exactly where the ';' goes)
    fn expect_semicolon(&mut self, msg: &str) -> Result<(), LangError> {
        if self.check(&Token::Semicolon) {
            self.advance();
            Ok(())
        } else {
            // self.pos is never 0 here, we always parsed something before
            // calling this, so there's always a "previous" token to blame instead
            let prev = &self.tokens[self.pos - 1];
            Err(LangError::new(prev.line, prev.col, msg.to_string())
                .with_suggestion("Add a ';' after this."))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{Expr, ExprNode, Literal, Stmt, UnaryOp},
        lexer::Lexer,
    };

    fn parse(src: &str) -> Result<Vec<StmtNode>, LangError> {
        Parser::new(Lexer::new(src).tokenize()?).parse()
    }

    fn parse_err(src: &str) -> String {
        parse(src).unwrap_err().message
    }

    fn symexpr(expr: &ExprNode) -> String {
        match &expr.node {
            Expr::Literal(Literal::Int(n)) => n.to_string(),
            Expr::Literal(Literal::Float(f)) => f.to_string(),
            Expr::Literal(Literal::Str(s)) => format!("{s:?}"),
            Expr::Literal(Literal::Bool(b)) => b.to_string(),
            Expr::Literal(Literal::Array(items)) => format!("(array{})", list(items)),
            Expr::Variable(name) => name.clone(),
            Expr::Unary(UnaryOp::Neg, x) => format!("(- {})", symexpr(x)),
            Expr::Unary(UnaryOp::Not, x) => format!("(not {})", symexpr(x)),
            Expr::Binary(l, op, r) => format!("({} {op} {})", symexpr(l), symexpr(r)),
            Expr::Index(target, index) => format!("({} index {})", symexpr(target), symexpr(index)),
            Expr::Call(name, args) => format!("(call {name}{})", list(args)),
            Expr::MethodCall(target, name, args) => {
                format!("(method {name} {}{})", symexpr(target), list(args))
            }
        }
    }

    fn list(items: &[ExprNode]) -> String {
        items.iter().map(|e| format!(" {}", symexpr(e))).collect()
    }

    fn expr(src: &str) -> String {
        let program = parse(&format!("let x = {src};")).unwrap();
        let Stmt::Let(_, _, Some(e)) = &program[0].node else {
            panic!("should've been a let statement with an initializer")
        };
        symexpr(e)
    }

    #[test]
    fn arithmetic_precedence() {
        // normal arithmetic operators
        assert_eq!(expr("1 + 2 * 3"), "(1 + (2 * 3))");
        assert_eq!(expr("(1 + 2) * 3"), "((1 + 2) * 3)");
        assert_eq!(expr("1 - 2 - 3"), "((1 - 2) - 3)");
        assert_eq!(expr("8 / 4 % 3"), "((8 / 4) % 3)");
        // powers (right associative btw)
        assert_eq!(expr("5 ^ 4 + 3 * 2"), "((5 ^ 4) + (3 * 2))");
        assert_eq!(expr("2 ^ 2 ^ 2"), "(2 ^ (2 ^ 2))");
        assert_eq!(expr("2 ^ -1"), "(2 ^ (- 1))");
    }

    #[test]
    fn logical_precedence() {
        assert_eq!(expr("a or b and c"), "(a or (b and c))");
        assert_eq!(expr("not a == b"), "(not (a == b))");
        assert_eq!(expr("a < b and c"), "((a < b) and c)");
    }

    #[test]
    fn postfix_chains() {
        assert_eq!(expr("m[1][2]"), "((m index 1) index 2)");
        assert_eq!(expr("a[0].pop()"), "(method pop (a index 0))");
        assert_eq!(expr("len([1, 2])"), "(call len (array 1 2))");
        assert_eq!(expr("f()"), "(call f)");
        assert_eq!(expr("-a[0]"), "(- (a index 0))");
    }

    #[test]
    fn valid_statements_parse() {
        let programs = [
            "let x: int;",
            "var a: array[array[int]] = [];",
            "x += 1;",
            "a[0] = 5;",
            "while true { break; }",
            "fn f(a: int, b: str) -> int { return a; }",
            r#"println("{} {}", 1, 2);"#,
        ];

        for src in programs {
            assert!(parse(src).is_ok(), "should parse: {src}")
        }
    }

    #[test]
    fn else_if_nests_inside_the_else_block() {
        let program = parse("if a { } else if b { }").unwrap();
        let Stmt::If(_, _, Some(else_block)) = &program[0].node else {
            panic!("expected an if with an else");
        };
        assert_eq!(else_block.len(), 1);
        assert!(matches!(else_block[0].node, Stmt::If(..)))
    }

    #[test]
    fn control_flow_outside_its_place() {
        assert!(parse_err("break;").contains("outside of a loop"));
        assert!(parse_err("continue;").contains("outside of a loop"));
        assert!(parse_err("return;").contains("outside of a function"));
    }

    #[test]
    fn function_rules() {
        assert!(parse_err("{ fn () {} }").contains("top level"));
        assert!(parse_err("fn f(a: int, a: int) {}").contains("Duplicate parameter"));
        assert!(parse_err("fn len {}").contains("reserved built-in"));
    }

    #[test]
    fn declaration_needs_a_type_or_a_value() {
        assert!(parse_err("let x;").contains("needs either a type annotation or an initializer"));
    }

    #[test]
    fn invalid_assignment_targets() {
        assert!(parse_err("5 = 3;").contains("Invalid assignment target"));
        assert!(parse_err("f() = 1;").contains("Invalid assignment target"));
    }

    #[test]
    fn operator_rules() {
        assert!(parse_err("let x = 1 < 2 < 3;").contains("cannot be chained"));
        assert!(parse_err("let x = --5;").contains("cannot be stacked"));
        assert!(parse("let x = -(-5);").is_ok());
    }

    #[test]
    fn print_rules() {
        assert!(parse_err(r#"let s = "x"; println(s);"#).contains("string literal"));
        assert!(parse_err(r#"println("{x}");"#).contains("is not a placeholder"));
        assert!(parse_err(r#"println("{");"#).contains("unmatched '{'"));
    }

    #[test]
    fn missing_semicolon_points_at_the_previous_token() {
        let err = parse("let x = 1\nlet y = 2;").unwrap_err();
        assert_eq!((err.line, err.col), (1, 9));
        assert_eq!(err.suggestion.as_deref(), Some("Add a ';' after this."));
    }

    #[test]
    fn never_panics_on_short_programs() {
        const PIECES: [&str; 18] = [
            "(",
            ")",
            "-",
            "1",
            "x",
            "=",
            ";",
            "{",
            "}",
            "[",
            "]",
            ".",
            "let",
            "var",
            "if",
            "fn",
            "return",
            r#"println("{}")"#,
        ];

        fn go(depth: usize, buf: &mut Vec<&str>) {
            if let Ok(tokens) = Lexer::new(&buf.join(" ")).tokenize() {
                let _ = Parser::new(tokens).parse();
            }
            if depth == 0 {
                return;
            }
            for piece in PIECES {
                buf.push(piece);
                go(depth - 1, buf);
                buf.pop();
            }
        }

        go(4, &mut Vec::new());
    }
}
