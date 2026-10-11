# Profiles

There are two profiles: **debug** and **release**.

```schema
PROFILE debug                       rk compile
├─ your code, as written
├─ name                             a function to its name
├─ debug-lines                      a code offset to its line
├─ debug-functions                  a function to its POU
├─ debug-locals                     a local to its variable
├─ test-manifest                    the {test} functions
├─ debug-symbols                    a variable to its address
├─ retain-map
├─ rk.schedule
└─ the memory layout                the same in both
PROFILE release                     rk compile --release
├─ your code, optimized by Binaryen
├─ name                             a function to its name
├─ debug-symbols                    a variable to its address
├─ retain-map
├─ rk.schedule
└─ the memory layout                the same in both
```

## Why a release build cannot be stepped

The stepping tables point **inside the code**:

- `debug-lines` maps a code offset to a file, a line and a column.
- `debug-functions` maps a WASM function index to a POU.
- `debug-locals` maps a WASM local slot to a variable.

A release build is optimized by [Binaryen](https://github.com/webassembly/binaryen).
Binaryen rewrites the body of each function:

- **Instructions** are merged, reordered or removed.
- **Locals** are reassigned.

The tables would then point to the wrong place.
A release build does not carry them.
It is **watchable**, but not **steppable**.

> [!TIP]
> The missing line table is also how a runtime knows which kind of binary it was given.

## What stays the same

**The memory layout is identical between the two profiles.**

You can stop a release build, rebuild the same source as debug and carry the live state across.

You can still read and write a variable by name in a release build.
`debug-symbols` points to the memory, not to the code.

The names of the functions stay as well.
A trap in a release build names the function it happened in, not a number.

Binaryen can change which function that is:

- **An inlined function** is reported as its caller.
- **Two functions with the same body** become one, under one of the two names.

> [!NOTE]
> Binaryen removes every function you never call.

## Optimization levels

`-O` takes `0` to `4`, `s` or `z`.
The default is `2`.

```sh
rk compile --release -O z
```

> [!WARNING]
> `-O4` runs with `--skip-pass=flatten`.
> The Flatten pass of Binaryen does not support `try_table` yet, see [binaryen#8372](https://github.com/WebAssembly/binaryen/issues/8372).
>
> What is left of `-O4` is close to `-O3`.

`binaryen` is an external binary:

- If one is on your `PATH`, it is used.
- Otherwise, Binaryen 131 is downloaded once into your cache directory (`~/.cache/rk/binaryen` on Linux).
  Its checksum is verified.

See [Environment variables](environment-variables.md) to opt out of the download.

If the optimizer fails:

- `rk test -O` warns, then runs the tests on the unoptimized build.
- `rk compile --release` refuses outright.

> [!IMPORTANT]
> A release build never silently degrades.
