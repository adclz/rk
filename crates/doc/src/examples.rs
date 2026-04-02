/// Each error example: (error_code, category, title, description, ST source code).
///
/// The source code MUST trigger the corresponding error when compiled.
pub struct ErrorExample {
    pub code: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub sources: &'static [&'static str],
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
        },
        ErrorExample {
            code: "E0009",
            category: "Syntax",
            title: "THIS not valid in this context",
            description: "`THIS` can only be used inside a CLASS or FUNCTION_BLOCK that has methods.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := THIS.x;
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "E0010",
            category: "Syntax",
            title: "SUPER not valid in this context",
            description: "`SUPER` can only be used inside a CLASS or FUNCTION_BLOCK that EXTENDS another.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := SUPER.x;
END_FUNCTION
"#],
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
        },
        ErrorExample {
            code: "E0018",
            category: "Syntax",
            title: "Invalid POU keyword",
            description: "An invalid keyword was used where a POU declaration (FUNCTION, FUNCTION_BLOCK, CLASS, etc.) was expected.",
            sources: &[r#"
HELLO world
END_FUNCTION
"#],
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
        },
        ErrorExample {
            code: "E0030",
            category: "Syntax",
            title: "VAR_GLOBAL not allowed in this context",
            description: "`VAR_GLOBAL` can only be used inside PROGRAM or CONFIGURATION.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    VAR_GLOBAL

    END_VAR
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0032",
            category: "Syntax",
            title: "SINGLE after INTERVAL in TASK",
            description: "In a TASK configuration, SINGLE must be declared before INTERVAL.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(INTERVAL := T#20ms, SINGLE := var1, PRIORITY := 1);
END_CONFIGURATION
"#],
        },
        ErrorExample {
            code: "E0033",
            category: "Syntax",
            title: "INTERVAL after PRIORITY in TASK",
            description: "In a TASK configuration, INTERVAL must be declared before PRIORITY.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(PRIORITY := 1, INTERVAL := T#20ms);
END_CONFIGURATION
"#],
        },
        ErrorExample {
            code: "E0034",
            category: "Syntax",
            title: "SINGLE after PRIORITY in TASK",
            description: "In a TASK configuration, SINGLE must be declared before PRIORITY.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(PRIORITY := 1, SINGLE := var1);
END_CONFIGURATION
"#],
        },
        ErrorExample {
            code: "E0035",
            category: "Syntax",
            title: "Missing PRIORITY in TASK",
            description: "PRIORITY is required in TASK configuration.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(INTERVAL := T#20ms);
END_CONFIGURATION
"#],
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
        },
        ErrorExample {
            code: "E0113",
            category: "Duplicates",
            title: "Duplicate CONFIGURATION",
            description: "Two CONFIGURATION declarations have the same name.",
            sources: &[r#"
CONFIGURATION c1
END_CONFIGURATION

CONFIGURATION c1
END_CONFIGURATION
"#],
        },
        ErrorExample {
            code: "E0114",
            category: "Duplicates",
            title: "Duplicate TASK name",
            description: "Two TASKs in the same configuration have the same name.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    TASK task1(PRIORITY := 2);
END_CONFIGURATION
"#],
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
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : prog1;
    PROGRAM inst1 WITH task1 : prog1;
END_CONFIGURATION
"#],
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
        },
        ErrorExample {
            code: "E0218",
            category: "Resolution",
            title: "Unknown program type in CONFIGURATION",
            description: "The program type referenced in a CONFIGURATION entry does not exist.",
            sources: &[r#"
CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : unknown_prog;
END_CONFIGURATION
"#],
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
    PROGRAM inst1 WITH unknown_task : prog1;
END_CONFIGURATION
"#],
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
        },
        ErrorExample {
            code: "E0222",
            category: "Resolution",
            title: "Configuration instance unknown",
            description: "A `VAR_CONFIG` path references a program instance that does not exist.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : prog1;
    VAR_CONFIG
        unknown_inst.x : INT;
    END_VAR
END_CONFIGURATION
"#],
        },
        ErrorExample {
            code: "E0223",
            category: "Resolution",
            title: "Configuration field not found",
            description: "A `VAR_CONFIG` path references a field that does not exist on the resolved type.",
            sources: &[r#"
PROGRAM prog1
END_PROGRAM

CONFIGURATION config1
    TASK task1(PRIORITY := 1);
    PROGRAM inst1 WITH task1 : prog1;
    VAR_CONFIG
        inst1.nonexistent_field : INT;
    END_VAR
END_CONFIGURATION
"#],
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
        },
        // Assignment / call violations (moved from E10xx)
        ErrorExample {
            code: "E0226",
            category: "Resolution",
            title: "Assignment of callable type",
            description: "A callable type (function, function block type) cannot be assigned directly.",
            sources: &[r#"
FUNCTION fn1 : INT
    fn1 := 0;
END_FUNCTION

FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x := fn1;
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0230",
            category: "Resolution",
            title: "Extern variable not found",
            description: "An `{extern}` pragma references a variable that does not exist in the current scope.",
            sources: &[r#"
FUNCTION test : INT
VAR_INPUT x : INT; END_VAR
    {extern 'math' 'abs' (params unknown_var) (result test)}
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "E0231",
            category: "Resolution",
            title: "INTO reference not found",
            description: "An `INTO(ref)` type specification references an identifier that is not found in scope.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        x: INTO(nonexistent);
    END_VAR
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "E0232",
            category: "Resolution",
            title: "INTO reference must be an ANY type",
            description: "An `INTO(ref)` constraint must reference a variable declared with an `ANY_*` type specification.",
            sources: &[r#"
FUNCTION fn1
    VAR_INPUT
        value: INT;
        target: INTO(value);
    END_VAR
END_FUNCTION
"#],
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
        },
        ErrorExample {
            code: "E0305",
            category: "Type System",
            title: "Types not powerable",
            description: "The two types cannot be used with the power operator (`**`).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
    y : STRING;
    z : INT;
END_VAR
    z := x ** y;
END_FUNCTION_BLOCK
"#],
        },
        ErrorExample {
            code: "E0306",
            category: "Type System",
            title: "Expected a boolean",
            description: "A boolean expression is required (e.g. in IF, WHILE conditions).",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    IF x THEN
    END_IF;
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0309_WSTRING_LEN",
            category: "Type System",
            title: "WSTRING literal exceeds max length",
            description: "A WSTRING literal exceeds the declared maximum length.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    s: WSTRING[3] := "hello world";
END_VAR
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0309_WCHAR",
            category: "Type System",
            title: "Invalid WCHAR literal length",
            description: "A WCHAR literal must be exactly 1 character.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    c: WCHAR := WCHAR#"ab";
END_VAR
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0319",
            category: "Type System",
            title: "Assignment attempt requires REF_TO",
            description: "The `?=` assignment attempt operator requires a `REF_TO` variable on the left-hand side.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: INT;
    y: INT;
END_VAR
    x ?= y;
END_FUNCTION_BLOCK
"#],
        },
        ErrorExample {
            code: "E0320",
            category: "Type System",
            title: "Assignment attempt invalid RHS",
            description: "The right-hand side of `?=` must be a `REF_TO` or interface type.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x: REF_TO INT;
    y: INT;
END_VAR
    x ?= y;
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0510",
            category: "Inheritance",
            title: "Unresolved THIS method",
            description: "The method called on `THIS` does not exist in the current POU.",
            sources: &[r#"
FUNCTION_BLOCK fb1
    METHOD decl
    END_METHOD

    THIS.decl1();
END_FUNCTION_BLOCK
"#],
        },
        ErrorExample {
            code: "E0511",
            category: "Inheritance",
            title: "Unresolved SUPER method",
            description: "The method called on `SUPER` does not exist in the inherited methods.",
            sources: &[r#"
CLASS base
    METHOD PUBLIC super_method END_METHOD
END_CLASS

FUNCTION_BLOCK fb1 EXTENDS base
    SUPER.super_method1()
END_FUNCTION_BLOCK
"#],
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
        },
        ErrorExample {
            code: "E0702",
            category: "Enums",
            title: "Not an ENUM type",
            description: "The `#` enum access syntax was used on a type that is not an ENUM.",
            sources: &[r#"
FUNCTION_BLOCK fb1
VAR
    x : INT;
END_VAR
    x := INT#Red;
END_FUNCTION_BLOCK
"#],
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
        },
        // ── W01xx: Linter warnings ───────────────────────────────────────
        ErrorExample {
            code: "L0101",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0102",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0103",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0104",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0105",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0106",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0107",
            category: "Linter Warnings",
            title: "Unreachable code",
            description: "A statement appears after an unconditional control flow statement (RETURN, EXIT, CONTINUE) and can never be reached.",
            sources: &[r#"
FUNCTION test : INT
    test := 1;
    RETURN;
    test := 2;
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "L0108",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0110",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0109",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0116",
            category: "Linter Warnings",
            title: "Missing input parameter",
            description: "A function or method call does not pass all required `VAR_INPUT` parameters.",
            sources: &[r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
END_FUNCTION

FUNCTION_BLOCK caller
    add(a := 1);
END_FUNCTION_BLOCK
"#],
        },
        ErrorExample {
            code: "L0115",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0114",
            category: "Linter Warnings",
            title: "Uninitialized output",
            description: "A `VAR_OUTPUT` variable is never assigned in the body. Callers may read an undefined value.",
            sources: &[r#"
FUNCTION compute : INT
VAR_OUTPUT
    status : INT;
END_VAR
    compute := 42;
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "L0113",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0112",
            category: "Linter Warnings",
            title: "Constant condition",
            description: "A condition in an IF, WHILE, or REPEAT statement is always TRUE or always FALSE.",
            sources: &[r#"
FUNCTION test : INT
    IF TRUE THEN
        test := 1;
    END_IF;
END_FUNCTION
"#],
        },
        ErrorExample {
            code: "L0111",
            category: "Linter Warnings",
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
        },
        ErrorExample {
            code: "L0117",
            category: "Linter Warnings",
            title: "Call site pragma notice",
            description: "A call targets a POU annotated with {warn = '...'} or {info = '...'}, indicating deprecation or other notices.",
            sources: &[r#"
{warn = 'this function is deprecated, use fn2 instead'}
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION caller : INT
VAR x : INT; END_VAR
    x := fn1();
END_FUNCTION
"#],
        },
    ]
}
