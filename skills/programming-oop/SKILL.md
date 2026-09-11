---
name: programming-oop
description: Object-oriented Structured Text — CLASS, METHOD, visibility, EXTENDS,
  THIS, SUPER and INTERFACE. Use when writing or reviewing a CLASS or an INTERFACE,
  overriding a method, or deciding where an interface type may legally appear.
---

## Summary

RK implements the OOP part of IEC 61131-3: `CLASS`, `INTERFACE`, `METHOD`, `EXTENDS`, `IMPLEMENTS`, `THIS`, `SUPER`, and the `ABSTRACT` / `FINAL` / `OVERRIDE` qualifiers.
A `CLASS` is a `FUNCTION_BLOCK` without a body: it holds state and methods, but it is never invoked, so `c();` on a class instance is `E0808` "not a callable type".
Everything else runs through methods.

There is no `PROPERTY`.
There are no getters or setters, no `GET` / `SET` blocks.
Writing `PROPERTY` produces `E0003` and `E0101` / `E0001` / `E0002` errors, depending on the shape of the block; expose a `VAR` or write a method instead.

Interfaces exist but are not dynamically dispatched.
An interface type is legal only as a `VAR_INPUT` or `VAR_IN_OUT` parameter of a `FUNCTION` or a `METHOD`, where the compiler monomorphizes the call to the concrete type the caller passed.
Storing one (`VAR`, an FB member, `VAR_OUTPUT`, `VAR_TEMP`, a global, a return type, an array element, a struct field) is refused with `E1121` / `E1122` / `E1123`.
See the interfaces section below before designing around them.

Visibility is enforced on methods only.
`PUBLIC` is the default when nothing is written, which is a deliberate departure from the standard's tables (they default to `PROTECTED`).

## Classes

A class carries `VAR` sections and methods.
It has no body and no I/O sections: `VAR_INPUT` and `VAR_OUTPUT` do not parse inside a `CLASS`, `VAR_IN_OUT` is `E0018`, `VAR_TEMP` is `E0019`, and statements after the last method are a syntax error.

```iecst
CLASS Motor
VAR
    running : BOOL;
    speed   : INT;
END_VAR

METHOD PUBLIC Start
    running := TRUE;
END_METHOD

METHOD PUBLIC SetSpeed
VAR_INPUT
    target : INT;
END_VAR
    IF running THEN
        speed := target;
    END_IF;
END_METHOD
END_CLASS
```

A class is instantiated like a function block and its members are reached through the instance.

```iecst continues
PROGRAM Main
VAR
    m : Motor;
END_VAR
    m.Start();
    m.SetSpeed(target := 100);
END_PROGRAM
```

`CLASS` vs `FUNCTION_BLOCK`: the function block adds a body, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` and `VAR_TEMP`, and it is callable (`c(step := 2);` runs the body).
Both take `EXTENDS`, `IMPLEMENTS`, methods, `ABSTRACT` and `FINAL`.
Prefer a function block when the thing has a cyclic behaviour, a class when it is only state plus operations.

```iecst
FUNCTION_BLOCK Counter
VAR
    n : INT;
END_VAR
VAR_INPUT
    step : INT := 1;
END_VAR
METHOD PUBLIC Reset
    n := 0;
END_METHOD
    n := n + step;
END_FUNCTION_BLOCK
```

Classes and interfaces are allowed at the top level and inside a `NAMESPACE`.
`PROGRAM` cannot declare methods: `METHOD` inside a program body is `E0028`.

## Methods

A method is declared inside a `CLASS` or a `FUNCTION_BLOCK`.
The keyword order is fixed and the compiler rejects any other: `METHOD [access] [FINAL|ABSTRACT] [OVERRIDE] name [: return_type]`.

A method may declare `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR` and `VAR_TEMP`.
Its return value is assigned by name, like a function.
A method without a return type is called as a statement.

Inside a method, the enclosing POU's variables and sibling methods are visible unqualified.
`THIS.` is accepted and equivalent; it is not required.

```iecst
FUNCTION_BLOCK EStopMonitor
VAR_INPUT
    channel1 : BOOL;
    channel2 : BOOL;
END_VAR
VAR
    mismatch : INT;
END_VAR

METHOD PUBLIC Check : BOOL
    IF channel1 <> channel2 THEN
        mismatch := mismatch + 1;
    END_IF;
    Check := NOT channel1;
END_METHOD
END_FUNCTION_BLOCK
```

## Visibility

`PUBLIC`, `PRIVATE`, `PROTECTED` and `INTERNAL` all exist and all are enforced, on `METHOD` declarations.
A method with no specifier is `PUBLIC`.

`PRIVATE` means the declaring POU only, reached through `THIS` or unqualified.
Reaching it from a derived POU via `SUPER`, or from outside through an instance, is `E1001`.
`PROTECTED` adds derived POUs; a call from an unrelated POU is `E1002`.
`INTERNAL` means the same namespace; a call from another namespace, or from the global scope, is `E1003`, and so is a call from a namespace to an `INTERNAL` item declared at the global scope.

```iecst
NAMESPACE plant
    CLASS Tank
    METHOD PRIVATE Drain
    END_METHOD

    METHOD PROTECTED Refill
    END_METHOD

    METHOD INTERNAL Calibrate
    END_METHOD

    METHOD Level : INT
        THIS.Drain();
        Level := 0;
    END_METHOD
    END_CLASS
END_NAMESPACE
```

The grammar also accepts an access specifier on a `VAR` section header (`VAR PRIVATE ... END_VAR`) The compiler ignores it there: a `VAR PRIVATE` member is still readable from outside.
Do not rely on it.
On a `FUNCTION` the specifier is real: `FUNCTION PRIVATE` is callable only from its own namespace on its own side of the library line (`E1005`), and `PROTECTED`/`INTERNAL` do not apply there (`E1006`) — see the namespaces skill.

`INTERNAL` on a `NAMESPACE` (`NAMESPACE INTERNAL ns1`) is enforced: its contents are reachable only from inside the enclosing namespace, on the same side of the library line (`E1004`) — see the namespaces skill.


## Reference files

- `references/inheritance.md` — `EXTENDS`, overriding, `THIS` and `SUPER`
- `references/interfaces.md` — where an interface type is legal and where it is refused
- `references/errors.md` — the E05xx OOP diagnostics and what each one means
