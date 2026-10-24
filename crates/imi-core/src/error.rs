//! I'm implementing this crate now to replace all system panics with proper error propagation
//! Hopefully this will get rid of the ugly backtrace messages

use std::fmt;

/// this struct carries some useful data like the line and column in which the error occurred and includes an error message with an optional suggestion if necessary
#[derive(Debug, Clone, PartialEq)]
pub struct LangError {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub suggestion: Option<String>,
}

impl LangError {
    pub fn new(line: usize, col: usize, message: impl Into<String>) -> Self {
        LangError {
            line,
            col,
            message: message.into(),
            suggestion: None,
        }
    }

    /// this method is optional but provides a very comfortable api.
    /// such as `error("blah blah blah").with_suggestion("help: blah blah")`
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// rework: no ansi escape codes in here anymore.
/// the library has no idea whether its output ends up in a terminal, a file or a pipe,
/// so the responsability for coloring (and the source line + caret) carries over to report.rs, which does know
impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "error at line {}, column {}: {}",
            self.line, self.col, self.message
        )?;
        if let Some(s) = &self.suggestion {
            write!(f, "\n\thelp: {}", s)?;
        }
        Ok(())
    }
}

// so LangError composes with Box<dyn Error> and friends
impl std::error::Error for LangError {}
