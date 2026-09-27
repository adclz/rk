# Bundled Traps

Everything the compiler checks raises **one** exception, and it carries a message.

A module declares a single exception tag, `(i32, i32)`: the pointer and length of a STRING in the memory you provided.
It is never exported, so a host reads it from the pending-exception slot.

> [!TIP]
> `__RAISE` triggers a WASM [throw](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/throw) and can be contained in a [try_table](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/try_table), `__TRY` and `__CATCH` are not implemented yet,
> but they are on the roadmap.

Three checks are inserted by the compiler:

| Inserted at | Message |
|---|---|
| every array subscript, per dimension | `array index out of bounds` |
| every subrange store | `value out of subrange bounds` |
| every `^` you wrote | `dereference of a null reference` |

What the compiler can prove is refused at compile time instead.

__And three checks that do not exist.__

- **Integer overflow is never checked.** `DINT#2147483647 + 1` is `-2147483648`, silently, at every width.
- **Division by zero is not ours.** It is the VM's own trap: no message, and a `{test}` cannot catch it.
- **STRING capacity is not checked.** It truncates. See [Strings](strings.md).

`REAL#1.0 / 0.0` is `+inf` and the scan continues. Nothing faults on NaN or infinity.

A Rust panic inside a grafted builtin arrives as the same exception, with the panic text as the message.
None of this depends on the profile.
