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
An element of a multi-dimensional array is written `m[1, 2]`.
The reader also accepts `m[1][2]`.

## Pointers

`debug-symbols` lists each pointer with the type it points at:

- **A `REF_TO`** variable or member.
- **A `VAR_IN_OUT`** member of an instance, of an aggregate type. A scalar one is a value the instance holds.

Write `^` to go through a pointer, as in the program:

```st
P1.head^.value
P1.a.next^.next^.value
```

A `VAR_IN_OUT` reads as the variable it is bound to.
`P1.m.pos` and `P1.m.pos^` are the same.

An `ARRAY[*]` member keeps the address of the array it is bound to, and no bounds.
Two arrays can start at one address: a matrix and its first row, or an array of STRUCTs and the array its first element begins with.
So `debug-symbols` gives the member its rank and its element.
It reads as the array of that rank and element that starts at the address:

- **A whole array.**
- **The last dimensions of one**, as `mat[0]` is a row of `mat`.
- **An array in an element or a member**, as `recs[2].hist`.

The result carries that array's bounds, which list its elements.
An array among the locals of a function is not searched.
The path stops at the pointer there.

Following a pointer needs the memory of the running module.
`DebugInfo::follow` takes a reader of it.
A null pointer stops the path.
The result names the pointer.

The value of a pointer is the address it holds and the variable at that address.

`debug-locals` lists the pointers a function holds in wasm locals:

- **A `VAR_IN_OUT` or `VAR_OUTPUT`** parameter.
- **A `REF_TO`** input or local.
- **An aggregate input.** It reads as the input itself.

An `ARRAY[*]` parameter has the type of the array its copy of the function was compiled for, bounds included.

## Recursive calls

A recursive function keeps its arrays, structs and strings in a frame.
Each call pushes one.
A wasm local holds where the frame starts.

`debug-locals` gives:

- **The local** that holds the start of the frame.
- **The size** of the frame.
- **Each variable** at its offset from the start.

A debugger stopped in such a call reads the local.
It then resolves a path with `DebugInfo::resolve_in_frame` or `follow_local`.

`debug-symbols` gives the region of the stack the frames are pushed on.
An address in that region belongs to a call.
`frame_variable_at` names it from the live frames.
A reference to a STRUCT is named as the STRUCT, not as its first field at the same address.

## Versions

Each table carries a version.
A runtime can be older than the compiler that built the module.
A reader built from `debug_format` skips the fields it does not know.
The newer module still loads.
`DebugInfo::problems()` names each table the reader could not read, or that is newer than the reader.

> [!NOTE]
> **Rk ships no debugger.**
>
> It emits the tables, and the `debug_format` crate decodes them.
> The scan loop, the monitoring session and the debug adapter belong to a runtime.
