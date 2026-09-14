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

## Strings

## Math operations

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

## Profiles

## License

rk is distributed under [AGPL-3.0-only](LICENSE).
For the Apache-2.0 exceptions, and the permission that makes every generated module yours, see [LICENSING.md](LICENSING.md).
