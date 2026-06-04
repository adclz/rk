//! Encoding (de)normalization tests.
//!
//! The rest of the suite uses pure-ASCII `.st` sources, where UTF-8 byte columns, UTF-16 code
//! units, and UTF-32 code points all coincide — so the encoding conversions are never actually
//! exercised. These tests put **multi-byte** characters (in a comment, the only place IEC ST allows
//! non-ASCII) before a symbol and assert that:
//!
//! * outgoing ranges are denormalized to the client's negotiated encoding
//!   (`hir::denormalize` → `Document::denormalize_range`), and
//! * an incoming LSP position is converted back to the correct UTF-8 byte offset
//!   (`ide_proto::walk::position_to_offset`).
//!
//! The reference char is `🎈` (U+1F388): 4 bytes in UTF-8, **2** code units in UTF-16 (a surrogate
//! pair), 1 code point in UTF-32 — so all three encodings give a *different* column, which is what
//! makes these assertions meaningful.

use auto_lsp::core::document_symbols_builder::DocumentSymbolsBuilder;
use auto_lsp::default::db::{FileManager, file::File};
use auto_lsp::lsp_types::{Position, PositionEncodingKind, Url};
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use hir::{HasName, denormalize};
use ide_proto::handlers::DocumentSymbolsHandler;
use ide_proto::walk::position_to_offset;
use rstest::rstest;

use crate::tests::utils::{find_pou_with_name, with_db};

/// `(* 🎈 *) FUNCTION_BLOCK fb` then an empty body.
///
/// Column of `fb` on line 0, by encoding (prefix `"(* 🎈 *) FUNCTION_BLOCK "`):
/// * UTF-8  bytes:      1+1+1+4+1+1+1+1 = 11, + "FUNCTION_BLOCK " (15) = **26**
/// * UTF-16 code units: 1+1+1+2+1+1+1+1 = 9,  + 15                     = **24**
/// * UTF-32 code points:1+1+1+1+1+1+1+1 = 8,  + 15                     = **23**
const SOURCE: &str = "(* 🎈 *) FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK";

fn add_source_enc(db: &mut RootDatabase, source: &str, encoding: &PositionEncodingKind) -> File {
    let url = Url::parse("file:///enc.st").unwrap();
    let file = File::from_string()
        .db(db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .encoding(encoding)
        .call()
        .unwrap();
    db.add_file(file).unwrap();
    file
}

/// Denormalizing the raw (UTF-8 byte) name range yields the column in the negotiated encoding.
#[rstest]
#[case(PositionEncodingKind::UTF8, 26, 28)] // identity: byte columns
#[case(PositionEncodingKind::UTF16, 24, 26)] // surrogate pair counts as 2
#[case(PositionEncodingKind::UTF32, 23, 25)] // code points
fn denormalize_name_range_per_encoding(
    mut with_db: RootDatabase,
    #[case] encoding: PositionEncodingKind,
    #[case] start_char: u32,
    #[case] end_char: u32,
) {
    let file = add_source_enc(&mut with_db, SOURCE, &encoding);
    let fb = find_pou_with_name(&with_db, file, "fb").expect("fb not found");

    // get_name_span returns the raw tree-sitter range with UTF-8 byte columns.
    let raw = fb.get_name_span(&with_db);
    assert_eq!(
        raw.start_point.column, 26,
        "raw tree-sitter column is always UTF-8 bytes"
    );

    let range = denormalize(&with_db, file, &raw).expect("denormalize failed");
    assert_eq!(range.start, Position::new(0, start_char));
    assert_eq!(range.end, Position::new(0, end_char));
}

/// An incoming LSP position (in the negotiated encoding) maps back to the right UTF-8 byte offset.
#[rstest]
#[case(PositionEncodingKind::UTF8, 26)]
#[case(PositionEncodingKind::UTF16, 24)]
#[case(PositionEncodingKind::UTF32, 23)]
fn position_to_offset_per_encoding(
    mut with_db: RootDatabase,
    #[case] encoding: PositionEncodingKind,
    #[case] character: u32,
) {
    let file = add_source_enc(&mut with_db, SOURCE, &encoding);

    let offset = position_to_offset(&with_db, file, Position::new(0, character))
        .expect("position_to_offset failed");

    // Regardless of encoding, the cursor on `fb` resolves to the same UTF-8 byte offset (26).
    assert_eq!(offset, 26);
    let src = file.document(&with_db).as_str();
    assert_eq!(&src[offset..offset + 2], "fb");
}

/// `denormalize` (range → LSP) and `position_to_offset` (LSP → offset) are inverses at the name start.
#[rstest]
#[case(PositionEncodingKind::UTF8)]
#[case(PositionEncodingKind::UTF16)]
#[case(PositionEncodingKind::UTF32)]
fn denormalize_position_to_offset_round_trip(
    mut with_db: RootDatabase,
    #[case] encoding: PositionEncodingKind,
) {
    let file = add_source_enc(&mut with_db, SOURCE, &encoding);
    let fb = find_pou_with_name(&with_db, file, "fb").unwrap();
    let raw = fb.get_name_span(&with_db);

    let lsp = denormalize(&with_db, file, &raw).unwrap();
    let offset = position_to_offset(&with_db, file, lsp.start).unwrap();

    assert_eq!(offset, raw.start_byte as usize);
}

/// A multi-byte char on an earlier line must not shift columns on later lines (line-local conversion).
#[rstest]
fn multibyte_on_earlier_line_does_not_shift_later_lines(mut with_db: RootDatabase) {
    // `🎈` is only on line 0; `fb2` sits on line 2 in an all-ASCII line.
    let source = "(* 🎈 *)\n\nFUNCTION_BLOCK fb2\nEND_FUNCTION_BLOCK";
    let file = add_source_enc(&mut with_db, source, &PositionEncodingKind::UTF16);
    let fb = find_pou_with_name(&with_db, file, "fb2").unwrap();

    let range = denormalize(&with_db, file, &fb.get_name_span(&with_db)).unwrap();
    // "FUNCTION_BLOCK " == 15 ASCII chars, so the name starts at column 15 on its own line,
    // unaffected by the balloon two lines up.
    assert_eq!(range.start, Position::new(2, 15));
    assert_eq!(range.end, Position::new(2, 18));
}

/// End-to-end: a real handler (document symbols) emits encoding-adjusted ranges, not byte columns.
#[rstest]
fn document_symbols_emit_encoded_ranges(mut with_db: RootDatabase) {
    // `x` follows a balloon comment on the same line.
    let source = "FUNCTION_BLOCK fb\nVAR (* 🎈 *) x : INT; END_VAR\nEND_FUNCTION_BLOCK";
    let file = add_source_enc(&mut with_db, source, &PositionEncodingKind::UTF16);

    let mut builder = DocumentSymbolsBuilder::default();
    semantic_index(&with_db, file)
        .global_pous
        .iter()
        .for_each(|pou| pou.document_symbols(&with_db, &mut builder));
    let symbols = builder.finalize();

    let fb = symbols.iter().find(|s| s.name == "fb").expect("fb symbol");
    let x = fb
        .children
        .as_ref()
        .expect("fb children")
        .iter()
        .find(|s| s.name == "x")
        .expect("x symbol");

    // line 1: "VAR (* 🎈 *) x ..." — "VAR " (4) + "(* 🎈 *) " (9 UTF-16 units) => column 13,
    // whereas the UTF-8 byte column would be 15.
    assert_eq!(x.selection_range.start, Position::new(1, 13));
}
