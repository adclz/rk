# Getting started

This page takes you from an empty folder to a compiled module: you write a program, give it a task, check it, test it and compile it.

```diagram
 .st files ──► rk check ──► rk test ──► rk compile ──► core.wasm
```

## Install

Prebuilt binaries are not published yet.

Once `rk` is on your `PATH`, check what it found:

```sh
rk --version
rk env          # the standard library and where it was found
```

If `rk env` finds no standard library, set `RK_STDLIB_PATH` to the repository's `stdlib/`, see [Environment variables](environment-variables.md).

## Create a workspace

A workspace is a folder with a `config.toml` at its root:

```toml
[project]
name = "plant"
version = "0.1"
```

Every `.st` file under that folder is part of the program, in any subfolder you like.

## Write a program

A program only runs when a configuration gives it a task.
Write both, in `src/main.st` for example:

```iecst
FUNCTION Next : DINT
VAR_INPUT
    value : DINT;
END_VAR
    Next := value + 1;
END_FUNCTION

PROGRAM Counter
VAR
    count : DINT;
END_VAR
    count := Next(value := count);
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM C1 WITH Fast : Counter;
    END_RESOURCE
END_CONFIGURATION
```

The task `Fast` runs `C1`, an instance of `Counter`, every 10 ms.
[Configuration](configuration.md) shows how to run more programs, at other rates.

## Check it

```sh
rk check
```

No output means no error.
Otherwise each diagnostic comes with a code, and `rk explain E0301` tells you what one means.
The [diagnostics](https://rk.clauzeladrien2170.workers.dev/diagnostics) page lists them all, with an example each.

## Test it

Mark any `FUNCTION` with `{test}` and it becomes a test, checking its results with `Std.Unit`.
Add one to the same file:

```iecst continues
USING Std.Unit;

{test}
FUNCTION test_next
    ASSERT_EQ(value := Next(value := 41), target := DINT#42, message := 'Next(41)');
END_FUNCTION
```

Then run every test, or the ones whose name contains a word:

```sh
rk test
rk test next
```

## Compile it

```sh
rk fmt                 # formats every .st file in place
rk compile             # rk_build/debug/core.wasm
rk compile --release   # rk_build/release/core.wasm, optimized
```

The module runs on any WebAssembly runtime.
[WASM ABI](wasm-abi.md) shows how a host loads it and calls it, and [Profiles](profiles.md) what differs between the two builds.
