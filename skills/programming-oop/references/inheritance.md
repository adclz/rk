## Inheritance

`EXTENDS` names one base, `IMPLEMENTS` a comma-separated list of interfaces, and `IMPLEMENTS` must come after `EXTENDS` (`E1103`).
A second `EXTENDS` is `E1101`.
A derived POU inherits the base's variables and methods; both are reachable unqualified, through `THIS`, and from outside through the instance.

```iecst
FUNCTION_BLOCK ABSTRACT Controller
VAR
    ticks : INT;
END_VAR
METHOD PUBLIC ABSTRACT Execute : BOOL
END_METHOD

METHOD PUBLIC Log
    ticks := ticks + 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK PidController EXTENDS Controller
VAR
    setpoint : REAL;
END_VAR
METHOD PUBLIC Execute : BOOL
    Log();
    Execute := TRUE;
END_METHOD
END_FUNCTION_BLOCK
```

Redefining a concrete base method requires `OVERRIDE` (`E1112`).
Implementing an `ABSTRACT` method or an interface method does not require it, but writing it is accepted.
`OVERRIDE` on a name that exists in no base is `E1113`.

```iecst
CLASS Base
METHOD PUBLIC Run : INT
    Run := 1;
END_METHOD
END_CLASS

CLASS Child EXTENDS Base
METHOD PUBLIC OVERRIDE Run : INT
    Run := SUPER.Run() + 1;
END_METHOD
END_CLASS
```

`ABSTRACT` means "incomplete, extend me", and the compiler holds both ends of that.
A `CLASS` or `FUNCTION_BLOCK` declaring an `ABSTRACT` method must itself be `ABSTRACT` (`E1117`), and an `ABSTRACT` type cannot be instantiated (`E1118`) — declare a variable of a derived type.

A CONCRETE derived POU must implement every inherited `ABSTRACT` method (`E1116`); an `ABSTRACT` derived one may pass the obligation further down, which is what an abstract intermediate is for.
An `ABSTRACT` type with no abstract method at all is legal: that is the ordinary extend-only base type.
A `FINAL` method cannot be overridden (`E1114`).

`FINAL` closes a type to EXTENSION, not to use: a `FINAL` class is still declared, instantiated and called as normal, but `CLASS D EXTENDS SomeFinalClass` is `E1104`.
`ABSTRACT` and `FINAL` cannot both appear on one header — the grammar rejects it, which is right: the first says "extend me", the second forbids it.

A derived function block or class may not redeclare a variable name inherited from a base (`E1115`); without the check the two declarations shared one slot.
Redeclaring a `VAR_EXTERNAL` is the exception: both name the same global, so the derived POU redeclares it to reach it.

A derived function block's call site binds the base's parameters too: `d(io := x, inp := 1, own := 2)` wires an inherited `VAR_IN_OUT` and `VAR_INPUT` next to the derived POU's own input, positional arguments count base parameters first, and omitting an inherited `VAR_IN_OUT` is `E0802` like omitting an own one.

## THIS and SUPER

`THIS` is the current instance.
It is valid in the body and in the methods of a `CLASS` or a `FUNCTION_BLOCK`, written `THIS.member` or `THIS^.member`.
In a `FUNCTION` it is `E1105`.
`THIS.Method()` dispatches virtually: on a derived instance the override runs.

`SUPER.name` reaches the base's METHODS, not its variables: IEC tables 9b/10b make `SUPER` a method reference, so `SUPER.someVar` is `E0202` and an inherited variable is reached unqualified instead.
It requires an `EXTENDS` clause on the current POU (`E1107`) and a class or function block context (`E1106`).
The call is static: `SUPER.m()` names the base's method even when the instance overrides it, and even from a further inheritor.

`SUPER()` is the distinct form that runs the base function block's *body*.
It may appear once, in the function block body only.
A derived body REPLACES the base body rather than extending it: without a `SUPER()` the base body never runs, and a derived function block with an empty body does nothing when called, even though its inherited inputs and in-outs are bound.

```iecst
FUNCTION_BLOCK Base
VAR n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
VAR m : INT; END_VAR
    SUPER();
    m := m + 1;
END_FUNCTION_BLOCK
```

A second `SUPER()` in the same body is `E1110`, one inside a `FOR` / `WHILE` / `REPEAT` is `E1111`, one inside a method is `E1109`, and one in a `FUNCTION` is `E1108`.

One thing that does not work: `REF(THIS)` does not produce a reference.
It types as the POU itself, so assigning it to a `REF_TO FB` is `E0301`.
Written inside a `PROGRAM` body, `THIS` is `E1105` and `SUPER` is `E1106` — neither belongs to a POU that has no instance.
