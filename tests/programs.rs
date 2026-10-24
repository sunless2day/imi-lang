//! imi-lang needed some testing. I already used to test it
//! but this time I'm pushing these tests to the public.
//!
//! there are other tests in the library crate,
//! here I'm just testing that the whole program works as intended.

use std::{fs, process::Command};

/// runs `tests/programs/<whatever>.imi`, returning success state, stdout, and stderr
fn run_imi(name: &str) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_imi"))
        .arg(format!("tests/programs/{name}.imi"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    (output.status.success(), stdout, stderr)
}

/// the program must succeed and print exactly what `<whatever>.out` contains
fn expect_output(name: &str) {
    let (success, stdout, stderr) = run_imi(name);
    let expected = fs::read_to_string(format!("tests/expected_output/{name}.out")).unwrap();
    assert!(success, "{name}: imi failed:\n{stderr}");
    assert_eq!(stdout, expected, "{name}: wrong output");
}

// a few tests lie below. just testing out imi-lang's capabilities and sturdiness

#[test]
fn hello() {
    expect_output("hello");
}

#[test]
fn arithmetic() {
    expect_output("arithmetic");
}

#[test]
fn reading_file() {
    expect_output("io_stuff");
}

#[test]
fn loopies() {
    expect_output("loopy");
}

#[test]
fn first_whatever_prime_numbers() {
    expect_output("primes");
}
