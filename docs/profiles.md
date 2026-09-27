# Profiles

There are two profiles: **debug** and **release**.

```schema
PROFILE debug                       rk compile
├─ your code, as written
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

A release build is optimized by [Binaryen](https://github.com/webassembly/binaryen), which rewrites the body of every function: instructions are merged, reordered or removed, and locals are reassigned.

Once this is done, there is no way to track what has changed, so the tables would point to the wrong place.

This is why a release build does not carry them: it is **watchable**, but not **steppable**.

> [!TIP]
> The missing line table is also how a runtime knows which kind of binary it was given.

## What stays the same

**The memory layout is identical between the two profiles.**

That is what lets you stop a release build, rebuild the same source as debug, and carry the live state across.

Variables can still be read and written by name in a release build, because `debug-symbols` points to the memory and not to the code.

> [!NOTE]Binaryen removes every function you never call.

## Optimization levels

`-O` takes `0` to `4`, `s` or `z`, and defaults to `2`.

> [!WARNING]
> `-O4` runs with `--skip-pass=flatten`: the Flatten pass of Binaryen does not support `try_table` yet, see [binaryen#8372](https://github.com/WebAssembly/binaryen/issues/8372).
>
> What is left of `-O4` is close to `-O3`.

`binaryen` is an external binary:

- If one is on your `PATH`, it is used.
- Otherwise, Binaryen 131 is downloaded once into your cache directory (`~/.cache/rk/binaryen` on Linux), and its checksum is verified.

See [Environment variables](environment-variables.md) to opt out of the download.

If the optimizer fails:

- `rk test -O` warns, and runs a correct unoptimized build.
- `rk compile --release` refuses outright.

> [!IMPORTANT]
> A release build never silently degrades.
