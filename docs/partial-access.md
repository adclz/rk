# Partial access

A bit, a byte, a word or a double word of a variable, read and written by its position.

| Access | Reads and writes | Type |
|---|---|---|
| `x.%X3` or `x.3` | bit 3 | `BOOL` |
| `x.%B1` | byte 1, bits 8 to 15 | `BYTE` |
| `x.%W1` | word 1, bits 16 to 31 | `WORD` |
| `x.%D1` | double word 1, bits 32 to 63 | `DWORD` |
| `x.%L0` | long word 0, all 64 bits | `LWORD` |

The position counts in units of the part: `%B1` is the second byte, `%W1` the second word.
Bit 0 is the least significant, so `%B0` is the low byte, whatever order the bytes have in memory.

```iecst
FUNCTION Pack : DWORD
VAR_INPUT
    high : WORD;
    low : WORD;
END_VAR
    Pack.%W1 := high;
    Pack.%W0 := low;
END_FUNCTION

FUNCTION Ready : BOOL
VAR_INPUT
    status : WORD;
END_VAR
    Ready := status.%X3 AND NOT status.15;
END_FUNCTION
```

A write replaces those bits only, and the rest of the variable keeps its value.

## Where it applies

A partial access takes a bit string or an integer, from `BYTE` to `LWORD` and from `SINT` to `ULINT`:
- **A variable**, an input, an output or a local.
- **An array element**, `a[i].%B1`.
- **A field**, `s.flags.%X0`.
- **What a reference points to**, `r^.%W0`.
- **A member of an instance**, `motor.state.%X2`.
- **The result of a FUNCTION or a METHOD**, by its name in its own body, as `Pack` above.

A signed integer is read as two's complement holds it: `i.%X15` is the sign of an `INT`.
Writing `.%B1 := BYTE#16#7F` into an `INT` of -1 gives 32767.

## What is refused

The position must be a constant inside the type: `w.%B2` on a `WORD` is `E1429`.

```iecst expect=E1429
FUNCTION Third : BYTE
VAR
    w : WORD;
END_VAR
    Third := w.%B2;
END_FUNCTION
```

A value written must have the type of the part: a `WORD` into `d.%B0` is `E0301`, and `WORD_TO_BYTE` converts it.

## Byte order

Partial access counts from the least significant bit, and knows nothing of byte order.
To read the bytes of a frame in a given order, use `GET_DWORD_BE` and its siblings in `Std.Arrays`, or `TO_BIG_ENDIAN` in `Std.Bytes`.
