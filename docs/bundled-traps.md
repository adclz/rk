# Bundled Traps

Everything the compiler checks raises **one** exception, and it carries a message.

A module declares a single exception tag, `(i32, i32)`: the pointer and length of a STRING in the module's memory.
It is never exported, so a host reads it from the pending-exception slot.

> [!TIP]
> You can raise this exception yourself with `__RAISE` and catch it with `__TRY`, see [Exceptions](exceptions.md).

Six checks are inserted by the compiler:

| Inserted at | Message |
|---|---|
| every array subscript, per dimension | `array index out of bounds` |
| every subrange store | `value out of subrange bounds` |
| every `^` you wrote | `dereference of a null reference` |
| every integer `/` and `MOD` whose divisor may be 0 | `division by zero` |
| every `STRING` passed to an `{extern}` FUNCTION | `a STRING passed to the host is not UTF-8` |
| every `CHAR` passed to an `{extern}` FUNCTION | `a CHAR passed to the host is not a character` |

What the compiler can prove is refused at compile time instead.
A divisor it folds to a value other than 0, such as `x / 2` or `x / K` for a CONSTANT `K`, is not checked.

__And two checks that do not exist.__

- **Integer overflow is never checked.** `DINT#2147483647 + 1` is `-2147483648`, silently, at every width.
- **STRING capacity is not checked.** It truncates. See [Strings](strings.md).

`REAL#1.0 / 0.0` is `+inf` and the scan continues. Nothing faults on NaN or infinity.

A Rust panic inside a grafted builtin arrives as the same exception, with the panic text as the message.
None of this depends on the profile.
