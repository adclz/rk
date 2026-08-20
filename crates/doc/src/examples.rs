/// Each error example: (error_code, category, title, description, ST source code).
///
/// The source code MUST trigger the corresponding error when compiled.
pub struct ErrorExample {
    pub code: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub sources: &'static [&'static str],
    /// When set, only this lint rule is enabled for the example.
    pub lint_rule: Option<&'static str>,
}

pub fn all_examples() -> Vec<ErrorExample> {
    vec![
        // ── E00xx: Syntax errors ─────────────────────────────────────────
        ErrorExample {
            code: "E0001",
            category: "Syntax",
            title: "Multiple EXTENDS declarations",
            description: "A class or function block can only have one `EXTENDS` clause.",
            sources: &[r#"
CLASS c1 EXTENDS c2 EXTENDS c3
END_CLASS

CLASS c2
END_CLASS

CLASS c3
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0002",
            category: "Syntax",
            title: "Multiple IMPLEMENTS declarations",
            description: "A class or function block can only have one `IMPLEMENTS` clause. Combine all interfaces in a single `IMPLEMENTS` list.",
            sources: &[r#"
INTERFACE a
END_INTERFACE

INTERFACE b
END_INTERFACE

FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0003",
            category: "Syntax",
            title: "IMPLEMENTS before EXTENDS",
            description: "The `EXTENDS` clause must appear before `IMPLEMENTS`.",
            sources: &[r#"
CLASS b
END_CLASS

FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0004",
            category: "Syntax",
            title: "Variable declarations after methods (CLASS)",
            description: "In a CLASS, variable declarations must appear before methods.",
            sources: &[r#"
CLASS c1
    METHOD m1 END_METHOD
    VAR
        x: INT;
    END_VAR
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0005",
            category: "Syntax",
            title: "Variable declarations after methods (FB)",
            description: "In a FUNCTION_BLOCK, variable declarations must appear before methods.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    METHOD m1 END_METHOD
    VAR
        x: INT;
    END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0006",
            category: "Syntax",
            title: "Variable type is missing",
            description: "Every variable declaration must have a type annotation.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0007",
            category: "Syntax",
            title: "Unexpected variable initialization",
            description: "Variable initialization is not allowed in this context.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR_IN_OUT
    x : INT := 5;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0008",
            category: "Syntax",
            title: "Incomplete edge qualifier",
            description: "An edge qualifier like `R_` or `F_ED` was started but not completed. Use `R_EDGE` or `F_EDGE`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR_INPUT
    x : BOOL R_ED;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0009",
            category: "Syntax",
            title: "THIS used as a path member",
            description: "`THIS` may only start a path expression (`THIS.member`, `THIS^.member`). It is not a member name, so it cannot appear after a dot. Using `THIS` in a POU that has no instance — a FUNCTION — is reported as E0503 instead.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x := x.THIS;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0010",
            category: "Syntax",
            title: "SUPER after a dot in a path",
            description: "`SUPER` must be the first element of a path expression, as in `SUPER.m()`. It cannot appear as a member after a dot — `THIS.SUPER.m()` and `inst.SUPER.x` are rejected. (Using a correctly placed `SUPER` in a POU with no EXTENDS clause is E0513; using it in a POU that cannot extend at all, such as a FUNCTION, is E0502.)",
            sources: &[r#"
FUNCTION_BLOCK base
METHOD m
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
    THIS.SUPER.m();
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0011",
            category: "Syntax",
            title: "Assignment to function call",
            description: "You cannot assign a value to a function call expression.",
            sources: &[r#"
FUNCTION fn1 : INT END_FUNCTION

FUNCTION_BLOCK fb1
    fn1() := 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0012",
            category: "Syntax",
            title: "Empty right-hand side of assignment",
            description: "The right-hand side of an assignment cannot be empty.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
END_VAR
    x := ;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0013",
            category: "Syntax",
            title: "Invalid assignment sign '='",
            description: "Use `:=` for assignment, not `=`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
END_VAR
    x = 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0014",
            category: "Syntax",
            title: "Invalid assignment sign ':'",
            description: "Use `:=` for assignment, not `:`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
END_VAR
    x : 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0015",
            category: "Syntax",
            title: "Invalid assignment sign '=' in FOR list",
            description: "Use `:=` for FOR loop initialization, not `=`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    i: INT;
END_VAR
    FOR i = 0 TO 10 DO
    END_FOR;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0016",
            category: "Syntax",
            title: "Invalid assignment sign ':' in FOR list",
            description: "Use `:=` for FOR loop initialization, not `:`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    i: INT;
END_VAR
    FOR i : 0 TO 10 DO
    END_FOR;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0017",
            category: "Syntax",
            title: "Function call in initialization expression",
            description: "Function calls are not allowed in variable initialization expressions.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    ml : ARRAY [0..2] OF INT := [1(call(IN := 5, OUT => OUT))];
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0018",
            category: "Syntax",
            title: "Comma-separated indices in array access",
            description: "Array element access cannot use comma-separated indices (`a[i, j]`). The comma form is initializer-only syntax; element access must chain one subscript per dimension: `a[i][j]`.",
            sources: &[r#"
FUNCTION fn1 : INT
VAR
    a : ARRAY [0..1, 0..1] OF INT;
END_VAR
    fn1 := a[0, 1];
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0019",
            category: "Syntax",
            title: "Missing node (syntax error)",
            description: "The parser encountered a syntax error — a required element is missing.",
            sources: &[r#"
NAMESPACE
END_NAMESPACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0020",
            category: "Syntax",
            title: "Invalid assignment sign '=>' in assignment",
            description: "Use `:=` for assignment, not `=>`. The `=>` operator is for output parameters in function calls.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
END_VAR
    x => 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0021",
            category: "Syntax",
            title: "Invalid assignment sign '=>' in FOR list",
            description: "Use `:=` for FOR loop initialization, not `=>`.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    i: INT;
END_VAR
    FOR i => 0 TO 10 DO
    END_FOR;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0022",
            category: "Syntax",
            title: "PROGRAM not allowed in namespace",
            description: "PROGRAM declarations cannot appear inside a NAMESPACE.",
            sources: &[r#"
NAMESPACE ns1
    PROGRAM p1
    END_PROGRAM
END_NAMESPACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0023",
            category: "Syntax",
            title: "CONFIGURATION not allowed in namespace",
            description: "CONFIGURATION declarations cannot appear inside a NAMESPACE.",
            sources: &[r#"
NAMESPACE ns1
    CONFIGURATION c1
    END_CONFIGURATION
END_NAMESPACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0024",
            category: "Syntax",
            title: "VAR_IN_OUT not allowed in this context",
            description: "`VAR_IN_OUT` can only be used inside FUNCTION and FUNCTION_BLOCK.",
            sources: &[r#"
CLASS c1
    VAR_IN_OUT

    END_VAR
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0025",
            category: "Syntax",
            title: "VAR_TEMP not allowed in this context",
            description: "`VAR_TEMP` can only be used inside FUNCTION and FUNCTION_BLOCK.",
            sources: &[r#"
CLASS c1
    VAR_TEMP

    END_VAR
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0026",
            category: "Syntax",
            title: "VAR_ACCESS not allowed in this context",
            description: "`VAR_ACCESS` can only be used inside PROGRAM.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    VAR_ACCESS

    END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0027",
            category: "Syntax",
            title: "VAR_CONFIG not allowed in this context",
            description: "`VAR_CONFIG` can only be used inside CONFIGURATION.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    VAR_CONFIG

    END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0028",
            category: "Syntax",
            title: "VAR_LOCATED not allowed in this context",
            description: "`VAR_LOCATED` can only be used inside PROGRAM.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    VAR_LOCATED

    END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0029",
            category: "Syntax",
            title: "VAR_EXTERNAL not allowed in this context",
            description: "`VAR_EXTERNAL` can only be used inside PROGRAM, FUNCTION_BLOCK, or FUNCTION.",
            sources: &[r#"
INTERFACE in
    METHOD m
        VAR_EXTERNAL

        END_VAR
    END_METHOD
END_INTERFACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0030",
            category: "Syntax",
            title: "VAR_GLOBAL not allowed in this context",
            description: "`VAR_GLOBAL` can only be used inside a CONFIGURATION. Global data is application-scoped: a POU reaches it with `VAR_EXTERNAL`, and a RESOURCE holds no variables of its own.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    VAR_GLOBAL

    END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0031",
            category: "Syntax",
            title: "VAR not allowed in this context",
            description: "`VAR` can only be used inside FUNCTION, FUNCTION_BLOCK, or PROGRAM.",
            sources: &[r#"
CONFIGURATION MyCfg
    VAR

    END_VAR
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0032",
            category: "Syntax",
            title: "SINGLE after INTERVAL in TASK",
            description: "In a TASK configuration, SINGLE must be declared before INTERVAL.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(INTERVAL := T#20ms, SINGLE := var1, PRIORITY := 1);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0033",
            category: "Syntax",
            title: "INTERVAL after PRIORITY in TASK",
            description: "In a TASK configuration, INTERVAL must be declared before PRIORITY.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(PRIORITY := 1, INTERVAL := T#20ms);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0034",
            category: "Syntax",
            title: "SINGLE after PRIORITY in TASK",
            description: "In a TASK configuration, SINGLE must be declared before PRIORITY.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(PRIORITY := 1, SINGLE := var1);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0035",
            category: "Syntax",
            title: "Missing PRIORITY in TASK",
            description: "PRIORITY is required in TASK configuration.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(INTERVAL := T#20ms);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0036",
            category: "Syntax",
            title: "Array conformands not supported",
            description: "Array conformands (`ARRAY[*]`) are not supported.",
            sources: &[r#"
FUNCTION fn1
VAR_INPUT
    x : ARRAY[*] OF INT;
END_VAR
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0037",
            category: "Syntax",
            title: "Access specifier on interface method prototype",
            description: "Access specifiers (PUBLIC, PRIVATE, etc.) are not allowed on interface method prototypes. Interface methods are implicitly PUBLIC.",
            sources: &[r#"
INTERFACE iface
    METHOD PUBLIC m1
    END_METHOD
END_INTERFACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0038",
            category: "Syntax",
            title: "Method declaration in body",
            description: "A METHOD declaration cannot appear inside the body (statement list) of a POU. Methods must be declared at the top level of a FUNCTION_BLOCK or CLASS.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    x := 1;
    METHOD m1
    END_METHOD
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0039",
            category: "Syntax",
            title: "TASK or PROGRAM outside a RESOURCE",
            description: "A CONFIGURATION contains RESOURCE blocks; the tasks and the programs bound to them are declared inside one. Wrap them in `RESOURCE <name> ON <cpu> ... END_RESOURCE`.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION cfg1
    TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst1 WITH t1 : prog1;
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0050",
            category: "Syntax",
            title: "Generic syntax error",
            description: "The parser encountered a syntax error that doesn't fit other categories.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
END_VAR
    x := 1 +;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // ── E01xx: Duplicate definitions ─────────────────────────────────
        ErrorExample {
            code: "E0101",
            category: "Duplicates",
            title: "Duplicate POU",
            description: "Two POUs (functions, function blocks, classes, etc.) have the same name.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := 0;
END_FUNCTION

FUNCTION fn1 : INT
    fn1 := 1;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0102",
            category: "Duplicates",
            title: "Duplicate variable",
            description: "Two variables in the same scope have the same name.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
    x : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0103",
            category: "Duplicates",
            title: "Duplicate struct field",
            description: "Two fields in a STRUCT have the same name.",
            sources: &[r#"
TYPE s1 : STRUCT
    field1 : INT;
    field1 : BOOL;
END_STRUCT
END_TYPE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0104",
            category: "Duplicates",
            title: "Duplicate enum variant",
            description: "Two variants in an ENUM have the same name.",
            sources: &[r#"
TYPE e1 : (Red, Green, Red)
END_TYPE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0105",
            category: "Duplicates",
            title: "Duplicate method declaration",
            description: "Two methods in the same POU have the same name.",
            sources: &[r#"
CLASS c1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0106",
            category: "Duplicates",
            title: "Duplicate method prototype",
            description: "Two method prototypes in an interface have the same name.",
            sources: &[r#"
INTERFACE i1
    METHOD m1 END_METHOD
    METHOD m1 END_METHOD
END_INTERFACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0107",
            category: "Duplicates",
            title: "Duplicate inherited method",
            description: "Two interfaces implemented by the same POU define a method with the same name.",
            sources: &[r#"
INTERFACE i1
    METHOD m1 END_METHOD
END_INTERFACE

INTERFACE i2
    METHOD m1 END_METHOD
END_INTERFACE

CLASS c1 IMPLEMENTS i1, i2
    METHOD m1 END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0108",
            category: "Duplicates",
            title: "Duplicate parameter",
            description: "A function call passes the same parameter twice.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        param1: INT;
        param2: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(param1 := 0, param1 := 1);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0109",
            category: "Duplicates",
            title: "Duplicate USING namespace import",
            description: "The same namespace is imported twice with `USING`.",
            sources: &[r#"
NAMESPACE ns1
    FUNCTION fn1 : INT
        fn1 := 0;
    END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK fb1
    USING ns1;
    USING ns1;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0110",
            category: "Duplicates",
            title: "Duplicate field in initializer expression",
            description: "A struct initializer assigns the same field twice.",
            sources: &[r#"
TYPE s1 : STRUCT
    field1 : INT;
    field2 : INT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK fb1
VAR
    x : s1 := (field1 := 1, field1 := 2);
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0111",
            category: "Duplicates",
            title: "Duplicate PROGRAM",
            description: "Two PROGRAM declarations have the same name.",
            sources: &[r#"
PROGRAM p1
END_PROGRAM

PROGRAM p1
END_PROGRAM
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0114",
            category: "Duplicates",
            title: "Duplicate TASK name",
            description: "Two TASKs in the same configuration have the same name.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(PRIORITY := 1);
        TASK task1(PRIORITY := 2);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0115",
            category: "Duplicates",
            title: "Duplicate PROGRAM instance name",
            description: "Two PROGRAM instances in the same configuration have the same name.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(PRIORITY := 1);
        PROGRAM inst1 WITH task1 : prog1;
        PROGRAM inst1 WITH task1 : prog1;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0116",
            category: "Duplicates",
            title: "Duplicate RESOURCE name",
            description: "Two RESOURCE blocks in the same configuration have the same name.",
            sources: &[r#"
CONFIGURATION config1
    RESOURCE res1 ON CPU
        TASK task1(PRIORITY := 1);
    END_RESOURCE
    RESOURCE res1 ON CPU
        TASK task2(PRIORITY := 1);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        // ── E02xx: Resolution / Semantic ─────────────────────────────────
        ErrorExample {
            code: "E0204",
            category: "Resolution",
            title: "No item found in scope",
            description: "A referenced name does not exist in the current scope.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x := unknown_var;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0205",
            category: "Resolution",
            title: "Incorrect number of function parameters",
            description: "A function call passes more or fewer parameters than expected.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        param1: INT;
        param2: REAL;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(0, 1.5, 5);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0206",
            category: "Resolution",
            title: "Unknown non-formal parameter",
            description: "A positional parameter in a function call exceeds the expected parameter count.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(0, 1);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0207",
            category: "Resolution",
            title: "Output parameter used as input",
            description: "A VAR_OUTPUT parameter cannot be used as a positional (non-formal) input.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        a: INT;
    END_VAR
    VAR_OUTPUT
        b: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(1, 2);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0208",
            category: "Resolution",
            title: "Unknown input parameter",
            description: "A named input parameter does not exist on the called function.",
            sources: &[r#"
FUNCTION fn1
     VAR_INPUT
        u: BOOL;
     END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(unknown := TRUE);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0209",
            category: "Resolution",
            title: "Unknown output parameter",
            description: "A named output parameter does not exist on the called function.",
            sources: &[r#"
FUNCTION fn1
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(unknown => TRUE);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0210",
            category: "Resolution",
            title: "No namespace item found",
            description: "The referenced item does not exist in the specified namespace path.",
            sources: &[r#"
NAMESPACE ns1
    FUNCTION fn1 : INT
        fn1 := 0;
    END_FUNCTION
END_NAMESPACE

FUNCTION_BLOCK fb1
VAR
    x : ns1.unknown;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0211",
            category: "Resolution",
            title: "No such field",
            description: "The type has no field with the given name.",
            sources: &[r#"
TYPE s1 : STRUCT
    field1 : INT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK fb1
VAR
    x : s1;
END_VAR
    x.field2 := 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0212",
            category: "Resolution",
            title: "Dereference of non-reference type",
            description: "The `^` dereference operator can only be used on `REF_TO` types.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x^ := 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0213",
            category: "Resolution",
            title: "Index into non-array type",
            description: "Array indexing can only be used on ARRAY types.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x[0] := 5;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0214",
            category: "Resolution",
            title: "Elementary type initialized with parentheses",
            description: "Elementary types like INT, BOOL, REAL cannot be initialized with `()`",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT := ();
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0215",
            category: "Resolution",
            title: "Function used as type",
            description: "A FUNCTION cannot be used as a variable type or data type.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := 0;
END_FUNCTION

FUNCTION_BLOCK fb1
VAR
    x : fn1;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0216",
            category: "Resolution",
            title: "USING namespace not found",
            description: "The namespace referenced in a `USING` directive does not exist.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    USING unknown_namespace;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0219",
            category: "Resolution",
            title: "Unknown TASK reference",
            description: "The task name referenced in a `WITH` clause does not exist in this configuration.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION config1
    RESOURCE res1 ON CPU
        PROGRAM inst1 WITH unknown_task : prog1;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0220",
            category: "Resolution",
            title: "External variable not found",
            description: "A `VAR_EXTERNAL` declaration references a name not present in any accessible `VAR_GLOBAL`.",
            sources: &[r#"
PROGRAM prog1
VAR_EXTERNAL
    missing_global : INT;
END_VAR
END_PROGRAM

CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : prog1;
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0221",
            category: "Resolution",
            title: "VAR_ACCESS type mismatch",
            description: "A `VAR_ACCESS` declaration's type does not match the referenced variable's actual type.",
            sources: &[r#"
PROGRAM prog1
VAR
    my_var : INT;
END_VAR
VAR_ACCESS
    accessor : my_var : BOOL;
END_VAR
END_PROGRAM

CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : prog1;
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0222",
            category: "Resolution",
            title: "Configuration instance unknown",
            description: "A `VAR_CONFIG` path references a program instance that does not exist.",
            sources: &[r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1);
    PROGRAM inst1 WITH t1 : MyProg;

    VAR_CONFIG
        noSuchInst.x : INT := 42;
    END_VAR
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0223",
            category: "Resolution",
            title: "Configuration field not found",
            description: "A `VAR_CONFIG` path references a field that does not exist on the resolved type.",
            sources: &[r#"
PROGRAM MyProg
    VAR
        x : INT;
    END_VAR
END_PROGRAM

CONFIGURATION MyCfg
    RESOURCE res1 ON CPU
        TASK t1(PRIORITY := 1);
        PROGRAM inst1 WITH t1 : MyProg;
    END_RESOURCE

    VAR_CONFIG
        inst1.nonexistent : INT := 42;
    END_VAR
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0224",
            category: "Resolution",
            title: "Non-variadic type for variable",
            description: "Only elementary types can be declared as variadic (`...`). Structs and other composite types cannot.",
            sources: &[r#"
TYPE MyStruct :
    STRUCT
        field1: INT;
    END_STRUCT
END_TYPE

FUNCTION sum_all : INT
    VAR_INPUT
        args: MyStruct...
    END_VAR
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0225",
            category: "Resolution",
            title: "Multiple items with same name in scope",
            description: "Two or more items are available in scope with the same name, creating ambiguity.",
            sources: &[r#"
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
"#],
            lint_rule: None,
        },
        // Assignment / call violations (moved from E10xx)
        ErrorExample {
            code: "E0226",
            category: "Resolution",
            title: "Assignment to a function block instance",
            description: "A variable whose declared type is callable — a FUNCTION_BLOCK instance, or a name that resolves to a FUNCTION — cannot be the target of an assignment. Instances are called, not copied; pass one as a VAR_IN_OUT parameter, or assign its individual members.",
            sources: &[r#"
FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
VAR
    inst : fb1;
END_VAR
    inst := 0;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0227",
            category: "Resolution",
            title: "Multiple variadic variables",
            description: "Only one variadic parameter is allowed per POU.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        a: INT...
        b: INT...
    END_VAR
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0228",
            category: "Resolution",
            title: "Direct type usage",
            description: "A type name cannot be used directly as a value. Types are not first-class values.",
            sources: &[r#"
TYPE b1 : INT
END_TYPE

FUNCTION fn1
    VAR_OUTPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn1(param1 => b1);
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0229",
            category: "Resolution",
            title: "Call non-callable type",
            description: "Attempting to call something that is not a function or instantiated function block.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: INT;
END_VAR
    test();
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0233",
            category: "Resolution",
            title: "Missing required parameter",
            description: "A FUNCTION call must supply every VAR_INPUT (inputs have no defaults on FUNCTIONs). Name the missing parameter in the call.",
            sources: &[r#"
FUNCTION add2 : INT
VAR_INPUT a : INT; b : INT; END_VAR
    add2 := a + b;
END_FUNCTION

FUNCTION f1 : INT
    f1 := add2(a := 1);
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0234",
            category: "Resolution",
            title: "VAR_IN_OUT argument must be a variable",
            description: "A VAR_IN_OUT parameter is passed by reference: the argument must be an assignable variable, not an expression or a literal.",
            sources: &[r#"
FUNCTION bump : INT
VAR_IN_OUT io : INT; END_VAR
    io := io + 1;
    bump := io;
END_FUNCTION

FUNCTION f1 : INT
    f1 := bump(io := 1 + 2);
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0235",
            category: "Resolution",
            title: "RETAIN in a stateless POU",
            description: "RETAIN and NON_RETAIN require instance storage. A FUNCTION or METHOD has none - its variables live for one call.",
            sources: &[r#"
FUNCTION f1 : INT
VAR_OUTPUT RETAIN q : INT; END_VAR
    f1 := 1;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0236",
            category: "Resolution",
            title: "VAR_IN_OUT bound with =>",
            description: "The `=>` arrow binds outputs. A VAR_IN_OUT parameter is bound with `:=` - it flows both ways through one variable.",
            sources: &[r#"
FUNCTION bump : INT
VAR_IN_OUT io : INT; END_VAR
    bump := io;
END_FUNCTION

FUNCTION f1 : INT
VAR y : INT; END_VAR
    f1 := bump(io => y);
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0238",
            category: "Resolution",
            title: "Program instance without a task",
            description: "Every program instance in a RESOURCE must be attached to a TASK with `WITH` - an unattached instance would never be scheduled.",
            sources: &[r#"
PROGRAM P
VAR n : INT; END_VAR
    n := n + 1;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        PROGRAM P1 : P;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0239",
            category: "Resolution",
            title: "Unschedulable task",
            description: "A TASK needs a cyclic INTERVAL to be scheduled. Event-driven forms (SINGLE) are not supported by the scan dispatcher.",
            sources: &[r#"
PROGRAM P
VAR n : INT; END_VAR
    n := n + 1;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(SINGLE := TRUE, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0243",
            category: "Scope",
            title: "Not representable on an extern FUNCTION",
            description: "An `{extern}` FUNCTION is a WASM import, and its interface is exactly its declaration: `VAR_INPUT` become the params (copies; aggregates as a pointer to a call-entry snapshot), scalar `VAR_OUTPUT` become the results in declaration order, and the return type is the last result. Three things cannot cross that boundary: `VAR_IN_OUT` (a pointer into caller storage with a mutation contract; an extern takes copies), an aggregate or STRING output (no WASM result type to ride), and statements (the import IS the body).",
            sources: &[r#"
{extern 'host' 'fill'}
FUNCTION fill : INT
VAR_IN_OUT buf : INT; END_VAR
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0244",
            category: "Scope",
            title: "Extern pragma outside a FUNCTION",
            description: "Only a FUNCTION lowers to a WASM import. On a FUNCTION_BLOCK, PROGRAM or METHOD the pragma used to be silently ignored, leaving the body it stood on empty and the import never declared. Wrap the import in a FUNCTION and call it from the block.",
            sources: &[r#"
{extern 'host' 'fb-extern'}
FUNCTION_BLOCK Modbus
VAR_INPUT n : INT; END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0242",
            category: "Scope",
            title: "More than one CONFIGURATION",
            description: "A workspace declares one CONFIGURATION. A POU is a type any configuration may use, so with two of them there is no answer to which global variables are in scope inside a POU. Describe another PLC in its own workspace.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION cfg1
    RESOURCE res1 ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst1 WITH t1 : prog1;
    END_RESOURCE
END_CONFIGURATION

CONFIGURATION cfg2
    RESOURCE res2 ON CPU
        TASK t2(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM inst2 WITH t2 : prog1;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0241",
            category: "Scope",
            title: "Unusable TASK priority",
            description: "A TASK's `PRIORITY` must be a number the compiler can represent (a 32-bit unsigned integer), where 0 is the most urgent. An unusable value would otherwise reach the scheduler as \"no priority\" and quietly sort last.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION cfg1
    RESOURCE res1 ON CPU
        TASK t1(INTERVAL := T#10ms, PRIORITY := 99999999999);
        PROGRAM inst1 WITH t1 : prog1;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0240",
            category: "Resolution",
            title: "Unsupported configuration element",
            description: "Program connections (`inp := %IW1`) and per-FB task associations inside a program configuration are not supported.",
            sources: &[r#"
PROGRAM P
VAR_INPUT inp : INT; END_VAR
VAR n : INT; END_VAR
    n := inp;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P (inp := %IW1);
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0237",
            category: "Resolution",
            title: "Ambiguous overloaded call",
            description: "A call matches more than one FUNCTION overload and no overload is an exact \
                          match for every argument, so the compiler refuses to guess. Disambiguate \
                          with a typed literal or an explicit conversion (e.g. `DINT#5`).",
            sources: &[r#"
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
    // SINT widens to both DINT and LINT — neither overload is exact.
    caller := pick(y);
END_FUNCTION
"#],
            lint_rule: None,
        },
        // ── E03xx: Type system ───────────────────────────────────────────
        ErrorExample {
            code: "E0301",
            category: "Type System",
            title: "Type mismatch",
            description: "The types on both sides of an assignment or expression are incompatible.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x := 'hello';
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0302",
            category: "Type System",
            title: "Types not comparable",
            description: "The two types cannot be compared with comparison operators.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
    y : STRING;
    z : BOOL;
END_VAR
    z := x > y;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0303",
            category: "Type System",
            title: "Types not addable",
            description: "The two types cannot be used with addition or subtraction operators.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
    y : STRING;
    z : INT;
END_VAR
    z := x + y;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0304",
            category: "Type System",
            title: "Types not multiplicable",
            description: "The two types cannot be used with multiplication or division operators.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
    y : STRING;
    z : INT;
END_VAR
    z := x * y;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // E0309 — Invalid literal (many subtypes)
        ErrorExample {
            code: "E0309",
            category: "Type System",
            title: "Invalid literal",
            description: "A literal value cannot be inferred to the target type.",
            sources: &[r#"
FUNCTION fn1 : INT
VAR
    x : LINT;
END_VAR
    x := 5.5;
END_FUNCTION
"#],
            lint_rule: None,
        },
        // ── E0309 subtypes: generated for all elementary types ──────────
        // Signed integers — overflow
        ErrorExample {
            code: "E0309_SINT",
            category: "Type System",
            title: "Invalid SINT literal",
            description: "The literal value exceeds the range of SINT (-128..127).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: SINT := 128;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_INT",
            category: "Type System",
            title: "Invalid INT literal",
            description: "The literal value exceeds the range of INT (-32768..32767).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: INT := 32768;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_DINT",
            category: "Type System",
            title: "Invalid DINT literal",
            description: "The literal value exceeds the range of DINT (-2147483648..2147483647).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: DINT := 2147483648;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_LINT",
            category: "Type System",
            title: "Invalid LINT literal",
            description: "The literal value exceeds the range of LINT.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: LINT := 9999999999999999999;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // Unsigned integers — overflow
        ErrorExample {
            code: "E0309_USINT",
            category: "Type System",
            title: "Invalid USINT literal",
            description: "The literal value exceeds the range of USINT/BYTE (0..255).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: USINT := 256;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_UINT",
            category: "Type System",
            title: "Invalid UINT literal",
            description: "The literal value exceeds the range of UINT/WORD (0..65535).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: UINT := 65536;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_UDINT",
            category: "Type System",
            title: "Invalid UDINT literal",
            description: "The literal value exceeds the range of UDINT/DWORD (0..4294967295).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: UDINT := 4294967296;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_ULINT",
            category: "Type System",
            title: "Invalid ULINT literal",
            description: "The literal value exceeds the range of ULINT/LWORD.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: ULINT := 99999999999999999999;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // Unsigned integers — negative sign
        ErrorExample {
            code: "E0309_USINT_NEG",
            category: "Type System",
            title: "Negative USINT literal",
            description: "Unsigned integer types cannot hold negative values.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: USINT := -1;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_UINT_NEG",
            category: "Type System",
            title: "Negative UINT literal",
            description: "Unsigned integer types cannot hold negative values.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: UINT := -1;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_UDINT_NEG",
            category: "Type System",
            title: "Negative UDINT literal",
            description: "Unsigned integer types cannot hold negative values.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: UDINT := -1;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_ULINT_NEG",
            category: "Type System",
            title: "Negative ULINT literal",
            description: "Unsigned integer types cannot hold negative values.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: ULINT := -1;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // BYTE/WORD/DWORD/LWORD — overflow (same check as unsigned)
        ErrorExample {
            code: "E0309_BYTE",
            category: "Type System",
            title: "Invalid BYTE literal",
            description: "The literal value exceeds the range of BYTE (0..255).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: BYTE := 256;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_WORD",
            category: "Type System",
            title: "Invalid WORD literal",
            description: "The literal value exceeds the range of WORD (0..65535).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: WORD := 65536;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_DWORD",
            category: "Type System",
            title: "Invalid DWORD literal",
            description: "The literal value exceeds the range of DWORD (0..4294967295).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: DWORD := 4294967296;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_LWORD",
            category: "Type System",
            title: "Invalid LWORD literal",
            description: "The literal value exceeds the range of LWORD.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: LWORD := 99999999999999999999;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // Boolean
        ErrorExample {
            code: "E0309_BOOL",
            category: "Type System",
            title: "Invalid BOOL literal",
            description: "The literal value is not valid for a BOOL type (must be 0, 1, TRUE, or FALSE).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    test: BOOL := 256;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // Strings — length overflow
        ErrorExample {
            code: "E0309_STRING_LEN",
            category: "Type System",
            title: "STRING literal exceeds max length",
            description: "A STRING literal exceeds the declared maximum length.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    s: STRING[2] := 'hello';
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_WSTRING_LEN",
            category: "Type System",
            title: "Double-quoted (legacy WSTRING) literal exceeds max length",
            description: "WSTRING no longer exists: STRING is a single UTF-8 type, and the legacy double-quoted literal form now resolves to STRING. A double-quoted literal is therefore measured against the declared maximum length of the sized STRING it initializes, exactly as a single-quoted one is. The length compared is the literal's UTF-8 byte count.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    s: STRING[2] := "hello";
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_CHAR",
            category: "Type System",
            title: "Invalid CHAR literal length",
            description: "A CHAR literal must be exactly 1 character.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    c: CHAR := CHAR#'ab';
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_WCHAR",
            category: "Type System",
            title: "Invalid CHAR literal length (double-quoted form)",
            description: "A CHAR literal must be exactly 1 character. The double-quoted form (historically WCHAR) resolves to CHAR and obeys the same rule.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    c: CHAR := CHAR#"ab";
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // Date/time literals
        ErrorExample {
            code: "E0309_TOD",
            category: "Type System",
            title: "Invalid TOD literal",
            description: "The time-of-day literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    t: TOD := TOD#25:36:55.36;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_LTOD",
            category: "Type System",
            title: "Invalid LTOD literal",
            description: "The long time-of-day literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    t: LTOD := LTOD#25:36:55.36;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_DATE",
            category: "Type System",
            title: "Invalid DATE literal",
            description: "The date literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    d: DATE := DATE#2024-13-45;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_LDATE",
            category: "Type System",
            title: "Invalid LDATE literal",
            description: "The long date literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    d: LDATE := LDATE#2024-13-45;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_DT",
            category: "Type System",
            title: "Invalid DT literal",
            description: "The date-and-time literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    dt: DT := DT#1984-06-25-25:36:55.360;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0309_LDT",
            category: "Type System",
            title: "Invalid LDT literal",
            description: "The long date-and-time literal is not valid.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    dt: LDT := LDT#1984-06-25-25:36:55.360;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0317",
            category: "Type System",
            title: "Non-variadic fold parameter",
            description: "The `...` fold operator can only be used on variadic parameters.",
            sources: &[r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT
    END_VAR
    sum_all := ...args+
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0318",
            category: "Type System",
            title: "Unsupported operator for type",
            description: "The operator cannot be applied to this type.",
            sources: &[r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK

PROGRAM A
    VAR
        x: INT;
    END_VAR
    x := 5 + Motor;
END_PROGRAM
"#],
            lint_rule: None,
        },
        // ── E04xx: Visibility ────────────────────────────────────────────
        ErrorExample {
            code: "E0401",
            category: "Visibility",
            title: "Cannot access PRIVATE item",
            description: "PRIVATE items can only be accessed from within the same POU.",
            sources: &[r#"
CLASS Base
    METHOD PRIVATE myPrivateMethod : INT  END_METHOD
END_CLASS

CLASS Mid EXTENDS Base
    METHOD testMethod : INT
        SUPER.myPrivateMethod();
    END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0402",
            category: "Visibility",
            title: "Cannot access INTERNAL item",
            description: "INTERNAL items can only be accessed from within the same namespace.",
            sources: &[r#"
NAMESPACE ns1
    CLASS Base
        METHOD INTERNAL myInternalMethod : INT  END_METHOD
    END_CLASS
END_NAMESPACE

NAMESPACE ns2
    USING ns1;

    CLASS Mid EXTENDS Base
        METHOD testMethod : INT
            SUPER.myInternalMethod();
        END_METHOD
    END_CLASS
END_NAMESPACE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0403",
            category: "Visibility",
            title: "Cannot access PROTECTED item",
            description: "PROTECTED items can only be accessed within the same POU or derived POUs.",
            sources: &[r#"
CLASS Base
    METHOD PROTECTED myProtectedMethod  END_METHOD
END_CLASS

FUNCTION_BLOCK fn1
    VAR
        obj: Base;
    END_VAR
    obj.myProtectedMethod();
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0404",
            category: "Visibility",
            title: "Cannot access test-only item",
            description: "Items annotated with `{test}` can only be accessed from other test-annotated code.",
            sources: &[r#"
{test}
FUNCTION test_helper : INT
END_FUNCTION

FUNCTION_BLOCK fb1
    test_helper();
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // ── E05xx: Inheritance ───────────────────────────────────────────
        ErrorExample {
            code: "E0501",
            category: "Inheritance",
            title: "SUPER() body not valid here",
            description: "`SUPER()` (body call) is not valid in this context.",
            sources: &[r#"
CLASS fb1
    METHOD method1
        SUPER()
    END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0502",
            category: "Inheritance",
            title: "SUPER not valid here",
            description: "`SUPER` is not valid in a POU that does not extend another.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := SUPER.x;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0503",
            category: "Inheritance",
            title: "THIS not valid here",
            description: "`THIS` is not valid in a FUNCTION context.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := THIS.x;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0504",
            category: "Inheritance",
            title: "Cannot override FINAL method",
            description: "Methods marked as FINAL cannot be overridden in derived classes.",
            sources: &[r#"
CLASS Base
    METHOD PUBLIC FINAL myMethod : INT  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE myMethod : INT  END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0505",
            category: "Inheritance",
            title: "Missing OVERRIDE keyword",
            description: "When redefining a method from a base class, the OVERRIDE keyword is required.",
            sources: &[r#"
CLASS Base
    METHOD PUBLIC myMethod : INT  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
    METHOD PUBLIC myMethod : INT  END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0506",
            category: "Inheritance",
            title: "Missing ABSTRACT method implementation",
            description: "Derived POUs must implement all ABSTRACT methods from their base class.",
            sources: &[r#"
CLASS ABSTRACT Base
    METHOD PUBLIC ABSTRACT myMethod : INT  END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0507",
            category: "Inheritance",
            title: "Invalid OVERRIDE usage",
            description: "OVERRIDE is only valid when a method with the same name exists in the base class.",
            sources: &[r#"
CLASS Base
END_CLASS

CLASS Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE myMethod : INT  END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0508",
            category: "Inheritance",
            title: "ABSTRACT class has no abstract methods",
            description: "A class marked as ABSTRACT must have at least one abstract method.",
            sources: &[r#"
CLASS ABSTRACT Base
    METHOD PUBLIC myMethod : INT  END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0509",
            category: "Inheritance",
            title: "Unimplemented interface method",
            description: "A class implementing an interface must provide all required methods.",
            sources: &[r#"
INTERFACE IMyInterface
    METHOD myMethod : INT  END_METHOD
END_INTERFACE

CLASS MyClass IMPLEMENTS IMyInterface
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0512",
            category: "Inheritance",
            title: "Method signature mismatch",
            description: "A method override has a different number of parameters or different parameter types than the base method.",
            sources: &[r#"
CLASS Base
    METHOD PUBLIC myMethod : INT
        VAR_INPUT a : INT; END_VAR
    END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE myMethod : INT
        VAR_INPUT a : INT; b : INT; END_VAR
    END_METHOD
END_CLASS
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0513",
            category: "Inheritance",
            title: "SUPER but no EXTENDS clause",
            description: "`SUPER` is used but the current POU has no `EXTENDS` clause.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    SUPER.method1()
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0514",
            category: "Inheritance",
            title: "Interface type only allowed as a parameter",
            description: "An interface type may appear only as a `VAR_INPUT` or `VAR_IN_OUT` parameter, where it is monomorphized to the concrete type passed by the caller. It cannot be a stored `VAR`, member, `VAR_OUTPUT`, `VAR_TEMP`, or global.",
            sources: &[r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM Main
    VAR
        dev : ITF1;
    END_VAR
END_PROGRAM
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0515",
            category: "Inheritance",
            title: "Interface not allowed as a return type",
            description: "An interface may not be used as a function or method return type: the concrete type would flow from callee to caller and could not be resolved at compile time.",
            sources: &[r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION Make : ITF1
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0516",
            category: "Inheritance",
            title: "Interface not allowed nested in an aggregate",
            description: "An interface may not be nested inside an array, reference, or struct (e.g. `ARRAY OF ITF1`, `REF_TO ITF1`, or a struct field). Such a placement is stored, heterogeneous state that cannot be monomorphized.",
            sources: &[r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

PROGRAM Main
    VAR
        arr : ARRAY[0..2] OF ITF1;
    END_VAR
END_PROGRAM
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0517",
            category: "Inheritance",
            title: "Assignment to an interface parameter",
            description: "An interface `VAR_IN_OUT` parameter is a fixed binding to the concrete type passed by the caller. Reassigning it would break monomorphization (the body is specialized to one concrete type), so it can be used — methods called, passed on — but not reassigned.",
            sources: &[r#"
INTERFACE ITF1
    METHOD DoWork END_METHOD
END_INTERFACE

FUNCTION_BLOCK Impl IMPLEMENTS ITF1
    METHOD DoWork END_METHOD
END_FUNCTION_BLOCK

FUNCTION Use : INT
    VAR_IN_OUT dev : ITF1; END_VAR
    VAR other : Impl; END_VAR
    dev := other;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0518",
            category: "Inheritance",
            title: "SUPER() called in a method",
            description: "`SUPER()` (the base function-block body call) may only appear in the function block body, not in a method of a function block.",
            sources: &[r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
    METHOD m1
        SUPER()
    END_METHOD
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0519",
            category: "Inheritance",
            title: "SUPER() called more than once",
            description: "The call of `SUPER()` shall occur once in the function block body.",
            sources: &[r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
    SUPER();
    SUPER();
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0520",
            category: "Inheritance",
            title: "SUPER() called inside a loop",
            description: "The call of `SUPER()` shall not be in a loop (`FOR`/`WHILE`/`REPEAT`).",
            sources: &[r#"
FUNCTION_BLOCK base
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
    VAR i : INT; END_VAR
    FOR i := 1 TO 3 DO
        SUPER();
    END_FOR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0521",
            category: "Inheritance",
            title: "Inherited variable name shadowed",
            description: "The names of the variables in the base and the derived function blocks shall be unique. A derived function block may not redeclare a variable name inherited from a base.",
            sources: &[r#"
FUNCTION_BLOCK base
VAR c : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
VAR c : INT; END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // ── E06xx: Arrays ────────────────────────────────────────────────
        ErrorExample {
            code: "E0601",
            category: "Arrays",
            title: "Invalid array lower bound",
            description: "The lower bound of an array declaration is not a valid constant value.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[TRUE..5] OF INT;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0602",
            category: "Arrays",
            title: "Invalid array upper bound",
            description: "The upper bound of an array declaration is not a valid constant value.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[0..TRUE] OF INT;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0603",
            category: "Arrays",
            title: "Upper bound must be greater than lower bound",
            description: "In an array declaration, the upper bound must be greater than the lower bound.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[10..0] OF INT;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0605",
            category: "Arrays",
            title: "Too many elements in array initializer",
            description: "The array initializer provides more elements than the declared array size.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[0..1] OF INT := [1, 2, 3, 4, 5];
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0607",
            category: "Arrays",
            title: "Invalid array index value",
            description: "An array initializer repeat count is not a valid integer value (e.g., overflow).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[0..2] OF INT := [99999999999999999999(0)];
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0608",
            category: "Arrays",
            title: "Constant index out of bounds",
            description: "A constant subscript outside the array's declared bounds is provable at compile time and rejected here, instead of faulting the scan at runtime. Checked per dimension.",
            sources: &[r#"
FUNCTION f1 : INT
VAR
    a : ARRAY[0..2] OF INT;
END_VAR
    a[5] := 1;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0609",
            category: "Arrays",
            title: "Array index must be an integer",
            description: "A subscript must be of an integer type (ANY_INT; subranges index like their base type). A BOOL, REAL or STRING cannot address an element.",
            sources: &[r#"
FUNCTION f1 : INT
VAR
    a : ARRAY[0..2] OF INT;
    r : REAL;
END_VAR
    a[r] := 1;
END_FUNCTION
"#],
            lint_rule: None,
        },
        // ── E07xx: Enums ─────────────────────────────────────────────────
        ErrorExample {
            code: "E0701",
            category: "Enums",
            title: "Invalid enum type",
            description: "Only numeric integer types are allowed for ENUM base types.",
            sources: &[r#"
TYPE e1 : REAL (Red, Green, Blue)
END_TYPE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0702",
            category: "Enums",
            title: "Not an ENUM type",
            description: "The `#` enum access syntax was used on a name that does not denote an ENUM — a non-ENUM type name, or a variable whose type is not an ENUM. Elementary type keywords (`INT#16`, `BOOL#TRUE`) are typed literals, a separate syntax that never reaches this check.",
            sources: &[r#"TYPE MyInt : INT; END_TYPE

FUNCTION fn1 : INT
    fn1 := MyInt#Red;
END_FUNCTION
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0703",
            category: "Enums",
            title: "ENUM variant not found",
            description: "The referenced variant does not exist on this ENUM type.",
            sources: &[r#"
TYPE Color : (Red, Green, Blue) END_TYPE

FUNCTION_BLOCK fb1
VAR
    c : Color;
END_VAR
    c := Color#Yellow;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        // ── E08xx: Subranges ─────────────────────────────────────────────
        ErrorExample {
            code: "E0801",
            category: "Subranges",
            title: "Invalid subrange type",
            description: "Only numeric integer types are allowed for subrange types.",
            sources: &[r#"
TYPE s1 : REAL (0..100)
END_TYPE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0802",
            category: "Subranges",
            title: "Value outside the subrange",
            description: "A constant assigned to a subrange-typed variable must lie within the declared bounds.",
            sources: &[r#"
FUNCTION f1 : INT
VAR x : INT (0..10); END_VAR
    x := 99;
    f1 := x;
END_FUNCTION
"#],
            lint_rule: None,
        },
        // ── E09xx: Recursion ─────────────────────────────────────────────
        ErrorExample {
            code: "E0901",
            category: "Recursion",
            title: "Direct recursion",
            description: "A type contains itself directly, creating infinite recursion.",
            sources: &[r#"
TYPE s1 : STRUCT
    field1 : s1;
END_STRUCT
END_TYPE
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E0902",
            category: "Recursion",
            title: "Mutual recursion",
            description: "Two or more types reference each other, creating a recursive cycle.",
            sources: &[r#"
TYPE a : STRUCT
    field1 : b;
END_STRUCT
END_TYPE

TYPE b : STRUCT
    field1 : a;
END_STRUCT
END_TYPE
"#],
            lint_rule: None,
        },
        // ── E10xx: Control flow ──────────────────────────────────────────
        ErrorExample {
            code: "E1001",
            category: "Control Flow",
            title: "CONTINUE outside loop",
            description: "`CONTINUE` can only be used inside a loop (FOR, WHILE, REPEAT).",
            sources: &[r#"
FUNCTION_BLOCK fb1
    CONTINUE;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E1002",
            category: "Control Flow",
            title: "EXIT outside loop",
            description: "`EXIT` can only be used inside a loop (FOR, WHILE, REPEAT).",
            sources: &[r#"
FUNCTION_BLOCK fb1
    EXIT;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E1003",
            category: "Control Flow",
            title: "Possibly null dereference",
            description: "Dereferencing a reference that may be null or uninitialized.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    ptr: REF_TO INT;
    x: INT;
END_VAR
    x := ptr^;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E1004",
            category: "Control Flow",
            title: "Assignment to constant",
            description: "Cannot assign to a variable declared as CONSTANT.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR CONSTANT
    x : INT := 10;
END_VAR
    x := 20;
END_FUNCTION_BLOCK
"#],
            lint_rule: None,
        },
        ErrorExample {
            code: "E1005",
            category: "Control Flow",
            title: "FOR control is not a variable",
            description: "A FOR statement's control must be a bare variable name. A struct field, an array element, a dereference or a bit access cannot be a loop counter.",
            sources: &[r#"
TYPE Rec : STRUCT i : INT; END_STRUCT; END_TYPE

FUNCTION f1 : INT
VAR
    r : Rec;
END_VAR
    FOR r.i := 0 TO 3 DO
        f1 := f1 + 1;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: None,
        },
        // ── Pragma ──────────────────────────────────────────────
        ErrorExample {
            code: "L0001",
            category: "Pragma",
            title: "Info pragma notice",
            description: "A call targets a POU annotated with `{info = '...'}`, providing an informational notice at the call site.",
            sources: &[r#"
{info = 'prefer new_fn for better performance'}
FUNCTION old_fn : INT
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; END_VAR
    x := old_fn();
END_FUNCTION
"#],
            lint_rule: Some("warn-pragma"),
        },
        ErrorExample {
            code: "L0002",
            category: "Pragma",
            title: "Warning pragma notice",
            description: "A call targets a POU annotated with `{warn = '...'}`, indicating deprecation or other warnings at the call site.",
            sources: &[r#"
{warn = 'this function is deprecated, use fn2 instead'}
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; END_VAR
    x := fn1();
END_FUNCTION
"#],
            lint_rule: Some("warn-pragma"),
        },
        ErrorExample {
            code: "L0003",
            category: "Pragma",
            title: "Invalid pragma for POU",
            description: "A pragma is used on a POU type where it is not valid. For example, `{test}` is only valid on FUNCTION — not on PROGRAM, FUNCTION_BLOCK or METHOD.",
            sources: &[r#"
{test}
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("invalid-pragma"),
        },
        ErrorExample {
            code: "L0004",
            category: "Pragma",
            title: "Once-function called multiple times",
            description: "A function, function block, or method marked with `{once}` is called more than once in the same body. The `{once}` pragma indicates it should only be invoked once per execution cycle.",
            sources: &[r#"
{once}
FUNCTION init : INT
    init := 42;
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; y : INT; END_VAR
    x := init();
    y := init();
    caller := x + y;
END_FUNCTION
"#],
            lint_rule: Some("once-violation"),
        },
        // ── L01xx: Linter Info ────────────────────────────────────
        ErrorExample {
            code: "L0101",
            category: "Linter Info",
            title: "Unused variable",
            description: "A variable is declared but never used in the body.",
            sources: &[r#"
FUNCTION fn1 : INT
VAR
    x : INT;
    y : INT;
END_VAR
    fn1 := x;
END_FUNCTION
"#],
            lint_rule: Some("unused-variable"),
        },
        ErrorExample {
            code: "L0102",
            category: "Linter Info",
            title: "Variable shadows POU",
            description: "A variable name shadows a POU (function, function block, class, etc.) available in scope.",
            sources: &[
                r#"
FUNCTION_BLOCK PrintLog
END_FUNCTION_BLOCK
"#,
                r#"
FUNCTION test : INT
VAR
    PrintLog : BOOL;
END_VAR
    PrintLog := TRUE;
    test := 0;
END_FUNCTION
"#,
            ],
            lint_rule: Some("shadowing-variable"),
        },
        ErrorExample {
            code: "L0103",
            category: "Linter Info",
            title: "Duplicate variable section",
            description: "The same variable section type (VAR, VAR_INPUT, etc.) appears more than once in a POU. Merge them into one.",
            sources: &[r#"
FUNCTION fn1 : INT
VAR
    x : INT;
END_VAR
VAR
    y : INT;
END_VAR
    fn1 := x + y;
END_FUNCTION
"#],
            lint_rule: Some("duplicate-var-section"),
        },
        ErrorExample {
            code: "L0104",
            category: "Linter Info",
            title: "Negated condition",
            description: "An `IF NOT ... THEN ... ELSE` can be simplified by swapping the branches and removing the negation.",
            sources: &[r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF NOT flag THEN
        test := 0;
    ELSE
        test := 1;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("negated-condition"),
        },
        ErrorExample {
            code: "L0106",
            category: "Linter Info",
            title: "Unnecessary ELSE",
            description: "The ELSE branch is unnecessary because all preceding IF/ELSIF branches end with RETURN, EXIT, or CONTINUE.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 0 THEN
        test := 1;
        RETURN;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("unnecessary-else"),
        },
        ErrorExample {
            code: "L0107",
            category: "Linter Info",
            title: "Boolean comparison",
            description: "A comparison with a boolean literal can be simplified. Use the variable directly or negate it.",
            sources: &[r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag = TRUE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("bool-comparison"),
        },
        ErrorExample {
            code: "L0108",
            category: "Linter Info",
            title: "Redundant NOT",
            description: "A double negation (`NOT NOT x`) can be simplified to just `x`.",
            sources: &[r#"
FUNCTION test : BOOL
VAR
    flag : BOOL;
END_VAR
    test := NOT NOT flag;
END_FUNCTION
"#],
            lint_rule: Some("redundant-not"),
        },
        ErrorExample {
            code: "L0109",
            category: "Linter Info",
            title: "Duplicate namespace",
            description: "The same namespace is declared more than once in the same file. Merge them into one.",
            sources: &[r#"
NAMESPACE Utils
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE

NAMESPACE Utils
    FUNCTION fn2 : INT
    END_FUNCTION
END_NAMESPACE
"#],
            lint_rule: Some("duplicate-namespace"),
        },
        ErrorExample {
            code: "L0112",
            category: "Linter Info",
            title: "Duplicate configuration",
            description: "Same-named CONFIGURATION blocks merge, which is how VAR_GLOBALs are split across files. Two in ONE file separate nothing — merge them.",
            sources: &[r#"
CONFIGURATION Plant
    VAR_GLOBAL
        a : INT;
    END_VAR
END_CONFIGURATION

CONFIGURATION Plant
    VAR_GLOBAL
        b : INT;
    END_VAR
END_CONFIGURATION
"#],
            lint_rule: Some("duplicate-configuration"),
        },
        ErrorExample {
            code: "L0110",
            category: "Linter Info",
            title: "Single-element array",
            description: "An array dimension has equal lower and upper bounds, resulting in an array with only one element.",
            sources: &[r#"
FUNCTION test : INT
VAR
    arr : ARRAY[5..5] OF INT;
END_VAR
    test := arr[5];
END_FUNCTION
"#],
            lint_rule: Some("single-element-array"),
        },
        ErrorExample {
            code: "L0111",
            category: "Linter Info",
            title: "Negated comparison",
            description: "`NOT (x = y)` can be simplified to `x <> y`, and similar for other comparison operators.",
            sources: &[r#"
FUNCTION test : BOOL
VAR
    x : INT;
    y : INT;
END_VAR
    test := NOT (x = y);
END_FUNCTION
"#],
            lint_rule: Some("negated-comparison"),
        },
        // ── L02xx: Linter Hint ────────────────────────────────────
        ErrorExample {
            code: "L0201",
            category: "Linter Hint",
            title: "Unused import",
            description: "A `USING` directive imports a namespace that is never referenced.",
            sources: &[
                r#"
NAMESPACE Tools
    FUNCTION_BLOCK Logger
    END_FUNCTION_BLOCK
END_NAMESPACE
"#,
                r#"
FUNCTION_BLOCK fb1
    USING Tools;
END_FUNCTION_BLOCK
"#,
            ],
            lint_rule: Some("unused-import"),
        },
        ErrorExample {
            code: "L0202",
            category: "Linter Hint",
            title: "Unused return value",
            description: "A function call discards a return value. If the return value is intentionally ignored, assign it to a variable.",
            sources: &[r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION_BLOCK fb1
    add(a := 1, b := 2);
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("unused-return-type"),
        },
        ErrorExample {
            code: "L0203",
            category: "Linter Hint",
            title: "CASE without ELSE",
            description: "A CASE statement has no ELSE branch, which may leave unhandled cases.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    CASE x OF
        1: test := 1;
        2: test := 2;
    END_CASE;
END_FUNCTION
"#],
            lint_rule: Some("case-without-else"),
        },
        ErrorExample {
            code: "L0204",
            category: "Linter Hint",
            title: "Missing input parameter",
            description: "A FUNCTION_BLOCK or PROGRAM call does not pass every declared `VAR_INPUT`. \
                          This is not an error - the instance keeps the input's previous value - but \
                          an unwired input is usually an oversight. A FUNCTION call that omits an \
                          input is the hard error E0233 instead.",
            sources: &[r#"
FUNCTION_BLOCK ramp
VAR_INPUT
    target : INT;
    rate : INT;
END_VAR
VAR_OUTPUT
    value : INT;
END_VAR
    value := target;
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
VAR
    r : ramp;
END_VAR
    r(target := 100);
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("missing-input-param"),
        },
        ErrorExample {
            code: "L0205",
            category: "Linter Info",
            title: "Uninitialized output",
            description: "A `VAR_OUTPUT` variable is never assigned in the body. The instance field \
                          is zero-initialized, so this is permitted - but an output the body never \
                          writes is usually an unfinished one.",
            sources: &[r#"
FUNCTION_BLOCK counter
VAR_INPUT
    step : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
VAR
    n : INT;
END_VAR
    n := n + step;
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("uninitialized-output"),
        },
        ErrorExample {
            code: "L0206",
            category: "Linter Hint",
            title: "Empty body",
            description: "A function, function block, method, or program has an empty body.",
            sources: &[r#"
FUNCTION compute : INT
END_FUNCTION
"#],
            lint_rule: Some("empty-body"),
        },
        ErrorExample {
            code: "L0207",
            category: "Linter Hint",
            title: "Empty CASE branch",
            description: "A CASE branch has no statements, which may indicate a missing implementation.",
            sources: &[r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1:
        2: test := 20;
    END_CASE;
END_FUNCTION
"#],
            lint_rule: Some("empty-case-branch"),
        },
        ErrorExample {
            code: "L0208",
            category: "Linter Hint",
            title: "Unnecessary parentheses",
            description: "Parentheses around a simple variable or literal have no effect and can be removed.",
            sources: &[r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    test := (x);
END_FUNCTION
"#],
            lint_rule: Some("unnecessary-parens"),
        },
        ErrorExample {
            code: "L0209",
            category: "Linter Hint",
            title: "Yoda condition",
            description: "A literal value appears on the left side of a comparison. Swap operands for conventional order.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF 5 = x THEN
        test := 1;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("yoda-condition"),
        },
        ErrorExample {
            code: "L0210",
            category: "Linter Hint",
            title: "Collapsible IF",
            description: "Two nested IF statements without ELSE branches can be collapsed into a single `IF a AND b THEN`.",
            sources: &[r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    IF a THEN
        IF b THEN
            test := 1;
        END_IF;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("collapsible-if"),
        },
        ErrorExample {
            code: "L0211",
            category: "Linter Hint",
            title: "Empty IF branch",
            description: "An IF or ELSIF branch has no statements.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : BOOL;
END_VAR
    IF x THEN
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("empty-if-branch"),
        },
        ErrorExample {
            code: "L0212",
            category: "Linter Hint",
            title: "Effectless statement",
            description: "A statement has no side effects and does nothing.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x;
    test := 0;
END_FUNCTION
"#],
            lint_rule: Some("effectless-statement"),
        },
        ErrorExample {
            code: "L0213",
            category: "Linter Hint",
            title: "Empty loop body",
            description: "A FOR, WHILE, or REPEAT loop has no statements in its body.",
            sources: &[r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("empty-loop-body"),
        },
        ErrorExample {
            code: "L0214",
            category: "Linter Hint",
            title: "Empty type declaration",
            description: "A STRUCT has no fields or an ENUM has no variants.",
            sources: &[r#"
TYPE EmptyStruct : STRUCT
END_STRUCT;
END_TYPE
"#],
            lint_rule: Some("empty-type"),
        },
        ErrorExample {
            code: "L0215",
            category: "Linter Info",
            title: "Explicit default FOR step",
            description: "`BY 1` is the default step - writing it out adds nothing.",
            sources: &[r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 5 BY 1 DO
        test := test + 1;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("default-for-step"),
        },
        // ── L03xx: Linter Warning ─────────────────────────────────
        ErrorExample {
            code: "L0301",
            category: "Linter Warning",
            title: "Unreachable code",
            description: "A statement appears after an unconditional control flow statement (RETURN, EXIT, CONTINUE) and can never be reached.",
            sources: &[r#"
FUNCTION test : INT
    test := 1;
    RETURN;
    test := 2;
END_FUNCTION
"#],
            lint_rule: Some("dead-code"),
        },
        ErrorExample {
            code: "L0302",
            category: "Linter Warning",
            title: "FOR loop step sign mismatch",
            description: "The FOR loop step direction does not match the bounds direction.",
            sources: &[r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 10 BY -1 DO
        test := i;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("for-loop-step-sign"),
        },
        ErrorExample {
            code: "L0303",
            category: "Linter Warning",
            title: "Assignment to input variable",
            description: "A `VAR_INPUT` variable is being assigned inside the POU body. Inputs are meant to be set by callers.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR_INPUT
    x : INT;
END_VAR
    x := 42;
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("input-assignment"),
        },
        ErrorExample {
            code: "L0304",
            category: "Linter Warning",
            title: "Constant condition",
            description: "A condition in an IF, WHILE, or REPEAT statement is always TRUE or always FALSE.",
            sources: &[r#"
FUNCTION test : INT
    IF TRUE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("constant-condition"),
        },
        ErrorExample {
            code: "L0305",
            category: "Linter Warning",
            title: "Division by zero",
            description: "A division or modulo operation uses a literal zero as the divisor.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 10 / 0;
    test := x;
END_FUNCTION
"#],
            lint_rule: Some("division-by-zero"),
        },
        ErrorExample {
            code: "L0306",
            category: "Linter Warning",
            title: "Duplicate CASE value",
            description: "Two or more CASE branches use the same selector value. Only the first branch will ever match.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    CASE x OF
        1: test := 10;
        1: test := 20;
    END_CASE;
END_FUNCTION
"#],
            lint_rule: Some("duplicate-case"),
        },
        ErrorExample {
            code: "L0307",
            category: "Linter Warning",
            title: "FOR loop with zero step",
            description: "A FOR loop with a step of 0 will never terminate.",
            sources: &[r#"
FUNCTION test : INT
VAR i : INT; END_VAR
    FOR i := 0 TO 10 BY 0 DO
        test := i;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("for-zero-step"),
        },
        ErrorExample {
            code: "L0308",
            category: "Linter Warning",
            title: "Loop variable modified",
            description: "A FOR loop control variable is modified inside the loop body. This can cause unexpected iteration behavior.",
            sources: &[r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        i := i + 2;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("loop-var-modified"),
        },
        ErrorExample {
            code: "L0309",
            category: "Linter Warning",
            title: "Self-assignment",
            description: "A variable is assigned to itself, which has no effect.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x;
    test := 0;
END_FUNCTION
"#],
            lint_rule: Some("self-assignment"),
        },
        ErrorExample {
            code: "L0310",
            category: "Linter Warning",
            title: "Self-comparison",
            description: "A variable is compared to itself. The result is always TRUE or always FALSE.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x = x THEN
        test := 1;
    END_IF;
END_FUNCTION
"#],
            lint_rule: Some("self-comparison"),
        },
        ErrorExample {
            code: "L0311",
            category: "Linter Warning",
            title: "Identical subexpressions",
            description: "Both sides of a boolean operator are identical. This is likely a copy-paste error.",
            sources: &[r#"
FUNCTION test : BOOL
VAR
    a : BOOL;
END_VAR
    test := a AND a;
END_FUNCTION
"#],
            lint_rule: Some("identical-sub-expr"),
        },
        ErrorExample {
            code: "L0312",
            category: "Linter Warning",
            title: "Identity operation",
            description: "An operation with an identity value has no effect (adding 0, multiplying by 1, etc.).",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x + 0;
    test := x * 1;
END_FUNCTION
"#],
            lint_rule: Some("identity-operation"),
        },
        ErrorExample {
            code: "L0313",
            category: "Linter Warning",
            title: "Subtraction from self",
            description: "Subtracting a variable from itself always results in 0.",
            sources: &[r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    test := x - x;
END_FUNCTION
"#],
            lint_rule: Some("sub-self"),
        },
        ErrorExample {
            code: "L0314",
            category: "Linter Warning",
            title: "Constant FOR loop bounds",
            description: "A FOR loop has equal start and end bounds, so the body executes exactly once.",
            sources: &[r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 5 TO 5 DO
        test := i;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("constant-loop-bounds"),
        },
        ErrorExample {
            code: "L0315",
            category: "Linter Warning",
            title: "Variable shadows its own POU",
            description: "A variable has the same name as the function block, method, or program it is declared in.",
            sources: &[r#"
FUNCTION_BLOCK MyFB
VAR
    MyFB : INT;
END_VAR
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("self-shadowing"),
        },
        ErrorExample {
            code: "L0316",
            category: "Linter Warning",
            title: "Missing return assignment",
            description: "A function or method declares a return type but never assigns the return value.",
            sources: &[r#"
FUNCTION foo : INT
VAR
    x : INT;
END_VAR
    x := 42;
END_FUNCTION
"#],
            lint_rule: Some("missing-return"),
        },
        ErrorExample {
            code: "L0317",
            category: "Linter Warning",
            title: "External instance mutation",
            description: "A field of a function block or class instance is directly modified from outside. Instances should own their data.",
            sources: &[r#"
FUNCTION_BLOCK MyFB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb.x := 42;
END_FUNCTION
"#],
            lint_rule: Some("external-mutation"),
        },
        ErrorExample {
            code: "L0318",
            category: "Linter Warning",
            title: "Method variable shadows an owner member",
            description: "A method's local or parameter has the same name as a member of the function block or class it belongs to. This is legal — the method variable shadows the member and bare-name access resolves to the local — but it is easy to misread.",
            sources: &[r#"
FUNCTION_BLOCK Counter
VAR
    c : INT;
END_VAR
    METHOD Inc : INT
    VAR
        c : INT;
    END_VAR
        c := c + 1;
        Inc := c;
    END_METHOD
END_FUNCTION_BLOCK
"#],
            lint_rule: Some("method-shadows-member"),
        },
        ErrorExample {
            code: "L0319",
            category: "Linter Warning",
            title: "FOR loop never terminates",
            description: "The end bound sits at the control type's own limit, so the counter wraps at the type width before the exit check can fail — the loop runs forever.",
            sources: &[r#"
FUNCTION test : INT
VAR i : USINT; END_VAR
    FOR i := 0 TO 255 DO
        test := test + 1;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("for-bound-at-type-limit"),
        },
        ErrorExample {
            code: "L0320",
            category: "Linter Warning",
            title: "Non-constant FOR step",
            description: "The BY step is not a compile-time literal. The loop's direction is decided at compile time (ascending), so a negative value at runtime will not run the loop backwards.",
            sources: &[r#"
FUNCTION test : INT
VAR i : INT; s : INT; END_VAR
    s := 2;
    FOR i := 1 TO 10 BY s DO
        test := test + 1;
    END_FOR;
END_FUNCTION
"#],
            lint_rule: Some("nonconstant-for-step"),
        },
        ErrorExample {
            code: "L0410",
            category: "Linter Warning",
            title: "Global accessed without VAR_EXTERNAL",
            description: "A configuration VAR_GLOBAL is read directly by name. Strict IEC wants the program to import it explicitly with VAR_EXTERNAL.",
            sources: &[r#"
PROGRAM P
VAR n : INT; END_VAR
    n := gCount;
END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL gCount : INT; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#],
            lint_rule: Some("global-without-external"),
        },
    ]
}
