// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! One source in a fresh database, as `rk check` sees a single file, read
//! beside one library, with one of the four ways `[addresses]` can number
//! the addresses it names.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, PositionEncodingKind, Url};
use db::config_file::{AddressesConfig, ByteOrder, Numbering};
use db::{RootDatabase, WorkspaceDataBase};
use hir::hir_def::semantic_index::semantic_index;
use ide_diagnostic::IdeDiagnostic;

/// Every input is the same file: a finding reproduces from its text alone.
pub(crate) const URL: &str = "file:///fuzz.st";

pub(crate) fn url() -> Url {
    Url::parse(URL).expect("a valid url")
}

/// The library every input is read with, named `Lib`: a FUNCTION at its
/// top level, two namespaces, and what a workspace cannot reach. A name
/// resolves through a library's root on every input, and a generated
/// program calls these ([`crate::generate`]); a mutated one reaches for
/// `Lib.Flags.Hidden` and `Lib.Keep` and is refused.
pub(crate) const LIBRARY: &str = "FUNCTION Half : DINT
VAR_INPUT n : DINT; END_VAR
    Half := n / 2;
END_FUNCTION

NAMESPACE Math
    FUNCTION Clamp0 : DINT
    VAR_INPUT n : DINT; END_VAR
        IF n < 0 THEN
            Clamp0 := 0;
        ELSE
            Clamp0 := n;
        END_IF;
    END_FUNCTION
END_NAMESPACE

NAMESPACE Flags
    FUNCTION Both : BOOL
    VAR_INPUT a : BOOL; b : BOOL; END_VAR
        Both := a AND b;
    END_FUNCTION

    NAMESPACE INTERNAL Hidden
        FUNCTION Secret : BOOL
            Secret := TRUE;
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE

FUNCTION PRIVATE Keep : DINT
    Keep := 1;
END_FUNCTION
";

pub(crate) const LIBRARY_URL: &str = "file:///lib/Lib.st";

/// The `[addresses]` an input is read with: one of the four, picked from
/// its text, so that a seed naming addresses is also read counting bytes
/// and big-endian, and a finding reproduces from the text alone. A source
/// that names no address reads the same with any of them.
pub(crate) fn addresses_of(source: &str) -> AddressesConfig {
    // FNV-1a, which no toolchain changes, unlike the standard hasher.
    let hash = source.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    AddressesConfig {
        numbering: match hash & 1 {
            0 => Numbering::Width,
            _ => Numbering::Byte,
        },
        byte_order: match hash & 2 {
            0 => ByteOrder::Little,
            _ => ByteOrder::Big,
        },
    }
}

/// The `config.toml` that says `addresses`: the standard library's, which
/// sets nothing, or a copy of it with an `[addresses]` section, in
/// `configs/`.
fn config_file(addresses: AddressesConfig) -> std::path::PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if addresses == AddressesConfig::default() {
        return crate_dir.join("../../stdlib/config.toml");
    }
    let numbering = match addresses.numbering {
        Numbering::Width => "width",
        Numbering::Byte => "byte",
    };
    let order = match addresses.byte_order {
        ByteOrder::Little => "little",
        ByteOrder::Big => "big",
    };
    crate_dir.join(format!("configs/{numbering}-{order}.toml"))
}

/// A database holding `source` as its only workspace file, beside
/// [`LIBRARY`], read with the `[addresses]` [`addresses_of`] picks for it,
/// or `None` when the parser gives up on it (tree-sitter only does on a
/// timeout or cancellation).
pub(crate) fn load(source: &str) -> Option<(RootDatabase, File)> {
    load_with(source, addresses_of(source))
}

/// [`load`] with `addresses`: for an oracle that reads two sources of one
/// input, both as the input says.
pub(crate) fn load_with(source: &str, addresses: AddressesConfig) -> Option<(RootDatabase, File)> {
    let mut db = RootDatabase::default();
    // The library's name is the root its declarations are under; the
    // config file named is the standard library's, with no settings but
    // `[addresses]`, so that no report carries the "no config.toml" hint.
    db::workspace::Workspace::new(
        &db,
        None,
        vec![db::dependencies::Library {
            name: "Lib".to_string(),
            dir: std::path::PathBuf::from("/lib"),
            standard: false,
        }],
        Some(config_file(addresses)),
        db::config_file::ProjectKind::Program,
        PositionEncodingKind::UTF8,
    );
    let library_url = Url::parse(LIBRARY_URL).expect("a valid url");
    let library = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&library_url)
        .source(LIBRARY.to_string())
        .call()
        .ok()?;
    db.insert_library_file(library);

    let url = url();
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .ok()?;
    db.add_file(file).ok()?;
    let file = db.get_file(&url)?;
    Some((db, file))
}

/// The module of `file` and the library, as `rk compile` lowers a workspace
/// with its libraries, keeping what its exports reach.
pub(crate) fn lower(
    db: &RootDatabase,
    file: File,
) -> Result<mir::MirModule, mir::lower::lower_type::LowerTypeError> {
    let mut indices: Vec<_> = db
        .get_library_files()
        .iter()
        .map(|file| semantic_index(db, *file))
        .collect();
    indices.push(semantic_index(db, file));
    let mut module = mir::lower::lower_module::lower_modules(db, &indices)?;
    module.keep_reached();
    Ok(module)
}

/// As `rk check` counts: an unset severity is an error by convention.
pub(crate) fn is_error(diagnostic: &IdeDiagnostic) -> bool {
    matches!(
        diagnostic.diagnostic.severity,
        Some(DiagnosticSeverity::ERROR) | None
    )
}

/// What `rk check` reports on `file`: its own check, and the frames of
/// its recursive calls.
pub(crate) fn diagnostics(db: &RootDatabase, file: File) -> Vec<IdeDiagnostic> {
    let graph = hir::hir_ty::calls::CallGraph::of_files(db, [file]);
    hir::check::check_file(db, &graph, file)
}

/// Whether `rk compile` would go on to lower this file.
pub(crate) fn compiles(db: &RootDatabase, file: File) -> bool {
    !diagnostics(db, file).iter().any(is_error)
}

/// The diagnostics as the editor shows them, positions included, in a
/// stable order.
pub(crate) fn positioned_diagnostics(db: &RootDatabase, file: File) -> Vec<String> {
    let mut out: Vec<String> = diagnostics(db, file)
        .iter()
        .map(|d| format!("{:?}", d.to_lsp_diagnostic(db)))
        .collect();
    out.sort();
    out
}

/// What the diagnostics SAY, without where: formatting moves every span, and
/// only the set of problems must survive it.
pub(crate) fn diagnostic_identities(db: &RootDatabase, file: File) -> Vec<String> {
    let mut out: Vec<String> = diagnostics(db, file)
        .iter()
        .map(|d| {
            let inner = &d.diagnostic;
            format!("{:?} {:?} {}", inner.code, inner.severity, inner.message)
        })
        .collect();
    out.sort();
    out
}
