<div align="center">
  <img src="imi-logo.png" alt="imi-lang logo" width="300">
  <h1>imi-lang</h1>
</div>

*this is part of yet another school project...*

imi is a very simple and minimal language following a procedural and imperative paradigm.

here is a little about imi as a language:

* statically typed but leans heavily on type inference, think of it like the `auto` keyword in C++ or Rust's native type inference
* arrays are homogeneous and can be nested, so `array[array[int]]` is completely fine
* both strings and arrays can be indexed with `[]`
* functions and variables live in separate namespaces, so a function and a variable can share a name without ever colliding
* if you're used to Rust the syntax will feel familiar

## Installation

you'll need cargo installed for this, which comes with the Rust toolchain.

easiest way, straight from the repo:

```sh
cargo install --git https://github.com/sunless2day/imi-lang
```

or clone it and install from your local copy instead:

```sh
git clone https://github.com/sunless2day/imi-lang
cd imi-lang
cargo install --path .
```

either way this gives you a binary called `imi`. if running `imi` afterward says command not found, cargo's bin folder probably isn't on your PATH yet, add this to your shell config (`.bashrc`, `.zshrc`, whatever you use):

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

***

everything you need to know about the language lives in the spec that's part of this repository. I recommend reading it if you want the details of every single thing. everything past below this is just a summary of what imi-lang contains and is capable of.

## Running a program

write your program in a file ending in `.imi`, then hand it to the interpreter.

```sh
imi program.imi
```

`imi` only accepts exactly one argument and it has to end in `.imi`. no arguments, more than one, or the wrong extension, all of those are an error.

## Types

| Type       | What it does                                                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `int`      | signed 64-bit integer                                                                                                          |
| `float`    | 64-bit floating point number, aka a double                                                                                     |
| `str`      | UTF-8 text, heap allocated. can be indexed with `[]` to pull out a single character                                            |
| `bool`     | just `true` or `false`, nothing fancier                                                                                        |
| `array[T]` | a heap allocated list of `T`. every element must share the same type, and arrays can be nested so `array[array[int]]` is valid |

## Variables

`let` declares a variable that can't be reassigned. `var` declares one that can.

```rust
let x = 5;
x = 6; // ERROR, x is immutable

var y = 5;
y = 6; // fine
```

the type can be explicit or left for imi to infer from whatever you initialize it with.

```rust
let a: int = 5; // explicit
let b = 5;       // inferred as int, same thing
```

### Mutability belongs to the binding, not the data

whether something can be changed is a property of the variable holding it, not the value itself. copy a value from a `let` into a `var` and it becomes fully mutable, copy it the other way and it becomes fully frozen. nothing about the value itself remembers where it came from.

```rust
let x = 10;
var y = x;
y = 15; // fine, y is var, doesn't matter that x was let

var m = 10;
let n = m;
n = 15; // ERROR, n is let, doesn't matter that m was var
```

this applies to arrays too, all the way down. a `var` array of arrays is mutable at every level, a `let` one is frozen at every level, and copying one into the other flips that entirely.

### Copying is always by value

assigning a variable to another, or passing it into a function, always makes a full independent copy. this includes arrays and strings, there's no shared reference sitting underneath like there would be in Python or JavaScript.

```rust
let a: array[int] = [1, 2, 3];
var b = a;
b.push(4);
println("{}", len(a)); // 3, a is untouched
println("{}", len(b)); // 4
```

in imi, two variables never point at the same data, ever.

### Scopes and shadowing

every `{ }` block introduces its own scope. variables declared inside a block are local to that block and can shadow variables with the same name from an outer scope.

a variable can also be redeclared in the same scope using `let` or `var`. the new binding replaces the previous one.

```rust
var a = 10;

{
    let b = a * 2;
    println("{}", b); // prints 20

    var b: str = "imi"; // redeclaring b in the same scope is fine
    println("{}", b); // prints imi
} // b goes out of scope here, a remains accessible

{
    a += 15; // a is mutated
}

println("{}", a); // prints 25

{
    let a = 50; // shadows the a from the outer scope
    println("{}", a); // prints 50
} // the shadowing declaration goes out of scope here

println("{}", a); // a is accessible again, prints 25
```

## Built-in functions

these can't be overridden by user-declared functions (will produce a runtime error).

| Function   | What it does                                                                                                                                                                                                                                                       |
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

the first program anyone writes in any language, and imi is no exception.

```rust
println("Hello, world!");
```

`println` appends a newline for you. `print` doesn't, so you'd add it yourself:

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

comparisons work the way you'd expect too.

```rust
println("{}", 5 > 3);          // true
println("{}", 5 == 5.0);       // true, int and float can be compared directly
println("{}", "cat" == "dog"); // false
```

for logic, imi spells things out instead of using symbols. there's no `&&`, `||` or `!`, just `and`, `or` and `not`.

```rust
let age = 20; // `int` is being inferred from the initializer's type

if age >= 18 and age < 67 {
    println("you're an adult");
}

if not (age < 18) {
    println("same thing, just written differently");
}
```

normal precedence rules apply too, so `2 + 3 * 4` is `14`, not `20`. use parentheses whenever you want to be explicit about it.

## Control Flow

`if`/`else` and `while` work about how you'd expect.

```rust
let n = 7;

if n % 2 == 0 {
    println("even");
} else {
    println("odd");
}
```

```rust
var i = 0;
while i < 5 {
    println("{}", i);
    i += 1;
}
```

`break` exits a loop, `continue` skips straight to the next iteration.

one thing worth knowing, the condition in an `if` or `while` has to be a `bool`, no exceptions. imi has no concept of "truthiness" like Python or C have, so an `int` such as `0` or `1` can't be used as a condition directly.

```rust
if 0 { }      // will error at runtime, 0 is not a bool
if n != 0 { } // this is fine
```

## Functions
 
functions can be called and declared anywhere in the file, order doesn't matter. declarations only work at the top level though, not nested inside another function or block. imi processes the whole file in two passes, first registering every function declaration, then running the actual program, so a function can be called before its own declaration even shows up further down.
 
```rust
println("{}", double(5)); // works fine even though double isn't declared yet
 
fn double(n: int) -> int {
    return n * 2;
}
```
 
declaring two functions with the same name is an error, even if they'd never both actually get called.
 
```rust
fn greet() {
    println("hi");
}
 
fn greet() { // ERROR, greet is already declared
    println("hey");
}
```
 
functions and variables live in completely separate namespaces, so a function and a variable can share the same name without any conflict at all.
 
```rust
fn foo() {
    println("i'm a function");
}
 
let foo = 5; // fine, this is a totally different foo
 
foo();               // calls the function, prints "i'm a function"
println("{}", foo);  // reads the variable, prints 5
```

## A few simple programs

### Greeter

```rust
let name = input("Enter your name: ");
println("Hello, {}!", name);
```

note: the first argument to `print`/`println`/`format` must be a string literal. using a string variable, or any other kind of expression, is illegal on purpose. keep that in mind.

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

it can even solve leetcode problems.

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

arrays can hold other arrays, and strings can be indexed just like arrays.

```rust
var grid: array[array[int]] = [[1, 2, 3], [4, 5, 6]];
grid[1][2] = 42;
println("{}", grid[1][2]); // 42

let word = "hello";
println("{}", word[0]);            // "h"
println("{}", strslice(word, 1, 4)); // "ell"
```

### Array methods

mutable arrays also ship with useful built-in methods: `.push()`, `.pop()`, and `.remove()`.

```rust
var fruits = ["apple", "banana", "cherry"];

fruits.push("strawberry");
// fruits is now ["apple", "banana", "cherry", "strawberry"]

let fruit = fruits.pop();
// `.pop()` removes and returns the last element

let another_fruit = fruits.remove(1);
// `.remove(n)` removes and returns the element at index n

println("{}\n{}\n{}", fruit, another_fruit, fruits);
/*
prints:
strawberry
banana
[apple, cherry]
*/
```

`.push()` adds an element to the end of the array. `.pop()` removes and returns the last element, while `.remove()` removes and returns an element at a specified index.

these methods can only be used on mutable arrays, using them on `let` arrays causes yet another runtime error.

## What imi can't do (yet)

read or write files. I haven't implemented that yet, but probably will at some point.

other than that I can't think of much it can't do. It's turing complete after all.

I wonder if anyone would ever implement imi-lang in imi-lang if something like `fread()` was ever added to the language.
