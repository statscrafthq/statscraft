<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OMSEND

End OMS requests and the OMS log.

## Syntax

```text
OMSEND [TAG=['name' ...] | TAG=ALL] [FILE=['file' ...]] [LOG].
```

TAG ends the requests of those names, FILE those that write those files; a name or file that no request has gives a warning. LOG ends the log of OMSLOG. With nothing given, every request and the log end.

## Example

```text
OMSEND TAG=['desc'].
```

`statscraft help OMSEND` prints the same text in the terminal. [All commands](README.md)
