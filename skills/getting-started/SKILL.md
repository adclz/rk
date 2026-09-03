---
name: getting-started
description: Install rk and run the loop once — a workspace, a first program,
  check, test, compile, run. Use when rk is not set up yet, when starting a new
  project, or when unsure which skill to load next.
---

## What rk is

`rk` is an agent-first platform for industrial automation. One binary takes
a PLC program written in IEC 61131-3 Structured Text through the whole
loop: check it, test it, compile it to WebAssembly, run it on this machine,
deploy it to a controller, debug it there. Modbus and MQTT are in the
standard library. Everything is a CLI command with a stable exit code, so
an agent can drive it without a human in the loop.

## Install

Binaries are published on this site, for Linux, macOS and Windows, under
`/download`. Put the one for your platform on `PATH` as `rk`. Package
managers (`brew`, `apt`, `winget`, `cargo`) come next; until then the site
is the source.

Verify the installation and see what it resolved:

```sh
rk --version
rk env          # the standard library and where it was found
```

If `rk env` reports no standard library, the archive was unpacked without
its `lib/` directory next to the binary. Unpack it whole.

## A workspace

A workspace is a directory with a `config.toml` and `.st` files anywhere
under it. The compiler finds them itself; how they are organised is up to
you.

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

Check. No output means clean; anything else is a diagnostic with a code,
a location and the source around it:

```sh
rk check
```

A code you do not recognise: `rk explain E0301`. The full reference is on
this site under `/diagnostics/`.

Test. A test is a function with the `{test}` pragma, asserting with
`Std.Unit`:

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

Format, compile, run:

```sh
rk fmt             # rewrites the .st files in place; --check only reports
rk compile         # the WebAssembly module, under rk_build/
```

A `PROGRAM` only runs when a `CONFIGURATION` binds it to a task; without
one the compiler says so. The `programming-config` skill shows the three
lines that do it.

## Where next

- `programming-st` for the language as `rk` compiles it.
- `programming-tests` before writing more than one test.
- `programming-config` to bind programs to tasks and share globals.
- `tool-linter` when a lint fires and its intent is unclear.
- `tool-lsp` for large workspaces, so an agent queries types and
  definitions instead of reading whole files.
