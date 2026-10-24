use super::Parser;
use crate::{
    ast::{BinaryOp, Expr, ExprNode, Literal, UnaryOp},
    error::LangError,
    token::{Spanned, Token},
};

impl Parser {
    /// simply parses an expression, lol
    /// on a serious note, `.parse_or()` triggers a chain of recursive method calls
    /// I made sure they were declared in the right order, I suppose
    ///
    /// for the record, this is the full precedence tower, loosest to tightest:
    /// or -> and -> not -> comparison -> additive -> multiplicative -> unary -> power -> postfix -> primary
    ///
    /// two non-obvious decisions are baked into this order:
    /// - `not` binds looser than comparison, so `not x == y` is `not (x == y)` (python-style)
    /// - unary minus binds tighter than `*` but looser than `^`, so `-2^2` is `-(2^2)` (math-style)
    pub(super) fn parse_expression(&mut self) -> Result<ExprNode, LangError> {
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
    /// now I am! ... a month later
    /// stacked minus signs (`--x`, `- -x`) are rejected on purpose.
    /// math convention writes that as `-(-x)`, so the parentheses are required.
    /// (`2 - -3` is still fine, that's a binary minus followed by a single unary one)
    /// `-2^2` is still `-(2^2)`, because the operand is parsed by parse_power
    fn parse_unary(&mut self) -> Result<ExprNode, LangError> {
        if self.check(&Token::Minus) {
            let line = self.current_line();
            let col = self.current_col();
            self.advance();
            if self.check(&Token::Minus) {
                return Err(self
                    .current_span()
                    .error("Unary minus cannot be stacked.")
                    .with_suggestion("Use parentheses instead, like -(-x)."));
            }
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
                            // the template is a literal, so its braces can be validated right here,
                            // before the program runs (rust's println! does the same at compile time)
                            Some(a) => {
                                if let Expr::Literal(Literal::Str(template)) = &a.node {
                                    if let Err((msg, help)) = check_format_template(template) {
                                        return Err(a.error(msg).with_suggestion(help));
                                    }
                                } else {
                                    return Err(a.error(format!("'{}' requires a string literal as its first argument, not a variable.", name)));
                                }
                            }
                            None => {
                                return Err(LangError::new(
                                    line,
                                    col,
                                    format!(
                                        "'{}' expects a string literal as its first argument.",
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
                    .error(format!("Expected an expression, found {}.", other)));
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
}

/// validates the braces of a print/println/format template, following rust's rules.
/// `{}` is a placeholder, `{{` and `}}` are literal braces, and ANY other brace is an error.
/// so `println("{x}");` is rejected instead of actually printing "{x}"
/// (imi has no inline variables in templates, values always go in as arguments).
///
/// it only checks the shape of the template. the placeholder count is still
/// matched against the arguments at runtime, in the evaluator's do_format
fn check_format_template(template: &str) -> Result<(), (String, String)> {
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => match chars.peek() {
                // `{{` (escape) and `{}` (placeholder) are the only valid ways to open a brace
                Some('{') | Some('}') => {
                    chars.next();
                }
                _ => {
                    // collect whatever is between the braces, just to show it in the error
                    let mut inner = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => {
                                return Err((
                                    format!(
                                        "Invalid format string: '{{{}}}' is not a placeholder.",
                                        inner
                                    ),
                                    "Only '{}' is a placeholder. Pass values as arguments, like println(\"{}\", x).".to_string(),
                                ));
                            }
                            Some(ch) => inner.push(ch),
                            None => {
                                return Err((
                                    "Invalid format string: unmatched '{'.".to_string(),
                                    "Use '{{' to print a literal '{'.".to_string(),
                                ));
                            }
                        }
                    }
                }
            },
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                } else {
                    return Err((
                        "Invalid format string: unmatched '}'.".to_string(),
                        "Use '}}' to print a literal '}'.".to_string(),
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_format_template;

    #[test]
    fn valid_templates() {
        for t in ["", "{}", "{{}}", "a {} b {}", "{{x}}"] {
            assert!(check_format_template(t).is_ok(), "{t:?} should be valid");
        }
    }

    #[test]
    fn invalid_templates() {
        for t in ["{x}", "{", "}", "{ }", "a } b"] {
            assert!(check_format_template(t).is_err(), "{t:?} should be invalid");
        }
    }
}
