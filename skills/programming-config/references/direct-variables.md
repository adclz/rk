# Direct variables

A direct variable is an address in one of three areas:

| Prefix | Area | Written by |
| --- | --- | --- |
| `%I` | inputs | the host, before every scan |
| `%Q` | outputs | the program; the host reads them after the scan |
| `%M` | markers | the program, which owns them |

The letter after the area is the width: `X` one bit, `B` 8 bits, `W` 16, `D` 32, `L` 64.
A bit can leave it out: `%I1` is `%IX1`.

## Declaring one

`AT` binds a name to an address, on a `VAR_GLOBAL` of the configuration or on a `VAR` of a `PROGRAM`:

```iecst
PROGRAM Mixer
VAR_EXTERNAL
    level : INT;
END_VAR
VAR
    valve AT %QX0.0 : BOOL;
END_VAR
    valve := level < 500;
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        level AT %IW0 : INT;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM M1 WITH Fast : Mixer;
    END_RESOURCE
END_CONFIGURATION
```

An address can also be written in a body, `IF %IX0.1 THEN`, and its type is its width: `%IX` is a `BOOL`, `%IW` a `WORD`.

- The declared type is an elementary type as wide as the address: `%IW` takes a `WORD`, an `INT` or a `UINT`, `%ID` a `DWORD`, a `DINT` or a `REAL`. Anything else is E1422.
- `%I` is read-only: a write, or an initial value, is E1419.
- Only `%M` can be `RETAIN`: a restored `%I` or `%Q` would run the first scan on the last power cycle's values (E1420).
- An address is declared once: a second declaration is E1421.
- A variable of a FUNCTION, FUNCTION_BLOCK or CLASS cannot have a complete address (E1417), and neither can a library, which is code for any machine.

## Parts of a wider address

When the program names an address and a narrower one inside it, the narrower one is part of the wider one and has no storage of its own.
Each width counts in its own units and the bytes are little-endian: beside a `%QW0`, `%QB1` is its high byte, `%QX0.3` its bit 3 and `%QX1.2` its bit 10.

```iecst
PROGRAM Lamps
VAR
    all  AT %QW0   : WORD;
    high AT %QB1   : BYTE;
    red  AT %QX0.3 : BOOL;
END_VAR
    all := 16#0000;
    red := TRUE;
END_PROGRAM
```

A part can be read, assigned and bound to an output.
A bit cannot be passed to a `VAR_IN_OUT` or referenced with `REF()`, and no part can be `RETAIN` or have an initial value (E1423).

## In function blocks

A function block exists once per instance, so it declares an address partly, `AT %I*`, `%Q*` or `%M*`, and `VAR_CONFIG` gives each instance its own:

```iecst
FUNCTION_BLOCK Motor
VAR
    run   AT %Q* : BOOL;
    speed AT %I* : INT;
END_VAR
    run := speed > 0;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    a : Motor;
    b : Motor;
END_VAR
    a();
    b();
END_PROGRAM

CONFIGURATION Plant
    VAR_CONFIG
        Main.L1.a.run   AT %QX0.0 : BOOL;
        Main.L1.a.speed AT %IW0   : INT;
        Main.L1.b.run   AT %QX0.1 : BOOL;
        Main.L1.b.speed AT %IW1   : INT;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH Fast : Line;
    END_RESOURCE
END_CONFIGURATION
```

- An instance left without its address is E1425.
- An entry with `AT` that does not locate a variable declared `AT %I*`, `%Q*` or `%M*` is E1424.
- Such a variable points at the address `VAR_CONFIG` gives it, so an instance initializer cannot set it (E1427).

## On the host

Each area the program uses is a band of the memory, exported as `input_base` / `input_size`, `output_base` / `output_size` and `marker_base` / `marker_size`.
The host writes the input band before every scan and reads the output band after it, a whole band at a time.

A band is sorted by address, not by declaration order, so a new variable never moves the others.
A cell takes the size of its declared type, and the numbers of an address order it without being byte offsets: `%IW2` and `%IW8` sit side by side.

The `located-map` custom section gives each address its cell, its width, its type, and for a part of a wider address, the address that owns it and its bit.
A located `VAR_GLOBAL` is in its area's band, not in the globals band.

A program instance with connections copies them through `Inst$__scan__`: see the `Connections` section of the skill.
