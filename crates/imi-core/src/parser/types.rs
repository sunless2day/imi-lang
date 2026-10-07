use super::Parser;
use crate::{ast::Type, error::LangError, token::Token};

impl Parser {
    /// parses tokens to the type they represent
    /// (both tokens and types are enums with different purposes)
    pub(super) fn parse_type(&mut self) -> Result<Type, LangError> {
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
