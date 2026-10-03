# Strict casts

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
    |           `--- 'test' is declared here
    |
 11 |     test := fn1();
    |             ^^|^^
    |               `---- expected 'INT', got 'BOOL'
    |
    | Help: insert explicit cast 'BOOL_TO_INT(fn1())'
----'
```
