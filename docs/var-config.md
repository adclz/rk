# VAR_CONFIG

`VAR_CONFIG` gives one instance of a block its own address, or its own start value.
The block says what each instance needs, and the configuration says what each instance gets.

```schema
CONFIGURATION Plant
├─ RESOURCE Main
│  └─ TASK Fast
│     └─ PROGRAM L1 : Line
│        ├─ a : Motor                speed AT %IW0, from VAR_CONFIG
│        └─ b : Motor                speed AT %IW1, from VAR_CONFIG
└─ VAR_CONFIG                       one line per instance           ◄
   ├─ Main.L1.a.speed AT %IW0
   └─ Main.L1.b.speed AT %IW1
```

## An address for each instance

A block declares the address partly, `AT %I*`: the area is the block's, the number is the instance's.

```iecst
FUNCTION_BLOCK Motor
VAR
    speed AT %I* : INT;
    run   AT %Q* : BOOL;
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
        Main.L1.a.speed AT %IW0   : INT;
        Main.L1.a.run   AT %QX0.0 : BOOL;
        Main.L1.b.speed AT %IW1   : INT;
        Main.L1.b.run   AT %QX0.1 : BOOL;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH Fast : Line;
    END_RESOURCE
END_CONFIGURATION
```

- **The path** goes from the resource through the instances: `Main.L1.a.speed`.
- **The type** is written again, and is the variable's own (`E1426`).
- **The address** is complete, in the variable's area and as wide as its type (`E1424`).
- **An instance left without an address** is `E1425`.

A variable of a `PROGRAM` takes a partial address too, and is located the same way: `Main.L1.level AT %IW2 : INT;`.

## A start value for one instance

`VAR_CONFIG` also sets the start value of a variable, for one instance only:

```iecst sketch
VAR_CONFIG
    Main.Belt2.limit : UINT := 40;
END_VAR
```

`Belt2` runs the same program as `Belt1`, and only this value differs.
Every other instance keeps the value of the declaration, see [Configuration](configuration.md).

## In a library

A library is code for any machine: it names no address.
Its blocks declare `AT %I*`, `%Q*` or `%M*`, and your configuration locates each instance, as above.
A complete address in a library, in an `AT` or written in a body, is `E1432`.
This holds for every dependency, a `path` as a `url`, and for the standard library.

A library that reads a signal of the machine declares the variable `VAR_EXTERNAL`.
Your configuration declares the `VAR_GLOBAL` and its address:

```iecst
FUNCTION_BLOCK Guard
VAR_EXTERNAL
    emergency_stop : BOOL;
END_VAR
VAR_OUTPUT
    halted : BOOL;
END_VAR
    halted := emergency_stop;
END_FUNCTION_BLOCK

PROGRAM Cell
VAR
    safety : Guard;
END_VAR
    safety();
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        emergency_stop AT %IX0.0 : BOOL;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM C1 WITH Fast : Cell;
    END_RESOURCE
END_CONFIGURATION
```

See [Dependencies](dependencies.md) and [Direct variables](direct-variables.md).
