//! everything about showing an error to the user lives here... for now
//! main uses this module when something fails, and this module decides what the error looks like

use imi_core::LangError;
use std::io::{IsTerminal, stderr};

/// prints an error like this:
/// ```text
/// error at line x, column y: Division by zero is not allowed.
/// x |     return a / 0;
///   |                ^
///         help: ...
/// ```
/// the hard-coded ANSI escape codes (red, green, dim) are only used when stderr is an actual terminal,
/// so `imi file.imi 2> log.txt` or piping into another program provides ANSI-escape-code-free text.
/// it also respects the NO_COLOR convention (https://no-color.org). set it and your output won't be colored
pub fn report(e: &LangError, code: &str) {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let color = stderr().is_terminal() && !no_color;
    let (red, green, dim, reset) = if color {
        ("\x1b[1;31m", "\x1b[3;32m", "\x1b[2m", "\x1b[0m")
    } else {
        ("", "", "", "")
    };

    eprintln!(
        "\n{red}error at line {}, column {}:{reset} {}",
        e.line, e.col, e.message
    ); // I'd love to carry over the reb and geen spelling mistakes but whatever
    // (here is the gif if you wanna see it: https://tenor.com/view/roblox-profound-mental-intelligence-profound-mental-capability-intelligence-capability-gif-6959631427918311453)

    // I don't like repeating myself, but again: lines and columns are 1-based
    if let Some(src) = code.lines().nth(e.line.saturating_sub(1)) {
        // the caret is pushed right by copying the line's own characters as whitespace,
        // keeping tabs as tabs, so it lines up no matter how wide a tab is rendered
        let pad: String = src
            .chars()
            .take(e.col.saturating_sub(1))
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();
        let gutter = e.line.to_string();
        eprintln!("{dim}{gutter} |{reset} {src}");
        eprintln!(
            "{dim}{} |{reset} {pad}{red}^{reset}",
            " ".repeat(gutter.len())
        );
    }

    if let Some(s) = &e.suggestion {
        eprintln!("\t{green}help:{reset} {s}");
    }
}
