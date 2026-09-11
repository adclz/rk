---
name: programming-config
description: Declare how a program actually runs — CONFIGURATION, RESOURCE, TASK, intervals, priorities and VAR_GLOBAL. Use when wiring a PROGRAM to a task, setting a scan interval, or sharing globals between POUs.
---

## Summary

`CONFIGURATION` is where the PLC logic is declared: which programs exist as instances, how often each one is scanned, and which globals the application owns.

A `PROGRAM` on its own is only a type; it is compiled like a function block and nothing runs it.

Without a `CONFIGURATION` instantiating it the compiled module carries no schedule at all and the runtime falls back to calling a single exported entry.

A workspace has exactly one CONFIGURATION.

Blocks with the same name in different files are fragments of that one configuration and merge, which is what lets a library ship its `VAR_GLOBAL`s in their own file.
Only cyclic `INTERVAL` tasks run; `SINGLE` (event-driven) is refused.

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

`SINGLE := <event>` Event-driven tasks are not implemented.
E1410.
Only cyclic INTERVAL tasks run.

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

## Parsed but inert

These are accepted by the grammar and then do nothing.
Each of the first three reports E1416 so the silence is not mistaken for wiring.

`PROGRAM PA WITH T : A (inp := src, outp => snk)` Program connection lists are never resolved and emit no copy.
Assign in the program body instead.

`PROGRAM PA WITH T : A (fb WITH other_task)` Associating a nested function block with its own task.
Run it from the enclosing program's task.

`VAR_CONFIG PA.x : INT := 42; END_VAR` Resolved and type-checked against the instance's field, then discarded; the field keeps its declared value.
Set it in the program's own `VAR` section.
The resource-qualified path (`Res.PA.x`) and a location-only entry (`Res.PA.y AT %QB25 : BYTE;`) are accepted the same way.

`VAR_ACCESS acc : g : INT READ_WRITE; END_VAR` at configuration level parses and is stored, but nothing reads it and no diagnostic is emitted.
Do not rely on it.

## How the schedule reaches the runtime

Context, not API.
`rk compile` writes the resolved schedule into the module as an `rk.schedule` custom section: the base tick, and for each task its resource, name, period in ticks, priority, and the program instances it runs with their state addresses.

The base tick (`common_ticktime_ns`) is the GCD of every task interval, and each task's `period_ticks` is its interval divided by that base — `T#10ms` and `T#20ms` give a 10 ms base with 1 and 2 ticks.

Tasks are ordered most urgent first.
A module with no CONFIGURATION carries no such section at all.

At run time the tick index is the wall clock's, not a count of the scans that ran: a wakeup that comes late runs one scan and the ticks it slept through are dropped, never replayed on stale inputs, so a late tick shifts no task's phase.

The dropped ticks are counted since the last start and `rk list` shows them as `running, 12 ticks lost`.

## config.toml — the project file

Unrelated to the CONFIGURATION above.
It sits at the workspace root and describes the project, not the PLC logic.

```toml
[project]
name = "conveyor"
version = "0.1.0"

[settings]
opt_level = "2"

[settings.output]
directory = "build"

[linter]

[linter.rules]
unused-variable = false
global-without-external = true
```

`[project]` Required, with both `name` and `version` as strings.
A missing `[project]` section is an error.

`[settings] opt_level` WASM optimization level for release builds: `"0"`, `"1"`, `"2"`, `"3"`, `"s"`, `"z"`.

Defaults to `"2"`, and a `--opt-level` flag on `rk compile` wins over it.
Release builds require `wasm-opt` on PATH and fail without it; the debug artifact is never optimized, so this key does not touch it.

`[settings.output] directory` Accepted by the schema and currently ignored — artifacts always land in `rk_build/debug/core.wasm` or `rk_build/release/core.wasm`.

`[linter]` Optional, and only for TUNING: the linter runs by default with its recommended rules whether or not this section exists.

`select` picks the baseline (`all` / `recommended` / `none`), `[linter.rules]` overrides individual rules on top of it.

See the `tool-linter` skill.

`[linter.rules]` Per-rule on/off, keyed by rule name.
Rules not listed default to enabled.

The schema denies unknown fields: a typo in a key is a hard error with a caret on the offending line, not a warning.
