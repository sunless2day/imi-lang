mod builtins;
mod expressions;
mod functions;
mod indexing;
mod statements;
mod types;
mod value;

pub use builtins::BUILTINS;
pub use types::{coerce_value_to_type, type_name, value_matches_type};
pub use value::Value;

use crate::ast::Param;
use crate::ast::Stmt;
use crate::ast::StmtNode;
use crate::ast::Type;
use crate::environment::Environment;
use crate::error::LangError;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

/// how deep user recursion is allowed to go.
/// conservative on purpose: each call in imi costs a surprising amount of rust stack
/// (eval() itself recurses for every subexpression), and past this limit I'd rather
/// report a clean error than let the process die with a native stack overflow
const MAX_CALL_DEPTH: usize = 512;

/// how statements report control flow to whoever called them.
/// instead of unwinding the rust stack for break/continue/return, every statement
/// returns one of these as a plain value, and exec_stmt/exec_block propagate them
/// upward until the loop or function that owns them consumes them.
/// Return(None) is a bare `return;`, which becomes Value::Void at the call site
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// functions will also go through this `Rc` optimization.
/// an entry is simply the function's parameters, optional return type and body.
/// using a struct here instead of a tuple makes the call path a little easier to read
/// without changing what a function actually stores.
struct FnEntry {
    params: Vec<Param>,
    ret_type: Option<Type>,
    body: Vec<StmtNode>,
}

/// functions belong to a different namespace than variables
/// this is particularly relevant since things like:
/// ```text
/// fn foo() { return; }
///
/// let foo: int;
/// ```
/// are completely fine because the function `foo()` and variable `foo`
/// are in different namespaces and don't collide with each other
///
/// as you can see the hashmap simply consists of a `String` acting as the key
/// and the data mapped to that key are the parameters, return type and statements
/// of that function
///
/// now, we can wrap `FnEntry` in `Rc` so that every function call is a refcount bump instead of deep cloning the AST body.
/// should be easy to implement since function's are read-only after being written to for the first time
type FnTable = HashMap<String, Rc<FnEntry>>;

/// the interpreter state: the function table, the program timer, and the recursion depth
pub struct Interpreter {
    functions: FnTable,
    /// `Instant` is necessary for the function `elapsed()` to be supported
    start: Instant,
    /// current recursion depth, so runaway recursion dies as a LangError
    /// instead of a native stack overflow (which bypasses ALL of our error machinery)
    call_depth: usize,
}

impl Interpreter {
    /// in here we initialize the function table and start the timer
    /// we will call `self.start.elapsed()` whenever the interpreter
    /// encounters a function call to the in-built function `elapsed()`
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            start: Instant::now(),
            call_depth: 0,
        }
    }

    // this interpreter is multi-pass, and this is what each pass does:
    pub fn interpret(&mut self, program: &[StmtNode]) -> Result<(), LangError> {
        // pass 1: register every function declaration BEFORE running anything.
        // this is hoisting. it's what lets a function defined at the bottom of the
        // file be called from the top. the FnTable living on the interpreter (not
        // inside the environment) is also what makes recursion work for free
        for stmt in program {
            if let Stmt::FnDecl(name, params, ret, body) = &stmt.node {
                // same spirit as the parser's duplicate-parameter check: two
                // functions sharing a name would otherwise silently last-wins,
                // with the earlier declaration vanishing with no error at all
                if self.functions.contains_key(name) {
                    return Err(stmt.error(format!("Function '{}' is already declared.", name)));
                }
                self.functions.insert(
                    name.clone(),
                    Rc::new(FnEntry {
                        params: params.clone(),
                        ret_type: ret.clone(),
                        body: body.clone(),
                    }),
                );
            }
        }

        let mut env = Environment::new();

        // pass 2: run everything that isn't a function declaration.
        for stmt in program {
            if matches!(stmt.node, Stmt::FnDecl(..)) {
                continue;
            }
            self.exec_stmt(stmt, &mut env)?;
        }
        Ok(())
    }
}
