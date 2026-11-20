---
name: cli-compile
description: Compile a workspace to a WebAssembly module with `rk compile` — the debug and release profiles, the optimization levels, and where wasm-opt comes from. Use when asked to build, to produce a release binary, or when an optimized build fails.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Compiles the workspace to one WebAssembly component: a core module, wrapped with the WIT world that types what it imports and exports.

A workspace with errors does not compile, and `rk check` reports the same diagnostics without writing anything.
The linter does not run here, so a lint can never block a build.
A library (`type = "library"` in its `config.toml`) is refused: it is built into the programs that list it, and `rk check` and `rk test` take it in its own folder.

The component is written to `rk_build/debug/<project>.wasm`, or to `rk_build/release/<project>.wasm` with `--release`.
It carries the types of what it imports and exports, so no WIT is written beside it: `wasm-tools component wit <project>.wasm` prints them.
The components of the libraries the program calls go beside it, in `components/`, each named after its library: the runtime runs them beside the program and links its imports to them. The folder holds the last build's only.

## Usage

`--release` The release profile: optimized by wasm-opt, stepping tables omitted.

`-O, --opt-level <OPT_LEVEL>` The optimization level of a release build: `0` to `4`, `s` (size) or `z` (aggressive size).
Defaults to `2`, or to `opt_level` in `config.toml`; the flag wins.

`-o, --output <OUTPUT>` Where to write the component.

`-w, --watch` Re-compile on every file change.

`--workspace <WORKSPACE>` Workspace path, `.` by default.

`-v, --verbose` Verbose output.

## Profiles

There are two profiles, debug and release.

| | `rk compile` | `rk compile --release` |
| --- | --- | --- |
| stepping tables | yes | no |
| tests | yes | no |
| symbols, retain map, schedule | yes | yes |
| optimized by wasm-opt | never | always |

The stepping tables point inside the code: `debug-lines` maps a code offset to a line, `debug-functions` a WASM function index to a POU, `debug-locals` a WASM local slot to a variable.
A release build is optimized by Binaryen, which rewrites the body of every function, so those tables would point to the wrong place.
This is why a release build does not carry them: it is watchable, but not steppable.

The memory layout is identical between the two profiles.
That is what lets a host stop a release build, rebuild the same source as debug, and carry the live state across.
Variables can still be read and written by name in a release build, because `debug-symbols` points to the memory and not to the code.

Every build keeps only what a host calls and what that reaches (`__init`, the bodies the schedule runs, the `{export}` FUNCTIONs and the tests), so a function nothing calls, the stdlib's included, is in neither profile.
What a release build saves on top is the dropped tables, and what Binaryen's optimization takes off the code.
A release build carries no test either: the tests are compiled, so the layout does not move, but they are not exported and the `test-manifest` section is left out.
`rk test` always builds the debug module, and `rk test -O` optimizes that one.

## wasm-opt

`wasm-opt` is an external binary.
If one is on `PATH`, it is used.
Otherwise Binaryen 131 is downloaded once into the user cache directory (`~/.cache/rk/binaryen` on Linux), and its checksum is verified.
Setting `RK_NO_DOWNLOAD` to any value opts out of the download.

`-O4` runs with `--skip-pass=flatten`: the Flatten pass of Binaryen does not support `try_table` yet (WebAssembly/binaryen#8372).
What is left of `-O4` is close to `-O3`.

If the optimizer fails, `rk compile --release` refuses outright, where `rk test -O` warns and runs a correct unoptimized build.
A release build never silently degrades.

## Reference files

Read the one that covers the question; each is self-contained.

- `references/abi.md` — what the module imports and exports, how each ST type crosses a call boundary, and how to call an export
- `references/sections.md` — the custom sections: symbols, schedule, retain map, test manifest, and the exception a module raises
