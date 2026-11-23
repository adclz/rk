# Constants

A constant is a name for a value that never changes.

You can declare constants in three places:

- **`VAR_CONSTANT`**, at the top of a file or in a `NAMESPACE`, declares constants for the whole workspace.
- **`VAR CONSTANT`**, in a POU, declares constants for that POU.
- **`VAR_GLOBAL CONSTANT`**, in the `CONFIGURATION`, declares constants of the plant.

```iecst
VAR_CONSTANT
    ROWS : INT := 8;
END_VAR

TYPE Row : ARRAY[1..ROWS] OF INT; END_TYPE

FUNCTION_BLOCK Grid
VAR CONSTANT
    COLUMNS : INT := 4;
END_VAR
VAR
    cells : ARRAY[1..ROWS, 1..COLUMNS] OF INT;
END_VAR
END_FUNCTION_BLOCK
```

A constant is read like a variable, but it cannot be written (`E0404`).
You cannot pass it to a `VAR_IN_OUT` or take a `REF()` of it either.

Its value must be a constant: a literal, another constant, or arithmetic over those (`E0401`).
A value that reads the constant itself, directly or through other constants, is `E0408`.

## VAR_CONSTANT

`VAR_CONSTANT` declares constants outside of any POU.
They belong to their namespace, like its TYPEs and FUNCTIONs.

```iecst
NAMESPACE Limits
    VAR_CONSTANT
        HIGH : INT := 100;
        LOW : INT := -HIGH;
    END_VAR
END_NAMESPACE

TYPE Level : INT(Limits.LOW..Limits.HIGH); END_TYPE

FUNCTION Clamp : INT
USING Limits;
VAR_INPUT
    value : INT;
END_VAR
    Clamp := value;
    IF value > HIGH THEN
        Clamp := HIGH;
    ELSIF value < LOW THEN
        Clamp := LOW;
    END_IF;
END_FUNCTION
```

You name them like a TYPE:

- **Bare** in their namespace, and in the namespaces inside it.
- **Qualified** from anywhere else: `Limits.HIGH`.
- **Through `USING`**: `USING Limits;`, then `HIGH`.

A constant shares its namespace's names with the POUs and TYPEs: a FUNCTION with the same name is `E0102`.

You can use them anywhere a constant is expected:

- **In a TYPE**: an array's bounds, a `STRING`'s length, a subrange, an enum's values, a STRUCT field's initial value.
- **In a POU**: its declarations, its initial values and its body, `CASE` labels included.
- **In the `CONFIGURATION`**: a `VAR_GLOBAL`'s initial value and a task's `INTERVAL`.
- **In another constant's value.**

A constant can be of any type.
A STRUCT or an array constant is read part by part, or copied whole:

```iecst
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

VAR_CONSTANT
    ORIGIN : Point := (x := 0, y := 0);
    PRIMES : ARRAY[0..2] OF INT := [2, 3, 5];
END_VAR

FUNCTION Third : INT
VAR
    start : Point;
END_VAR
    start := ORIGIN;
    Third := PRIMES[2] + ORIGIN.y;
END_FUNCTION
```

A library's constants are named the same way, under the library's name: `Drives.MAX_SPEED`.
Your own constant hides a library's constant with the same name.

> [!NOTE]
> `VAR_CONSTANT` is not part of IEC 61131-3, it is an rk extension.

## VAR CONSTANT

`VAR CONSTANT` declares constants inside a POU.
They belong to that POU, like its other variables.

## VAR_GLOBAL CONSTANT

`VAR_GLOBAL CONSTANT` declares constants in the `CONFIGURATION`.
A POU names one after importing it with `VAR_EXTERNAL CONSTANT`.
One can also give a task its interval, see [Configuration](configuration.md).

```iecst
PROGRAM Main
VAR_EXTERNAL CONSTANT
    SAMPLES : INT;
END_VAR
VAR
    history : ARRAY[1..SAMPLES] OF REAL;
END_VAR
END_PROGRAM

CONFIGURATION Plant
    VAR_GLOBAL CONSTANT
        SAMPLES : INT := 16;
        period : TIME := T#20ms;
    END_VAR
    RESOURCE Cpu ON PLC
        TASK Cyclic(INTERVAL := period, PRIORITY := 1);
        PROGRAM Main1 WITH Cyclic : Main;
    END_RESOURCE
END_CONFIGURATION
```

A TYPE cannot name a `VAR_GLOBAL CONSTANT`: use a `VAR_CONSTANT` for that.

## Differences

| | `VAR_CONSTANT` | `VAR CONSTANT` | `VAR_GLOBAL CONSTANT` |
| --- | --- | --- | --- |
| Declared | at the top of a file, or in a `NAMESPACE` | in a POU | in the `CONFIGURATION` |
| Named from | anywhere, like a TYPE | its POU | a POU, with `VAR_EXTERNAL CONSTANT` |
| In a TYPE | yes | no | no |
| In a library | yes | yes | no |
| In IEC 61131-3 | no | yes | yes |

## Named values and initial values

Two other declarations look like constants, but are not:

- **A named value** is one of the values of a TYPE: `Mode#Run` in `TYPE Mode : INT (Idle := 0, Run := 1); END_TYPE`. Use it for the codes a value can take, see [Enums](enums.md).
- **A TYPE's initial value** is the value each variable of that TYPE starts with: `TYPE Tally : INT := 5; END_TYPE`. The variables can change it.
