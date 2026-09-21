---
name: getting-started
description: Install rk and run the loop once — a workspace, a first program,
  check, test, compile. Use when rk is not set up yet, when starting a new
  project, or when unsure which skill to load next.
---

## Install

Prebuilt binaries are not published yet.
(soon!)
Verify the installation and see what it resolved:

```sh
rk --version
rk env          # the standard library and where it was found
```

If `rk env` reports no standard library, point `RK_STDLIB_PATH` at the checkout's `stdlib/`, or put that directory beside the binary.

## A workspace

A workspace is a directory with a `config.toml` and `.st` files anywhere under it.
The compiler finds them itself; how they are organised is up to you.

```sh
mkdir plant && cd plant
```

`config.toml`, the only required file:

```toml
[project]
name = "plant"
version = "0.1"
```

The first program, in `src/main.st`:

```iecst
FUNCTION Add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(a := 10, b := 20);
END_PROGRAM
```

## The loop

Check.
No output means clean; anything else is a diagnostic with a code, a location and the source around it:

```sh
rk check
```

A code you do not recognise: `rk explain E0301`.
The full reference is on this site under `/diagnostics/`.

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

Exit codes are stable either way.

Test.
A test is a function with the `{test}` pragma, asserting with `Std.Unit`:

```iecst continues
USING Std.Unit;

{test}
FUNCTION test_add
    ASSERT_EQ(value := Add(a := 2, b := 3), target := INT#5, message := 'Add(2, 3)');
END_FUNCTION
```

```sh
rk test            # every test
rk test add        # the ones whose name contains "add"
```

Format and compile:

```sh
rk fmt             # rewrites the .st files in place; --check only reports
rk compile         # the WebAssembly module, under rk_build/debug/
rk compile --release   # optimized, no stepping tables, under rk_build/release/
```

A `PROGRAM` only runs when a `CONFIGURATION` binds it to a task; without one the compiler says so.
The `programming-config` skill shows the three lines that do it.

## The module

`rk compile` writes one core WebAssembly module.
It imports its linear memory as `env.memory`, exports `__init`, one body per program and every FUNCTION marked `{export}`, and carries its task schedule, retained-state map and debug symbols as custom sections.
Any WebAssembly host can instantiate it; the ABI is documented in the repository's README.

## Environment

| Variable | Used by | Effect |
| --- | --- | --- |
| `RK_STDLIB_PATH` | CLI, language server | Where the standard library is. |
| `RK_NO_DOWNLOAD` | CLI | When set, Binaryen is never downloaded. |
| `NO_COLOR` | CLI | When set, the output is not colored. |

`RK_STDLIB_PATH` can also be written in a `.env` file at the workspace root.
The process environment wins when both are set, and it is the only variable read from that file.

## Where next

- `cli-compile` for the profiles, the optimizer and the module's ABI.

- `programming-st` for the language as `rk` compiles it.
- `programming-tests` before writing more than one test.
- `programming-config` to bind programs to tasks and share globals.
- `tool-linter` when a lint fires and its intent is unclear.
- `tool-lsp` for large workspaces, so an agent queries types and definitions instead of reading whole files.
