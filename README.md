# Rk 'Rukbat'

> Or __Alpha Sagittarii__  💫

<div align="center" style="font-weight: bold"><strong>Rk</strong> is an IEC-61131-3 Structured Text toolchain that compiles to WebAssembly, focused on strictness and portability.</div>
<br>

 - See [diagnostics](https://rk.clauzeladrien2170.workers.dev/diagnostics) for error codes.

 - See [official documentation](https://rk.clauzeladrien2170.workers.dev) to learn about _**formatter**_ and _**linter**_.


> [!NOTE]
> Several technical decisions diverge from  classic implementations of ST, the summary below shows some of the most significant ones.
>
> To learn more about `CONFIGURATION`, `PROGRAM`, and other types: `ARRAY`, `STRUCT`, `ENUM`... read the skills.

## Why WebAssembly ?

### **Portable** 
One unique binary that can be loaded in any runtime.

### **Sandboxed** 
WebAssembly modules are sandboxed by default, errors are caught but do not propagate to the host.

### **Agnostic host** 
In any language, on any [platform](https://withbighair.com/webassembly/2025/05/11/Runtime-choices.html) with a WASM runtime.

## The compiler in a nutshell

### **Text only**

No limitations on how your code can be organized, see [Namespaces](#namespaces)

### **Highly strict**
  
Built with strictness as its core, with 220+ diagnostics.

### **One core module, with memory dedicated once** 
  
No allocator, no GC, nothing calls `memory.grow`, so the footprint is settled at compile time.

### **The core logic only** 
  
Tasks, retained ranges, symbols and tests ride as custom sections after the code.

### **No dependency on ABI glue** 

The code does not depend on the WASI Component model, read the [WASM ABI](#wasm-abi) to see how data can be exchanged.

### **Bundled traps** 
  
Array, subrange bounds, and dereference checks are generated inside the binary, once spotted, they trigger a **WebAssembly exception**.

> [!IMPORTANT]
> Other checks such as division by zero are handled by the WASM runtime itself.

### **Lightweight** 
  
Rk does not depend on any compiler backend except wasm-encoder. This makes the compiler very lightweight __(28 MB for the whole executable)__.
  
> [!TIP]
> The CLI provides [binaryen](https://github.com/webassembly/binaryen) as an optional tool you can use to optimize binaries, see [Profiles](#profiles).

<hr>

<!-- no toc -->
- [Syntax](#syntax)
- Programming basics
  - [Namespaces](#namespaces)
  - [Strict casts](#strict-casts)
  - [Overloading](#overloading)
  - [Monomorphized OOP](#monomorphized-oop)
  - [References](#references)
- Extras
  - [Tests](#tests)
  - [Pragmas](#pragmas)
- Internal behavior
  - [Bundled Traps](#bundled-traps)
  - [Strings](#strings)
  - [Math operations](#math-operations)
- [StdLib](#stdlib)
- [WASM ABI](#wasm-abi)
- [Debug Symbols](#debug-symbols)
- [Profiles](#profiles)
- [Environment variables](#environment-variables)
- [License](#license)

## Syntax

- Unlike traditional ST compilers, semicolons `;` are not mandatory.

The following program can compile without any problem:
```st
FUNCTION MyFn
    VAR
        test: INT
        test2: INT
    END_VAR

    IF test > test2 THEN

    END_IF
END_FUNCTION

```

The formatter writes the missing ones in.

- Keywords and identifiers are case-insensitive: `myFn` and `MyFn` are one name.

- Enum values are always qualified: `Color#Green`. A bare `Green` is `E0201`.

## Programming basics

### Namespaces

**Rule of thumb:** A file IS NOT a single POU.

A file can contain as many POUs as you want, as long as you give them a different name,
and folders do not affect name resolution.

> [!TIP]
> There is no limitation in how you want to organize your workspace, so feel free to split your code the way you like.


```st
FUNCTION MyFn END_FUNCTION
FUNCTION MyFn END_FUNCTION // Not Ok
```

This can be fixed by putting the second in a `NAMESPACE`

```st
FUNCTION MyFn END_FUNCTION // Global

NAMESPACE MyNamespace
    FUNCTION MyFn END_FUNCTION // Is now MyNamespace.MyFn
END_NAMESPACE
```

While the first MyFn stays **global** across the workspace, the second one is now **MyNamespace.MyFn**

To access it:

```st
USING MyNamespace // <-- Import the namespace and MyFn
```

`USING` directives can be used inside Namespaces or POUs


```st
USING Namespace <-- Ok

NAMESPACE MyNs
    USING AnotherNamespace <-- Also Ok

    FUNCTION MyFn
        USING YetAnotherNamespace <-- Still Ok

    END_FUNCTION
END_NAMESPACE
```

Or you can qualify the full path if you do not want to rely on USING directives:

```st
FUNCTION MyFn
    MyNamespace.MyFn // <-- Qualify the full path
END_FUNCTION
```

__The compiler will kindly tell you if an imported POU has the same name as a local one__

```st
NAMESPACE ns1
    FUNCTION SharedName : INT
        SharedName := 0;
    END_FUNCTION
END_NAMESPACE

NAMESPACE ns2
    FUNCTION SharedName : INT
        SharedName := 0;
    END_FUNCTION
END_NAMESPACE

FUNCTION test : INT
    USING ns1;
    USING ns2;
    test := SharedName(); 
END_FUNCTION
```

Since SharedName is available in both scopes:

```sh
[E0205] Error: multiple items in scope
    ╭─[ file:///example0.st:16:13 ]
    │
 16 │     test := SharedName();
    │             ─────┬────  
    │                  ╰────── multiple items named 'SharedName' available in scope
    │ 
    │ Note: qualify the name to resolve the ambiguity: ns1.SharedName or ns2.SharedName
────╯
```

Globals are shared everywhere, and Namespaces are partial,
so they can be defined across multiple files, and will be merged automatically.

```st
// file1.st
NAMESPACE MyNs
    FUNCTION MyFn END_FUNCTION
END_NAMESPACE
```

```st
// file2.st
NAMESPACE MyNs
    FUNCTION MyFn END_FUNCTION // <-- will trigger a duplicate error, because MyNs.MyFn already exists in file1.st
END_NAMESPACE
```

There is no limitation for namespace nesting.

Two keywords keep a part of a library private:

- A `FUNCTION PRIVATE` can only be called from its own namespace.
- A `NAMESPACE INTERNAL` can only be reached from the namespace that encloses it.

```st
NAMESPACE MyNs

    NAMESPACE MyNs2

        NAMESPACE MyNs3

            FUNCTION MyFn END_FUNCTION // <-- Is MyNs.MyNs2.MyNs3.MyFn

        END_NAMESPACE

    END_NAMESPACE

END_NAMESPACE
```

### Strict casts

Rk is strict on elementary types usages.
It strictly follows the IEC conventions about implicit and explicit casts.

<!-- casts:begin -->

<details>
<summary><strong>Implicit</strong>, what an assignment or a call widens on its own.</summary>

| from | to |
|---|---|
| `BOOL` | `BYTE`, `WORD`, `DWORD`, `LWORD` |
| `BYTE` | `WORD`, `DWORD`, `LWORD` |
| `WORD` | `DWORD`, `LWORD` |
| `DWORD` | `LWORD` |
| `LWORD` | — |
| `SINT` | `INT`, `DINT`, `LINT`, `REAL`, `LREAL` |
| `INT` | `DINT`, `LINT`, `REAL`, `LREAL` |
| `DINT` | `LINT`, `LREAL` |
| `LINT` | — |
| `USINT` | `INT`, `DINT`, `LINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `LREAL` |
| `UINT` | `DINT`, `LINT`, `UDINT`, `ULINT`, `REAL`, `LREAL` |
| `UDINT` | `LINT`, `ULINT`, `LREAL` |
| `ULINT` | — |
| `REAL` | `LREAL` |
| `LREAL` | — |
| `CHAR` | — |
| `STRING` | — |
| `TIME` | `LTIME` |
| `LTIME` | — |
| `DATE` | `LDATE` |
| `LDATE` | — |
| `TOD` | `LTOD` |
| `LTOD` | — |
| `DT` | `LDT` |
| `LDT` | — |

</details>

<details>
<summary><strong>Explicit</strong>, the <code>Std.Convert</code> functions, each named <code>FROM_TO_TO</code>.</summary>

| from | to |
|---|---|
| `BOOL` | `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `BYTE` | `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `CHAR`, `STRING` |
| `WORD` | `BYTE`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `DWORD` | `BYTE`, `WORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING` |
| `LWORD` | `BYTE`, `WORD`, `DWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `LREAL`, `STRING` |
| `SINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `INT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `DINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING`, `TIME`, `DATE`, `TOD` |
| `LINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `LREAL`, `STRING`, `LTIME`, `LDATE`, `LTOD`, `DT`, `LDT` |
| `USINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `STRING` |
| `UINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `USINT`, `STRING` |
| `UDINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `USINT`, `UINT`, `REAL`, `STRING` |
| `ULINT` | `BYTE`, `WORD`, `DWORD`, `LWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `REAL`, `LREAL`, `STRING` |
| `REAL` | `DWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `STRING` |
| `LREAL` | `LWORD`, `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT`, `REAL`, `STRING` |
| `CHAR` | `BYTE`, `STRING` |
| `STRING` | — |
| `TIME` | `DINT`, `STRING`, `LTIME` |
| `LTIME` | `LINT`, `STRING`, `TIME` |
| `DATE` | `DINT`, `STRING`, `LDATE` |
| `LDATE` | `LINT`, `STRING`, `DATE` |
| `TOD` | `DINT`, `STRING`, `LTOD` |
| `LTOD` | `LINT`, `STRING`, `TOD` |
| `DT` | `LINT`, `STRING`, `DATE`, `LDATE`, `TOD`, `LTOD`, `LDT` |
| `LDT` | `LINT`, `STRING`, `DATE`, `LDATE`, `TOD`, `LTOD`, `DT` |

</details>

<!-- casts:end -->

Any type conversion that is not lossless must be explicitly done with one of the conversion FUNCTIONs available in `Std.Convert`

In cases where an explicit cast is available, the compiler will advise you to use it:

```st
FUNCTION fn1 : BOOL

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := fn1();

END_FUNCTION_BLOCK
```

```sh
[E0301] Error: type mismatch
    ,-[ file:///test0.st:11:13 ]
    |
  8 |         test: INT;
    |         ^^|^
    |           `--- type is declared by variable 'test' here
    |
 11 |     test := fn1();
    |             ^^|^^
    |               `---- expected 'INT', got 'BOOL'
    |               |
    |               `---- consider explicitly casting with 'BOOL_TO_INT(fn1())'
    |
    | Help: insert explicit cast 'BOOL_TO_INT(fn1())'
----'
```

### Overloading

The IEC standard defines generics for **ANY_INT**, **ANY_MAGNITUDE** etc ...
Those are normally reserved for the standard library,
but it turns out that using overloads can mimic this system by writing each possible variant.

This comes with the advantage that the compiler does not need extra plumbing for these generics,
because the behavior is implemented directly in **ST**.

>[!IMPORTANT]
>__Overloading can only be used on **FUNCTIONs**__


```st
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

```st
MyFn(0) <-- Will pick the first overload
MyFn(1.0) <-- Will pick the second overload
```

> [!CAUTION]
> **Return type** affects the signature of overloads.

If a call site can fit multiple overloads, this will trigger an error:

```st
FUNCTION pick : INT
VAR_INPUT x : DINT; END_VAR
    pick := 1;
END_FUNCTION

FUNCTION pick : INT
VAR_INPUT x : LINT; END_VAR
    pick := 2;
END_FUNCTION

FUNCTION caller : INT
VAR y : SINT; END_VAR
    // SINT widens to both DINT and LINT, neither overload is exact.
    caller := pick(y);
END_FUNCTION
```

```sh
[E0809] Error: ambiguous overloaded call
    ╭─[ file:///example0.st:14:15 ]
    │
  1 │ ╭───▶ FUNCTION pick : INT
    ┆ ┆     
  4 │ ├───▶ END_FUNCTION
    │ │                    
    │ ╰──────────────────── candidate overload declared here
    │ 
  6 │   ╭─▶ FUNCTION pick : INT
    ┆   ┆   
  9 │   ├─▶ END_FUNCTION
    │   │                  
    │   ╰────────────────── candidate overload declared here
    │ 
 14 │           caller := pick(y);
    │                     ──┬─  
    │                       ╰─── call to 'pick' is ambiguous: 2 overloads accept these arguments: disambiguate with an explicit cast
────╯
```

### Monomorphized OOP

All Object Oriented Programming concepts are implemented (`CLASS`, `METHOD` ...).

>[!NOTE]
> A `CLASS` is the same syntax as a `FUNCTION_BLOCK`, but it cannot have a body, nor `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` and `VAR_TEMP` sections.
>
> A `METHOD` with no access specifier is `PUBLIC`, where the standard says `PROTECTED`.

The only point where we *voluntarily* diverge from the standard, is about `INTERFACE`.

You can use them to define contracts for inheritance, but defining them as variable types has limitations.

The compiler resolves everything at compile time and uses **monomorphization** to know exactly what an interface is.

So:

- You can pass `INTERFACE` as a `VAR_IN_OUT` or `VAR_INPUT` parameter, in both cases an `INTERFACE` is passed as a reference

- You can **only** define an `INTERFACE` parameter in a FUNCTION or a METHOD.

- An `ARRAY` OF `INTERFACE` is not allowed.

- **There is no virtual class.** A parameter typed `Base` takes a `Base`, and a `Derived` is `E0301`.
  
`ABSTRACT` does not change that: it forces the implementation and forbids the instantiation, it does not hand you a handle.

The `INTERFACE` is the only substitution point, and the POU has to declare `IMPLEMENTS` itself - inheriting it from a base is not enough.

`THIS` can be used to access variables or methods of a `FUNCTION_BLOCK` or `CLASS` from within.
`THIS.Method()` is virtual: an override wins.

`SUPER` reaches the base's **methods**, and statically - it names the base's version even when the instance overrides it.
Inherited *variables* are reached unqualified, so `SUPER.someVar` is `E0202`.

```st
FUNCTION_BLOCK Base
    METHOD PUBLIC Hook : INT
        Hook := 1;
    END_METHOD
    METHOD PUBLIC Call : INT
        Call := THIS.Hook();       // virtual, so 2 on a Derived instance
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Hook : INT
        Hook := 2;
    END_METHOD
    METHOD PUBLIC ViaSuper : INT
        ViaSuper := SUPER.Hook();  // static, so always 1
    END_METHOD
END_FUNCTION_BLOCK
```

`SUPER()` is a different thing: it runs the base `FUNCTION_BLOCK`'s **body** on the same instance.
A derived body replaces the base one, so without `SUPER()` the base body never runs.

```st
FUNCTION_BLOCK Counter
VAR
    n: INT;
END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Silent EXTENDS Counter
END_FUNCTION_BLOCK                 // n stays 0, however often it is called

FUNCTION_BLOCK Chained EXTENDS Counter
    SUPER();                       // n counts up
END_FUNCTION_BLOCK
```

>[!IMPORTANT]
> It never auto-chains: every level that wants its base's body has to ask.
> 
> And it belongs in a function block body, once, outside any loop 
> - `E1108` in a `FUNCTION`
> - `E1109` in a `METHOD` 
> - `E1110` for a second one 
> - `E1111` inside a loop.

### References

`REF_TO`, `REF()`, `^` and `NULL` as in the standard.

A reference that may be `NULL` cannot be dereferenced: the compiler refuses `ptr^` unless it can prove `ptr` is set.

```st
FUNCTION_BLOCK fb1
VAR
    ptr: REF_TO INT; // ptr is declared, but never initialized
    x: INT;
END_VAR
    x := ptr^;
END_FUNCTION_BLOCK
```


```sh
[E0902] Error: possibly null dereference
   ╭─[ file:///example0.st:6:10 ]
   │
 3 │     ptr: REF_TO INT;
   │     ───────┬───────  
   │            ╰───────── 'ptr' declared without initializer here
   │ 
 6 │     x := ptr^;
   │          ─┬─  
   │           ╰─── dereference of reference 'ptr' which is never initialized
───╯
```

To fix this problem, you must use guards:

```st
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

> [!WARNING]
> `AND` and `OR` do not short-circuit, so a guard written in the same expression does not protect the dereference:
> ```st
> ok := (ptr <> NULL) AND (ptr^ > 0);   // E0902, both sides are always evaluated
> ```
> Use a nested `IF` instead.

A reference is also **invariant**: a `REF_TO REAL` cannot point to an `INT`, even though an `INT` widens to a `REAL` when it is assigned.

## Extras

### Tests

A test is a `FUNCTION` marked `{test}`, asserting with `Std.Unit`.

```st
USING Std.Unit;

{test}
FUNCTION test_add
    ASSERT_EQ(Add(a := 2, b := 3), INT#5, 'Add(2, 3)');
END_FUNCTION
```


`rk test` compiles the workspace and runs all of them, showing a summary at the end.

```sh
    // ...
        PASS [   83µs] Std.Timers.Test.test_tp_ignores_input_during_pulse
        PASS [  1.1ms] Std.Timers.Test.test_tp_ltime_pulse
        PASS [  1.1ms] Std.Timers.Test.test_tp_post_pulse_retrigger
        PASS [  1.1ms] Std.Timers.Test.test_tp_pulse_expires
        PASS [  101µs] Std.Timers.Test.test_tp_pulse_starts
        PASS [   59µs] Std.Unit.Test.test_assert_eq_accepts_every_date_and_time_type
        PASS [   82µs] Std.Unit.Test.test_assert_neq_sees_a_different_date_or_time
────────────────────────────────────────────────────────────
    Summary [158.5ms] 341 tests run: 341 passed, 0 failed
```

The `Std.Unit` namespace contains three assertion FUNCTIONs, each one of them carries an optional **STRING** payload that the CLI or runtime should display in case of failure.

- `ASSERT` which asserts the condition being passed is TRUE:
```st
ASSERT(TRUE, "payload")
```

- `ASSERT_EQ` which asserts the left parameter is equal to the right parameter:
```st
ASSERT_EQ(1, 1, "1 + 1 should be 2")
```

- `ASSERT_NEQ`  which is the opposite of `ASSERT_EQ`.

All three use [overloading](#overloading) so they can be used with all elementary types.

- Internally all assertions use the built-in `__RAISE` that triggers a WASM exception with a STRING payload
  
__Every test gets fresh instances, so tests never leak state into each other.__

>[!TIP]
> A test can be selected by its qualified path in the workspace, or by any part of it.
> ```st
>   NAMESPACE Ns1
>       NAMESPACE Tests
>       USING Std.Unit;
>       
>           {test}
>           FUNCTION MyTest 
>               ASSERT_EQ(1, 1, " 1 + 1 = 2")
>           END_FUNCTION
>
>       END_NAMESPACE
>   END_NAMESPACE
> ```
> can be accessed with `rk test`:
> 
>  `rk test Ns1.Tests.MyTest`
>
> The name is matched as a substring, so `rk test Tests` runs every test of that namespace.


### Pragmas

Only these exist; anything else in braces is a syntax error,
so `{attribute '…'}` from other toolchains cannot be copied in.

- `{test}` marks a `FUNCTION` the test runner calls.

- `{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.

- `{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU.

- `{allow 'rule' 'rule'}` silences lint rules: above a POU for the whole POU, as a statement for the next statement.


- `{export}` marks a `FUNCTION` a host can call, see [WASM ABI](#wasm-abi).

- `{extern 'module' 'name'}` declares a `FUNCTION` as a WASM import; the declaration is the signature.


- `{wasm 'instruction' (params a b) (result r)}` is a statement that emits one WASM instruction on the named operands.

## Internal behavior

### Bundled Traps

Everything the compiler checks raises **one** exception, and it carries a message.

A module declares a single exception tag, `(i32, i32)`: the pointer and length of a STRING in the memory you provided.
It is never exported, so a host reads it from the pending-exception slot.

> [!TIP]
> `__RAISE` triggers a WASM [throw](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/throw) and can be contained in a [try_table](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Exception_handling/try_table), `__TRY` and `__CATCH` are not implemented yet,
> but they are on the roadmap.

Three checks are inserted by the compiler:

| Inserted at | Message |
|---|---|
| every array subscript, per dimension | `array index out of bounds` |
| every subrange store | `value out of subrange bounds` |
| every `^` you wrote | `dereference of a null reference` |

What the compiler can prove is refused at compile time instead.

__And three checks that do not exist.__

- **Integer overflow is never checked.** `DINT#2147483647 + 1` is `-2147483648`, silently, at every width.
- **Division by zero is not ours.** It is the VM's own trap: no message, and a `{test}` cannot catch it.
- **STRING capacity is not checked.** It truncates. See [Strings](#strings).

`REAL#1.0 / 0.0` is `+inf` and the scan continues. Nothing faults on NaN or infinity.

A Rust panic inside a grafted builtin arrives as the same exception, with the panic text as the message.
None of this depends on the profile.

### Strings

Rk uses one string type, **UTF-8**, there is no `WSTRING` and no `WCHAR`.

A slot is a 4-byte length followed by its capacity in bytes, so a plain `STRING` occupies 84.

```st
s1: STRING;      // 80 bytes of buffer
s2: STRING[5];   // 5 BYTES, not characters - 'café' needs exactly this
```

The StdLib has several FUNCTIONs in `Std.Strings` to handle STRING operations:

- `IS_UTF8` checks if a STRING is valid UTF-8.

Byte read-only operations:

- `LEN` returns the byte-length of a STRING.
- `LEFT` returns the first n bytes.
- `RIGHT` returns the last n bytes.
- `MID` returns the n bytes starting at a 1-indexed byte position.
- `FIND` finds the 1-indexed byte position of a STRING in a STRING.

Char read-only operations:

- `CHAR_COUNT` returns the character-length of a STRING, so `CHAR_COUNT('café')` is 4 where `LEN` is 5.
- `CHAR_LEFT` returns the first n characters.
- `CHAR_RIGHT` returns the last n characters.
- `CHAR_MID` returns the n characters starting at a 1-indexed character position.
- `CHAR_FIND` finds the 1-indexed character position of a STRING in a STRING.
- `CHAR_AT` returns the character at a 1-indexed character position, as a `CHAR`.

>[!NOTE]
All char operations agree on ASCII.

Write operations:

- `CONCAT` concatenates 2 STRING together, if the result is too large, the final STRING is truncated.
- `INSERT` inserts a STRING after a given byte position.
- `DELETE` removes n bytes starting at a 1-indexed byte position.
- `REPLACE` replaces n bytes at a 1-indexed byte position with a STRING.
- `CHAR_INSERT` inserts a STRING after a given character position.
- `CHAR_DELETE` removes n characters starting at a 1-indexed character position.
- `CHAR_REPLACE` replaces n characters at a 1-indexed character position with a STRING.


- A **literal** too long for its destination is a compile error.
- A **variable** too long truncates silently, and truncating bytes can split a character.
  `IS_UTF8` exists for exactly that.

- No indexing - `s[1]` is `E0508`, use `CHAR_AT`. 

- No `+` - use `CONCAT`.

Comparison is byte-lexicographic, so a `STRING` is a legal `CASE` label.

```st
FUNCTION Mode : INT
    VAR_INPUT
        cmd: STRING;
    END_VAR
    CASE cmd OF
        'start':
            Mode := 1;
        'stop', 'halt':   // one branch, several labels
            Mode := 2;
    ELSE
        Mode := 0;        // 'Start' lands here: labels compare as bytes
    END_CASE;
END_FUNCTION
```

`CHAR` is a code point in 4 bytes. It does **not** widen to `STRING`; `CHAR_TO_STRING` does that.

> [!IMPORTANT]
> At a call boundary a `STRING` input or return is a borrowed `(ptr, len)`, never a copy.
>
> A `VAR_IN_OUT` or `VAR_OUTPUT` is instead `(addr, capacity)`, so the callee's writes clamp.

### Math operations

Every maths function is in the module. **It imports nothing but `env.memory`.**

`+ - * / MOD` are single wasm instructions, and so are `SQRT` and `ABS`.
The eleven that have no instruction - `SIN COS TAN ASIN ACOS ATAN ATAN2 EXP LN LOG` and `**` - are grafted in from libm when you call them, in `REAL` and `LREAL` form.
Nothing is imported, so nothing has to be wired up by the host.

Arithmetic is wasm arithmetic:

- 8- and 16-bit widths wrap by explicit masking; 32- and 64-bit wrap silently.
- `NaN` and `±inf` are ordinary values. `SQRT(-1)` is NaN, `LN(0)` is `-inf`, nothing faults.
- Float to integer **saturates**, so NaN converts to `0`.
- `IS_NAN` is ordinary ST: `IN <> IN`.

`**` needs a float base and returns the base's type; `EXPT` is the same code path.

> [!NOTE]
> As stated in [Overloading](#overloading), there is no `ANY_INT` or `ANY_REAL` in the type system.
> 
> Each generic is an overload set written out in ST, which is why `Std.Math` is readable and replaceable.

Implicit casts follow the standard's table, which is stricter than most toolchains:

```st
r := i;    // INT to REAL, Ok
r := d;    // DINT to REAL, E0301 - the standard's table omits it
i := r;    // never implicit; the error names the cast for you
```

> A bare literal expression computes at the literal's default type, then widens.
> `x : LREAL := 0.1 + 0.0` is REAL arithmetic.
> Write `LREAL#0.1 + 0.0`.

## StdLib

Ordinary Structured Text in `stdlib/`, one namespace per file: `Std.Math`, `Std.Strings`, `Std.Timers`, `Std.Counters`, `Std.Unit`, ... and the rest.

The compiler knows nothing about it beyond where the files are.

So that means you can replace any part of the stdlib, extend it, or read it to see how a `TON` is written.

## WASM ABI

A workspace compiles to one core module.

It imports its linear memory as `env.memory`, exports `__init` to set cold-start values, one body per `PROGRAM`, and every `FUNCTION` marked `{export}`.

Nothing else is exported, the stdlib included: a release build only keeps what you call.

Retained and global bands as `retain_base`/`retain_size` and `globals_base`/`globals_size`.

Tasks, retained state and debug symbols travel as custom sections.

| ST | at a call boundary | in memory |
|---|---|---|
| `BOOL`, `SINT`…`DINT`, `USINT`…`UDINT`, `BYTE`, `WORD`, `DWORD`, `CHAR`, `TIME`, `DATE`, `TOD`, `DT` | `i32` | 4 bytes |
| `LINT`, `ULINT`, `LWORD`, `LTIME`, `LDATE`, `LTOD`, `LDT` | `i64` | 8 bytes |
| `REAL` / `LREAL` | `f32` / `f64` | 4 / 8 bytes |
| enum, subrange | the lane of its storage type | as its storage type |
| `STRING` | `(ptr, len)`, two `i32` | 4-byte length, then `capacity` bytes of UTF-8 |
| `STRUCT`, `ARRAY`, FB instance | `i32` pointer | laid out in place |
| `REF_TO T` | `i32` address; `NULL` is `0` | 4 bytes |

Calling an export:

- `__init` first, `() -> ()`.
 
- A `PROGRAM` body takes its instance address and does not have a return type: `(i32) -> ()`.

- A `FUNCTION` marked `{export}` takes every parameter in the order it is declared: 
  - `VAR_INPUT` scalars by value.
  - One pointer per `VAR_IN_OUT` and `VAR_OUTPUT`.
  
  - the return type is the result, a `STRING` result is `(ptr, len)`, an aggregate result is a pointer to the callee's slot to copy out of.
  
- Everything lives in the memory you provided; the first 16 KiB are reserved for the grafted builtins.

So this function:

```st
{export}
FUNCTION Scale : REAL
	VAR_INPUT
		raw: INT;
		gain: REAL;
	END_VAR
	VAR_IN_OUT
		total: REAL;
	END_VAR
	VAR_OUTPUT
		clamped: BOOL;
	END_VAR

END_FUNCTION
```

exports this:

```wat
(func (export "Scale")
	(param i32)   ;; raw      INT, by value
	(param f32)   ;; gain     REAL, by value
	(param i32)   ;; total    VAR_IN_OUT, address
	(param i32)   ;; clamped  VAR_OUTPUT, address
	(result f32)) ;; Scale    REAL
```

Write `total` and read `clamped` back at the addresses you passed.
Declaring `VAR_OUTPUT` before `VAR_INPUT` moves it up the parameter list.

A host import declared with `{extern}` is the same convention in reverse: 
- `VAR_INPUT` are the parameters, 
- scalar `VAR_OUTPUT` the results, the return type last; 

it takes copies, so `VAR_IN_OUT`, aggregate outputs and a `STRING` return are refused.

## Debug Symbols

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

> [!NOTE]
> **Rk ships no debugger.**
>
> It emits the tables, and the `debug_format` crate decodes them.
> The scan loop, the monitoring session and the debug adapter belong to a runtime.

## Profiles

There are two profiles: **debug** and **release**.

| | `rk compile` | `rk compile --release` |
|---|---|---|
| stepping tables | yes | no |
| tests | yes | no |
| symbols, retain map, schedule | yes | yes |
| optimized by wasm-opt | never | always |

### Why a release build cannot be stepped

The stepping tables point **inside the code**:

- `debug-lines` maps a code offset to a file, a line and a column.
- `debug-functions` maps a WASM function index to a POU.
- `debug-locals` maps a WASM local slot to a variable.

A release build is optimized by [Binaryen](https://github.com/webassembly/binaryen), which rewrites the body of every function: instructions are merged, reordered or removed, and locals are reassigned.

Once this is done, there is no way to track what has changed, so the tables would point to the wrong place.

This is why a release build does not carry them: it is **watchable**, but not **steppable**.

> [!TIP]
> The missing line table is also how a runtime knows which kind of binary it was given.

### What stays the same

**The memory layout is identical between the two profiles.**

That is what lets you stop a release build, rebuild the same source as debug, and carry the live state across.

Variables can still be read and written by name in a release build, because `debug-symbols` points to the memory and not to the code.

> [!NOTE]Binaryen removes every function you never call.

### Optimization levels

`-O` takes `0` to `4`, `s` or `z`, and defaults to `2`.

> [!WARNING]
> `-O4` runs with `--skip-pass=flatten`: the Flatten pass of Binaryen does not support `try_table` yet, see [binaryen#8372](https://github.com/WebAssembly/binaryen/issues/8372).
>
> What is left of `-O4` is close to `-O3`.

`binaryen` is an external binary:

- If one is on your `PATH`, it is used.
- Otherwise, Binaryen 131 is downloaded once into your cache directory (`~/.cache/rk/binaryen` on Linux), and its checksum is verified.

See [Environment variables](#environment-variables) to opt out of the download.

If the optimizer fails:

- `rk test -O` warns, and runs a correct unoptimized build.
- `rk compile --release` refuses outright.

> [!IMPORTANT]
> A release build never silently degrades.

## Environment variables

| Variable | Used by | Effect |
|---|---|---|
| `RK_STDLIB_PATH` | CLI, language server | Where the standard library is. |
| `RK_NO_DOWNLOAD` | CLI | When set, Binaryen is never downloaded. |
| `NO_COLOR` | CLI | When set, the output is not colored, see [no-color.org](https://no-color.org). |

### The `.env` file

`RK_STDLIB_PATH` can also be written in a `.env` file at the root of your workspace:

```sh
# .env
RK_STDLIB_PATH="/path/to/stdlib"
```

> [!NOTE]
> The process environment wins when both are set.
>
> This is the only variable read from the `.env` file.

If it is not set anywhere, the standard library is looked up beside the `rk` binary, or in the `stdlib/` folder of the checkout when running from a build.

> [!TIP]
> `rk env` prints the standard library in use, and where it was found.

## License

rk is distributed under [AGPL-3.0-only](LICENSE).
For the Apache-2.0 exceptions, the permission that makes every generated module yours,
and commercial licensing, see [LICENSING.md](LICENSING.md).
