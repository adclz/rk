# Debug Symbols

A module carries its own symbol table, so a host can read and write variables **by name** instead of by address.

All these tables are stored as custom sections, encoded with [MessagePack](https://msgpack.org/).

| Section | Present in | Carries |
|---|---|---|
| `debug-symbols` | every build | every elementary leaf: path, address, size, type |
| `debug-functions` | debug only | wasm function index → POU name |
| `debug-lines` | debug only | code offset → file, line, column |
| `debug-locals` | debug only | wasm local slot → variable name and type |
| `rk.schedule` | every build | tasks, periods, priorities, instance addresses |
| `retain-map` | every build | the byte ranges a power cycle must preserve |
| `test-manifest` | debug only, when there are tests | the exports `rk test` calls |

> [!WARNING]
> The last three are **not** debug information.
>
> If you drop `retain-map`, every `RETAIN` variable silently becomes transient.

Paths are resolved through arrays and struct fields without enumerating them.
So `pts[7423].history[2].y` is a single lookup, and writes go back the same way.

## Versions

Each table carries a version, and a runtime can be older than the compiler that built a module.
A reader built from `debug_format` skips the fields it does not know, so a newer module still loads.
`DebugInfo::problems()` then names each table that is newer than the reader, or that it could not read at all.

> [!NOTE]
> **Rk ships no debugger.**
>
> It emits the tables, and the `debug_format` crate decodes them.
> The scan loop, the monitoring session and the debug adapter belong to a runtime.
