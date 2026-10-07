use super::Parser;
use crate::{
    ast::{AssignOp, Expr, Param, Stmt, StmtNode},
    error::LangError,
    evaluator::BUILTINS,
    token::{Spanned, Token},
};

impl Parser {
    /// all methods declared below this one were needed to write this one.
    /// this function just calls the corresponding method for the right token.
    /// and the method called calls other methods recursively,
    /// this was pretty hard for me to follow through tbh
    ///
    /// `allow_fn` is only true at the top level (see `.parse()`),
    /// which is how function declarations get restricted to it
    pub(super) fn parse_statement(&mut self, allow_fn: bool) -> Result<StmtNode, LangError> {
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
}
