//! ts was painful, must admit
//! I will be skipping the documentation on some functions
//! because their name and behavior is pretty trivial

use crate::{
    ast::{AssignOp, BinaryOp, Expr, ExprNode, Literal, Param, Stmt, StmtNode, Type, UnaryOp},
    error::LangError,
    evaluator::BUILTINS,
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

    /// all methods declared below this one were needed to write this one.
    /// this function just calls the corresponding method for the right token.
    /// and the method called calls other methods recursively,
    /// this was pretty hard for me to follow through tbh
    ///
    /// `allow_fn` is only true at the top level (see `.parse()`),
    /// which is how function declarations get restricted to it
    fn parse_statement(&mut self, allow_fn: bool) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();

        match self.peek_token().clone() {
            Token::Let => {
                self.advance();
                self.parse_var_decl(false)
            }
            Token::Var => {
                self.advance();
                self.parse_var_decl(true)
            }
            Token::Fn => {
                self.advance();
                if !allow_fn {
                    return Err(LangError::new(
                        line,
                        col,
                        "Function declarations are only allowed at the top level scope.",
                    ));
                }
                self.parse_fn_decl()
            }
            Token::If => {
                self.advance();
                self.parse_if_body()
            }
            Token::While => {
                self.advance();
                self.parse_while()
            }
            Token::Break => {
                self.advance();
                if self.loop_depth == 0 {
                    return Err(LangError::new(line, col, "'break' used outside of a loop."));
                }
                self.expect_semicolon("Expected ';' after 'break'.")?;
                Ok(Spanned {
                    node: Stmt::Break,
                    line,
                    col,
                })
            }
            Token::Continue => {
                self.advance();
                if self.loop_depth == 0 {
                    return Err(LangError::new(
                        line,
                        col,
                        "'continue' used outside of a loop.",
                    ));
                }
                self.expect_semicolon("Expected ';' after 'continue'.")?;
                Ok(Spanned {
                    node: Stmt::Continue,
                    line,
                    col,
                })
            }
            Token::Return => {
                self.advance();
                // same trick as 'break'/'continue': catch it at parse time.
                // this used to be caught at runtime instead, oops
                if self.fn_depth == 0 {
                    return Err(LangError::new(
                        line,
                        col,
                        "'return' used outside of a function.",
                    ));
                }
                if self.check(&Token::Semicolon) {
                    self.advance();
                    Ok(Spanned {
                        node: Stmt::Return(None),
                        line,
                        col,
                    })
                } else {
                    let expr = self.parse_expression()?;
                    self.expect_semicolon("Expected ';' after return value.")?;
                    Ok(Spanned {
                        node: Stmt::Return(Some(expr)),
                        line,
                        col,
                    })
                }
            }
            Token::LBrace => {
                let block = self.parse_block()?;
                Ok(Spanned {
                    node: Stmt::Block(block),
                    line,
                    col,
                })
            }
            _ => self.parse_expr_or_assign_statement(),
        }
    }

    /// I believe this is very easy to follow
    fn parse_var_decl(&mut self, is_mutable: bool) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();
        let name = self.expect_ident()?;

        let ty = if self.check(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.check(&Token::Eq) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect_semicolon("Expected ';' after variable declaration.")?;

        if ty.is_none() && init.is_none() {
            return Err(LangError::new(
                line,
                col,
                format!(
                    "Variable '{}' needs either a type annotation or an initializer.",
                    name
                ),
            ));
        }

        let stmt = if is_mutable {
            Stmt::Var(name, ty, init)
        } else {
            Stmt::Let(name, ty, init)
        };
        Ok(Spanned {
            node: stmt,
            line,
            col,
        })
    }

    /// parses a function declaration (including its name, parameters, return type and body)
    /// if the declared function's name happens to match any of the in-built functions,
    /// then the parser raises an error
    fn parse_fn_decl(&mut self) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();
        let name = self.expect_ident()?;

        // the list lives in the evaluator now, so this can never drift out of sync
        if BUILTINS.contains(&name.as_str()) {
            return Err(self.current_span().error(format!(
                "'{}' is a reserved built-in function name and cannot be redeclared",
                name
            )));
        }

        self.expect(&Token::LParen, "Expected '(' after function name.")?;

        let mut params: Vec<Param> = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                let pname_line = self.current_line();
                let pname_col = self.current_col();
                let pname = self.expect_ident()?;
                // two params sharing a name would silently last-wins when declared
                if params.iter().any(|p| p.name == pname) {
                    return Err(LangError::new(
                        pname_line,
                        pname_col,
                        format!(
                            "Duplicate parameter name '{}' in function '{}'.",
                            pname, name
                        ),
                    ));
                }
                self.expect(&Token::Colon, "Expected ':' after parameter name.")?;
                let ptype = self.parse_type()?;
                params.push(Param {
                    name: pname,
                    ty: ptype,
                });
                if self.check(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&Token::RParen, "Expected ')' after parameters.")?;

        let return_type = if self.check(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        // functions are only ever parsed at the top level, so this goes 0 -> 1 -> 0,
        // someone else will eventually check if `self.fn_depth` is larger than 0
        // (hint: it's in `parse_statement`)
        self.fn_depth += 1;
        let body = self.parse_block();
        self.fn_depth -= 1;
        let body = body?;

        Ok(Spanned {
            node: Stmt::FnDecl(name, params, return_type, body),
            line,
            col,
        })
    }

    /// similar to parsing a while block tbh
    fn parse_if_body(&mut self) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();
        let condition = self.parse_expression()?;
        let then_block = self.parse_block()?;

        let else_block = if self.check(&Token::Else) {
            self.advance();
            if self.check(&Token::If) {
                self.advance();
                // `else if` is parsed by reusing parse_if_body and faking a
                // one-statement block, which is what keeps else-chains recursive
                Some(vec![self.parse_if_body()?])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        Ok(Spanned {
            node: Stmt::If(condition, then_block, else_block),
            line,
            col,
        })
    }

    /// parsing this is more or less the same as parsing a block of code,
    /// just gotta keep track of the loop depth for nested loops
    fn parse_while(&mut self) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();
        let condition = self.parse_expression()?;
        self.loop_depth += 1;
        // the dance here restores the depth even if the body fails to parse.
        // it doesn't strictly matter (a failed parse stops the interpreter anyway),
        // but it keeps the invariant honest
        let body = self.parse_block();
        self.loop_depth -= 1;
        let body = body?;
        Ok(Spanned {
            node: Stmt::While(condition, body),
            line,
            col,
        })
    }

    /// parses a block of code (which is defined by an opening and closing curly braces)
    /// it keeps parsing and collecting statements until it finds the closing curly brace
    /// then returns the vector of statements wrapped in a result type
    fn parse_block(&mut self) -> Result<Vec<StmtNode>, LangError> {
        self.expect(&Token::LBrace, "Expected '{'.")?;
        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_statement(false)?);
        }
        self.expect(&Token::RBrace, "Expected '}'.")?;
        Ok(stmts)
    }

    /// I don't know what to document here, the name of the function is literally what it does
    fn parse_expr_or_assign_statement(&mut self) -> Result<StmtNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();
        let expr = self.parse_expression()?;

        if let Some(op) = self.match_assign_op() {
            let value = self.parse_expression()?;
            self.expect_semicolon("Expected ';' after assignment.")?;

            let stmt = match expr.node {
                Expr::Variable(name) => Stmt::Assign(name, op, value),
                Expr::Index(target, index) => Stmt::IndexAssign(*target, *index, op, value),
                _ => return Err(LangError::new(line, col, "Invalid assignment target.")),
            };
            Ok(Spanned {
                node: stmt,
                line,
                col,
            })
        } else {
            self.expect_semicolon("Expected ';' after expression.")?;
            Ok(Spanned {
                node: Stmt::ExprStmt(expr),
                line,
                col,
            })
        }
    }

    /// checks if the next token is a single operator
    /// and converts it to its AST node variant
    fn match_assign_op(&mut self) -> Option<AssignOp> {
        let op = match self.peek_token() {
            Token::Eq => AssignOp::Assign,
            Token::PlusEq => AssignOp::AddEq,
            Token::MinusEq => AssignOp::SubEq,
            Token::StarEq => AssignOp::MulEq,
            Token::SlashEq => AssignOp::DivEq,
            Token::CaretEq => AssignOp::PowEq,
            Token::PercentEq => AssignOp::ModEq,
            _ => return None,
        };
        self.advance(); // like always, we consume the symbol before returning
        Some(op)
    }

    /// lol
    /// on a serious note, `.parse_or()` triggers a chain of recursive method calls
    /// I made sure they were declared in the right order, I suppose
    ///
    /// for the record, this is the full precedence tower, loosest to tightest:
    /// or -> and -> not -> comparison -> additive -> multiplicative -> unary -> power -> postfix -> primary
    ///
    /// two non-obvious decisions are baked into this order:
    /// - `not` binds looser than comparison, so `not x == y` is `not (x == y)` (python-style)
    /// - unary minus binds tighter than `*` but looser than `^`, so `-2^2` is `-(2^2)` (math-style)
    fn parse_expression(&mut self) -> Result<ExprNode, LangError> {
        self.parse_or()
    }

    /// every binary level below follows the same ritual: the new node inherits the
    /// LEFT operand's span, so an error on a big expression points at where it starts
    fn parse_or(&mut self) -> Result<ExprNode, LangError> {
        let mut left = self.parse_and()?;
        while self.check(&Token::Or) {
            let line = left.line;
            let col = left.col;
            self.advance();
            let right = self.parse_and()?;
            left = Spanned {
                node: Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right)),
                line,
                col,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<ExprNode, LangError> {
        let mut left = self.parse_not()?;
        while self.check(&Token::And) {
            let line = left.line;
            let col = left.col;
            self.advance();
            let right = self.parse_not()?;
            left = Spanned {
                node: Expr::Binary(Box::new(left), BinaryOp::And, Box::new(right)),
                line,
                col,
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<ExprNode, LangError> {
        if self.check(&Token::Not) {
            let line = self.current_line();
            let col = self.current_col();
            self.advance();
            let operand = self.parse_not()?;
            return Ok(Spanned {
                node: Expr::Unary(UnaryOp::Not, Box::new(operand)),
                line,
                col,
            });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<ExprNode, LangError> {
        let mut left = self.parse_additive()?;
        if let Some(op) = self.match_comparison_op() {
            let right = self.parse_additive()?;
            if self.peek_is_comparison_op() {
                return Err(self
                    .current_span()
                    .error("Comparison operators cannot be chained.")
                    .with_suggestion("Use 'and'/'or' instead."));
            }
            let line = left.line;
            let col = left.col;
            left = Spanned {
                node: Expr::Binary(Box::new(left), op, Box::new(right)),
                line,
                col,
            };
        }
        Ok(left)
    }

    fn match_comparison_op(&mut self) -> Option<BinaryOp> {
        let op = match self.peek_token() {
            Token::EqEq => BinaryOp::Eq,
            Token::BangEq => BinaryOp::NotEq,
            Token::Lt => BinaryOp::Lt,
            Token::Gt => BinaryOp::Gt,
            Token::LtEq => BinaryOp::LtEq,
            Token::GtEq => BinaryOp::GtEq,
            _ => return None,
        };
        self.advance();
        Some(op)
    }

    fn peek_is_comparison_op(&self) -> bool {
        matches!(
            self.peek_token(),
            Token::EqEq | Token::BangEq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq
        )
    }

    fn parse_additive(&mut self) -> Result<ExprNode, LangError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek_token() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            let line = left.line;
            let col = left.col;
            left = Spanned {
                node: Expr::Binary(Box::new(left), op, Box::new(right)),
                line,
                col,
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<ExprNode, LangError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_token() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let line = left.line;
            let col = left.col;
            left = Spanned {
                node: Expr::Binary(Box::new(left), op, Box::new(right)),
                line,
                col,
            };
        }
        Ok(left)
    }

    /// I'm starting to question if I should document these little methods...
    fn parse_unary(&mut self) -> Result<ExprNode, LangError> {
        if self.check(&Token::Minus) {
            let line = self.current_line();
            let col = self.current_col();
            self.advance();
            let operand = self.parse_power()?;
            return Ok(Spanned {
                node: Expr::Unary(UnaryOp::Neg, Box::new(operand)),
                line,
                col,
            });
        }
        self.parse_power()
    }

    /// yeah... pretty self explanatory
    ///
    /// the exponent is parsed at unary level (which recurses back down through power),
    /// which is exactly what makes `^` right-associative: `2^3^2` == `2^(3^2)`
    fn parse_power(&mut self) -> Result<ExprNode, LangError> {
        let left = self.parse_postfix()?;
        if self.check(&Token::Caret) {
            self.advance();
            let exponent = self.parse_unary()?;
            let line = left.line;
            let col = left.col;
            return Ok(Spanned {
                node: Expr::Binary(Box::new(left), BinaryOp::Pow, Box::new(exponent)),
                line,
                col,
            });
        }
        Ok(left)
    }

    /// parses postfixes after identifiers or literals such as x.push(y) or arr[i],
    /// where `.push` or `[]` was the postfix in question
    fn parse_postfix(&mut self) -> Result<ExprNode, LangError> {
        let mut expr = self.parse_primary()?;
        loop {
            let line = expr.line;
            let col = expr.col;
            if self.check(&Token::LBracket) {
                self.advance();
                let index = self.parse_expression()?;
                self.expect(&Token::RBracket, "Expected ']' after index expression.")?;
                expr = Spanned {
                    node: Expr::Index(Box::new(expr), Box::new(index)),
                    line,
                    col,
                };
            } else if self.check(&Token::Dot) {
                self.advance();
                let method = self.expect_ident()?;
                self.expect(&Token::LParen, "Expected '(' after method name.")?;
                let args = self.parse_expr_list(&Token::RParen)?;
                self.expect(&Token::RParen, "Expected ')' after method arguments.")?;
                expr = Spanned {
                    node: Expr::MethodCall(Box::new(expr), method, args),
                    line,
                    col,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// this method parses a single literals such as ints, floats strings, identifiers, etc...
    fn parse_primary(&mut self) -> Result<ExprNode, LangError> {
        let line = self.current_line();
        let col = self.current_col();

        let expr = match self.peek_token().clone() {
            Token::Int(n) => {
                self.advance();
                Expr::Literal(Literal::Int(n))
            }
            Token::Float(f) => {
                self.advance();
                Expr::Literal(Literal::Float(f))
            }
            Token::Str(s) => {
                self.advance();
                Expr::Literal(Literal::Str(s))
            }
            Token::True => {
                self.advance();
                Expr::Literal(Literal::Bool(true))
            }
            Token::False => {
                self.advance();
                Expr::Literal(Literal::Bool(false))
            }
            Token::Ident(name) => {
                self.advance();
                if self.check(&Token::LParen) {
                    self.advance();
                    let args = self.parse_expr_list(&Token::RParen)?;
                    self.expect(&Token::RParen, "Expected ')' after arguments.")?;

                    // if an identifier happens to be one of these functions
                    // it will ensure that the first parameter is a string literal at parse-time
                    // rather than runtime. I chose it this way so bad code is rejected before even running
                    // (the evaluator's formatting needs a template, and since imi can't statically type-check expressions,
                    // a literal is the only guarantee we get)
                    if matches!(name.as_str(), "print" | "println" | "format") {
                        match args.first() {
                            Some(a) if matches!(a.node, Expr::Literal(Literal::Str(_))) => {}
                            Some(a) => {
                                return Err(a.error(format!("'{}' requires a string literal as its first argument, not a variable.", name)));
                            }
                            None => {
                                return Err(LangError::new(
                                    self.current_line(),
                                    self.current_col(),
                                    format!(
                                        "'{}' expects a string literal as its first argument",
                                        name
                                    ),
                                ));
                            }
                        }
                    }
                    Expr::Call(name, args)
                } else {
                    Expr::Variable(name)
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&Token::RParen, "Expected ')' after expression.")?;
                return Ok(expr);
            }
            Token::LBracket => {
                self.advance();
                let elements = self.parse_expr_list(&Token::RBracket)?;
                self.expect(&Token::RBracket, "Expected ']' after array literal.")?;
                Expr::Literal(Literal::Array(elements))
            }

            other => {
                return Err(self
                    .current_span()
                    .error(format!("Expected an expression, found {:?}.", other)));
            }
        };

        Ok(Spanned {
            node: expr,
            line,
            col,
        })
    }

    /// a list of values is simply just a list of expressions, so we simply parse each individual expression within the list
    /// we check if the next token is the provided terminator (for example parens or brackets), if not,
    /// we keep parsing expressions until we no longer find commas, and then return the vector of values
    fn parse_expr_list(&mut self, terminator: &Token) -> Result<Vec<ExprNode>, LangError> {
        let mut items: Vec<ExprNode> = Vec::new();
        if !self.check(terminator) {
            items.push(self.parse_expression()?);
            while self.check(&Token::Comma) {
                self.advance();
                items.push(self.parse_expression()?);
            }
        }
        Ok(items)
    }

    /// parses tokens to the type they represent
    /// (both tokens and types are enums with different purposes)
    fn parse_type(&mut self) -> Result<Type, LangError> {
        match self.peek_token() {
            Token::KwInt => {
                self.advance();
                Ok(Type::Int)
            }
            Token::KwFloat => {
                self.advance();
                Ok(Type::Float)
            }
            Token::KwStr => {
                self.advance();
                Ok(Type::Str)
            }
            Token::KwBool => {
                self.advance();
                Ok(Type::Bool)
            }
            Token::Array => {
                self.advance();
                self.expect(&Token::LBracket, "Expected '[' after 'array'.")?;
                let inner = self.parse_type()?;
                self.expect(&Token::RBracket, "Expected ']' after array element type.")?;
                Ok(Type::Array(Box::new(inner)))
            }
            other => Err(LangError::new(
                self.current_line(),
                self.current_col(),
                format!("Expected a type, found {:?}.", other),
            )),
            // in case the function failed to parse the type, we simply raise a `LangError` explaining what caused it
        }
    }
}
