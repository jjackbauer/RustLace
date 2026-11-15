# Lexical structure

This page describes the text-to-token phase: source encoding, whitespace, comments, identifiers,
every literal form, statement separators, and the exact token set the lexer recognises. The
implementation is the tokenizer and recursive-descent parser in `lovelace-lang/src/syntax.rs`.

## Source text and encoding

A script is Unicode text. The `lovelace-run` file surface detects the encoding from a byte-order
mark, exactly as a text editor would:

| Input | Decoding |
|---|---|
| UTF-8 without BOM | UTF-8 |
| UTF-8 with BOM | UTF-8; the BOM becomes U+FEFF and is stripped by the engine |
| UTF-16 LE / BE with BOM | UTF-16, BOM stripped by the engine |
| UTF-32 LE / BE with BOM | UTF-32, BOM stripped by the engine |

A leading U+FEFF is accepted and ignored by the engine; `\u{FEFF}1+2` evaluates to `3`. A file
with invalid UTF-8 is a caller-level `FileReadError` / `TypeMismatch` failure, not a language
parse error.

All three common line endings are accepted. A CRLF pair ends one line; a lone CR and a lone LF
each end one line. The tokenizer sees `\n` as a statement separator and treats the preceding
`\r` as ordinary whitespace, so Windows files need no conversion.

Positions published in diagnostics are **UTF-16 code-unit offsets** into the caller's text, with
1-based line and column numbers computed from those units. That matters only for non-BMP
characters (an emoji counts as two units); for ASCII source, a character offset and a UTF-16
offset are the same number.

The parser refuses more than **256** nested statement/expression/power levels with a
`DepthExceeded` / `BudgetExceeded` failure before a recursive walk can exhaust the process stack.
Evaluation has its own budget of **512** depth units per top-level statement, and symbolic
expressions and values have separate construction budgets. Those limits are visible as typed
errors rather than stack overflows.

## Whitespace

Spaces, tabs, carriage returns, and every other character for which Rust's `char::is_whitespace`
is true are ignored between tokens. A newline is not just whitespace: it is a statement
separator (see [Separators](#separators)).

Whitespace is never required where the grammar makes a boundary unambiguous, but it is required
where two tokens would otherwise merge. There is no implicit multiplication or function
application: `2x` is a number followed by an identifier, which is a syntax error, not `2 * x`.

## Comments: there are none

The language has **no comment syntax**. The tokenizer has no comment branch, and `#`, `//`,
`/*`, and `--` are not recognised. A comment attempt is a parse error naming the first character
or token it could not use:

```lovelace
# comment
```
```text
Error [ParseError/ParseError]: Unexpected character '#' at position 0.
```

```lovelace
1 // comment
```
```text
Error [ParseError/ParseError]: Unexpected token '/' at position 3: expected a number, string, identifier, '[', or '('.
```

The absence is deliberate: a permissive comment branch would accept scripts the reference parser
refuses and would change the published positions. Strip comments in the host if you need them.

## Identifiers

Identifiers are ASCII:

```text
[A-Za-z_][A-Za-z0-9_]*
```

The first character is an ASCII letter or underscore; subsequent characters are ASCII letters,
digits, or underscores. There is no `$` sigil, no Unicode identifier category, and no
case-insensitive matching. `_x`, `x2`, and `_` are identifiers; `2x` starts with a digit and is
therefore a number followed by an identifier; `héllo` fails at the `é`:

```lovelace
héllo = 1
```
```text
Error [ParseError/ParseError]: Unexpected character 'é' at position 1.
```

The command above is shown for documentation; the runner's diagnostic file is UTF-8, and the
character named in the sentence is the source character, not an escape.

Keywords such as `if`, `else`, `while`, `for`, `in`, `return`, `break`, `continue`, `func`,
`true`, `false`, and `not` are recognised **contextually**, not by a reserved-word lexer. They
taste like identifiers to the tokenizer and are interpreted by the parser at statement and
expression positions. Do not rely on using a keyword as an ordinary variable or function name;
doing so usually changes the parse.

## Numeric literals

There are no signs in numeric literals. `-5` is unary minus applied to the natural `5` and
produces an `Integer`; the literal itself is always non-negative.

### Integers and decimals

| Form | Examples | Kind |
|---|---|---|
| One or more ASCII digits | `0`, `42`, `1267650600228229401496703205376` | `Natural` |
| Digits, dot, digits | `3.14`, `0.5` | `Real` |
| Dot, digits | `.5` | `Real` |

The dot form requires at least one digit after the dot. `1.` tokenizes as the number `1` followed
by a member-access dot, and the parser then demands a field name:

```lovelace
1.
```
```text
Error [ParseError/ParseError]: Expected 'Ident' but found '' at position 2.
```

Decimals are exact decimal values, not binary floating point: `0.1` is exactly one tenth, and
`0.1 + 0.2` is exactly `0.3`.

### Periodic decimals

A periodic decimal names the repeating block in parentheses. Two shapes are accepted by the
tokenizer:

| Shape | Example | Value |
|---|---|---|
| Integer part, dot, parenthesised period | `0.(3)` | 1/3 |
| Integer part, dot, preperiod, parenthesised period | `0.1(6)` | 1/6 |
| Integer part, dot, preperiod, parenthesised period | `1.2(34)` | 1.2 followed by repeating 34 |

The period must contain at least one digit and must be closed by `)`. The value is exact and
carries period metadata, so `0.(3) * 3` is exactly `1`, not a rounded approximation.

```lovelace
0.(3)
```
```text
= 0.(3) (Real)
  _ = 0.(3)
```

The tokenizer also recognises the integer form `1(3)` as a period token (digits followed by a
parenthesised block), but the evaluator rejects it because the notation requires a decimal point:

```lovelace
1(3)
```
```text
Error [InvalidOperation/DomainError]: Invalid number literal '1(3)'.
```

Malformed forms such as `0.(3` or `0.()` do not become periodic tokens; they fall back to a
number followed by `(`, which the parser refuses. A period token must be self-contained: `0.(3)x`
is a periodic literal immediately followed by an identifier and fails on the missing separator.

### No scientific notation, radix prefixes, or separators

The lexer has no exponent branch. `1e3` is the number `1` followed by the identifier `e3`, not
one thousand:

```lovelace
1e3
```
```text
Error [ParseError/ParseError]: Expected ';' or end of input but found 'e3' at position 1.
```

For the same reason there is no hexadecimal/binary/octal prefix (`0x10` is `0` then `x10`), no
underscore separator (`1_000` is `1` then `_000`), and no leading `+` or `-` in a literal. Write
`10 ^ 3`, `0x`-style values as decimals, and `1000`; express signs with the unary operator.

## String literals

A plain string is delimited by double quotes and may contain any character except an unescaped
double quote. Single-quoted strings do not exist.

```lovelace
"hello"
```
```text
= hello
  _ = hello
```

The scanner is deliberately small: a backslash consumes the next character, and the pair is kept
for the evaluator's unescape step. The recognised escapes are `\"` (quote), `\\` (backslash),
`\n` (newline), `\t` (tab), and `\r` (carriage return). An unrecognised escape keeps the
backslash literally: `"a\qb"` is `a\qb`. A string may span lines; the newline is part of the
string, not a statement separator.

```lovelace
"a\"b\\c"
```
```text
= a"b\c
  _ = a"b\c
```

An unterminated string is a typed parse failure:

```lovelace
"abc
```
```text
Error [InvalidInput/ParseError]: Unterminated string literal at position 0.
```

Note the code: `InvalidInput` is the stable code, while `ParseError` is the frozen category.

## Interpolated strings

A string prefixed with `$` is interpolated. Inside it, `{expression}` is replaced by the display
form of the expression's value; `{{` and `}}` produce literal braces. The expression is parsed
with the ordinary expression grammar, and nested braces are balanced while scanning.

```lovelace
$"x={1+2}"
```
```text
= x=3
  _ = x=3
```

```lovelace
$"{{x}} = {1+2}"
```
```text
= {x} = 3
  _ = {x} = 3
```

Interpolation renders with the value's display form, not its typed form: a `Real` shows its
digits without `(Real)`, a record shows its record rendering, and `Void` renders as the empty
string. Strings inside the braces must themselves be valid; there is no nesting of `$"…"` into
itself beyond what the expression parser can handle.

A lone `$` that is not followed by `"` is not a token:

```lovelace
$1
```
```text
Error [ParseError/ParseError]: Unexpected character '$' at position 0.
```

## Boolean literals: `true` and `false`

The parser recognises `true` and `false` as primary expressions, but it lowers them to the
numeric literals `1` and `0`. They are **Natural values**, not Booleans:

```lovelace
[true, false, type(true)]
```
```text
= [1, 0, Natural] (Vector)
  _ = [1, 0, Natural]
```

This is a real language quirk, not a documentation shortcut. A `Boolean` value is produced by a
comparison (`1 < 2`) or by the `not` builtin; there is no Boolean literal. `if (true)` therefore
fails with "if condition must be Boolean, but got 'Natural'." See
[Values and types](values-and-types.md#boolean) and
[Statements and control flow](statements-and-control-flow.md#conditions).

## The token set

The lexer emits the following token variants:

| Variant | Source spelling | Notes |
|---|---|---|
| `Num` | digits, `digits.digits`, or `.digits` | Integer or decimal literal text, kept as source text. |
| `Repr` | `digits.(digits)` or `digits.digits(digits)` | Periodic literal, kept as source text; integer-only `1(3)` tokenizes but is refused at evaluation. |
| `Str` | `"…"` | Plain string; inner text still contains backslash escapes. |
| `Interp` | `$"…"` | Interpolated string; raw body split by the parser. |
| `Ident` | `[A-Za-z_][A-Za-z0-9_]*` | Identifier or contextual keyword. |
| `Punct` | see below | Punctuator, including multi-character forms. |
| `Semi` | `;` or a newline | Statement separator. |
| `Eof` | end of input | Always last. |

The complete `Punct` vocabulary, longest match first:

| Two-character | One-character |
|---|---|
| `..` `==` `!=` `<=` `>=` | `+` `-` `*` `/` `%` `^` `!` `=` `<` `>` `(` `)` `[` `]` `{` `}` `,` `:` `.` |

`;` is emitted as `Semi`, not `Punct`. There is no `&&`, `||`, `**`, `+=`, `-=`, `++`, `--`,
`->`, `=>`, `::`, `//`, or `#` token. Characters outside this set are the
`Unexpected character '<c>' at position N.` parse failure. That includes `&`, `|`, `~`, `?`,
`'`, `` ` ``, `@`, `\` outside a string, and any non-ASCII letter.

```lovelace
1 && 2
```
```text
Error [ParseError/ParseError]: Unexpected character '&' at position 2.
```

## Separators, newlines, and line continuation

The language has exactly two statement separators: the semicolon and the newline. Both are
emitted as the same `Semi` token. One or more separators between statements are allowed, and
leading and trailing separators are ignored:

```lovelace
;;1;;2;;
```
```text
= 2 (Natural)
  _ = 2
```

There is **no line-continuation syntax**. A newline is a separator even inside parentheses,
brackets, and function-call argument lists. An expression cannot be split across lines unless
the split happens where a block parser is already skipping separators:

```lovelace
1 +
2
```
```text
Error [ParseError/ParseError]: Unexpected token ';' at position 3: expected a number, string, identifier, '[', or '('.
```

Inside braces the parser accepts separators between statements, so a multi-statement block can
be written on several lines:

```lovelace
{
  x = 1
  x + 2
}
```
```text
= 3 (Natural)
  _ = 3
```

The semicolon form is equivalent and works everywhere. A newline inside a string literal is part
of the string, not a separator.

Because a separator is mandatory between two statements, juxtaposition is never an implicit
product or application:

```lovelace
2x
```
```text
Error [ParseError/ParseError]: Expected ';' or end of input but found 'x' at position 1.
```

## What is not in the lexical grammar

For completeness, these common lexical features are absent:

- comments of any style;
- single-quoted or triple-quoted strings;
- raw strings and backtick strings;
- character literals;
- scientific notation and hexadecimal/binary/octal literals;
- digit separators;
- Unicode identifiers;
- boolean literals that produce `Boolean` values (lowercase `true`/`false` are numbers);
- compound-assignment and increment/decrement operators;
- logical operators `&&`, `||`, and `!` as negation (`!` is only postfix factorial);
- ternary `?:`;
- line-continuation backslashes;
- shebangs or directives.

Everything else in the language is built from the tokens above. The next page turns those tokens
into values: [Values and types](values-and-types.md).
