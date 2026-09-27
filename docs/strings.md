# Strings

Rk uses one string type, **UTF-8**, there is no `WSTRING` and no `WCHAR`.

A slot is a 4-byte length followed by its capacity in bytes, so a plain `STRING` occupies 84.

```diagram
s : STRING[5] := 'café'

┌──────────────┬─────┬─────┬─────┬───────────┐
│  length: 5   │  c  │  a  │  f  │     é     │
└──────────────┴─────┴─────┴─────┴───────────┘
    4 bytes       1     1     1        2
               └───── capacity: 5 bytes ─────┘

LEN(s) is 5 bytes, CHAR_COUNT(s) is 4 characters
```

```st
s1: STRING;      // 80 bytes of buffer
s2: STRING[5];   // 5 BYTES, not characters - 'café' needs exactly this
```

The StdLib has several FUNCTIONs in `Std.Strings` to handle STRING operations:

- `IS_UTF8` checks if a STRING is valid UTF-8.

Byte read-only operations:

- `LEN` returns the byte-length of a STRING.
- `LEFT` returns the first n bytes.
- `RIGHT` returns the last n bytes.
- `MID` returns the n bytes starting at a 1-indexed byte position.
- `FIND` finds the 1-indexed byte position of a STRING in a STRING.

Char read-only operations:

- `CHAR_COUNT` returns the character-length of a STRING, so `CHAR_COUNT('café')` is 4 where `LEN` is 5.
- `CHAR_LEFT` returns the first n characters.
- `CHAR_RIGHT` returns the last n characters.
- `CHAR_MID` returns the n characters starting at a 1-indexed character position.
- `CHAR_FIND` finds the 1-indexed character position of a STRING in a STRING.
- `CHAR_AT` returns the character at a 1-indexed character position, as a `CHAR`.

>[!NOTE]
All char operations agree on ASCII.

Write operations:

- `CONCAT` concatenates 2 STRING together, if the result is too large, the final STRING is truncated.
- `INSERT` inserts a STRING after a given byte position.
- `DELETE` removes n bytes starting at a 1-indexed byte position.
- `REPLACE` replaces n bytes at a 1-indexed byte position with a STRING.
- `CHAR_INSERT` inserts a STRING after a given character position.
- `CHAR_DELETE` removes n characters starting at a 1-indexed character position.
- `CHAR_REPLACE` replaces n characters at a 1-indexed character position with a STRING.


- A **literal** too long for its destination is a compile error.
- A **variable** too long truncates silently, and truncating bytes can split a character.
  `IS_UTF8` exists for exactly that.

- No indexing - `s[1]` is `E0508`, use `CHAR_AT`. 

- No `+` - use `CONCAT`.

Comparison is byte-lexicographic, so a `STRING` is a legal `CASE` label.

```st
FUNCTION Mode : INT
    VAR_INPUT
        cmd: STRING;
    END_VAR
    CASE cmd OF
        'start':
            Mode := 1;
        'stop', 'halt':   // one branch, several labels
            Mode := 2;
    ELSE
        Mode := 0;        // 'Start' lands here: labels compare as bytes
    END_CASE;
END_FUNCTION
```

`CHAR` is a code point in 4 bytes. It does **not** widen to `STRING`; `CHAR_TO_STRING` does that.

> [!IMPORTANT]
> At a call boundary a `STRING` input or return is a borrowed `(ptr, len)`, never a copy.
>
> A `VAR_IN_OUT` or `VAR_OUTPUT` is instead `(addr, capacity)`, so the callee's writes clamp.
