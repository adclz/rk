---
name: programming-config
description: Declare how a program actually runs — CONFIGURATION, RESOURCE, TASK, intervals, priorities, VAR_GLOBAL, program connections, VAR_CONFIG and the direct variables (%I, %Q, %M). Use when wiring a PROGRAM to a task or to inputs and outputs, setting a scan interval, sharing globals between POUs, or giving one instance its own values or addresses.
---

## Summary

`CONFIGURATION` is where the PLC logic is declared: which programs exist as instances, how often each one is scanned, and which globals the application owns.

A `PROGRAM` on its own is only a type; it is compiled like a function block and nothing runs it.

Without a `CONFIGURATION` instantiating it the compiled module carries no schedule at all and the runtime falls back to calling a single exported entry: a FUNCTION marked `{export}` that takes nothing and returns nothing.

A workspace has exactly one CONFIGURATION.

Blocks with the same name in different files are fragments of that one configuration and merge, which is what lets a library ship its `VAR_GLOBAL`s in their own file.

Do not confuse this with `config.toml`, which is the project file (name, version, optimization, lints) and has nothing to do with the PLC logic.
It is described at the end.

## Syntax

```iecst
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : INT;
END_VAR
VAR RETAIN
    total : DINT;
END_VAR
    total := total + line_speed;
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : INT := 5;
    END_VAR
    VAR_GLOBAL CONSTANT
        slow_period : TIME := T#100ms;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Slow(INTERVAL := slow_period, PRIORITY := 5);
        PROGRAM Belt1 WITH Fast : Conveyor;
        PROGRAM NON_RETAIN Belt2 WITH Slow : Conveyor;
    END_RESOURCE
END_CONFIGURATION
```

`TASK` and `PROGRAM` live inside a `RESOURCE`, never directly under the CONFIGURATION; written outside one they are a syntax error (E1404).
`VAR_GLOBAL` is the opposite: it is application-scoped and belongs to the CONFIGURATION, so a `VAR_GLOBAL` inside a RESOURCE is rejected (E0021).
A RESOURCE holds no variables of its own.

The sections of a CONFIGURATION may appear in any order and any number of times, and the CONFIGURATION itself may come before or after the POUs it instantiates.

`ON <name>` is required by the grammar and names an execution unit.
Nothing in the compiler resolves it; it only shows up in hover and document symbols.

`PROGRAM RETAIN <inst>` forces the whole instance to be persisted across a power cycle even when it declares no `RETAIN` field; `PROGRAM NON_RETAIN <inst>` suppresses persistence even when it does.
Written without a qualifier, the program's own declarations decide.

Two instances of the same PROGRAM type are fine — each gets its own state.

## One configuration, one resource

More than one CONFIGURATION *name* in the workspace is E1402, reported at every one of them (there is no first — file order is not meaningful).
A POU is a type usable by any configuration, so a second one leaves no answer to which globals are in scope inside a POU.
A second PLC is a second workspace.

More than one RESOURCE across the configuration is E1403.
This mirrors the runtime, which drives one resource on one thread; it is a temporary restriction, not an IEC rule.

## Fragments

Same-named CONFIGURATION blocks merge.
A file may declare only globals, another only the resources:

```iecst
(* globals.st *)
CONFIGURATION Plant
    VAR_GLOBAL
        line_speed : INT := 5;
    END_VAR
END_CONFIGURATION
```

```iecst continues
(* main.st *)
CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Belt1 WITH Fast : Conveyor;
    END_RESOURCE
END_CONFIGURATION
```

What may not collide across fragments is the same as what may not collide inside one block: a `VAR_GLOBAL` declared by two fragments is E0101, a `RESOURCE` name claimed by two fragments is E0115.
Both are reported symmetrically, at each declaring fragment, with the sibling as related information.

Two same-named blocks in the *same* file are valid and merge, but separate nothing; the linter says so with L0205 (`duplicate-configuration`).

## Globals

`VAR_GLOBAL` at configuration level is the application's memory.
The IEC way to reach one from a POU is `VAR_EXTERNAL`:

```iecst continues
PROGRAM Conveyor
VAR_EXTERNAL
    line_speed : INT;
END_VAR
    line_speed := line_speed + 1;
END_PROGRAM
```

A `VAR_EXTERNAL` naming no global is E0206.
Reading the global directly by name, without declaring `VAR_EXTERNAL`, also works — it resolves and compiles — but the linter warns with L0118 (`global-without-external`).
It is a warning, not an error.

## Tasks

`INTERVAL := <TIME or LTIME literal>` or `INTERVAL := <name of a VAR_GLOBAL CONSTANT holding one>`.
The period is baked into the emitted schedule, so it must be fixed at compile time; a CONSTANT global qualifies, a plain `VAR_GLOBAL` does not because it may be written while the PLC runs.

`PRIORITY := <unsigned int>` is mandatory.
Lower is more urgent, `0` is the most urgent, and the value must fit in a 32-bit unsigned integer.
Omitting it is E1405; a value that does not parse is E1406.

`WITH <task>` on a program instance names a task declared in the same RESOURCE.

What is refused:

`TASK T(PRIORITY := 1)` A task with neither SINGLE nor INTERVAL triggers nothing.
E1410.

`INTERVAL := T#0ms` A zero period describes no cadence.
E1410.

`INTERVAL := <mutable global, %MW0, or an unknown name>` Not a compile-time period.
E1410.

`PROGRAM P1 : Prog;` No `WITH <task>`, so the instance would never run.
E1412.

`PROGRAM P1 WITH Nope : Prog;` The task name is not declared in this resource.
E1411.

`PROGRAM P1 WITH T : Missing;` Unknown program type.
E0203.

E1410 is only raised for a task a PROGRAM is actually bound to.
Declaring a task ahead of using it, including an unschedulable one, is clean — what must never be silent is a program that cannot run.

Duplicate names inside a resource: E0114 for a task, E0113 for a program instance, E0115 for a resource.

## Connections

A program instance can list connections after its type.
`:=` feeds a `VAR_INPUT` before every run, `=>` copies a `VAR_OUTPUT` out after it:

```iecst
PROGRAM Counter
VAR_INPUT
    pulse : BOOL;
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

The other end is an address as wide as the variable, a global of the same type, or, for an input, a constant.
Anything else in the list is E1428.

The task then runs the instance through `C1$__scan__`: the inputs copied in, the body, the outputs copied out.
The program's own code names no address, so the same program runs on two lines wired to different sensors.

## A function block on its own task

`fb WITH <task>` in the list gives one of the program's function block instances a task of its own:

```iecst
FUNCTION_BLOCK LowPass
VAR_EXTERNAL
    level : INT;
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
    alarm := filter.smooth > 500;
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

`Fast` runs `T1.filter` every millisecond through the block's body, `LowPass$__body__`, and `Slow` runs the rest of `T1`.
The program only reads the block's outputs: a program that also calls the block is E1428, since the task already runs it.

## VAR_CONFIG

`VAR_CONFIG` gives one instance its own starting values, by the variable's path from the resource, through the instances the program holds:

```iecst
FUNCTION_BLOCK Motor
VAR PUBLIC
    speed : INT := 2;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    x : INT := 1;
    drive : Motor;
END_VAR
END_PROGRAM

CONFIGURATION Plant
    VAR_CONFIG
        Main.L1.x           : INT := 10;
        Main.L1.drive.speed : INT := 20;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH Fast : Line;
        PROGRAM L2 WITH Fast : Line;
    END_RESOURCE
END_CONFIGURATION
```

`L1` starts with `x = 10` and `drive.speed = 20`, `L2` with its declarations' `1` and `2`.
Two instances of one program that differ in a setting need no second program and no input connected to a constant.

An entry repeats the variable's type.
E1426 when that type differs, when the path indexes an array or dereferences a reference, when another entry already sets the variable or an instance holding it, when the variable sits at an address a declaration names (it starts at that declaration's value), or when the entry targets the PROGRAM instance itself.

`VAR_CONFIG` also gives each instance its address for a variable declared `AT %I*`, `%Q*` or `%M*`: see `references/direct-variables.md`.

## How the schedule reaches the runtime

Context, not API.
`rk compile` writes the resolved schedule into the module as an `rk.schedule` custom section: the base tick, and for each task its resource, name, period in ticks, priority, and the program instances it runs with their state addresses.

The base tick (`common_ticktime_ns`) is the GCD of every task interval, and each task's `period_ticks` is its interval divided by that base — `T#10ms` and `T#20ms` give a 10 ms base with 1 and 2 ticks.

Tasks are ordered most urgent first.
A module with no CONFIGURATION carries no such section at all.

At run time the tick index is the wall clock's, not a count of the scans that ran: a wakeup that comes late runs one scan and the ticks it slept through are dropped, never replayed on stale inputs, so a late tick shifts no task's phase.

A host counts the dropped ticks since the last start and can report them.

## config.toml — the project file

Unrelated to the CONFIGURATION above.
It sits at the workspace root and describes the project, not the PLC logic.

```toml
[project]
name = "conveyor"
version = "0.1.0"

[settings]
opt_level = "2"
stack_size = 1048576

[settings.output]
directory = "build"

[linter]

[linter.rules]
unused-variable = false
global-without-external = true
```

`[project]` Required, with both `name` and `version` as strings.
A missing `[project]` section is an error.

`[settings] opt_level` WASM optimization level for release builds: `"0"`, `"1"`, `"2"`, `"3"`, `"4"`, `"s"`, `"z"`.

Defaults to `"2"`, and a `--opt-level` flag on `rk compile` wins over it.
Release builds use the `wasm-opt` on PATH, else a checksum-verified Binaryen downloaded once (see `cli-compile`); the debug artifact is never optimized, so this key does not touch it.

`[settings] stack_size` The size of the stack recursive calls push their frames on, in bytes: `1048576` for 1 MiB.
At most 4294967296, the 4 GiB a module can address.
Without it, the stack holds the largest frame and 64 KiB more.
A recursive function's frame larger than the stack is refused by `rk check` (`E1430`), and a stack that does not fit in 4 GiB after the rest of the memory stops the build with an error naming the setting.

`[settings.output] directory` Accepted by the schema and currently ignored — artifacts always land in `rk_build/debug/core.wasm` or `rk_build/release/core.wasm`.

`[linter]` Optional, and only for TUNING: the linter runs by default with its recommended rules whether or not this section exists.

`select` picks the baseline (`all` / `recommended` / `none`), `[linter.rules]` overrides individual rules on top of it.

See the `tool-linter` skill.

`[linter.rules]` Per-rule on/off, keyed by rule name.
Rules not listed default to enabled.

The schema denies unknown fields: a typo in a key is a hard error with a caret on the offending line, not a warning.

## Reference files

- `references/direct-variables.md` — the addresses `%I`, `%Q` and `%M`: declaring one with `AT`, the parts of a wider address, locating a function block's variables per instance, and what the host sees
