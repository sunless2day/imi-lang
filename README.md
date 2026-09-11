<div align="center">
  <img src="imi-logo.png" alt="imi-lang logo" width="300">
  <h1>imi-lang</h1>
</div>

*this is part of yet another school project...*

imi is a very simple and minimal language following a procedural and imperative paradigm. no OOP, no closures, no such nonsense.

here is a little about imi as a language:

* statically typed but leans heavily on type inference, think of it like the `auto` keyword in C++ or Rust's native type inference
* arrays are homogeneous and can be nested, so `array[array[int]]` is completely fine
* both strings and arrays can be indexed with `[]`
* functions and variables live in separate namespaces, so a function and a variable can share a name without ever colliding
* is free-form (like C or Cmilar) *hehe, get it? C-milar*
* if you're used to Rust the syntax will feel familiar

everything you need to know about the language lives in the spec that's part of this repository. I recommend reading it if you want the details of every single thing. everything past below this is just a summary of what imi-lang contains and is capable of.

## Installation

you'll need cargo installed for this, which comes with the Rust toolchain.

easiest way, straight from the repo:

```sh
cargo install --git https://github.com/sunless2day/imi-lang
```

or clone it and install from your local copy instead:

```sh
git clone 
cd imi-lang
cargo install --path .
```

either way this gives you a binary called `imi`. if running `imi` afterward says command not found, cargo's bin folder probably isn't on your PATH yet, add this to your shell config (`.bashrc`, `.zshrc`, whatever you use):

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

## Running a program

write your program in a file ending in `.imi`, then hand it to the interpreter.

```sh
imi program.imi
```

`imi` only accepts exactly one argument and it has to end in `.imi`. no arguments, more than one, or the wrong extension, all of those are an error.

## Types

| Type       | Description                                                                                                                    |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `int`      | signed 64-bit integer                                                                                                          |
| `float`    | 64-bit floating point number, aka a double                                                                                     |
| `str`      | UTF-8 text, heap allocated. can be indexed with `[]` to pull out a single character                                            |
| `bool`     | just `true` or `false`, nothing fancier                                                                                        |
| `array[T]` | a heap allocated list of `T`. every element must share the same type, and arrays can be nested so `array[array[int]]` is valid |

## Built-in functions

These can't be overridden by user-declared functions (will produce a runtime error).

| Function   | Description                                                                                                                                                                                                                                                        |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `print`    | prints a formatted string to stdout, no newline included. one thing tho, whole `float`s print without their trailing `.0`, so `print("{}\n", 2.0)` prints `2` not `2.0`                                                                                            |
| `println`  | same as `print` but adds a newline at the end                                                                                                                                                                                                                      |
| `format`   | same formatting rules as `print`/`println`, but hands the result back as a `str` instead of printing it                                                                                                                                                            |
| `len`      | returns the length of a `str` or `array`, as an `int`. anything else, or the wrong number of arguments, is a runtime error                                                                                                                                         |
| `strslice` | grabs a "slice" of a string (the returned string is technically a new string now). takes the string, a start index and an end index, start inclusive and end exclusive, so `strslice("hello world", 0, 5)` returns `"hello"`                                       |
| `sleep`    | pauses the program for that many seconds, `int` or `float` both work, e.g. `sleep(2.5)` waits two and a half seconds. a negative duration is a runtime error                                                                                                       |
| `type`     | the type of a value as a `str`, arrays included, e.g. `type(true)` is `"bool"` and `type([[3, 5], [10, 67]])` is `"array[array[int]]"`                                                                                                                             |
| `elapsed`  | wall clock seconds since the program started, as a `float`. call it twice and subtract to time something                                                                                                                                                           |
| `input`    | prints an "optional" prompt (with optional I mean that the string can be empty, the function still needs a single argument of type `str`), then reads a line from stdin as a `str`. the trailing newline is stripped, everything else the user typed is kept as is |
| `parse`    | turns a `str` into the most specific type it looks like, `int` first, then `float`, then `bool`. never fails, if nothing matches it just hands back the original string. pair it with `type` to check what you got                                                 |
| `exit`     | stops the program right there. takes an optional `int` between 0 and 255 as the exit code, 0 if you don't give one                                                                                                                                                 |

ahead are some examples of what imi can do and how it is implemented

## Hello, world

imi has two functions for printing to the console: `print` and `println`.

```rust
println("Hello, world!");
```

`println` prints your string with a newline included. with `print` you have to add it yourself:

```rust
print("Hello, world!\n");
```

## Doing some simple math

imi has all the arithmetic operators you'd expect.

```rust
println("{}", 2 + 3);   // 5
println("{}", 10 - 4);  // 6
println("{}", 3 * 3);   // 9
println("{}", 7 % 2);   // 1
println("{}", 2 ^ 8);   // 256, ^ means exponentiation here, not xor
```

`int / int` truncates toward zero, same as Rust or C. to get a real quotient, at least one side has to be a `float`.

```rust
println("{}", 5 / 2);   // 2, truncated
println("{}", 5.0 / 2); // 2.5
```

`%` follows the sign of the dividend, not the divisor, again imitating Rust and C rather than Python.

```rust
println("{}", -7 % 3); // -1
println("{}", 7 % -3); // 1
```

`int` and `float` mix freely in arithmetic. a single `float` coerces the whole expression into that type.

```rust
println("{}", 5 + 2.5); // 7.5
```

Comparisons work the way you'd expect too.

```rust
println("{}", 5 > 3);          // true
println("{}", 5 == 5.0);       // true, int and float can be compared directly
println("{}", "cat" == "dog"); // false
```

For logic, imi spells things out instead of using symbols. there's no `&&`, `||` or `!`, just `and`, `or` and `not`.

```rust
let age = 20; // `int` is being infered from the initializer's type

if age >= 18 and age < 67 {
    println("you're an adult");
}

if not (age < 18) {
    println("same thing, just written differently");
}
```

Normal precedence rules apply too, so `2 + 3 * 4` is `14`, not `20`. use parentheses whenever you want to be explicit about it.

## A few simple programs

### Greeter

```rust
let name = input("Enter your name: ");
println("Hello, {}!", name);
```

Note: the first argument to `print`/`println`/`format` must be a string literal. Using a string variable, or any other kind of expression, is illegal on purpose. Keep that in mind.

### A very simple calculator

```rust
let num1 = parse(input("enter a number: "));
let num2 = parse(input("enter a number again: "));

// uses the type function to check if parse was able to do its job
if type(num1) != "int" or type(num2) != "int" {
    println("both inputs must be of type int!");
    exit(1); // exits with status code 1
}

println("{} + {} = {}", num1, num2, num1 + num2);
```

### FizzBuzz

It can even solve LeetCode problems.

```rust
// this is also how you declare a function in imi.
// yes, it looks identical to Rust for comfort
fn fizzbuzz(n: int) -> array[str] {
    var solution: array[str] = [];

    var i = 1;

    while i <= n {
        if i % 3 == 0 and i % 5 == 0 {
            solution.push("fizzbuzz");
        } else if i % 5 == 0 {
            solution.push("buzz");
        } else if i % 3 == 0 {
            solution.push("fizz");
        } else {
            solution.push(format("{}", i));
        }
        i += 1;
    }
    return solution;
}
```

### Nested arrays and strings

Arrays can hold other arrays, and strings can be indexed just like arrays.

```rust
var grid: array[array[int]] = [[1, 2, 3], [4, 5, 6]];
grid[1][2] = 42;
println("{}", grid[1][2]); // 42

let word = "hello";
println("{}", word[0]);            // "h"
println("{}", strslice(word, 1, 4)); // "ell"
```

## What imi can't do (yet)

Read or write files. Haven't implemented that yet, but probably will at some point.

Other than that I can't think of much it can't do. It's turing complete after all.

I wonder if anyone would ever implement imi-lang in imi-lang if something like `fread()` was ever added to the language.
