//! Case is not significant in IEC 61131-3 — §6.1.2 for identifiers, §6.1.1
//! for keywords — and this is where that is held to account, end to end.
//!
//! The tests are here rather than beside each feature because the property is
//! ONE property: a name written in another case is the same name, wherever it
//! is written. Spread across `codegen/enums.rs`, `semantics/duplicates.rs` and
//! the rest, that reads as a dozen unrelated quirks; together it reads as the
//! rule, and a gap in it is visible.
//!
//! What each section is for:
//!
//! - **Guards** — the half no type can enforce. A lookup cannot forget to fold
//!   (resolution maps are keyed by `CaselessIdent`, and an `Ident` will not
//!   open one), but a bare `a == b` between two `Ident`s compiles and stays
//!   case-SENSITIVE. It cannot be made to fold either: folding needs the
//!   database and `PartialEq::eq` has none. So those comparisons are listed,
//!   each with the reason it is sound, and a new one fails until it is folded
//!   or explained.
//! - **Resolution** — names that differ only in case are one name, so
//!   declaring both is declaring it twice.
//! - **Execution** — the value, not the diagnostic. Every bug this arc found
//!   passed `check` and went wrong later: an initializer dropped, an
//!   assignment that landed nowhere, a slice measured against the wrong width.
//!   Asserting the runtime answer is what would have caught them.
//! - **Formatting** — the formatter matches ~150 canonical node names and never
//!   reads source text, so it is immune by construction; nothing says so in
//!   the formatter, which is why it is said here.

use crate::tests::codegen::{compile_to_mir_and_wasm, compile_to_wasm, execute_wasm};
use crate::tests::lsp::formatter::fmt;
use crate::tests::utils::add_sources;
use auto_lsp::default::db::BaseDatabase;
use crate::tests::lsp::formatter_preserves_meaning::assert_meaning_preserved;
use crate::tests::utils::{test_diagnostics, with_db};
use db::RootDatabase;
use insta::assert_snapshot;
use rstest::*;
use std::path::{Path, PathBuf};


// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

/// A comparison that is allowed to skip the fold, and why.
struct Allowed {
    /// The trimmed source line, matched exactly — editing it re-opens the
    /// review rather than silently keeping the exemption.
    line: &'static str,
    why: &'static str,
}

const ALLOWED: &[Allowed] = &[
    Allowed {
        line: "self.ident == other.ident",
        why: "SpanIdent equality IS its name's: two occurrences of one name are equal",
    },
    Allowed {
        line: "self.ident == *other",
        why: "the same, against a bare Ident",
    },
    Allowed {
        line: "self.instance_types.iter().find(|it| it.name == name)",
        why: "MIR instance types, named and queried by MIR itself",
    },
    Allowed {
        line: "if let Some(field) = s.fields.iter().find(|f| f.name == *name) {",
        why: "init path steps carry the DECLARED field name (ResolvedInit), \
              so both sides are declarations",
    },
    Allowed {
        line: "let f = s.fields.iter().find(|f| f.name == *name)?;",
        why: "the same walk, one level down",
    },
    Allowed {
        line: "v.name(db) == name",
        why: "a PROGRAM's declared variable against a declared retain name",
    },
    Allowed {
        line: "let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name)",
        why: "HIR already matched the param to a VariableDecl; var_name is that \
              declaration's own name",
    },
    Allowed {
        line: "let Some(field) = struct_type.fields.iter().find(|f| f.name == var_name) else {",
        why: "the same match, the output-binding arm",
    },
];

/// Lines that COMPARE two names. Deliberately broad: a false positive costs
/// one line in `ALLOWED` and a sentence saying why, which is the point.
fn compares_names(line: &str) -> bool {
    let has_eq = line.contains("==") || line.contains("!=");
    if !has_eq || line.contains("caseless(") {
        return false;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("///") {
        return false;
    }
    ["get_name_ident(db)", ".name(db)", ".ident", ".name", "name ==", "ident =="]
        .iter()
        .any(|needle| line.contains(needle))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every name comparison that skips the fold is accounted for.
#[test]
fn name_comparisons_are_folded_or_explained() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for crate_dir in ["crates/hir/src", "crates/mir/src"] {
        rust_files(&root.join(crate_dir), &mut files);
    }
    assert!(!files.is_empty(), "found no sources to scan");

    let mut unexplained = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            if !compares_names(line) {
                continue;
            }
            let trimmed = line.trim();
            if ALLOWED.iter().any(|a| a.line == trimmed) {
                continue;
            }
            let rel = file.strip_prefix(root).unwrap_or(file);
            unexplained.push(format!("{}:{}\n    {trimmed}", rel.display(), n + 1));
        }
    }

    assert!(
        unexplained.is_empty(),
        "a name is compared without folding, at {} site(s):\n\n{}\n\n\
         Names match with case out of the way (§6.1.2). Either fold both sides \
         with `.caseless(db)`, or — if BOTH sides come from a declaration and a \
         fold would be a no-op — add the line to `ALLOWED` in this file with the \
         reason.",
        unexplained.len(),
        unexplained.join("\n")
    );
}

/// The exemptions describe something that still exists.
///
/// Without this, a comparison that gets folded or deleted leaves its entry
/// behind, and the list slowly stops meaning anything.
#[test]
fn no_stale_exemptions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for crate_dir in ["crates/hir/src", "crates/mir/src"] {
        rust_files(&root.join(crate_dir), &mut files);
    }
    let all: Vec<String> = files
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();

    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|a| !all.iter().any(|text| text.lines().any(|l| l.trim() == a.line)))
        .map(|a| format!("{}\n      exempt because: {}", a.line, a.why))
        .collect();

    assert!(
        stale.is_empty(),
        "exemption(s) for code that is gone — delete them:\n  {}",
        stale.join("\n  ")
    );
}


// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Names differing only in case are ONE name (6.1.2), so every duplicate
/// detector folds — not just resolution. A detector that compares raw
/// spellings while resolution folds lets both declarations survive the check
/// and then collapse onto one folded map entry: a silently dropped
/// declaration, which is how `v.a := 1` once bound to `A : STRING`.
#[rstest]
fn duplicates_are_detected_in_any_case(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            S : STRUCT
                fld : INT;
                FLD : STRING;
            END_STRUCT;
            E : (Red, RED);
        END_TYPE

        FUNCTION_BLOCK FB
        METHOD PUBLIC Run : INT
            Run := 1;
        END_METHOD
        METHOD PUBLIC run : INT
            run := 2;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0103] Error: duplicate definitions
       ,-[ file:///test0.st:5:17 ]
       |
     4 |                 fld : INT;
       |                 ^|^
       |                  `--- field 'fld' is already defined here
     5 |                 FLD : STRING;
       |                 ^|^
       |                  `--- duplicate field 'FLD'
    ---'
    [E0104] Error: duplicate definitions
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             E : (Red, RED);
       |                  ^|^  ^|^
       |                   `-------- duplicate enum variant 'Red'
       |                        |
       |                        `--- enum variant 'RED' is already defined here
    ---'
    [E0105] Error: duplicate definitions
        ,-[ file:///test0.st:14:23 ]
        |
     11 |         METHOD PUBLIC Run : INT
        |                       ^|^
        |                        `--- method 'Run' is already defined here
        |
     14 |         METHOD PUBLIC run : INT
        |                       ^|^
        |                        `--- duplicate method 'run'
    ----'
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:12:13 ]
        |
     12 |             Run := 1;
        |             ^|^
        |              `--- cannot use direct type 'run' here
    ----'
    ");
}

/// A USING repeated in another case imports the same namespace twice.
#[rstest]
fn using_duplicates_fold(mut with_db: RootDatabase) {
    let source = r#"
        NAMESPACE Tools
        FUNCTION H : INT
            H := 1;
        END_FUNCTION
        END_NAMESPACE

        FUNCTION f : INT
            USING Tools;
            USING tools;
            f := H();
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0109] Error: duplicate definitions
        ,-[ file:///test0.st:10:19 ]
        |
      9 |             USING Tools;
        |                   ^^|^^
        |                     `---- namespace 'Tools' is already imported here
     10 |             USING tools;
        |                   ^^|^^
        |                     `---- duplicate `USING` for namespace 'tools'
    ----'
    ");
}

/// The folded form collides identically: `wide` is `Wide`.
#[rstest]
fn folded_variable_collides_with_the_return_value(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Wide : INT
        VAR
            wide : LINT;
        END_VAR
            Wide := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0117] Error: duplicate definitions
       ,-[ file:///test0.st:4:13 ]
       |
     4 |             wide : LINT;
       |             ^^|^
       |               `--- variable 'wide' is the FUNCTION's return value
    ---'
    ");
}

/// The size character is a keyword, so it is read in either case.
///
/// `%b1` is `%B1`: a BYTE, which is why assigning it to a BOOL is the error
/// below rather than silence. Lower case used to decode as no slice at all.
#[rstest]
fn slice_size_is_case_insensitive(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT
        VAR
            w : WORD;
            b : BOOL;
            y : BYTE;
        END_VAR
            y := w.%b1;
            b := w.%b1;
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:18 ]
       |
     5 |             b : BOOL;
       |             |
       |             `-- type is declared by variable 'b' here
       |
     9 |             b := w.%b1;
       |                  ^^|^^
       |                    `---- expected 'BOOL', got 'BYTE'
    ---'
    ");
}


// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// The size character is a keyword, so it names the same slice in either case.
///
/// Lower case used to decode as no slice AT ALL: `multibits_slice` matched only
/// upper case, the type fell back to BOOL, `rk check` passed — and lowering
/// then died with an internal compiler error on a program the front end had
/// just accepted. The expected values here are the ones the upper-case cases
/// above already pin, so the two spellings are held to one answer.
#[rstest]
#[case("d.%x2", "BOOL", 1)]
#[case("d.%b1", "BYTE", 0x33)]
#[case("d.%w1", "WORD", 0x1122)]
#[case("d.%d0", "DWORD", 0x11223344)]
fn size_character_reads_the_same_slice_in_either_case(
    mut with_db: db::RootDatabase,
    #[case] access: &str,
    #[case] returns: &str,
    #[case] expected: i32,
) {
    let source = format!(
        r#"
        FUNCTION get : {returns}
        VAR
            d : DWORD := 16#11223344;
        END_VAR
            get := {access};
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, expected, "{access} on 16#11223344");
}

/// An enum variant written in another case denotes the same variant, and must
/// carry the same VALUE into lowering.
///
/// MIR matches the written variant against the declared ones; if the two are
/// compared without agreeing on case, the lookup misses and the initializer is
/// refused (or worse, silently takes another variant's value).
#[rstest]
fn enum_variant_case_reaches_the_same_value(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (RED, GREEN, BLUE); END_TYPE

        FUNCTION get : INT
        VAR
            c : Color := color#green;
        END_VAR
            IF c = Color#GREEN THEN
                get := 42;
            ELSE
                get := 0;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 42, "color#green IS Color#GREEN");
}

/// A struct initializer naming its field in a different case still lands.
///
/// The initializer's path steps carry the spelling as WRITTEN, and MIR walks
/// them against the field names as DECLARED (`lower_func::walk_init_path`).
/// If those two are compared without folding, the step matches nothing, the
/// value is silently dropped, and the field reads back as zero on a program
/// that compiled at exit 0.
#[rstest]
fn struct_initializer_field_name_folds(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT
            Fld : INT;
            Other : INT;
        END_STRUCT; END_TYPE

        FUNCTION get : INT
        VAR
            s : S := (fld := 7, OTHER := 5);
        END_VAR
            get := s.Fld * 10 + s.Other;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 75, "both initializers landed despite the spelling");
}

/// The return value answers to its callable's name in any case: `compute :=`
/// inside `FUNCTION Compute` assigns the RETURN VALUE, and the value lands.
#[rstest]
fn return_value_assigned_in_another_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Compute : INT
            compute := 42;
        END_FUNCTION

        FUNCTION get : INT
            get := Compute();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 42, "the folded self-reference is the return value");
}

/// Member and struct-field paths spelled in another case reach the same
/// storage, read and written — `pt.x` inside the FB is `Pt.X`.
#[rstest]
fn member_paths_fold_end_to_end(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT
            X : INT;
            Y : INT;
        END_STRUCT; END_TYPE

        FUNCTION_BLOCK Holder
        VAR
            Pt : Point;
        END_VAR
        VAR_OUTPUT
            O : INT;
        END_VAR
            pt.x := 30;
            PT.y := 12;
            o := pt.X + Pt.y;
        END_FUNCTION_BLOCK

        FUNCTION get : INT
        VAR h : Holder; END_VAR
            h();
            get := h.o;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 42, "every spelling reached the same field");
}

/// A monitoring path resolves regardless of case, as the source does.
///
/// The compiler resolves `MyVar` and `myvar` to one variable, so a debugger
/// asking by the other spelling must not come back empty — a variable
/// readable from source and unreadable from a tool is the two layers
/// disagreeing about what a name is.
#[rstest]
fn a_monitoring_path_is_not_case_sensitive(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT
            X : INT;
        END_STRUCT; END_TYPE

        PROGRAM MyProg
        VAR
            MyVar : INT := 7;
            Pt : Point := (X := 9);
        END_VAR
            MyVar := MyVar;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : MyProg;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let info = runtime::debug::DebugInfo::from_wasm(&wasm);

    for spelling in ["P1.MyVar", "p1.myvar", "P1.MYVAR", "p1.MyVar"] {
        assert!(
            info.symbol(spelling).is_some(),
            "`{spelling}` names the same variable"
        );
    }
}


// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Keywords in lower case, which the language has always allowed and the
/// grammar only started accepting when `kw()` landed.
///
/// The formatter is written entirely against the CANONICAL upper-case node
/// names — `"IF"`, `"END_VAR"`, `"DO"` — around 150 of them in
/// `crates/formatter/src/lib.rs`. Those keep matching lower-case source for
/// exactly one reason: `kw()` aliases each case-insensitive token back to its
/// upper-case spelling, so the node TYPE is still `IF` when the text is `if`.
///
/// What this case catches is a rule that fires WRONGLY on lower-case input and
/// corrupts it — the `BOOLREAD_ONLY` failure this file exists for. It does not
/// catch rules that stop firing ALTOGETHER, because unformatted output still
/// parses and so still means the same thing; `formatting_is_blind_to_keyword_case`
/// below is the case that covers that.
#[rstest]
fn lower_case_keywords_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "lower case",
        r#"
type
	point : struct
		x : int;
		y : int;
	end_struct;
	mode : (idle, running);
end_type

function_block Ramp
var_input
	target : int;
	rate : int;
end_var
var_output
	done : bool;
end_var
var
	acc : int;
end_var
	acc := acc + rate;
	if acc >= target then
		done := TRUE;
	elsif acc < 0 then
		acc := 0;
	else
		done := FALSE;
	end_if;
end_function_block

function calc : real
var_input
	a : real;
	b : real;
end_var
var
	i : int;
	t : array[0..3] of real;
	s : string[10];
	flags : word;
end_var
	for i := 0 to 3 by 1 do
		t[i] := a * 2.0 + b / 3.0;
	end_for;
	while i > 0 do
		i := i - 1;
	end_while;
	repeat
		i := i + 1;
	until i >= 3
	end_repeat;
	case i of
		0: s := 'zero';
		1: s := 'one';
	else
		s := 'many';
	end_case;
	flags := flags and 16#FF;
	if not (i = 0) and (i mod 2 = 0) then
		i := 0;
	end_if;
	calc := t[0];
end_function

program Main
var retain
	n : dint;
end_var
var
	r : Ramp;
	p : point;
	m : mode;
	d : time;
end_var
	n := n + 1;
	r(target := 1000, rate := 7);
	p.x := 1;
	m := mode#running;
	d := T#10ms;
end_program

configuration Cfg
	resource Res on CPU
		task T(interval := T#10ms, priority := 1);
		program P1 with T : Main;
	end_resource
end_configuration
"#,
    );
}

/// Keywords in mixed case, the spelling a human actually types.
///
/// `Function` and `End_If` reach the formatter through the same aliased nodes
/// as `function` and `END_IF`, so this is the case above aimed at the middle
/// of the range rather than its end. It is separate because a regression that
/// special-cased the two ALL-ONE-CASE spellings — the easy way to "support
/// case-insensitivity" — would pass the test above and fail this one.
#[rstest]
fn mixed_case_keywords_survive_formatting(#[allow(unused)] with_db: RootDatabase) {
    assert_meaning_preserved(
        "mixed case",
        r#"
Type
	Level : (Low, High);
End_Type

Function_Block Gate
Var_Input
	Open : Bool;
End_Var
Var_Output
	State : Bool;
End_Var
	If Open Then
		State := TRUE;
	Else
		State := FALSE;
	End_If;
End_Function_Block

Function Run : Int
Var
	i : Int;
	g : Gate;
	l : Level;
End_Var
	For i := 0 To 3 Do
		g(Open := TRUE);
	End_For;
	While i > 0 Do
		i := i - 1;
	End_While;
	Case i Of
		0: i := 1;
	Else
		i := 2;
	End_Case;
	l := Level#High;
	Run := i;
End_Function
"#,
    );
}

/// Formatting is BLIND to keyword case: the same program in lower case and in
/// upper case formats to the same text, character for character, once case is
/// folded away.
///
/// This is the case that fails if the aliasing in `kw()` is dropped. Rules
/// that stop matching do not corrupt anything — they simply never fire, and
/// unformatted output still parses and still means the same thing, so every
/// meaning-preservation case above stays green while the formatter quietly
/// does nothing at all on lower-case files. Comparing the two spellings'
/// output catches exactly that: one side gets indented and the other does not.
///
/// The source is deliberately written with NO indentation and tight operators,
/// so the formatter has real work to do and passing the text through unchanged
/// cannot satisfy the assertion.
#[rstest]
fn formatting_is_blind_to_keyword_case(#[allow(unused)] with_db: RootDatabase) {
    const LOWER: &str = "function calc : real
var
i : int;
end_var
if i>0 then
i:=1;
elsif i<0 then
i:=2;
end_if;
for i := 0 to 3 do
i:=i+1;
end_for;
while i>0 do
i:=i-1;
end_while;
end_function
";
    const UPPER: &str = "FUNCTION calc : REAL
VAR
i : INT;
END_VAR
IF i>0 THEN
i:=1;
ELSIF i<0 THEN
i:=2;
END_IF;
FOR i := 0 TO 3 DO
i:=i+1;
END_FOR;
WHILE i>0 DO
i:=i-1;
END_WHILE;
END_FUNCTION
";

    let format = |source: &str| {
        let mut db = RootDatabase::default();
        add_sources(&mut db, &[source]);
        let file = *db.get_files().iter().next().expect("one file");
        fmt(file.document(&db))
    };

    let lower = format(LOWER);
    let upper = format(UPPER);

    // The formatter must have done something; identical-but-untouched would
    // satisfy the comparison below for the wrong reason.
    assert_ne!(lower, LOWER, "the formatter left the lower case source untouched");
    assert_ne!(upper, UPPER, "the formatter left the upper case source untouched");

    assert_eq!(
        lower.to_ascii_lowercase(),
        upper.to_ascii_lowercase(),
        "formatting differs by keyword case.\n--- from lower case ---\n{lower}\n--- from upper case ---\n{upper}"
    );
}
