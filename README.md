Rk is a Structured Text toolchain that emits WebAssembly, focused on strictness and portability.

The emitted WebAssembly can be executed by any WASM runtime, although some imports of the std lib require a WASI P1 compliant runtime.

See the diagnostics for error codes.

*Some technical decisions diverge from standard implementations of ST, the summary below shows some the most significative ones*

- [Semicolons](#semicolons)
- [Case sensitivity](#case-sensitivity)
- [Namespaces](#namespaces)
- [Overloading](#overloading)
- [Monomorphized OOP](#monomorphized-oop)
- [References](#references)
- [Tests](#tests)
- [Pragmas](#pragmas)
- [StdLib](#stdlib)
- [Bundled Traps](#bundled-traps)
- [Strings](#strings)
- [Math operations](#math-operations)
- [WASM ABI](#wasm-abi)
- [Debug Symbols](#debug-symbols)
- [Profiles](#profiles)
- [License](#license)

## Semicolons

The compiler accepts missing semiclons `;`.
The formatter writes the missing ones in.

## Case sensitivity

Keywords and identifiers are case-insensitive: `myFn` and `MyFn` are one name.

## Namespaces
**A file IS NOT a single POU**.
A file can contain as many POUs as you want, as long as you give them a different name, and folders do not affect name resolution.

> There is no limitation in how you want to organize your workspace, so feel free to split your code the way you like.

```
FUNCTION MyFn END_FUNCTION
FUNCTION MyFn END_FUNCTION // Not Ok
```

can be fixed by putting the second in a `NAMESPACE`

```
FUNCTION MyFn END_FUNCTION // Global

NAMESPACE MyNamespace
    FUNCTION MyFn END_FUNCTION // Is now MyNamespace.MyFn
END_NAMESPACE
```

While the first MyFn stays **global** across the workspace, the second one is now **MyNamespace.MyFn**

To access it:

```
USING MyNamespace // <-- Import the namespace and MyFn
```

`USING` directives can be used inside Namespaces or POUs


```
USING Namespace <-- Ok

NAMESPACE MyNs
    USING AnotherNamespace <-- Also Ok

    FUNCTION MyFn
        USING YetAnotherNamespace <-- Still Ok

    END_FUNCTION
END_NAMESPACE
```

__The compiler will kindly tell you if an imported POU has the same name as a local one__

OR

```pascal
FUNCTION MyFn
    MyNamespace.MyFn // <-- Qualify the full path
END_FUNCTION
```

Globals are shared everywhere, and Namespaces are partial, so they can be defined across multiple files, and will be merged automatically.

```pascal
// file1.st
NAMESPACE MyNs
    FUNCTION MyFn END_FUNCTION
END_NAMESPACE
```

```pascal
// file2.st
NAMESPACE MyNs
    FUNCTION MyFn END_FUNCTION // <-- will trigger a duplicate error, because MyNs.MyFn already exists in file1.st
END_NAMESPACE
```

There is no limitation for namespace nesting.

```pascal
NAMESPACE MyNs

    NAMESPACE MyNs2

        NAMESPACE MyNs3

            FUNCTION MyFn END_FUNCTION // <-- Is MyNs.MyNs2.MyNs3.MyFn

        END_NAMESPACE

    END_NAMESPACE

END_NAMESPACE
```

## Overloading

The IEC standard defines generics for **ANY_INT**, **ANY_MAGNITUDE** etc ...
Those are normally reserved for the standard library, but it turns out that using overloads can mimic this system by writing each possible variant.

This comes with the advantage that the compiler does not need extra plumbing for these generics, because the behavior is implemented directly in **ST**.

__Overloading can only be used on **FUNCTION**__


```pascal
FUNCTION MyFn
    VAR_INPUT
        Input: INT;
    END_VAR
END_FUNCTION

FUNCTION MyFn  // <-- Legal
    VAR_INPUT
        Input: REAL;
    END_VAR
END_FUNCTION
```

And then on usage:

```pascal
MyFn(0) <-- Will pick the first overload
MyFn(1.0) <-- Will pick the second overload
```


> **Return type** affects the signature of overloads.


## Monomorphized OOP

All Object Oriented Programming concepts are implemented (`CLASS`, `METHOD` ...).

The only point where we *voluntarily* diverge from the standard, is about `INTERFACE`.

You can use them to define contracts for inheritance, but defining them as variable types has limitations.

The compiler resolves everything at compile time and uses **monomorphization** to know exactly what an interface is.

So:

- You can pass `INTERFACE` as a `VAR_IN_OUT` or `VAR_INPUT` parameter, in both cases an `INTERFACE` is passed as a reference

- You can **only** define an `INTERFACE` parameter in a FUNCTION or a METHOD.

- An `ARRAY` OF `INTERFACE` is not allowed.


## References

`REF_TO`, `REF()`, `^` and `NULL` as in the standard.

A reference that may be `NULL` cannot be dereferenced: the compiler refuses `ptr^` unless it can prove `ptr` is set.

```pascal
FUNCTION fn1 : INT
    VAR
        x: INT := 1;
        ptr: REF_TO INT := NULL;
    END_VAR

    fn1 := ptr^;                // Not Ok, ptr may be NULL

    IF ptr <> NULL THEN
        fn1 := ptr^;            // Ok, guarded
    END_IF;

    IF ptr = NULL THEN
        RETURN;
    END_IF;
    fn1 := ptr^;                // Ok, the early RETURN guards everything below
END_FUNCTION
```

A guard narrows only the reference it tests, only inside its branch, and `ptr := REF(x)` narrows too.

`VAR_INPUT` and `VAR_IN_OUT` references are trusted: the caller is responsible for them.

## Tests

A test is a `FUNCTION` marked `{test}`, asserting with `Std.Unit`.

`rk test` compiles the workspace and runs them; `rk test add` runs those whose name contains `add`.

```pascal
USING Std.Unit;

{test}
FUNCTION test_add
    ASSERT_EQ(Add(a := 2, b := 3), INT#5, 'Add(2, 3)');
END_FUNCTION
```

- An assertion that fails raises, and that is what fails the test, because internally `ASSERT_EQ` uses the built-in `__RAISE` that triggers a WASM exception.
  
Every test gets fresh instances, so tests never leak state into each other.

## Pragmas

Only these exist; anything else in braces is a syntax error, so `{attribute '…'}` from other toolchains cannot be copied in.

- `{test}` marks a `FUNCTION` the test runner calls.
- `{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.
- `{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU.
- `{allow 'rule' 'rule'}` silences lint rules: above a POU for the whole POU, as a statement for the next statement.
- `{extern 'module' 'name'}` declares a `FUNCTION` as a WASM import; the declaration is the signature.
- `{wasm 'instruction' (params a b) (result r)}` is a statement that emits one WASM instruction on the named operands.

## StdLib

Ordinary Structured Text in `stdlib/`, one namespace per file: `Std.Math`, `Std.Strings`, `Std.Timers`, `Std.Counters`, `Std.Unit`, ... and the rest.

The compiler knows nothing about it beyond where the files are.

So that means you can replace any part of the stdlib, extend it, or read it to see how a `TON` is written.

## Bundled Traps

Everything the compiler checks raises **one** exception, and it carries a message.

A module declares a single exception tag, `(i32, i32)`: the pointer and length of a STRING in the memory you provided. It is never exported, so a host reads it from the pending-exception slot.

`__RAISE(message)` is the only throw. There is no `TRY`, no `CATCH`, no `ON ERROR` — a raise always leaves the module.

> The one catcher an emitted module contains is the wrapper around a `{test}` function, which is why a failing assertion is reported rather than fatal.

Three checks are inserted for you:

| Inserted at | Message |
|---|---|
| every array subscript, per dimension | `array index out of bounds` |
| every subrange store | `value out of subrange bounds` |
| every `^` you wrote | `dereference of a null reference` |

What the compiler can prove is refused at compile time instead, and costs nothing at runtime.

__And three checks that do not exist.__

- **Integer overflow is never checked.** `DINT#2147483647 + 1` is `-2147483648`, silently, at every width.
- **Division by zero is not ours.** It is the VM's own trap: no message, and a `{test}` cannot catch it.
- **STRING capacity is not checked.** It truncates. See [Strings](#strings).

`REAL#1.0 / 0.0` is `+inf` and the scan continues. Nothing faults on NaN or infinity.

A Rust panic inside a grafted builtin arrives as the same exception, with the panic text as the message. None of this depends on the profile.

## Strings

One string type, **UTF-8**. There is no `WSTRING` and no `WCHAR`.

A slot is a 4-byte length followed by its capacity in bytes, so a plain `STRING` occupies 84.

```pascal
s1: STRING;      // 80 bytes of buffer
s2: STRING[5];   // 5 BYTES, not characters — 'café' needs exactly this
```

`LEN` is bytes and O(1). Every operation exists twice: `LEFT`, `MID`, `FIND` count bytes, `CHAR_LEFT`, `CHAR_MID`, `CHAR_FIND` count characters. On ASCII they agree, and the byte family is faster.

- A **literal** too long for its destination is a compile error.
- A **variable** too long truncates silently, and truncating bytes can split a character. `IS_UTF8` exists for exactly that.

No indexing — `s[1]` is `E0508`, use `CHAR_AT`. No `+` — use `CONCAT`.

Comparison is byte-lexicographic, so `'Z' < 'a'`, and a `STRING` is a legal `CASE` label.

`CHAR` is a code point in 4 bytes. It does **not** widen to `STRING`; `CHAR_TO_STRING` does that.

> At a call boundary a `STRING` input or return is a borrowed `(ptr, len)`, never a copy. A `VAR_IN_OUT` or `VAR_OUTPUT` is instead `(addr, capacity)`, so the callee's writes clamp.

## Math operations

Every maths function is in the module. **It imports nothing but `env.memory`.**

`+ - * / MOD` are single wasm instructions, and so are `SQRT` and `ABS`. The eleven that have no instruction — `SIN COS TAN ASIN ACOS ATAN ATAN2 EXP LN LOG` and `**` — are grafted in from libm when you call them, in `REAL` and `LREAL` form. Nothing is imported, so nothing has to be wired up by the host.

Arithmetic is wasm arithmetic:

- 8- and 16-bit widths wrap by explicit masking; 32- and 64-bit wrap silently.
- `NaN` and `±inf` are ordinary values. `SQRT(-1)` is NaN, `LN(0)` is `-inf`, nothing faults.
- Float to integer **saturates**, so NaN converts to `0`.
- `IS_NAN` is ordinary ST: `IN <> IN`.

`**` needs a float base and returns the base's type; `EXPT` is the same code path.

There is no `ANY_INT` or `ANY_REAL` in the type system. Each generic is an overload set written out in ST, which is why `Std.Math` is readable and replaceable.

Implicit casts follow the standard's table, which is stricter than most toolchains:

```pascal
r := i;    // INT to REAL, Ok
r := d;    // DINT to REAL, E0301 — the standard's table omits it
i := r;    // never implicit; the error names the cast for you
```

> A bare literal expression computes at the literal's default type, then widens. `x : LREAL := 0.1 + 0.0` is REAL arithmetic. Write `LREAL#0.1 + 0.0`.

## WASM ABI

A workspace compiles to one core module.
It imports its linear memory as `env.memory`, exports `__init` to set cold-start values, one body per POU, and the retained and global bands as `retain_base`/`retain_size` and `globals_base`/`globals_size`.
Tasks, retained state and debug symbols travel as custom sections.

| ST | at a call boundary | in memory |
|---|---|---|
| `BOOL`, `SINT`…`DINT`, `USINT`…`UDINT`, `BYTE`, `WORD`, `DWORD`, `CHAR`, `TIME`, `DATE`, `TOD`, `DT` | `i32` | 1, 2 or 4 bytes |
| `LINT`, `ULINT`, `LWORD`, `LTIME`, `LDATE`, `LTOD`, `LDT` | `i64` | 8 bytes |
| `REAL` / `LREAL` | `f32` / `f64` | 4 / 8 bytes |
| enum, subrange | the lane of its storage type | as its storage type |
| `STRING` | `(ptr, len)`, two `i32` | 4-byte length, then `capacity` bytes of UTF-8 |
| `STRUCT`, `ARRAY`, FB instance | `i32` pointer | laid out in place |
| `REF_TO T` | `i32` address; `NULL` is `0` | 4 bytes |

Calling an export:

- `__init` first, `() -> ()`.
- A `PROGRAM` body takes its instance address: `(i32) -> ()`.
- A `FUNCTION` takes `VAR_INPUT` in declaration order, scalars by value, then one pointer per `VAR_IN_OUT` and `VAR_OUTPUT`; the return type is the result, a `STRING` result is `(ptr, len)`, an aggregate result is a pointer to the callee's slot to copy out of.
- Everything lives in the memory you provided; the first 16 KiB are reserved.

A host import declared with `{extern}` is the same convention in reverse: 
- `VAR_INPUT` are the parameters, 
- scalar `VAR_OUTPUT` the results, the return type last; 

it takes copies, so `VAR_IN_OUT`, aggregate outputs and a `STRING` return are refused.

## Debug Symbols

A module carries its own symbol table, so a host reads variables **by name** rather than by address.

| Section | Present in | Carries |
|---|---|---|
| `debug-symbols` | every build | every elementary leaf: path, address, size, type |
| `debug-functions` | debug only | wasm function index → POU name |
| `debug-lines` | debug only | code offset → file, line, column |
| `debug-locals` | debug only | wasm local slot → variable name and type |
| `rk.schedule` | every build | tasks, periods, priorities, instance addresses |
| `retain-map` | every build | the byte ranges a power cycle must preserve |
| `test-manifest` | when there are tests | the exports `rk test` calls |

All MessagePack.

The last three are **not** debug information. Drop `retain-map` and every `RETAIN` variable silently becomes transient.

Paths resolve through arrays and struct fields without enumerating them, so `pts[7423].history[2].y` is one lookup, and writes go back the same way.

`debug-lines` is absent from a release build, and that absence *is* the steppability answer: watchable, not steppable.

> **rk ships no debugger.** It emits the tables and `debug_format` decodes them. The scan loop, the monitoring session and the debug adapter belong to a runtime.

## Profiles

Two, and they differ **only in which sections ride along**. The code is the same.

| | `rk compile` | `rk compile --release` |
|---|---|---|
| stepping tables | yes | no |
| symbols, retain map, schedule | yes | yes |
| wasm-opt | never | always |

A release build is watchable but not steppable, and the missing line table is how a runtime knows which artifact it was handed.

**Memory layout is identical between the two.** That is what lets you stop a release build, rebuild the same source as debug, and carry live state across. It is asserted, not assumed.

`-O` takes `0`-`3`, `s` or `z`, and defaults to `2`. `-O4` is refused: Binaryen's Flatten pass still aborts on the `try_table` that every raise emits.

wasm-opt is an external binary — whatever is on `PATH`, else a checksum-verified Binaryen downloaded once into your cache. `RK_NO_DOWNLOAD=1` opts out.

> `rk test -O` warns and runs a correct unoptimized build if the optimizer fails. `rk compile --release` refuses outright. A release build never silently degrades.

Most of what a release saves is dropped tables, not optimized code.

## License

rk is distributed under [AGPL-3.0-only](LICENSE).
For the Apache-2.0 exceptions, the permission that makes every generated module yours, and commercial licensing, see [LICENSING.md](LICENSING.md).
