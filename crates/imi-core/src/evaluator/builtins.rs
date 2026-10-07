use super::{
    Interpreter,
    types::{type_name, value_type_name},
    value::{Value, as_f64, display_value},
};
use crate::{ast::ExprNode, error::LangError};
use std::{
    io::{self, Write},
    rc::Rc,
    thread,
    time::Duration,
};

/// every built-in function the evaluator implements.
/// the parser imports this to stop users from shadowing them,
/// so adding a builtin here automatically updates the parser's reserved list too
pub const BUILTINS: &[&str] = &[
    "print", "println", "format", "len", "strslice", "sleep", "type", "elapsed", "input", "parse",
    "exit", "fread", "fwrite",
];

impl Interpreter {
    /// built-ins live inside an isolated method instead of sharing eval_call with user functions.
    /// keeping this as one method seemed like a necessary refactor for this huge mess of a module
    pub(super) fn eval_builtin(
        &mut self,
        name: &str,
        arg_values: &[Value],
        call_site: &ExprNode,
    ) -> Result<Value, LangError> {
        // each arm does its own arity/type checking because imi has no static
        // function signatures to lean on at runtime.
        match name {
            "println" | "print" | "format" => {
                let s = self.do_format(arg_values, call_site)?;
                match name {
                    // println!/print! panic if stdout is closed (e.g. piping into `head`),
                    // so we write through io::stdout() and ignore the error instead,
                    // a closed stream is not worth crashing the whole interpreter over
                    "println" => {
                        let _ = writeln!(io::stdout(), "{}", s);
                    }
                    "print" => {
                        let _ = write!(io::stdout(), "{}", s);
                        io::stdout().flush().ok();
                    }
                    "format" => return Ok(Value::Str(Rc::new(s))),
                    _ => {
                        return Err(call_site.error(
                            "Internal evaluator error: invalid formatting builtin dispatch.",
                        ));
                    }
                }
                // functions like print and println have nothing relevant to return,
                // unlike format (that returns a formatted string).
                // we end up returning Void anyway to have something to return (see docs in Value)
                Ok(Value::Void)
            }
            "len" => match arg_values {
                [Value::Str(s)] => Ok(Value::Int(s.chars().count() as i64)),
                [Value::Array(items)] => Ok(Value::Int(items.len() as i64)),
                [v] => Err(call_site.error(format!(
                    "'len' expects a str or array, found {}.",
                    type_name(v)
                ))),
                _ => Err(call_site.error(format!(
                    "'len' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "strslice" => match arg_values {
                [Value::Str(s), Value::Int(start), Value::Int(end)] => {
                    let (start, end) = (*start, *end);
                    let chars: Vec<char> = s.chars().collect();
                    if start < 0 || end < start || end as usize > chars.len() {
                        return Err(call_site.error(format!(
                            "'strslice' range {}..{} out of bounds (str has {} characters).",
                            start,
                            end,
                            chars.len()
                        )));
                    }
                    // i'm pretty sure type casting an i64 to usize COULD cause trouble in very niche cases
                    // I trust the conditions above are enough to disallow any incoming bugs (fingers crossed)
                    Ok(Value::Str(Rc::new(
                        chars[start as usize..end as usize].iter().collect(),
                    )))
                }
                [Value::Str(_), a, b] => Err(call_site.error(format!(
                    "'strslice' expects (str, int, int), found ({}, {}).",
                    type_name(a),
                    type_name(b)
                ))),
                [v, _, _] => Err(call_site.error(format!(
                    "'strslice' expects a str as its first argument, found {}.",
                    type_name(v)
                ))),
                _ => Err(call_site.error(format!(
                    "'strslice' expects 3 arguments, got {}.",
                    arg_values.len()
                ))),
            },

            "sleep" => match arg_values {
                [v] => {
                    let secs = as_f64(v).map_err(|e| call_site.error(e))?;
                    // from_secs_f64 panics on negative, NaN and absurdly huge values,
                    // and I want every possible error (at the language-level) to be a LangError instead of Rust's runtime panics
                    if !(0.0..=Duration::MAX.as_secs_f64()).contains(&secs) {
                        return Err(
                            call_site.error(format!("'sleep' can't sleep for {} seconds.", secs))
                        );
                    }
                    thread::sleep(Duration::from_secs_f64(secs));
                    Ok(Value::Void)
                }
                _ => Err(call_site.error(format!(
                    "'sleep' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "type" => match arg_values {
                [v] => Ok(Value::Str(Rc::new(value_type_name(v)))),
                _ => Err(call_site.error(format!(
                    "'type' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "elapsed" => match arg_values {
                [] => Ok(Value::Float(self.start.elapsed().as_secs_f64())),
                _ => Err(call_site.error(format!(
                    "'elapsed' expects 0 arguments, got {}.",
                    arg_values.len()
                ))),
            },
            "input" => match arg_values {
                [Value::Str(prompt)] => {
                    // unlike print!, write! gives us the I/O error instead of panicking,
                    // so input() can keep the evaluator's "runtime failures become LangError" rule.
                    write!(io::stdout(), "{}", prompt)
                        .map_err(|e| call_site.error(format!("Failed to write prompt: {}", e)))?;
                    io::stdout()
                        .flush()
                        .map_err(|e| call_site.error(format!("Failed to flush stdout: {}", e)))?;

                    let mut input = String::new();
                    io::stdin()
                        .read_line(&mut input)
                        .map_err(|e| call_site.error(format!("Failed to read input: {}", e)))?;

                    // strips the trailing newline, and '\r' too so CRLF on windows doesn't sneak into the value
                    Ok(Value::Str(Rc::new(
                        input.trim_end_matches(['\n', '\r']).to_string(),
                    )))
                }
                [v] => {
                    Err(call_site.error(format!("'input' expects a 'str', got {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'input' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "exit" => {
                let code = match arg_values {
                    [] => 0,
                    [Value::Int(n)] => {
                        if *n < 0 || *n > 255 {
                            return Err(call_site.error(format!(
                                "'exit' expects a code between 0 and 255, got {}.",
                                n
                            )));
                        }
                        *n as i32
                    }
                    [v] => {
                        return Err(call_site
                            .error(format!("'exit' expects an 'int', got {}.", type_name(v))));
                    }
                    _ => {
                        return Err(call_site.error(format!(
                            "'exit' expects 0 or 1 arguments, got {}.",
                            arg_values.len()
                        )));
                    }
                };
                // yeah... this just kills the process. no LangError, no panic, just death.
                // I make an exception from the "every error should be a LangError" rule
                // for this function because it quite literally is a process killer
                io::stdout().flush().ok();
                std::process::exit(code);
            }
            "parse" => match arg_values {
                [Value::Str(s)] => {
                    // never fails by design:
                    // anything it can't parse comes back as the original string.
                    // whether that's forgiving or a footgun, it IS the spec
                    // (and yes, I wrote it)
                    if let Ok(i) = s.parse::<i64>() {
                        Ok(Value::Int(i))
                    } else if s.contains('.') {
                        if let Ok(f) = s.parse::<f64>() {
                            Ok(Value::Float(f))
                        } else {
                            Ok(Value::Str(s.clone()))
                        }
                    } else if s.as_str() == "true" {
                        Ok(Value::Bool(true))
                    } else if s.as_str() == "false" {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Str(s.clone()))
                    }
                }
                [v] => {
                    Err(call_site.error(format!("'parse' expects a 'str', got {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'parse' expects 1 argument, got {}",
                    arg_values.len()
                ))),
            },
            "fread" => match arg_values {
                // no special error handling beyond what std::fs::read_to_string offers us,
                // it already returns a message containing the reason of why the function failed
                // (file not found, permission denied, not valid UTF-8. etc...)
                [Value::Str(path)] => match std::fs::read_to_string(path.as_str()) {
                    Ok(contents) => Ok(Value::Str(Rc::new(contents))),
                    Err(e) => {
                        Err(call_site.error(format!("'fread' failed to read '{}': {}", path, e)))
                    }
                },
                [v] => {
                    Err(call_site.error(format!("'fread' expects a str, found {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'fread' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "fwrite" => match arg_values {
                // "o" overwrites the file (creating it if it's missing), "a" appends to it
                // (also creating it if it's missing). no default mode on purpose, silently
                // overwriting a file is precisely the thing that requires an explicit option
                // same error handling as fread
                [Value::Str(path), Value::Str(contents), Value::Str(mode)] => {
                    let result = match mode.as_str() {
                        "o" => std::fs::write(path.as_str(), contents.as_bytes()),
                        "a" => std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path.as_str())
                            .and_then(|mut f| f.write_all(contents.as_bytes())),
                        _ => {
                            return Err(call_site.error(format!(
                                "'fwrite' expects 'a' or 'o' as its mode, found '{}'.",
                                mode
                            )));
                        }
                    };
                    match result {
                        Ok(()) => Ok(Value::Void),
                        Err(e) => Err(call_site.error(format!(
                            "'fwrite' failed to write '{}' to '{}': {}",
                            contents, path, e
                        ))),
                    }
                }
                [Value::Str(_), Value::Str(_), v] => Err(call_site.error(format!(
                    "'fwrite' expects (str, str, str), found (str, str, {}).",
                    type_name(v)
                ))),
                [Value::Str(_), v, _] => Err(call_site.error(format!(
                    "'fwrite' expects (str, str, str), found (str, {}, ...).",
                    type_name(v)
                ))),
                [v, _, _] => Err(call_site.error(format!(
                    "'fwrite' expects a str as its first argument, got {}.",
                    type_name(v)
                ))),
                // damn, seems like I forgot to change the error message
                _ => Err(call_site.error(format!(
                    "'fwrite' expects 3 arguments, got {}.",
                    arg_values.len()
                ))),
            },
            _ => Err(call_site.error(format!(
                "Internal evaluator error: '{}' is not a registered builtin.",
                name
            ))),
        }
    }

    /// positional formatting: every `{}` in the template consumes the next
    /// argument in order. `{{` and `}}` are escapes for literal braces.
    /// note that placeholder COUNT is only checked here, at runtime
    /// the parser's "first argument must be a string literal" rule guarantees we
    /// have a template, but it can't count the {}s against the arguments
    fn do_format(&self, args: &[Value], call_site: &ExprNode) -> Result<String, LangError> {
        let Some(Value::Str(template)) = args.first() else {
            // safe (but fragile) since the parser ensures the first argument is a string
            unreachable!("bro, I swear you told me this was safe. damned be the parser");
        };

        let mut result = String::new();
        let mut arg_iter = args[1..].iter();
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next();
                result.push('{'); // '{{' is an escaped '{'
            } else if c == '{' && chars.peek() == Some(&'}') {
                chars.next();
                match arg_iter.next() {
                    Some(v) => result.push_str(&display_value(v).map_err(|e| call_site.error(e))?),
                    None => {
                        return Err(call_site.error(format!(
                            "Not enough arguments for format string '{}'.",
                            template
                        )));
                    }
                }
            } else if c == '}' && chars.peek() == Some(&'}') {
                chars.next();
                result.push('}'); // '}}' for symmetry
            } else {
                result.push(c);
            }
        }
        if arg_iter.next().is_some() {
            return Err(call_site.error(format!(
                "Too many arguments for format string '{}'.",
                template
            )));
        }
        Ok(result)
    }
}
