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
The linter reports it as `L0132`, since a declared name says what is wired there.

A declaration's type is an elementary type as wide as the address (`E1422`): `%IW` takes a `WORD`, an `INT` or a `UINT`, and `%ID` a `DWORD`, a `DINT` or a `REAL`.

- **Inputs are read-only.** A write to `%I` is `E1419`, and so is an initial value.
- **Only markers can be `RETAIN`.** `%I` and `%Q` are refused (`E1420`).
- **An address is declared once.** A second declaration is `E1421`.
- **A located variable of a `PROGRAM` follows its own `RETAIN`.** Its cell is one for every instance of the program, so `PROGRAM RETAIN` or `NON_RETAIN` on an instance does not reach it.

## Parts of a wider address

When the program names an address and a narrower one inside it, the narrower one is part of the wider one: it has no storage of its own.
By default, each size counts in its own units, and the bytes are little-endian:

```diagram
                        %QW0
bit  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
    ┌───────────────────────┬───────────────────────┐
    │          %QB1         │          %QB0         │
    └───────────────────────┴───────────────────────┘
                     ▲                    ▲
                  %QX1.2               %QX0.3
```

A part can be read, assigned, bound to an output, and bound to a `VAR_IN_OUT` a `FUNCTION_BLOCK` keeps as a member, which is copied in and back.
A bit cannot be passed to a `VAR_IN_OUT` of a `FUNCTION` or a `METHOD`, which binds its address, or referenced with `REF()`, and no part can be `RETAIN` or `NON_RETAIN`, or have an initial value (`E1423`).

## Numbering by byte

The number of an address says where its bytes are.
By default, each size counts in its own units: `%MW1` is the second word, bytes 2 and 3.
Some PLCs count bytes for every size instead: `%MW1` is bytes 1 and 2.

`[addresses]` in `config.toml` says how your workspace numbers its addresses:

```toml
[addresses]
numbering = "byte"
byte-order = "big"
```

- **`numbering = "width"`** counts each size in its own units. This is the default.
- **`numbering = "byte"`** counts bytes for every size.
- **`byte-order = "little"`** puts the low byte of a value first. This is the default.
- **`byte-order = "big"`** puts the high byte first.

A bit is numbered the same way in both: `%MX1.2` is bit 2 of byte 1.

Counting bytes, big-endian, `%MB208` is the high byte of `%MW208`:

```diagram
                       %MW208
bit  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
    ┌───────────────────────┬───────────────────────┐
    │         %MB208        │         %MB209        │
    └───────────────────────┴───────────────────────┘
                     ▲                    ▲
                 %MX208.2             %MX209.3
```

After `%MW208 := 16#1234`, `%MW208` reads `16#1234` in both orders.
`%MB208` reads `16#12` big-endian, and `16#34` little-endian.

> [!NOTE]
> IEC 61131-3 leaves both choices to the implementation.

### Addresses that share bytes

Counting bytes, two addresses can overlap without either holding the other: `%MW7` is bytes 7 and 8, and `%MW8` is bytes 8 and 9.
They are stored together.
A write to one changes what the other reads.

- **They are all `RETAIN`, or none of them is.** A declaration that is not, beside one that is, is `E1434`.
- **None of them has an initial value.** It would set part of the others (`E1435`).

To give them an initial value, declare a variable at an address that holds all their bytes, like `%MD7`.
`%MW7` and `%MW8` are then parts of it.

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

A library cannot name a complete address, since it is code for any machine (`E1432`), see [VAR_CONFIG](var-config.md).

## On the host

The module exports each area it uses as a band, and its `located-map` section lists every address with the place it takes in memory and its offset in its area.
A component's host finds an address by that offset in the lists `run-task` takes and returns.
A host binds a channel by its address, see [Memory bands](memory-bands.md).
