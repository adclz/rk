# References

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

A reference cannot be `RETAIN` (`E0904`): a new build may move what it points at.
