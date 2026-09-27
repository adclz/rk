# Configuration

A `PROGRAM` does nothing until a `CONFIGURATION` runs it.

A program is a type, like a function block: it describes what a machine does.
The configuration describes your plant: which programs run, how often, what they share and where they are wired.

> [!NOTE]
> `config.toml` is something else: it describes the project, its name, its version, its lints.

A configuration looks like this, each program inside the task that runs it:

```schema
CONFIGURATION Plant                 the plant
├─ VAR_GLOBAL                       values every program can use
│  └─ line_speed : UINT := 80
├─ RESOURCE Main                    the processor
│  ├─ TASK Fast                     every 10 ms
│  │  └─ PROGRAM Belt1 : Conveyor
│  └─ TASK Slow                     every 100 ms
│     └─ PROGRAM Belt2 : Conveyor
└─ VAR_CONFIG                       settings for one instance
   └─ Main.Belt2.limit := 40
```

In code:

```iecst
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : UINT;
END_VAR
VAR
    limit : UINT := 100;
    speed : UINT;
END_VAR
    speed := line_speed;
    IF speed > limit THEN
        speed := limit;
    END_IF;
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : UINT := 80;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Slow(INTERVAL := T#100ms, PRIORITY := 5);
        PROGRAM Belt1 WITH Fast : Conveyor;
        PROGRAM Belt2 WITH Slow : Conveyor;
    END_RESOURCE
    VAR_CONFIG
        Main.Belt2.limit : UINT := 40;
    END_VAR
END_CONFIGURATION
```

Each section below takes one piece of this schema.

## CONFIGURATION

```schema
CONFIGURATION Plant                 the plant                     ◄
├─ VAR_GLOBAL …
├─ RESOURCE …
└─ VAR_CONFIG …
```

Your programs do not know how many copies of them run, or how often.
The configuration decides, and the same programs can run in another plant with another configuration.

- A workspace has one configuration (`E1402`).
- Another PLC is another workspace.

Like a namespace, a configuration can be split across files.
Blocks with the same name are one configuration:

```iecst
// globals.st
CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : UINT := 80;
    END_VAR
END_CONFIGURATION
```

```iecst continues
// main.st
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : UINT; // <-- declared in globals.st
END_VAR
END_PROGRAM

CONFIGURATION Plant // <-- the same configuration
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Belt1 WITH Fast : Conveyor;
    END_RESOURCE
END_CONFIGURATION
```

## RESOURCE

```schema
CONFIGURATION Plant
└─ RESOURCE Main                    the processor                 ◄
   ├─ TASK …
   └─ TASK …
```

A resource is one processor of the PLC: it holds the tasks, and the programs they run.
Tasks and programs go inside it, never directly in the configuration (`E1404`).

The standard allows one resource per processor.
rk compiles one module for one processor: a configuration has exactly one resource (`E1403`).

## TASK

```schema
CONFIGURATION Plant
└─ RESOURCE Main
   ├─ TASK Fast                     every 10 ms, priority 1       ◄
   │  └─ PROGRAM …
   └─ TASK Slow                     every 100 ms, priority 5      ◄
      └─ PROGRAM …
```

Not every part of a machine needs the same reaction time.
A belt stopping at a sensor must react within milliseconds, while a tank heating up can be checked ten times a second.
Running everything at the fastest rate wastes processor time: give each part a task at its own rate.

Time advances in ticks of 10 ms here, the largest step dividing both intervals:

```diagram
ms     0   10   20   30   40   50   60   70   80   90  100
Fast   ●    ●    ●    ●    ●    ●    ●    ●    ●    ●    ●
Slow   ●                                                 ●
```

When both are due on the same tick, the one with the lower `PRIORITY` runs first.

- `INTERVAL` is a `TIME` or `LTIME` literal, or a `VAR_GLOBAL CONSTANT` holding one. It cannot be zero (`E1410`).
- `PRIORITY` is required (`E1405`). `0` is the most urgent.
- A run that starts late happens once: the ticks it missed are dropped, never replayed.

The module carries the schedule in its `rk.schedule` section.
The host needs nothing else to run it.

## PROGRAM

```schema
CONFIGURATION Plant
└─ RESOURCE Main
   ├─ TASK Fast
   │  └─ PROGRAM Belt1 : Conveyor                                 ◄
   └─ TASK Slow
      └─ PROGRAM Belt2 : Conveyor                                 ◄
```

Your plant has two belts doing the same job.
Write the program once, and declare one instance per belt.

`PROGRAM Belt1 WITH Fast : Conveyor` reads: the instance `Belt1`, run by the task `Fast`, of the program `Conveyor`.
In code the instance sits in the resource, beside the tasks: `WITH` is what puts it in `Fast`.

Each instance has its own variables: `Belt1.speed` and `Belt2.speed` are two different variables.

An instance without `WITH` never runs:

```iecst expect=E1412
PROGRAM Conveyor
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Belt1 : Conveyor; // <-- no WITH
    END_RESOURCE
END_CONFIGURATION
```

The compiler tells you:

```console
[E1412] Error: program instance never runs
   ╭─[ main.st:7:17 ]
   │
 7 │         PROGRAM Belt1 : Conveyor; // <-- no WITH
   │                 ──┬──
   │                   ╰──── program instance 'Belt1' has no WITH <task>, so it will never run
───╯
```

The task must be one of the same resource (`E1411`).

### Wire it to inputs and outputs

```schema
RESOURCE Main
└─ TASK Fast
   ├─ PROGRAM C1 : Counter
   │  ├─ pulse := %IX0.0            copied in, before C1 runs     ◄
   │  └─ count => %QW0              copied out, after it          ◄
   └─ PROGRAM C2 : Counter
      ├─ pulse := %IX0.1                                          ◄
      └─ count => %QW1                                            ◄
```

A program that names its sensor's address only works on one line.
Declare the sensor as a `VAR_INPUT` instead, and wire it in the configuration.
Each instance gets its own sensor:

```iecst
PROGRAM Counter
VAR_INPUT
    pulse : BOOL; // <-- wherever the sensor is wired
END_VAR
VAR_OUTPUT
    count : UINT;
END_VAR
VAR
    last : BOOL;
END_VAR
    IF pulse AND NOT last THEN
        count := count + 1;
    END_IF;
    last := pulse;
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM C1 WITH Fast : Counter(pulse := %IX0.0, count => %QW0);
        PROGRAM C2 WITH Fast : Counter(pulse := %IX0.1, count => %QW1);
    END_RESOURCE
END_CONFIGURATION
```

`:=` copies a value into a `VAR_INPUT` before the program runs, and `=>` copies a `VAR_OUTPUT` out after it.
The other end is an address as wide as the variable, a global of the same type, or, for an input, a constant (`E1428`).

### Keep its values across a power cycle

```schema
RESOURCE Main
└─ TASK Fast
   ├─ PROGRAM RETAIN C1 : Counter       keeps its values                ◄
   └─ PROGRAM NON_RETAIN C2 : Counter   starts from its initial values  ◄
```

A part counter should survive a power cut.
A sequence should start again from its first step.

Inside a program, `VAR RETAIN` marks the variables to keep.
To decide for the whole instance, write the qualifier in the configuration:

- **`PROGRAM RETAIN C1 WITH Fast : Counter;`** keeps every variable of `C1`.
- **`PROGRAM NON_RETAIN C1 WITH Fast : Counter;`** keeps none of them.

### Run one of its function blocks at another rate

```schema
RESOURCE Main
├─ TASK Fast                        every 1 ms
│  └─ T1.filter : LowPass           the filter alone              ◄
└─ TASK Slow                        every 100 ms
   └─ PROGRAM T1 : Tank             the rest of T1
```

A noisy level sensor must be read every millisecond to be filtered.
The alarm reading the filtered value only needs to run ten times a second.
Rather than running the whole program every millisecond, give the filter a task of its own:

```iecst
FUNCTION_BLOCK LowPass
VAR_EXTERNAL
    level : INT; // <-- the sensor, read every millisecond
END_VAR
VAR_OUTPUT
    smooth : INT;
END_VAR
    smooth := (smooth + level) / 2;
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    filter : LowPass;
    alarm : BOOL;
END_VAR
    alarm := filter.smooth > 500; // <-- read every 100 ms
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        level AT %IW0 : INT;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#1ms, PRIORITY := 0);
        TASK Slow(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM T1 WITH Slow : Tank(filter WITH Fast);
    END_RESOURCE
END_CONFIGURATION
```

The task runs the block, and the program only reads its outputs.
If the program calls `filter()` too, the compiler tells you:

```console
[E1428] Error: program configuration element refused
    ╭─[ main.st:27:37 ]
    │
 16 │     filter(); // <-- the task already runs it
    │     ───┬──
    │        ╰──── 'filter' is called here
    │
 27 │         PROGRAM T1 WITH Slow : Tank(filter WITH Fast);
    │                                     ───┬──
    │                                        ╰──── 'filter' runs under its task, and 'Tank' calls it too
    │
    │ Note: the task runs the instance on its own; remove the call from the program
────╯
```

## VAR_GLOBAL

```schema
CONFIGURATION Plant
├─ VAR_GLOBAL                       values every program can use  ◄
│  └─ line_speed : UINT := 80
└─ RESOURCE Main
   ├─ TASK Fast
   │  └─ PROGRAM Belt1 : Conveyor   reads line_speed
   └─ TASK Slow
      └─ PROGRAM Belt2 : Conveyor   reads line_speed
```

Some values belong to the whole line rather than to one program: its speed, a sensor several programs read, a period two tasks share.
Declare them once, in the configuration.

Each POU that uses one declares it again with `VAR_EXTERNAL`:

```iecst sketch
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : UINT; // <-- the global, declared in the configuration
END_VAR
    …
END_PROGRAM
```

This way a POU's declarations list everything it takes from outside.

- Reading a global without `VAR_EXTERNAL` still works, and the linter warns with `L0118`.
- A `VAR_EXTERNAL` naming a global that does not exist is `E0206`.
- A `VAR_GLOBAL CONSTANT` can give a task its interval: `TASK Slow(INTERVAL := slow_period, PRIORITY := 5);`.

## VAR_CONFIG

```schema
CONFIGURATION Plant
├─ RESOURCE Main
│  ├─ TASK Fast
│  │  └─ PROGRAM Belt1 : Conveyor   limit = 100, its declaration
│  └─ TASK Slow
│     └─ PROGRAM Belt2 : Conveyor   limit = 40, from VAR_CONFIG   ◄
└─ VAR_CONFIG                       settings for one instance     ◄
   └─ Main.Belt2.limit := 40
```

`Belt2` carries pallets, heavier than the boxes on `Belt1`, and must not go above 40.
It runs the same program: only one value differs.

You could write a second program, or add an input and connect a constant to it on every run.
`VAR_CONFIG` does it once: it sets the starting value of that variable, for that instance only.

```iecst sketch
VAR_CONFIG
    Main.Belt2.limit : UINT := 40; // <-- resource, instance, variable, and its type again
END_VAR
```

The path goes through the function blocks a program holds, like `Main.L1.drive.speed`.
The type must be the variable's own (`E1426`).

`VAR_CONFIG` also finishes the wiring of function blocks.
A block declared `AT %I*` cannot know its address, since each instance has its own.
Give each instance its address in the configuration:

```iecst sketch
VAR_CONFIG
    Main.L1.a.speed AT %IW0 : INT; // <-- motor a
    Main.L1.b.speed AT %IW1 : INT; // <-- motor b
END_VAR
```

[Direct variables](direct-variables.md#in-function-blocks) shows the whole example.

The [programming-config](../skills/programming-config/SKILL.md) skill has every rule.
