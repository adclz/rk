## Checks the compiler inserts

Three checks are generated inside the module, and each raises with a message:

| Inserted at | Message |
| --- | --- |
| every array subscript, per dimension | `array index out of bounds` |
| every subrange store | `value out of subrange bounds` |
| every `^` | `dereference of a null reference` |

What the compiler can prove is refused at compile time instead, and costs nothing at runtime.

The rest is WebAssembly arithmetic:

- Integer overflow wraps: `DINT#2147483647 + 1` is `-2147483648`, silently, at every width.
- Division by zero is the VM's own trap: it carries no message, and a `{test}` cannot catch it.
- A `STRING` too long for its destination truncates.
- `REAL#1.0 / 0.0` is `+inf` and the scan continues. Nothing faults on NaN or infinity.

A Rust panic inside a grafted builtin arrives as the same exception, with the panic text as the message.
None of this depends on the profile.

## Strings

There is one string type, UTF-8.

A slot is a 4-byte length followed by its capacity in bytes, so a plain `STRING` occupies 84 bytes and a `STRING[5]` holds 5 bytes, not 5 characters.

- A literal too long for its destination is a compile error (`E0314`).
- A variable too long truncates silently, and truncating bytes can split a character. `IS_UTF8` exists for exactly that.
- There is no indexing: `s[1]` is `E0508`, use `CHAR_AT`.
- There is no `+` on strings, use `CONCAT`.
- `LEN` is the byte length.

Comparison is byte-lexicographic, so `'Z' < 'a'`, and a `STRING` is a legal `CASE` label.
Labels compare as bytes, so they are case-sensitive even though identifiers are not.

```iecst
FUNCTION Mode : INT
	VAR_INPUT
		cmd: STRING;
	END_VAR
	CASE cmd OF
		'start':
			Mode := 1;
		'stop', 'halt':
			Mode := 2;
	ELSE
		Mode := 0;
	END_CASE;
END_FUNCTION
```

## Math

Every math function is in the module, which imports nothing for it.

`+ - * / MOD` are single WASM instructions, and so are `SQRT` and `ABS`.
The eleven that have no instruction — `SIN COS TAN ASIN ACOS ATAN ATAN2 EXP LN LOG` and `**` — are grafted in from libm when they are called, in `REAL` and `LREAL` form.
The same module therefore computes the same numbers on every host.

- 8- and 16-bit widths wrap by explicit masking; 32- and 64-bit wrap silently.
- `NaN` and `±inf` are ordinary values: `SQRT(-1)` is NaN, `LN(0)` is `-inf`.
- Float to integer saturates, and NaN converts to `0`.
- `IS_NAN` is ordinary ST: `IN <> IN`.
- `**` needs a float base and returns the base's type; `EXPT` is the same code path.

A bare literal expression computes at the literal's default type, then widens: `x : LREAL := 0.1 + 0.0` is REAL arithmetic.
Write `LREAL#0.1 + 0.0`.
