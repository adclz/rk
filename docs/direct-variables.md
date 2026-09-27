# Direct variables

A direct variable is an address in one of three areas.
Your program reads its inputs from `%I`, writes its outputs to `%Q`, and keeps its own memory in `%M`:

```schema
▒ the physical world                sensors, switches, buttons
▼ wired to
%I inputs                           the host writes them before each scan
└─ level AT %IW0
▼ read by
PROGRAM Mixer                       your logic
├─ valve := level < 500
└─ %M markers                       its own memory, kept between scans
▼ written to
%Q outputs                          the host reads them after the scan
└─ valve AT %QX0.0
▼ wired to
▒ the physical world                valves, motors, lamps
```

The letter after the area says how wide the address is:

- **`X`** is one bit, like `%IX0.1`. A bit can leave it out: `%I1` is `%IX1`.
- **`B`** is 8 bits, like `%QB0`.
- **`W`** is 16 bits, like `%IW2`.
- **`D`** is 32 bits, like `%MD4`.
- **`L`** is 64 bits, like `%ML8`.

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

An address can also be written directly in a body, like `IF %IX0.1 THEN`, and its type is its width: `%IX` is a `BOOL`, `%IW` a `WORD`.

A declaration's type is an elementary type as wide as the address (`E1422`): `%IW` takes a `WORD`, an `INT` or a `UINT`, and `%ID` a `DWORD`, a `DINT` or a `REAL`.

- **Inputs are read-only.** A write to `%I` is `E1419`, and so is an initial value.
- **Only markers can be `RETAIN`.** `%I` and `%Q` are refused (`E1420`).
- **An address is declared once.** A second declaration is `E1421`.

## Parts of a wider address

When the program names an address and a narrower one inside it, the narrower one is part of the wider one: it has no storage of its own.
Each size counts in its own units, and the bytes are little-endian:

```diagram
                        %QW0
bit  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
    ┌───────────────────────┬───────────────────────┐
    │          %QB1         │          %QB0         │
    └───────────────────────┴───────────────────────┘
                     ▲                    ▲
                  %QX1.2               %QX0.3
```

A part can be read, assigned, and bound to an output.
A bit cannot be passed to a `VAR_IN_OUT` or referenced with `REF()`, and no part can be `RETAIN` or have an initial value (`E1423`).

## In function blocks

Each instance of a function block needs an address of its own.
The block declares it partly, `AT %I*`, and `VAR_CONFIG` completes it for each instance:

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

An instance left without an address is `E1425`.

A library cannot name a complete address, since it is code for any machine (`E1417`).

## On the host

The module exports each area it uses as a band, and its `located-map` section lists every address with the place it takes in memory.
A host binds a channel by its address, see [Memory bands](memory-bands.md).
