# Language Specification

A small interpreted programming language, implemented in Rust as part of a high school diploma project.

## 1. Variable Declarations

* `let` declares an **immutable** variable. `var` declares a **mutable** one.
* A type can be explicit (`let x: int = 15;`) or inferred from the initializer (`let y = "hello";`).
* A variable with no initializer must have an explicit type (`let a: int;`), since there is nothing to infer from.
* A variable has a fixed type once it is declared. The type is either explicitly specified or inferred from the initializer. A variable's type cannot change during execution.
* Type violations are detected when the relevant statement or expression is evaluated.
* A declared-but-unassigned variable can be read only after it has actually been assigned a value. Reading it beforehand raises a runtime error explaining that it was used before assignment. This is checked at the moment of every read, not ahead of time, so it works the same whether the assignment happens unconditionally or inside a branch.

```rust
let x: int = 15;
let y = "hello";     // inferred as str
let a: int;          // declared, not yet initialized
a = 50;              // fine now

var w = 14;
w = 40;              // fine, w is mutable
a = 40;              // ERROR — a is immutable
```

### Assignment is by value

Assigning one variable to another always copies the value — including `str` and arrays. `let a = b;` gives `a` its own independent copy; `b` stays valid and unaffected by anything done to `a` afterward, and vice versa.

Passing a variable as a function argument works the same way: the callee receives its own copy and never a reference to the caller's original value.

```rust
let b: str = "hello";
let a = b;   // a is an independent copy; both remain valid
```

### Mutability belongs to the binding, not the data

`let` and `var` describe a variable, not a value. A value carries no memory of whether it came from a `let` or a `var` — only the binding it currently sits in decides whether it can be changed, checked fresh at the moment of every assignment or mutation.

This means copying data from an immutable binding into a mutable one makes it mutable, and copying from a mutable binding into an immutable one makes it immutable — in both directions the data simply follows the rules of whichever binding now holds it:

```rust
let x: int = 10;
var y = x;
y = 15;       // fine — y is var, regardless of where its initial value came from

var m: int = 10;
let n = m;
n = 15;       // ERROR — n is let, regardless of where its initial value came from
```

This is the same rule Rust itself uses for plain assignment (`let x = 10; let mut y = x;` — `y` is freely reassignable even though `x` was not), and it extends without any special-casing to arrays, including nested ones (see [Arrays](#3-arrays)): an array's own contents carry no mutability of their own, so a `let` array copied into a `var` array becomes fully mutable as part of that `var` array, and a `var` array copied into a `let` array becomes fully frozen as part of that `let` array.

### Redeclaration and shadowing

A declaration in an inner scope may use the same name as a variable in an outer scope; the inner variable then shadows the outer one (see [Scoping](#6-scoping)). Redeclaring a name in the *same* scope is also allowed: the new declaration simply replaces the previous binding. Since the language has no references or closures, the old binding becomes unreachable either way, so "rebind" and "shadow" behave identically in practice.

## 2. Primitive Types

| Type    | Notes                       |
| ------- | --------------------------- |
| `int`   | Signed 64-bit integer       |
| `float` | 64-bit floating-point value |
| `str`   | Text                        |
| `bool`  | `true` / `false`            |

The language does not implicitly convert values between unrelated types. The single exception is `int` → `float`, described below.

### The `int` → `float` promotion

The only implicit conversion in the language is `int` → `float`. It is applied automatically:

* when a value is bound to a `float`-typed variable — at declaration (`var x: float = 1;`) or later assignment (`x = 2;`),
* when an `int` is pushed onto or written into an `array[float]`, at any nesting depth,
* when an array literal mixes `int` and `float` values (see [Numeric Promotion](#numeric-promotion)).

No conversion ever happens in the other direction — a `float` is never implicitly narrowed to an `int`.

```rust
var x: float = 1;    // 1 promoted to 1.0
x = 2;               // promoted again — assignment follows the same rule
```

### String Indexing

A `str` can be indexed with an `int`, yielding the single character at that position as a one-character `str` — the language has no separate `char` type.

* `s[index]` reads a character and works on both `let` and `var` strings.
* `s[index] = value;` writes a character and requires a `var` string. `value` must be a `str` exactly one character long.
* Indices count Unicode scalar values (characters), not bytes — this always matches what [`len`](#len) reports for that string, even when it differs from byte length for non-ASCII text.
* The index must be an `int`. A negative or out-of-bounds index is a runtime error.
* Compound assignment (`s[0] += "x"`) is not supported on a single character — only plain `=` is allowed.
* The same `[]` syntax indexes into a `str` reached through an array, at any nesting depth: `words[0][1]` reads the second character of the first string.

```rust
let s: str = "cat";
let c = s[0];        // "c"

var t: str = "cat";
t[0] = "b";          // t is now "bat"
t[0] += "x";         // ERROR — compound assignment not supported on a character

var words: array[str] = ["cat", "dog"];
words[0][1] = "u";   // words is now ["cut", "dog"]
```

## 3. Arrays

* Arrays are declared as `array[T]`, where `T` is `int`, `float`, `bool`, `str`, or another `array[T]`.
* Arrays may be nested to any depth.
* Arrays are homogeneous: every element must have the same type.
* An array's element type is fixed when the array is created.
* `let`-declared arrays are fully immutable, including their individual elements. They cannot be structurally modified and their elements cannot be written to.
* `var`-declared arrays allow both structural modification and element assignment.
* This immutability is a property of the binding, not the array's contents (see [Mutability belongs to the binding, not the data](#mutability-belongs-to-the-binding-not-the-data)) — copying a `var` array's value into a `let` binding produces a fully frozen copy, and copying a `let` array's value into a `var` binding produces a fully mutable copy.

```rust
let list1: array[int] = [1, 34, 5, 6, 2, 42345];
var list2: array[str] = ["this", "is", "a growable", "array"];
var matrix: array[array[int]] = [[1, 2], [3, 4]];
```

### Array type inference

When an array's type is inferred, every element must have the same type. The type of an empty array cannot be inferred, so an explicit type is required:

```rust
var x: array[int] = [];
```

An empty array is compatible with any `array[T]` type, since it has no elements to contradict `T`.

### Numeric Promotion

If an array literal contains a mix of `int` and `float` values, the `int`s are automatically promoted to `float`s, making the array homogeneous of type `array[float]`. This applies recursively to nested arrays.

```rust
let a = [1, 2, 3];          // array[int]
let b = [1.0, 2.0, 3.0];    // array[float]
let c = [1, 2.0, 3];        // array[float] (1 and 3 promoted)
let d = [[1, 2], [3.0, 4]]; // array[array[float]]
```

Similarly, if a variable is explicitly declared as `array[float]` (or `array[array[float]]`), any `int` elements in the initializer are promoted. Pushing an `int` to an `array[float]` or assigning an `int` to an index of an `array[float]` will also automatically promote the value.

### Eager Homogeneity Checking

Homogeneity is enforced eagerly at the moment an array literal is constructed, even if the array is used transiently and never assigned to a variable.

```rust
let e = [1, "hello"];       // ERROR — elements have different types
let f = [1, true, "str"][0]; // ERROR — array is heterogeneous
```

### Methods

The following methods are available on `var` arrays:

* `.push(value)` — appends an element to the end of the array. Produces no usable value.
* `.pop()` — removes and returns the last element.
* `.remove(index)` — removes and returns the element at `index`. `index` must be an `int` within bounds.

Calling these methods on a `let` array is an error.

Calling `.pop()` on an empty array is a runtime error.

`.remove` with an out-of-bounds index is a runtime error. `push` takes exactly one argument; `pop` takes none.

### Indexing

* `arr[index]` reads an element and works on both `let` and `var` arrays.
* `arr[index] = value;` writes an element and requires a `var` array.
* The index must be an `int`.
* A negative index is invalid.
* An out-of-bounds index is a runtime error.
* Values assigned to an array element must match the array's element type, after `int` → `float` promotion when applicable.
* Compound assignment works on elements of a `var` array, including nested ones: `matrix[0][1] += 2;`.
* Indexing into a `str` uses the same `[]` syntax and follows the same shape of rules — see [String Indexing](#string-indexing).

```rust
let x: array[int] = [1, 2, 3];
let first = x[0];   // fine
x[0] = 5;           // ERROR — x is immutable

var y: array[int] = [1, 2, 3];
y[0] = 5;            // fine
y.push(10);          // fine

var matrix: array[array[int]] = [[1, 2], [3, 4]];
let val = matrix[0][1];     // 2
matrix[0][1] = 5;           // fine
matrix[1].push(10);         // fine
```

## 4. Lexical Structure

### Identifiers

Identifiers consist of ASCII letters, digits, and underscores, but must begin with an ASCII letter or underscore.

```text
name
_counter
value123
```

Identifiers are case-sensitive.

Keywords cannot be used as ordinary identifiers.

### Keywords

The following words are reserved keywords:

```text
let
var
fn
return
if
else
while
continue
break
and
or
not
true
false
array
int
float
str
bool
```

### Integer literals

Integer literals consist of one or more decimal digits.

```text
0
42
123456
```

They represent signed 64-bit `int` values.

A numeric literal outside the representable range of `int` is a lexical error.

### Floating-point literals

Floating-point literals consist of decimal digits, a decimal point, and at least one digit after the decimal point.

```text
3.14
0.5
100.25
```

Scientific notation is not supported.

### String literals

Strings are enclosed in double quotes.

The following escape sequences are supported:

```text
\n   newline
\t   tab
\r   carriage return
\\   backslash
\"   double quote
\0   null character
```

An invalid escape sequence is an error.

An unterminated string literal is an error.

Strings may contain newline characters.

### Comments

Single-line comments begin with `//` and continue until the end of the line.

Block comments begin with `/*` and end with `*/`.

Block comments may be nested.

```rust
// This is a comment.

/*
   This is a block comment.

   /*
      Nested block comment.
   */
*/
```

Whitespace and comments have no effect on program execution.

## 5. Program Structure

There is no `main` function. A program is simply a sequence of top-level statements, which are executed from top to bottom.

Function declarations are processed before execution begins, so functions can be called before their declaration appears in the file:

```rust
print("{}", add(1, 2));

fn add(a: int, b: int) -> int {
    return a + b;
}
```

The following rules apply:

* Function declarations are allowed only at the top level, not inside a block, loop, or another function.
* A function declaration always includes its full body. There is no separate prototype declaration.
* Function declarations are registered before top-level statements execute.
* The position of a function declaration in the file therefore does not affect whether it can be called.
* Top-level variables are not implicitly global.
* Functions cannot access variables from the scope in which they were called.
* Statements that are not block-shaped — declarations, assignments, expression statements, `return`, `break`, `continue` — must end with a semicolon. `if`, `while`, bare blocks, and function declarations are not semicolon-terminated.
* Newlines are otherwise insignificant.

Function names and variable names belong to separate namespaces.

## 6. Scoping

Every `{ }` block — including `if`/`else`/`while` bodies and standalone blocks — introduces its own scope.

* Reading a variable checks the current scope first, then walks outward through enclosing scopes until it finds the variable.
* Assigning to an existing variable (`x = 5;`) works the same way and modifies the binding where it was found.
* Declaring a variable with `let` or `var` always creates a new binding in the current scope.
* A declaration in an inner scope may use the same name as a variable in an outer scope. The inner variable then shadows the outer variable.
* Redeclaring a variable name in the same scope is allowed. The new declaration replaces the previous binding with that name in that scope.

```rust
let x = 123;

{
    let y = x;
    print("{}", y);
}

// y is no longer available here.
// x is still available.

var z = 10;

{
    z = 20;            // modifies the outer z
}

print("{}", z);        // 20

{
    var z = 30;        // shadows the outer z
    print("{}", z);    // 30
}

print("{}", z);        // 20
```

### Function scope

Function calls are a hard scope boundary.

A function body can access its parameters and its own local variables, but it cannot access variables from the scope in which the function was called.

```rust
let x = 10;

fn test() {
    print("{}", x);    // ERROR — x is not visible inside the function
}
```

This rule applies regardless of how deeply nested the call site is.

## 7. Control Flow

```rust
var i = 0;

while i <= 100 {
    i += 1;
}

if x == 4 {
    // ...
} else if x == 5 {
    // ...
} else {
    // ...
}
```

* No parentheses are required around conditions.
* A condition must evaluate to `bool`.
* There is no truthy/falsy coercion. For example, `if 0 { }` is an error.
* `break;` exits the nearest enclosing loop.
* `continue;` skips to the next iteration of the nearest enclosing loop.
* `break` and `continue` may only be used inside loops. Using either outside a loop is a syntax error.
* `return` used outside a function is a syntax error, caught while parsing — the same way `break`/`continue` outside a loop are.
* Mutating a `let` variable is an error. This also applies when attempting to use a `let` variable as a loop counter.

## 8. Operators

### Comparison

```text
== != > < <= >=
```

### Logical

```text
and or not
```

Logical operators are word-based only. The symbols `&&`, `||`, and `!` are not supported.

### Arithmetic

```text
+ - * / ^ %
```

### Compound assignment

```text
+= -= *= /= ^= %=
```

### Precedence

Operators are ordered from tightest to loosest:

```text
a[i]  f(x)  a.m(x)      (postfix — tightest)
^
- (unary minus)
* / %
+ -
== != < > <= >=
not
and
or                      (loosest)
```

Postfix operations bind tighter than everything. Unary minus sits between `^` and `* / %` — looser than `^` (see [Unary negation](#unary-negation)), tighter than multiplication.

Exponentiation is right-associative: `2 ^ 3 ^ 2` is `2 ^ (3 ^ 2)`, i.e. `512`. All other binary operators are left-associative.

Raising an `int` to a negative `int` exponent is a runtime error: `2 ^ -1` is rejected. Use floats for negative exponents — `2.0 ^ -1` evaluates to `0.5`.

Parentheses override precedence as usual:

```rust
(3 + 4) * 2
```

evaluates to `14`.

### Comparisons

Comparisons do not chain.

```rust
1 < 2 < 3
```

is invalid.

Use:

```rust
1 < 2 and 2 < 3
```

instead.

### Unary negation

Unary `-` binds looser than `^`.

```rust
-5 ^ 2
```

means:

```text
-(5 ^ 2)
```

and therefore evaluates to `-25`.

To negate the number before exponentiation:

```rust
(-5) ^ 2
```

evaluates to `25`.

There is no unary `+` operator.

### Numeric behavior

* Arithmetic involving two `int` values produces an `int`.
* `int / int` is integer division: the quotient is truncated toward zero. `5 / 2` is `2`, and `-7 / 2` is `-3`.
* To obtain a `float` quotient, at least one operand must be a `float`: `5.0 / 2` is `2.5`.
* `%` produces the remainder, with the sign of the dividend: `-7 % 3` is `-1`, and `7 % -3` is `1`.
* `int` and `float` values may be mixed in arithmetic without an explicit conversion; the result is a `float`.
* `+` concatenates two `str` values.
* Adding a `str` to a number is an error.
* `int` and `float` values may be compared with each other.
* Comparing a `str` against a number with `==` or `!=` is allowed and always produces `false` or `true`, respectively.
* Ordering a `str` against a number using `<`, `>`, `<=`, or `>=` is an error.
* `bool` values support equality and inequality comparisons.
* Ordering `bool` values is not supported.
* Division by zero is a runtime error.
* Modulo by zero is a runtime error.

```rust
let quotient = 5 / 2;        // 2
let precise = 5.0 / 2;       // 2.5
let joined = "foo" + "bar";  // "foobar"
let a = 5 == 5.0;            // true
let b = "5" == 5;            // false
let c = "5" < 5;             // ERROR
```

### Integer overflow

`int` values are signed 64-bit integers.

An arithmetic operation whose result cannot be represented by an `int` is a runtime error.

Integer arithmetic does not wrap or saturate.

## 9. Functions

```rust
fn add(a: int, b: int) -> int {
    return a + b;
}

fn log_message(msg: str) {
    print("{}", msg);
}
```

* A function may declare a return type using `->`.
* A function without a declared return type produces `void` when called — a value meant only to be discarded. Assigning it to a variable or formatting it is a runtime error.
* A function with a declared return type must return a value of that type.
* Reaching the end of a function with a declared return type without returning is a runtime error.
* Returning a value whose type does not match the declared return type is a runtime error.
* A bare `return;` is allowed only inside a function without a declared return type.
* A bare `return;` exits the function immediately.
* Calling a function with the wrong number of arguments is a runtime error.
* Calling a function with an argument whose type does not match the corresponding parameter is a runtime error.
* Argument types must match parameter types exactly. Unlike assignment and array writes, no `int` → `float` promotion is applied at function boundaries.
* Two parameters of the same function may not share a name; duplicates are a syntax error.
* Arguments are passed by value. The function receives its own copy of each argument.
* A function call is an expression and may be used anywhere an expression is allowed.
* Recursion is supported. There is a fixed maximum call depth (currently 512 nested calls); exceeding it is a runtime error, not a crash.

```rust
let result = add(1, 2) + 3;
```

Functions are not values.

They cannot be:

* stored in variables,
* passed as arguments,
* returned from functions,
* used to create closures.

A function call always resolves the function by name.

There is no function type.

### Function names

The following names are reserved for built-in functions and cannot be used for user-defined functions:

```text
print
println
format
len
strslice
sleep
type
elapsed
input
parse
exit
```

## 10. Built-ins

### `print`

```text
print(fmt, args...)
```

Formats the given values using `{}` placeholders and prints the resulting string without automatically adding a newline.

The first argument must be a string literal written directly in the call — not a variable, not a function call, not any other expression, even if it evaluates to a `str`. This is a syntax error caught while parsing, the same way misplaced `break`/`continue` are — it is reported regardless of whether that call is ever reached during execution, and regardless of what any variable involved holds. There is no shorthand for printing a single non-`str` value directly.

```rust
let score: int = 5;
println("{}", score);      // correct — score fills a placeholder

let fmt = "{}";
println(fmt, score);       // ERROR — first argument must be a literal, not a variable

println(score);            // ERROR — first argument must be a str literal
```

The number of `{}` placeholders must exactly match the number of additional arguments.

To print a literal `{` or `}`, write it doubled as `{{` or `}}`. A `{` or `}` that is not part of a `{}` placeholder or a `{{`/`}}` escape is printed unchanged.

```rust
println("{{{}}}", 5);      // prints: {5}
println("literal: {{}}");  // prints: literal: {}
println("bad: {");         // prints: bad: {
```

### `println`

```text
println(fmt, args...)
```

Works like `print`, but adds a newline after the formatted output.

`float` values are rendered without a trailing `.0` — `println("{}", 2.0)` prints `2`, not `2.0`.

### `format`

```text
format(fmt, args...)
```

Works like `print`, but returns the formatted result as a `str` instead of printing it.

```rust
let name = "world";
let greeting = format("hello, {}!", name);
```

### `len`

```text
len(value)
```

Returns the length of the supplied value as an `int`.

* For a `str`, the number of characters (Unicode scalar values, not bytes).
* For an `array`, the number of elements.

```rust
len("hello")        // 5
len([10, 20, 30])   // 3
```

Calling `len` on any other type, or with an argument count other than 1, is a runtime error.

### `strslice`

```text
strslice(value, start, end)
```

Returns the portion of a `str` from `start` (inclusive) up to `end` (exclusive), as a new `str`.

* `value` must be a `str`, and `start`/`end` must both be `int`.
* Indices count Unicode scalar values (characters), not bytes — the same units as [`len`](#len) and [string indexing](#string-indexing), so `strslice(s, 0, len(s))` always returns a full copy of `s`.
* `start` must be `>= 0`, `end` must be `>= start`, and `end` must be `<= len(value)`. Violating any of these is a runtime error.
* `start == end` returns `""`.

```rust
let s = "hello world";
let greeting = strslice(s, 0, 5);   // "hello"
let rest = strslice(s, 6, len(s));  // "world"
let empty = strslice(s, 3, 3);      // ""
```

Calling `strslice` with the wrong argument types, or with an argument count other than 3, is a runtime error.

### `sleep`

```text
sleep(seconds)
```

Pauses execution for the specified number of seconds.

The argument may be an `int` or `float`.

The duration must be zero or positive. A negative value is a runtime error.

### `type`

```text
type(value)
```

Returns the type name of the supplied value as a `str`. For arrays, this includes the inner element type.

Examples:

```rust
type(5)                   // "int"
type(5.0)                 // "float"
type("hello")             // "str"
type(true)                // "bool"
type([1, 2, 3])           // "array[int]"
type([[1.0], [2.0]])      // "array[array[float]]"
```

### `elapsed`

```text
elapsed()
```

Returns the time elapsed since the program started, as a `float` in seconds.

Takes no arguments.

`elapsed()` measures wall-clock time since the interpreter was created. To measure a smaller interval, call it twice and subtract:

```rust
let start = elapsed();
// ... do some work ...
let delta = elapsed() - start;
print("took {} seconds", delta);
```

### `input`

```text
input(prompt)
```

Prints `prompt` to standard output without a trailing newline, then blocks until a full line is read from standard input.

The argument must be a `str`.

The returned `str` has all trailing newline and carriage-return characters removed — other leading or trailing whitespace the user typed is preserved.

```rust
let name = input("What is your name? ");
println("Hello, {}!", name);
```

### `parse`

```text
parse(value)
```

Attempts to convert a `str` into the most specific type it looks like, in this order:

1. If the text parses as an `int` (an optional leading `-` or `+` is allowed), returns an `int`.
2. Otherwise, if the text contains a `.`, and parses as a decimal number, returns a `float`.
3. Otherwise, if the text is exactly `"true"` or `"false"`, returns the corresponding `bool`.
4. Otherwise, returns the original `str` unchanged.

`parse` never raises an error on unparseable input; it falls back to returning the input string as-is. Combined with a declared type, an unexpected shape surfaces as an ordinary type-mismatch runtime error instead:

```rust
let age: int = parse(input("Enter your age: "));   // runtime error if the input isn't a whole number
let raw = parse("hello");                          // "hello" (str), no error
```

Since `parse`'s result type depends on the shape of the input, `type()` can be used to check what it actually returned before relying on it — useful when the expected type isn't known ahead of time, or to detect the str-fallback case:

```rust
let value = parse(input("Enter a value: "));
if type(value) == "str" {
    println("Could not parse '{}' as a number or bool.", value);
} else {
    println("Parsed as {}: {}", type(value), value);
}
```

### `exit`

```text
exit()
exit(code)
```

Immediately terminates the program.

`code` is optional and must be an `int` between `0` and `255` inclusive. If omitted, the program exits with code `0`.

`exit` does not return; any code after the call, in the current function or any caller, does not run. This applies no matter how deeply nested the call is — inside a loop, inside a function, inside a conditional.

```rust
fn require_positive(n: int) {
    if n < 0 {
        println("Error: expected a positive number, got {}.", n);
        exit(1);
    }
}

require_positive(-5);
println("This line never runs.");
```

## 11. Errors

Errors fall into three categories depending on which stage of the interpreter detects them: the lexer, the parser, or the evaluator. This distinction matters for anyone implementing the language, not just for users — a lexical or syntax error must be detected before a single statement runs, even in code that would never actually execute (an unreachable branch, a function that's never called), while a runtime error can only be detected once the relevant expression or statement is actually evaluated, since it depends on values that only exist during execution.

All errors report the line and column at which the relevant construct occurs, whenever that information is available.

### Lexical Errors

Detected while turning source text into tokens, before parsing begins.

* A character that doesn't start any valid token.
* A `!` not immediately followed by `=` — `!=` is the only valid use of `!` (see [Logical](#logical)).
* A numeric literal with more than one decimal point.
* An integer literal outside the range of a signed 64-bit integer.
* An invalid escape sequence inside a string literal (see [String literals](#string-literals) for the supported set).
* An unterminated string literal.

### Syntax Errors

Detected while parsing, before execution begins.

Most syntax errors share one shape: the parser expected a specific token at a specific position and found something else — a missing `;`, `(`, `)`, `{`, `}`, `[`, `]`, `:`, or identifier. These aren't enumerated individually here, since they follow directly from the grammar this document describes — any conforming parser produces the same set purely by implementing that grammar.

Beyond that generic case, the following are also syntax errors — including several that sound semantic but are deliberately caught before execution rather than during it:

* `break` or `continue` outside a loop (see [Control Flow](#7-control-flow)).
* `return` outside a function (see [Functions](#9-functions)).
* A function declaration that isn't at the top level (see [Program Structure](#5-program-structure)).
* A function declaration reusing a [reserved built-in name](#function-names).
* Two parameters of the same function sharing a name (see [Functions](#9-functions)).
* A variable declaration with neither a type annotation nor an initializer (see [Variable Declarations](#1-variable-declarations)) — note that despite living among otherwise runtime-checked type rules, this specific check is a syntax error.
* An assignment target that is not a variable or an index expression (e.g. `5 = 3;`).
* Chained comparison operators (see [Comparisons](#comparisons)).
* The first argument to `print`, `println`, or `format` not being a string literal (see [`print`](#print)) — checked against the argument's syntactic shape, not its runtime type, so a `str`-typed variable in that position is still rejected.

### Runtime Errors

Detected during execution, once the relevant expression or statement actually runs.

**Variables and assignment**
* reading a variable before it has been assigned a value,
* reading or assigning to an undeclared variable,
* assigning a value of the wrong type to a variable,
* modifying a `let` variable after it has been initialized.

**Functions** (see [Functions](#9-functions) for the exact rules)
* calling an undeclared function,
* calling a function with the wrong number of arguments, or with an argument of the wrong type,
* a function with a declared return type reaching its end without returning, or returning a value of the wrong type,
* a function without a declared return type returning a value,
* two top-level functions sharing the same name — checked once, before any statement executes, rather than at the individual call site,
* exceeding the maximum recursion depth.

**Arithmetic and comparison**
* division or modulo by zero,
* a negative exponent applied to an `int` (see [Numeric behavior](#numeric-behavior)),
* an `int` exponent too large to represent,
* integer overflow in `+`, `-`, `*`, or `^`,
* adding a `str` to a non-`str`,
* ordering a `str` against a number with `<`, `>`, `<=`, or `>=` (equality is allowed and always `false`, see [Numeric behavior](#numeric-behavior)),
* ordering `bool` values with anything other than `==`/`!=`,
* applying arithmetic or comparison to two otherwise incompatible types.

**Control flow**
* using a value where a `bool` condition is required (`if`/`while`),
* using a `void` value where a real value is required.

**Arrays**
* constructing a heterogeneous array literal, even one that's never bound to a variable (see [Eager Homogeneity Checking](#eager-homogeneity-checking)),
* indexing an array with a negative or out-of-bounds index,
* pushing, or index-assigning, a value of the wrong type into an array,
* popping from an empty array,
* removing at an out-of-bounds index,
* calling an unknown method on an array,
* structurally modifying, or writing to an index of, a `let` array.

**Strings** (see [String Indexing](#string-indexing) for the exact rules)
* indexing a string with a negative or out-of-bounds index,
* indexing into a character — only the last index in a chain may land on a `str`,
* assigning something other than a single one-character `str` to a string index,
* using compound assignment on a string index,
* calling a method on a `str` — arrays have methods, strings don't,
* writing to an index of a `let` string.

**Indexing in general**
* indexing a value that is neither an array nor a string.

**Built-ins**
* sleeping for a negative duration,
* an `exit` code outside `0`–`255`.

**Environment**
* an underlying I/O failure unrelated to program logic — e.g. `input()` failing to read a line, or output failing to flush. These can occur even in a program with no bugs, since they stem from the surrounding environment rather than anything the program did.
