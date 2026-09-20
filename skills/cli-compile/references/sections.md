## Custom sections

A module carries its own tables, so a host can read and write variables by name instead of by address.
They are stored as custom sections after the code, encoded with MessagePack; the `debug_format` crate decodes them.

| Section | Present in | Carries |
| --- | --- | --- |
| `debug-symbols` | every build | every elementary leaf: path, address, size, type |
| `debug-functions` | debug only | wasm function index → POU name |
| `debug-lines` | debug only | code offset → file, line, column |
| `debug-locals` | debug only | wasm local slot → variable name and type |
| `rk.schedule` | every build | tasks, periods, priorities, instance addresses |
| `retain-map` | every build | the byte ranges a power cycle must preserve |
| `test-manifest` | when there are tests | the exports `rk test` calls |

The last three are not debug information.
If `retain-map` is dropped, every `RETAIN` variable silently becomes transient.

Paths are resolved through arrays and struct fields without enumerating them, so `pts[7423].history[2].y` is a single lookup, and writes go back the same way.

The `programming-config` skill describes what `rk.schedule` holds and how a host reads the ticks.

rk emits the tables; the scan loop, the monitoring session and the debug adapter belong to a runtime.

## The exception

Everything the compiler checks raises one exception, and it carries a message.

A module declares a single exception tag, `(i32, i32)`: the pointer and the length of a STRING in the memory the host provided.
The tag is never exported, so a host reads it from the pending-exception slot.

`__RAISE(message)` is the only throw, and a raise always leaves the module.
The one catcher a module contains is the wrapper around a `{test}` FUNCTION, which is why a failing assertion is reported rather than fatal.

A runtime therefore needs the WebAssembly exception-handling proposal: `rk test` enables it on its own wasmtime host.

The `programming-st` skill's `references/runtime.md` lists the checks the compiler inserts and their messages.
