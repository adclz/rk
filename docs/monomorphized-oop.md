# Monomorphized OOP

`CLASS`, `METHOD`, `EXTENDS`, `IMPLEMENTS`, `INTERFACE`, `THIS` and `SUPER` are all there.
Nothing is looked up while the program runs: the compiler knows which method every call reaches.

> [!NOTE]
> A `CLASS` is a `FUNCTION_BLOCK` without a body, and without `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` or `VAR_TEMP`.
>
> A `METHOD` without an access specifier is `PUBLIC`, where the standard says `PROTECTED`.

## Interfaces

This is the one place where rk *voluntarily* departs from the standard.

An interface is a parameter, a `VAR_INPUT` or a `VAR_IN_OUT` of a `FUNCTION` or a `METHOD`, passed by reference:

```iecst
INTERFACE IDevice
    METHOD Start END_METHOD
END_INTERFACE

FUNCTION_BLOCK Conveyor IMPLEMENTS IDevice
    METHOD PUBLIC Start END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pump IMPLEMENTS IDevice
    METHOD PUBLIC Start END_METHOD
END_FUNCTION_BLOCK

FUNCTION Run
VAR_IN_OUT
    dev : IDevice; // <-- a Conveyor or a Pump
END_VAR
    dev.Start();
END_FUNCTION

PROGRAM Main
VAR
    belt : Conveyor;
    pump : Pump;
END_VAR
    Run(dev := belt);
    Run(dev := pump);
END_PROGRAM
```

The compiler knows the type each call passes, and compiles one `Run` for each.

That is **monomorphization**, and it is why an interface cannot go anywhere its type could change:

- Not a variable, an output or a global (`E1121`).
- Not a return type (`E1122`).
- Not inside an `ARRAY`, a `REF_TO` or a `STRUCT` (`E1123`).

A POU declares `IMPLEMENTS` itself: inheriting it from a base is not enough.
The method that implements an interface's is `PUBLIC` (`E1135`), whether the POU declares it or inherits it.

## No virtual classes

A parameter typed `Base` takes a `Base`, and a `Derived` is `E0301`.
`ABSTRACT` forces the implementation and forbids the instance, but it does not change that.

To take several types, take an interface.

## THIS and SUPER

```iecst
FUNCTION_BLOCK Base
    METHOD PUBLIC Hook : INT
        Hook := 1;
    END_METHOD
    METHOD PUBLIC Call : INT
        Call := THIS.Hook();      // <-- 2 on a Derived: the override wins
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Hook : INT
        Hook := 2;
    END_METHOD
    METHOD PUBLIC ViaSuper : INT
        ViaSuper := SUPER.Hook(); // <-- always 1: the base's own
    END_METHOD
END_FUNCTION_BLOCK
```

- `THIS` reaches the instance's variables and methods, and `THIS.Method()` calls the instance's version.
- `SUPER.Method()` calls the base's version, even when the instance overrides it. Inside it, `THIS` is still the instance, so `THIS.Method()` there calls the instance's version too.
- Base code is compiled again for each derived block that runs it, so the call is known at compile time: an inherited method, and a base method or body reached through `SUPER`, is a copy per derived block.
- An inherited variable needs no prefix: `SUPER.someVar` is `E0202`.
- `SUPER.Method()` on an `ABSTRACT` method has no body to run, and is `E1133`.
- A `PRIVATE` method stays its block's own: a derived block does not inherit or override it, and may declare a method of the same name, which the base's code does not call.
- An override or an implementation gives each input the default the base method or the interface gives it (`E1131`): a call passes the default of the method it names.
- A variable and a method may share a name, as in CODESYS and TwinCAT, and `L0120` warns. Inside the block the name is the variable, `THIS` included; from outside, a `VAR` of that name gives way to the method.
- Inside a method, its own name is its result, before any member of that name. A method without a return type has no result, and there its name reaches the member.

## SUPER()

`SUPER()` runs the base function block's body, on the same instance.
A derived body replaces the base one: without `SUPER()`, the base body never runs.

```iecst
FUNCTION_BLOCK Counter
VAR
    n : INT;
END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Silent EXTENDS Counter
END_FUNCTION_BLOCK // <-- n stays 0

FUNCTION_BLOCK Chained EXTENDS Counter
    SUPER();       // <-- n counts up
END_FUNCTION_BLOCK
```

It never chains on its own: each level that wants its base's body calls `SUPER()`.
It goes in a function block body, once, outside any loop:

- `E1108` in a `FUNCTION`
- `E1109` in a `METHOD`
- `E1110` for a second one
- `E1111` inside a loop

A `CLASS` base has no body, so `SUPER()` in a function block that extends one is `E1132`.

## Copies

An instance is a value, like a struct.
Assigning one copies it whole: its variables, its inputs and outputs, and the instances it holds.
The two then run on their own.

```iecst
FUNCTION_BLOCK Counter
VAR n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR a : Counter; b : Counter; END_VAR
    a();
    b := a;    // <-- b.n is 1
    b();       // <-- b.n is 2, a.n is still 1
END_PROGRAM
```

A `REF_TO` member is copied as it is, and still points at the original's target.
To share one instance instead, pass it as a `VAR_IN_OUT`.
The copy costs as much as the instance is large: the `aggregate-copy` lint (`L0214`) points at every one.

A copy takes the exact type: a `Derived` into a `Base` is `E0301`, as it would keep the base part only.

A `VAR_INPUT` of an instance type is a copy too, as every input is.
An interface parameter is not: it binds the caller's instance by reference, so `dev : Conveyor` and `dev : IDevice` read the same at the call and do different things.

A `FUNCTION` or `METHOD` is not a value: assigning to its name outside its own body is `E0318`.
