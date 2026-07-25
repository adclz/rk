//! Codegen (HIR → MIR lowering) failures must carry a source location.
//!
//! A `LowerTypeError` used to surface as a bare string with no file, line, or
//! construct — so a type-clean program that hit an unsupported construct died
//! with `codegen error: <debug text>` and nothing to act on. Lowering now pins
//! each error to the offending expression (or, failing that, the enclosing POU
//! declaration), which the CLI renders like any other diagnostic.

use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

/// An unsupported construct inside a body reports the *expression*, not the
/// whole POU: lowering recurses through `lower_expr`, and the innermost frame
/// wins the location.
#[rstest]
fn codegen_error_points_at_the_offending_expression(mut with_db: RootDatabase) {
    // Type-checks clean (WORD := WORD) and only fails at lowering.
    let source = r#"
FUNCTION uses_direct : WORD
VAR w : WORD; END_VAR
    w := %IW4;
    uses_direct := w;
END_FUNCTION
"#;
    let file = add_source(&mut with_db, source);
    let sem_idx = semantic_index(&with_db, file);

    let err = mir::lower::lower_module::lower_module(&with_db, sem_idx)
        .expect_err("direct variable access is not lowerable");

    let (err_file, span) = err
        .location()
        .expect("a codegen error must carry a source location");
    assert_eq!(err_file, file, "location points at the offending file");
    assert_eq!(
        span.start_point.row, 3,
        "location points at the `%IW4` expression (line 4), not the POU"
    );
    assert!(
        format!("{err}").contains("Direct variable access"),
        "the underlying cause survives the location wrapper: {err}"
    );
}

/// `with_location` keeps the first (deepest) location attached, so the POU-level
/// fallback never overwrites a precise expression span.
#[rstest]
fn innermost_location_wins(mut with_db: RootDatabase) {
    use mir::lower::lower_type::LowerTypeError;

    let source = "FUNCTION f : INT\n    f := 1;\nEND_FUNCTION\n";
    let file = add_source(&mut with_db, source);
    let inner = auto_lsp::tree_sitter::Range {
        start_byte: 1,
        end_byte: 2,
        start_point: auto_lsp::tree_sitter::Point { row: 1, column: 1 },
        end_point: auto_lsp::tree_sitter::Point { row: 1, column: 2 },
    };
    let outer = auto_lsp::tree_sitter::Range {
        start_byte: 9,
        end_byte: 9,
        start_point: auto_lsp::tree_sitter::Point { row: 9, column: 9 },
        end_point: auto_lsp::tree_sitter::Point { row: 9, column: 9 },
    };

    let err = LowerTypeError::UnsupportedType("boom".into())
        .with_location(file, inner)
        .with_location(file, outer);

    let (_, span) = err.location().expect("located");
    assert_eq!(
        span.start_point.row, 1,
        "the deeper location is kept when an outer frame also locates the error"
    );
}
