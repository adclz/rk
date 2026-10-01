# Namespaces

**Rule of thumb:** A file IS NOT a single POU.

A file can contain as many POUs as you want, as long as you give them a different name,
and folders do not affect name resolution.

> [!TIP]
> There is no limitation in how you want to organize your workspace, so feel free to split your code the way you like.

However you split it, the compiler sees one workspace:

```schema
src/                                your folders, any way you like
├─ motion/
│  ├─ axis.st
│  │  └─ NAMESPACE Motion
│  │     └─ FUNCTION_BLOCK Axis
│  └─ ramp.st
│     └─ NAMESPACE Motion
│        └─ FUNCTION Ramp
└─ main.st
   ├─ FUNCTION MyFn
   └─ PROGRAM Main
──►
WORKSPACE                           what the compiler sees
├─ FUNCTION MyFn                    global
├─ PROGRAM Main                     global
└─ NAMESPACE Motion                 one namespace, from two files
   ├─ FUNCTION_BLOCK Axis
   └─ FUNCTION Ramp
```

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

A qualified name only reaches what that namespace declares, not what it imports.

A POU declared in an enclosing namespace or at the top level comes before one a `USING` imports, wherever the `USING` is written.
Two imports of the same name are ambiguous, and the compiler says so:

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
