## Interfaces

An `INTERFACE` declares method prototypes: a name, an optional return type, and `VAR_INPUT` / `VAR_OUTPUT` / `VAR_IN_OUT` sections. A prototype has no body, no `VAR_TEMP` (`E0025`) and no access specifier (`E0037`). An interface may extend several interfaces.

The compiler checks that an implementer provides every method (`E0509`), and the signature of each is matched position by position: the parameter count (`E0512`), and at each position the name (`E0525`), the section (`E0526`) and the type (`E0523`), plus the return type (`E0524`).

```iecst
INTERFACE IControllable
    METHOD Start END_METHOD
    METHOD GetStatus : BOOL END_METHOD
END_INTERFACE

FUNCTION_BLOCK Pump IMPLEMENTS IControllable
VAR
    running : BOOL;
END_VAR
METHOD PUBLIC Start
    running := TRUE;
END_METHOD

METHOD PUBLIC GetStatus : BOOL
    GetStatus := running;
END_METHOD
END_FUNCTION_BLOCK
```

Where an interface type may appear: `VAR_INPUT` or `VAR_IN_OUT` of a `FUNCTION` or of a `METHOD`. Nowhere else. Calls made through such a parameter are type-checked against the interface and monomorphized to the concrete argument at compile time, so several implementers may be passed to the same function and it compiles.

```iecst continues
FUNCTION StartIt : BOOL
VAR_IN_OUT
    dev : IControllable;
END_VAR
    dev.Start();
    StartIt := dev.GetStatus();
END_FUNCTION

PROGRAM Main
VAR
    p  : Pump;
    ok : BOOL;
END_VAR
    ok := StartIt(p);
END_PROGRAM
```

Passing a type that does not implement the interface is `E0301`, and calling a method the interface does not declare is `E0211`. The parameter itself is a fixed binding: reassigning it is `E0517`. `THIS` may be passed as an interface argument from a POU that implements it.

Everything else is refused. A stored `VAR` or FB member, a `VAR_OUTPUT`, a `VAR_TEMP`, a global, and an interface `VAR_INPUT` / `VAR_IN_OUT` on a `FUNCTION_BLOCK` or `PROGRAM` (whose instance would keep it across scans) are all `E0514`. An interface return type is `E0515`. An interface nested in an array, a `REF_TO` or a struct field is `E0516`. There is no vtable, no null interface reference, and no way to hold a heterogeneous collection of implementers.
