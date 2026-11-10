<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DEFINE

Define a macro: a name that stands for syntax, with arguments.

## Syntax

```text
DEFINE name ([argument [/ argument ...]])
```

body !ENDDEFINE.

argument: !POSITIONAL or keyword =, then one of !TOKENS(n), !CHAREND('c'), !ENCLOSE('a', 'b'), !CMDEND, optionally with !DEFAULT(value) and !NOEXPAND.

DEFINE and !ENDDEFINE are written in full; the name may start with !.

A call is the macro name followed by its arguments, anywhere in a command outside quotes: positional values first, in order, then keyword = value in any order. !TOKENS takes the next n tokens, !CHAREND the tokens up to the character, !ENCLOSE the tokens between the two characters, !CMDEND the rest of the command; the end of the command ends every argument, and an argument left out gets its default or nothing. Tokens no argument takes stay in place.

In the body !1, !2 ... are the positional arguments, !\* all of them, !name a keyword one.

Outside quotes the body is read in upper case.

Statements: !LET !var = expression; !IF (expression) !THEN ... \[!ELSE ...\] !IFEND; !DO !var = from !TO to \[!BY step\] ... !DOEND; !DO !var !IN (list) ... !DOEND; !BREAK; !OFFEXPAND and !ONEXPAND to stop and resume expanding macro calls.

Functions: !LENGTH(s) !CONCAT(s, ...) !SUBSTR(s, from\[, length\]) !INDEX(s, part) !HEAD(s) !TAIL(s) !QUOTE(s) !UNQUOTE(s) !UPCASE(s) !BLANKS(n) !NULL !EVAL(s).

Expressions: = \<\> \~= \< \> \<= \>= (or !EQ !NE !LT !GT !LE !GE), !AND !OR !NOT (or & \| \~), parentheses. Operands are unquoted and compared as numbers when both are numbers, else as text with case; the result is 1 or 0, and 0 is false.

The expansion is scanned again for macro calls, up to SET MNEST levels (50); a !DO loop runs at most SET MITERATE times (1000). SET MEXPAND=OFF stops expansion and SET MPRINT=YES shows every expanded command.

A body that ends with a period ends the command when the call is the last thing in it; in the middle of a command that period is dropped.

## Example

```text
DEFINE !freqs (vars = !CMDEND) FREQUENCIES VARIABLES=!vars. !ENDDEFINE.
```

`statscraft help DEFINE` prints the same text in the terminal. [All commands](README.md)
