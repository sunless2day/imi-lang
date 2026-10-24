//! I doubt there is much to say about this module || SCREW THAT
//! new changes added. now main does more than just call the run function!

mod report;

use report::report;
use std::{env::args, fs, process::ExitCode, thread};

/// the interpreter is a tree-walker, so every nested call/block/expression requires real rust stack.
/// the main thread only gets around 8MB of stack memory, which may seem like enough for most use cases
/// but will crash the moment you try to execute a larger file or one that gobbles a lot of stack memory,
/// so... the fix? the interpreter runs on its own thread with a much bigger stack.
/// (this memory is only reserved, the OS doesn't actually hand it out until it's touched)
const INTERPRETER_STACK_SIZE: usize = 256 * 1024 * 1024;

/// I tried to keep the main function as minimal as possible.
/// it collects the CLI arguments and ensures there is only one (except the program itself).
/// if the provided argument is not a file with the .imi extension, then the program refuses to open the file.
/// if no arguments are provided, it also exits early.
/// if the given argument matches the requirements (one single .imi file),
/// then it will try to open the file and read its contents (exits if it fails to read the file),
/// if we get past everything without an error, then the source code (read from the file) will be
/// passed to the `imi_core::run()` function from the library crate, on a thread with a BIG stack
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
            eprintln!("usage: imi <file.imi>");
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

    // I initially thought of cloning the source so the interpreter thread would get its own copy,
    // but a scoped thread is guaranteed to finish before `thread::scope` returns,
    // so it can borrow `code` without the compiler bitching about it.
    // main still owns the source afterwards, to show the offending line if an error shows up
    let spawned = thread::scope(|s| {
        thread::Builder::new()
            .name("imi".into())
            .stack_size(INTERPRETER_STACK_SIZE)
            // safely borrowing the source:
            .spawn_scoped(s, || imi_core::run(&code))
            // keep in mind that I'm joining by hand, so if the thread panicked
            // we get to handle that error below instead of the scope re-panicking on its way out
            .map(|handle| handle.join())
    });

    let result = match spawned {
        Ok(Ok(result)) => result,
        // the thread panicked, I trust rust to have already printed the error message
        Ok(Err(_)) => return ExitCode::FAILURE,
        Err(e) => {
            eprintln!("failed to start the interpreter: {e}");
            return ExitCode::FAILURE;
        }
    };

    // if at any point the interpreter raises an error,
    // the main function prints it to stderr and exits the program
    if let Err(e) = result {
        report(&e, &code);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
