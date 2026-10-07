//! I doubt there is much to say about this module
//! it just brings in all the modules and orchestrates the flow of the interpreter

use std::{env::args, fs, process::ExitCode};

/// I tried to keep the main function as minimal as possible.
/// it collects the CLI arguments and ensures there is only one (except the program itself).
/// if the provided argument is not a file with the .imi extension, then the program refuses to open the file.
/// if no arguments are provided, it also exits early.
/// if the given argument matches the requirements (one single .imi file),
/// then it will try to open the file and read its contents (exits if it fails to read the file),
/// if we get past everything without an error, then the source code (read from the file) will be
/// passed to the `imi_core::run()` function from the library crate
fn main() -> ExitCode {
    let args: Vec<String> = args().skip(1).collect();

    if args.len() > 1 {
        eprintln!("imi only supports 1 argument.");
        return ExitCode::FAILURE;
    }

    let path = match args.first() {
        Some(p) if p.ends_with(".imi") => p,
        Some(_) => {
            eprintln!("imi only supports .imi files");
            return ExitCode::FAILURE;
        }
        None => {
            eprintln!("usage: imi <file>");
            return ExitCode::FAILURE;
        }
    };

    let code = match fs::read_to_string(path) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // if at any point the interpreter raises an error,
    // the main function prints it to stderr and exits the program
    if let Err(e) = imi_core::run(&code) {
        eprintln!("\n{}", e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
