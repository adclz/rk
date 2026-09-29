# Overloading

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
    │                       ╰─── call to 'pick' is ambiguous: 2 overloads accept these arguments
    │       
    │       Note: an argument widens to each of them: a typed literal or a conversion picks one, such as `DINT#5` or `INT_TO_DINT(x)`
────╯
```
