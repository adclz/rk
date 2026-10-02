// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Case is not significant in IEC 61131-3 — §6.1.2 for identifiers, §6.1.1
//! for keywords — and this is where that is held to account, end to end.
//!
//! The tests are here rather than beside each feature because the property is
//! ONE property: a name written in another case is the same name, wherever it
//! is written. Spread across `codegen/enums.rs`, `hir/duplicates.rs` and
//! the rest, that reads as a dozen unrelated quirks; together it reads as the
//! rule, and a gap in it is visible.
//!
//! What each section is for:
//!
//! - **Guards** — the half no type can enforce. A name's accessors hand it
//!   out case folded (`name(db)`, `get_name_ident(db)`, `SpanIdent::ident(db)`),
//!   so every comparison and every key is caseless without anyone asking;
//!   the author's spelling is `*_with_case`, for what is shown. Both kinds
//!   are an `Ident`, so no type tells them apart: the guards check that a
//!   spelling is never matched and that nothing folds by hand, and each
//!   exception is listed with its reason.
//! - **Resolution** — names that differ only in case are one name, so
//!   declaring both is declaring it twice.
//! - **Execution** — the value, not the diagnostic. Every bug this arc found
//!   passed `check` and went wrong later: an initializer dropped, an
//!   assignment that landed nowhere, a slice measured against the wrong width.
//!   Asserting the runtime answer is what would have caught them.
//! - **Display** — the other half of the accessor rule: what is shown is the
//!   author's spelling. A message, a suggestion or a completion that reads a
//!   name through its folding accessor shows `motor` for `Motor`, and a
//!   completion that writes into the file writes it there.
//! - **Formatting** — the formatter matches ~150 canonical node names and never
//!   reads source text, so it is immune by construction; nothing says so in
//!   the formatter, which is why it is said here.

use crate::tests::codegen::{compile_to_mir_and_wasm, compile_to_wasm, execute_wasm};
use crate::tests::lsp::formatter::fmt;
use crate::tests::lsp::formatter_preserves_meaning::assert_meaning_preserved;
use crate::tests::utils::{
    add_source, find_pou_with_name, test_diagnostics, test_single_lint, with_db,
};
use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use db::RootDatabase;
use hir::HirNodeInfo;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode};
use ide_proto::handlers::{DocumentSymbolsHandler, InlayHintHandler};
use insta::assert_snapshot;
use rstest::*;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

/// A line the guards let through, and why.
struct Allowed {
    /// The trimmed source line, matched exactly — editing it re-opens the
    /// review rather than silently keeping the exemption.
    line: &'static str,
    why: &'static str,
}

/// Lines that match a spelling, each with the reason it is sound.
const SPELLING_MATCHED: &[Allowed] = &[
    Allowed {
        line: "self.with_case == other.with_case",
        why: "a SpanIdent's own change detection: an occurrence that only moved \
              is not a change, and a respelled one is",
    },
    Allowed {
        line: "self.path_with_case == other.path_with_case",
        why: "the same, for a written namespace path",
    },
];

/// Folds outside an accessor, each with the reason it is sound: the places a
/// name arrives as text, not as an identifier the source declared.
const FOLDED_OUTSIDE_AN_ACCESSOR: &[Allowed] = &[
    Allowed {
        line: "if fragments.iter().all(|f| f.folded(db) == *f) {",
        why: "the fold of a namespace path itself, fragment by fragment",
    },
    Allowed {
        line: "fragments.iter().map(|f| f.folded(db)).collect::<Vec<_>>(),",
        why: "the same fold, building the folded path",
    },
    Allowed {
        line: "let ident = Ident::from_slice(db, content).folded(db);",
        why: "a `[Name]` link in a comment is text the reader typed",
    },
    Allowed {
        line: ".map(|p| Ident::from_slice(db, p).folded(db))",
        why: "the same link's namespace fragments",
    },
    Allowed {
        line: "let target_ident = Ident::from_slice(db, target_name).folded(db);",
        why: "the same link's last fragment",
    },
    Allowed {
        line: "let name = Ident::from_slice(db, parts[0]).folded(db);",
        why: "a test's qualified name, as `rk test` is given it",
    },
    Allowed {
        line: ".map(|s| Ident::from_slice(db, s).folded(db))",
        why: "the same name's namespace fragments",
    },
    Allowed {
        line: "let name = Ident::from_slice(db, item_name).folded(db);",
        why: "the same name's last fragment",
    },
    Allowed {
        line: ".map(|step| Ident::new(db, compact_str::CompactString::from(*step)).folded(db))",
        why: "a VAR_CONFIG path's steps, split out of the path as text",
    },
];

/// The crates that hold names: where a spelling could be matched, or a
/// name folded by hand.
const NAME_CRATES: &[&str] = &[
    "crates/hir/src",
    "crates/mir/src",
    "crates/linter/src",
    "crates/ide_proto/src",
    "crates/wasm_codegen/src",
    "crates/debug_format/src",
    "crates/cli/src",
];

/// Lines that MATCH a written spelling: compare it or use it as a key.
fn matches_a_spelling(line: &str) -> bool {
    line.contains("with_case")
        && [
            "==",
            "!=",
            ".get(",
            ".insert(",
            ".contains(",
            ".entry(",
            "contains_key",
        ]
        .iter()
        .any(|needle| line.contains(needle))
}

/// Lines that fold by hand. An accessor over a stored spelling is where a
/// fold belongs: `self.name_with_case(db).folded(db)`.
fn folds_outside_an_accessor(line: &str) -> bool {
    line.contains(".folded(")
        && !(line.trim_start().starts_with("self.") && line.contains("with_case"))
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

/// Every source line of the name crates, with where it is, comments and the
/// fold's own definition left out.
fn name_crate_lines() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for crate_dir in NAME_CRATES {
        rust_files(&root.join(crate_dir), &mut files);
    }
    assert!(!files.is_empty(), "found no sources to scan");
    let mut lines = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let rel = file
            .strip_prefix(root)
            .unwrap_or(file)
            .display()
            .to_string();
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            lines.push((format!("{rel}:{}", n + 1), trimmed.to_string()));
        }
    }
    lines
}

fn unexplained(
    lines: &[(String, String)],
    flagged: fn(&str) -> bool,
    allowed: &[Allowed],
) -> Vec<String> {
    lines
        .iter()
        .filter(|(_, line)| flagged(line) && !allowed.iter().any(|a| a.line == line))
        .map(|(at, line)| format!("{at}\n    {line}"))
        .collect()
}

/// A written spelling is shown, never matched: names compare as their
/// accessors hand them out, case folded, and a spelling in a comparison or a
/// key is `Motor` and `motor` coming apart again.
#[test]
fn a_spelling_is_never_matched() {
    let found = unexplained(&name_crate_lines(), matches_a_spelling, SPELLING_MATCHED);
    assert!(
        found.is_empty(),
        "a written spelling is matched, at {} site(s):\n\n{}\n\n\
         Names match as their accessors give them — `name(db)`, \
         `get_name_ident(db)`, `SpanIdent::ident(db)` — case folded (§6.1.2). \
         `*_with_case` is for what is shown. If this match is sound, add the \
         line to `SPELLING_MATCHED` in this file with the reason.",
        found.len(),
        found.join("\n")
    );
}

/// Names fold in their accessors, once, and nowhere else: a fold by hand is
/// a name that reached matching raw, and the next one like it will not be
/// folded.
#[test]
fn names_fold_only_in_their_accessors() {
    let found = unexplained(
        &name_crate_lines(),
        folds_outside_an_accessor,
        FOLDED_OUTSIDE_AN_ACCESSOR,
    );
    assert!(
        found.is_empty(),
        "a name is folded by hand, at {} site(s):\n\n{}\n\n\
         Read it through its accessor instead, which folds. If it arrives as \
         text rather than as a declared identifier, add the line to \
         `FOLDED_OUTSIDE_AN_ACCESSOR` in this file with the reason.",
        found.len(),
        found.join("\n")
    );
}

/// The exemptions describe something that still exists.
///
/// Without this, a line that gets rewritten or deleted leaves its entry
/// behind, and the lists slowly stop meaning anything.
#[test]
fn no_stale_exemptions() {
    let lines = name_crate_lines();
    let stale: Vec<String> = SPELLING_MATCHED
        .iter()
        .chain(FOLDED_OUTSIDE_AN_ACCESSOR)
        .filter(|a| !lines.iter().any(|(_, l)| l == a.line))
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
    [E0104] Error: duplicate definitions
       ,-[ file:///test0.st:5:17 ]
       |
     4 |                 fld : INT;
       |                 ^|^
       |                  `--- field 'fld' is already defined here
     5 |                 FLD : STRING;
       |                 ^|^
       |                  `--- duplicate field 'FLD'
    ---'
    [E0105] Error: duplicate definitions
       ,-[ file:///test0.st:7:18 ]
       |
     7 |             E : (Red, RED);
       |                  ^|^  ^|^
       |                   `-------- duplicate enum variant 'Red'
       |                        |
       |                        `--- enum variant 'RED' is already defined here
    ---'
    [E0108] Error: duplicate definitions
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
    [E0111] Error: duplicate definitions
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
    [E0107] Error: duplicate definitions
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
    let info = debug_format::DebugInfo::from_wasm(&wasm);

    for spelling in ["P1.MyVar", "p1.myvar", "P1.MYVAR", "p1.MyVar"] {
        assert!(
            info.symbol(spelling).is_some(),
            "`{spelling}` names the same variable"
        );
    }
}

/// An initializer names a member in any case, whatever holds it: an FB
/// instance, an FB member of an FB, a STRUCT inside an array. Only a plain
/// STRUCT's names were resolved; the others matched the declared names as
/// written and were dropped.
#[rstest]
fn an_initializer_names_members_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            P2 : STRUCT x : INT; y : INT; END_STRUCT;
            Box : STRUCT inner : P2; END_STRUCT;
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR_INPUT limit : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR_OUTPUT m : Fb := (LIMIT := 5); END_VAR
        END_FUNCTION_BLOCK

        FUNCTION get : INT
        VAR
            f : Fb := (LIMIT := 9);
            arr : ARRAY[0..1] OF P2 := [(X := 3, Y := 4), (X := 5, Y := 6)];
            boxes : ARRAY[0..0] OF Box := [(INNER := (Y := 7))];
            h : Holder;
        END_VAR
            get := f.limit * 1000 + arr[1].y * 100 + boxes[0].inner.y * 10 + h.m.limit;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 9675, "every initializer landed");
}

/// A VAR_EXTERNAL is the VAR_GLOBAL it names, however it spells it. The
/// global table is keyed by the global's declared name, and the external's
/// own spelling missed it: "no storage was allocated for global".
#[rstest]
fn a_var_external_names_its_global_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL Counter : INT; END_VAR
        END_CONFIGURATION

        FUNCTION Bump
        VAR_EXTERNAL counter : INT; END_VAR
            counter := counter + 1;
        END_FUNCTION

        FUNCTION ReadIt : INT
        VAR_EXTERNAL COUNTER : INT; END_VAR
            ReadIt := COUNTER;
        END_FUNCTION

        FUNCTION get : INT
            Bump();
            Bump();
            get := ReadIt();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 2, "both externals are the one Counter");
}

/// An override named in another case than its base method is still the
/// override, through `THIS.m()` and a bare `m()` too. The dispatch named the
/// method with the base's spelling, which no function had.
#[rstest]
fn an_override_answers_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
            METHOD PUBLIC Hook : INT
                Hook := 1;
            END_METHOD
            METHOD PUBLIC Call : INT
                Call := THIS.Hook();
            END_METHOD
            METHOD PUBLIC CallBare : INT
                CallBare := Hook();
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            METHOD PUBLIC OVERRIDE HOOK : INT
                HOOK := 2;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION get : INT
        VAR d : Derived; b : Base; END_VAR
            get := d.hook() * 1000 + d.Call() * 100 + d.CallBare() * 10 + b.Call();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 2221, "Derived's HOOK wherever Hook is called on it");
}

/// An interface parameter is its declaration: `DEV` is the parameter `dev`,
/// and a member `h.dev` is not. The specialization was keyed by the
/// parameter's spelling, so the first missed it and the second matched it.
#[rstest]
fn an_interface_parameter_is_its_declaration(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IShow
            METHOD Show : INT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Pump IMPLEMENTS IShow
            METHOD PUBLIC Show : INT
                Show := 1;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Fan IMPLEMENTS IShow
            METHOD PUBLIC Show : INT
                Show := 2;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR_OUTPUT dev : Fan; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION Ask : INT
        VAR_IN_OUT s : IShow; END_VAR
            Ask := s.Show();
        END_FUNCTION

        FUNCTION Both : INT
        VAR_IN_OUT dev : IShow; h : Holder; END_VAR
            Both := DEV.Show() * 10 + Ask(s := h.dev);
        END_FUNCTION

        FUNCTION get : INT
        VAR p : Pump; hh : Holder; END_VAR
            get := Both(dev := p, h := hh);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 12, "DEV is the Pump, h.dev the Fan");
}

/// A fold names its pack in any case.
#[rstest]
fn a_fold_names_its_pack_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_all : INT
        VAR_INPUT args : INT...; END_VAR
            sum_all := ...ARGS+;
        END_FUNCTION

        FUNCTION get : INT
            get := sum_all(1, 2, 3);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 6);
}

/// A result written through a field or an element, its callable's name in
/// another case, is the result. It was read as a local of the written name,
/// which no function had, and the store went to address 0.
#[rstest]
fn a_result_written_through_a_field_in_any_case(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Pt : STRUCT X : INT; Y : INT; END_STRUCT;
            Arr3 : ARRAY[0..2] OF INT;
        END_TYPE

        FUNCTION MakePt : Pt
            makept.X := 3;
            MAKEPT.y := 4;
        END_FUNCTION

        FUNCTION MakeArr : Arr3
            makearr[1] := 9;
        END_FUNCTION

        FUNCTION_BLOCK Fb
            METHOD PUBLIC Mk : Pt
                mk.X := 7;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION get : INT
        VAR p : Pt; a : Arr3; f : Fb; q : Pt; END_VAR
            p := MakePt();
            a := MakeArr();
            q := f.Mk();
            get := p.X * 1000 + p.Y * 100 + a[1] * 10 + q.X;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "get", ());
    assert_eq!(result, 3497, "every store reached its result");
}

/// A declared address is published as the compiler knows it, upper-cased:
/// `%qw4` is the cell a bare `%QW4` names, so a host binds it as `%QW4`.
#[rstest]
fn a_located_address_is_published_upper_cased(#[allow(unused)] with_db: RootDatabase) {
    let program = |declared: &str| {
        format!(
            r#"
        PROGRAM Prog
        VAR n : WORD; END_VAR
            %QX8.3 := TRUE;
            n := %QW4;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL w AT {declared} : WORD; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Prog;
            END_RESOURCE
        END_CONFIGURATION
    "#
        )
    };
    let compile = |declared: &str| {
        let mut db = RootDatabase::default();
        compile_to_mir_and_wasm(&mut db, &program(declared)).0
    };
    let lower = compile("%qw4");
    let upper = compile("%QW4");
    let addresses = |mir: &mir::MirModule| {
        mir.located_map
            .entries
            .iter()
            .map(|e| e.address.clone())
            .collect::<Vec<_>>()
    };
    assert!(
        addresses(&lower).contains(&"%QW4".to_string()),
        "{:?}",
        addresses(&lower)
    );
    assert_eq!(addresses(&lower), addresses(&upper));
    assert_eq!(lower.located_map.layout_hash, upper.located_map.layout_hash);
}

/// A case-only rename is the same program, so its retained state is kept:
/// the retain map's identity is its folded paths and field names. It was the
/// spelled ones, and the runtime refused the file as another program's.
#[rstest]
fn a_case_only_rename_keeps_the_retain_layout(#[allow(unused)] with_db: RootDatabase) {
    let program = |count: &str, x: &str, instance: &str| {
        format!(
            r#"
        TYPE Pt : STRUCT {x} : INT; y : INT; END_STRUCT; END_TYPE

        PROGRAM Prog
        VAR RETAIN {count} : INT; pos : Pt; alpha : INT; END_VAR
            count := count + 1;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM {instance} WITH T : Prog;
            END_RESOURCE
        END_CONFIGURATION
    "#
        )
    };
    let compile = |count: &str, x: &str, instance: &str| {
        let mut db = RootDatabase::default();
        compile_to_mir_and_wasm(&mut db, &program(count, x, instance)).0
    };
    let base = compile("count", "x", "p1");
    for (count, x, instance) in [
        ("Count", "x", "p1"),
        ("count", "X", "p1"),
        ("COUNT", "X", "P1"),
    ] {
        let renamed = compile(count, x, instance);
        assert_eq!(
            base.retain_map.layout_hash, renamed.retain_map.layout_hash,
            "{count}, {x}, {instance}"
        );
        let sizes = |m: &mir::MirModule| {
            m.retain_map
                .ranges
                .iter()
                .map(|r| r.size)
                .collect::<Vec<_>>()
        };
        assert_eq!(sizes(&base), sizes(&renamed), "the same payload order");
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

/// Each name is quoted as written, and wherever it is quoted in any case, it
/// is in that one: `'MainDrive.setpoint'` is a folded name too.
fn assert_shown_as_written(shown: &str, names: &[&str]) {
    let lower = shown.to_lowercase();
    for name in names {
        let quoted = format!("'{name}'");
        assert!(shown.contains(&quoted), "{quoted} is not shown:\n{shown}");
        for (at, _) in lower.match_indices(&quoted.to_lowercase()) {
            assert_eq!(
                &shown[at..at + quoted.len()],
                quoted,
                "a name is shown in another case:\n{shown}"
            );
        }
    }
}

/// The messages that name a declaration or a written name: the overload
/// set, a missing field (in a path and in an initializer), a missing
/// namespace, a parameter or a field given twice.
#[rstest]
fn diagnostics_show_names_as_written(mut with_db: RootDatabase) {
    let source = r#"
USING NoSuchNs.SubNs;

TYPE PoinT : STRUCT OfFset : INT; END_STRUCT; END_TYPE

FUNCTION PiCkOne : INT VAR_INPUT a : INT; END_VAR PiCkOne := 1; END_FUNCTION
FUNCTION PiCkOne : INT VAR_INPUT a : DATE; END_VAR PiCkOne := 2; END_FUNCTION

FUNCTION AmBoth : INT VAR_INPUT a : DINT; END_VAR AmBoth := 1; END_FUNCTION
FUNCTION AmBoth : INT VAR_INPUT a : LINT; END_VAR AmBoth := 2; END_FUNCTION

FUNCTION TakeS : INT VAR_INPUT SetPoint : INT; END_VAR TakeS := SetPoint; END_FUNCTION

FUNCTION Caller : INT
VAR
    p : PoinT;
    q : PoinT := (OfFset := 1, OFFSET := 2);
    r : PoinT := (NoFieLd := 1);
    i : INT;
END_VAR
    Caller := PiCkOne(TRUE);
    Caller := AmBoth(i);
    i := p.MissingFieLd;
    i := TakeS(SetPoint := 1, SETPOINT := 2);
END_FUNCTION
"#;
    assert_shown_as_written(
        &test_diagnostics(&mut with_db, &[source]),
        &[
            "NoSuchNs.SubNs",
            "PiCkOne",
            "AmBoth",
            "MissingFieLd",
            "NoFieLd",
            "SETPOINT",
            "OFFSET",
        ],
    );
}

#[rstest]
fn a_lint_shows_names_as_written(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Drive
VAR SetPoint : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION Caller : INT
VAR MainDrive : Drive; END_VAR
    MainDrive.SetPoint := 42;
END_FUNCTION
"#;
    assert_shown_as_written(
        &test_single_lint(&mut with_db, &[source], "external-mutation"),
        &["MainDrive.SetPoint"],
    );
}

/// An item from another namespace offers the USING it needs, and inserts
/// it: written into the file, a folded name would be a spelling nobody wrote.
#[rstest]
fn a_completion_imports_the_namespace_as_written(mut with_db: RootDatabase) {
    let source = "NAMESPACE MyNs\n\tFUNCTION fn1\n\n\tEND_FUNCTION\nEND_NAMESPACE\n\nFUNCTION fn2\n\nEND_FUNCTION\n";
    let file = add_source(&mut with_db, source);
    let pou = find_pou_with_name(&with_db, file, "fn2").unwrap();
    let offset = source.find("FUNCTION fn2").unwrap() + "FUNCTION fn2\n".len();

    let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let shown = format!("{:?}", ctx.take_items());

    assert!(shown.contains("(USING MyNs)"), "the label:\n{shown}");
    assert!(shown.contains("USING MyNs;"), "the edit:\n{shown}");
    assert!(!shown.contains("myns"), "folded:\n{shown}");
}

/// A NAMESPACE in the outline and in its closing inlay hint.
#[rstest]
fn a_namespace_is_outlined_as_written(mut with_db: RootDatabase) {
    let source = "NAMESPACE MyNs\nEND_NAMESPACE\n";
    let file = add_source(&mut with_db, source);
    let sema = semantic_index(&with_db, file);

    let mut builder = DocumentSymbolsBuilder::default();
    let mut hints = Vec::new();
    for ns in sema.namespaces.iter() {
        ns.document_symbols(&with_db, &mut builder);
        hints.extend(ns.inlay_hint(&with_db));
    }
    let outline = format!("{:?}", builder.finalize());
    let hint = format!("{hints:?}");

    assert!(
        outline.contains(r#"name: "MyNs""#),
        "the outline:\n{outline}"
    );
    assert!(hint.contains(r#"value: "MyNs""#), "the inlay hint:\n{hint}");
    assert!(
        !outline.contains("myns") && !hint.contains("myns"),
        "folded"
    );
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
        let file = add_source(&mut db, source);
        fmt(file.document(&db))
    };

    let lower = format(LOWER);
    let upper = format(UPPER);

    // The formatter must have done something; identical-but-untouched would
    // satisfy the comparison below for the wrong reason.
    assert_ne!(
        lower, LOWER,
        "the formatter left the lower case source untouched"
    );
    assert_ne!(
        upper, UPPER,
        "the formatter left the upper case source untouched"
    );

    assert_eq!(
        lower.to_ascii_lowercase(),
        upper.to_ascii_lowercase(),
        "formatting differs by keyword case.\n--- from lower case ---\n{lower}\n--- from upper case ---\n{upper}"
    );
}
