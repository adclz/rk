use auto_lsp::{
    default::db::{BaseDatabase, File},
    lsp_types::{self, DiagnosticRelatedInformation},
};
use salsa::Accumulator;

use crate::{
    diagnostics::{DiagnosticAccumulator, IdeDiagnostic},
    solver::{namespace_path, namespaces_in_file},
};

pub fn duplicate_declarations<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    acc: &'db mut Vec<IdeDiagnostic>,
) {
    if let Some(namespaces) = namespaces_in_file(db, file) {
        for (path, namespace) in namespaces.namespaces(db).iter() {
            for (name, pou) in namespace.pous(db).iter() {
                for other_decl in namespace_path(db, *path) {
                    if other_decl.file(db) == file {
                        continue;
                    }

                    if let Some(other_pou) = other_decl.get_pou(db, *path, *name) {
                        let duplicate = name.text(db);
                        acc.push(duplicate_diagnostic(
                            db,
                            pou.span(db),
                            format!("Duplicate declarations of {duplicate} POU").into(),
                            other_pou.span(db),
                            duplicate,
                            other_decl.file(db),
                        ));
                    }
                }
            }
        }
    }
}

fn duplicate_diagnostic(
    db: &dyn BaseDatabase,
    range: auto_lsp::tree_sitter::Range,
    message: String,
    related_range: auto_lsp::tree_sitter::Range,
    related_name: String,
    related_file: File,
) -> IdeDiagnostic {
    auto_lsp::lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position::new(
                range.start_point.row as u32,
                range.start_point.column as u32,
            ),
            end: lsp_types::Position::new(
                range.end_point.row as u32,
                range.end_point.column as u32,
            ),
        },
        code_description: None,
        severity: Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR),
        code: None,
        source: Some("IEC".into()),
        message,
        related_information: Some(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: related_file.url(db).clone(),
                range: auto_lsp::lsp_types::Range {
                    start: auto_lsp::lsp_types::Position::new(
                        related_range.start_point.row as u32,
                        related_range.start_point.column as u32,
                    ),
                    end: auto_lsp::lsp_types::Position::new(
                        related_range.end_point.row as u32,
                        related_range.end_point.column as u32,
                    ),
                },
            },
            message: format!("'{related_name}' is previously declared here"),
        }]),
        tags: None,
        data: None,
    }
    .into()
}
