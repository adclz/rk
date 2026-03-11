# IEC 61131-3 Structured Text

Complete language support for **IEC 61131-3 Structured Text** (.st files), with LSP, formatter, and advanced static analyzer. 

## Getting Started

The extension requires a `config.toml` file at the root of your workspace to activate. A minimal configuration:

```toml
[project]
name = "my_project"
version = "0.1"
```

### Optional Configuration

```toml
[project]
name = "my_project"
version = "0.1"

# Path to the IEC standard library
stdlib_path = "/path/to/stdlib"

# Output directory for generated files
[output]
directory = "build"

# Toggle individual linter rules (all enabled by default)
[linter.rules]
unused-variable = true
shadowing-variable = true
duplicate-var-section = true
```

## Features

### Navigation

- **Go to Definition / Declaration** - jump to the definition or declaration of any symbol
- **Find References** - find all usages of a symbol across the workspace
- **Go to Implementation** - navigate to implementations of classes and interfaces
- **Document Symbols** - outline view of all POUs, namespaces, programs, and variables
- **Workspace Symbols** - search for symbols across the entire workspace
- **Document Links** - bracket references in comments (e.g. `[MyFB]`, `[NS.MyType]`) become clickable links

### Editing

- **Completions** - context-aware suggestions triggered by `.` (field access), `#` (namespace), and `(` (function calls), with snippet support for POUs
- **Signature Help** - parameter hints for function and method calls
- **Rename** - rename symbols across the entire workspace
- **Code Actions** - quick fixes for diagnostics
- **Formatting** - full document formatting

### Information

- **Hover** - type information and documentation on hover
- **Inlay Hints** - inline annotations for types, parameter names, and namespace paths
- **Semantic Tokens** - rich syntax highlighting for namespaces, functions, methods, interfaces, classes, structs, enums, and enum members
- **Code Lens** - shows implementation count on classes and interfaces
- **Folding Ranges** - foldable regions for POUs, variable sections, namespaces, and comments

### Diagnostics

148 real-time diagnostics covering syntax errors, duplicate definitions, type mismatches, visibility violations, inheritance issues, array bounds, and more.

The type checker is fully context-aware: most errors explain why it happened, points to related declarations, and often suggests how to fix it. This makes diagnostics actionable for both humans and AI coding agents.

#### Type mismatch with cast suggestion

```iecst
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR
    test := ULINT#5;
END_FUNCTION_BLOCK
```

```
[E0301] Error: type mismatch
   |
 4 |     test: INT;
   |     ^^|^
   |       `--- type is declared by variable 'test' here
   |
 7 |     test := ULINT#5;
   |             ^^^|^^^
   |                `--- expected 'INT', got 'ULINT'
   |                |
   |                `--- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
```

#### Null dereference tracking

```iecst
FUNCTION_BLOCK fn1
    VAR
        x: INT := 5;
        ptr: REF_TO INT;
        result: INT;
    END_VAR

    ptr := REF(x);
    ptr := NULL;
    result := ptr^;
END_FUNCTION_BLOCK
```

```
[E1007] Warning: possibly null dereference
    |
  9 |     ptr := NULL;
    |     ^^^^^|^^^^^
    |          `--- 'ptr' set to NULL here
    |
 10 |     result := ptr^;
    |               ^|^
    |                `--- dereference of reference 'ptr' which is null
```


#### Missing abstract method implementation

```iecst
CLASS Base
    METHOD ABSTRACT Tick : INT END_METHOD
END_CLASS

CLASS Mid EXTENDS Base
END_CLASS
```

```
[E0504] Error: inheritance violation
   |
 2 |     METHOD ABSTRACT Tick : INT END_METHOD
   |                    ^^|^
   |                      `--- ABSTRACT method 'Tick' is declared here
   |
 5 |     CLASS Mid EXTENDS Base
   |           ^|^
   |            `--- missing implementation for ABSTRACT method 'Tick'
   |
   | Note: ABSTRACT methods must be implemented by derived POUs
```

#### Mutual recursion detection

```iecst
FUNCTION_BLOCK fb1
    VAR_INPUT
        invalid : fb2;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
    VAR_INPUT
        invalid : fb1;
    END_VAR
END_FUNCTION_BLOCK
```

```
[E0902] Error: recursion detected
    |
  1 |     FUNCTION_BLOCK fb1
    |                    ^|^
    |                     `--- type 'fb1' is recursive
    |
 10 |         invalid : fb1;
    |                   ^|^
    |                    `--- recurses at this location
    |
    | Note: cycle goes
    |       -> fb1
    |       -> fb2
    |       ... and back to fb1
```

### Linter

8 configurable lint rules, all enabled by default:

| Rule | Description |
|------|-------------|
| `unused-variable` | Variables declared but never used |
| `shadowing-variable` | Variable shadows a POU name |
| `duplicate-var-section` | Duplicate variable sections |
| `unused-return-type` | Unused function return values |
| `effectless-statement` | Statements with no effect |
| `case-without-else` | CASE statements without ELSE branch |
| `dead-code` | Unreachable statements |
| `for-loop-step-sign` | FOR loop step direction mismatches bounds |

Toggle rules in `config.toml`:

```toml
[linter.rules]
dead-code = false
case-without-else = false
```
