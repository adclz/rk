use auto_lsp::{
    default::db::{BaseDatabase, File},
    lsp_types::DiagnosticRelatedInformation,
};

use crate::{
    diagnostics::{diagnostic_builder::{diag, OneOf, RangeKind}, IdeDiagnostic},
    solver::{namespace_path, namespaces_in_file},
};

pub fn duplicate_declarations<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    acc: &'db mut Vec<IdeDiagnostic>,
) {
    if let Some(namespaces) = namespaces_in_file(db, file) {
        for (path, namespace) in namespaces.namespaces(db).iter() {
            for (pou) in namespace.pous(db).iter() {
                let name = pou.name(db);
                for other_decl in namespace_path(db, *path) {
                    if other_decl.file(db) == file {
                        continue;
                    }

                    if let Some(other_pou) = other_decl.get_pou(db, *path, *name) {
                        let duplicate = pou.name(db).text(db);
                        acc.push(diag()
                            .range(OneOf::T(pou.span(db).into()))
                            .message(format!("duplicate declarations of {duplicate} POU"))
                            .source("IEC".into())
                            .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                            .related_information(vec![DiagnosticRelatedInformation {
                                location: auto_lsp::lsp_types::Location {
                                    uri: other_decl.file(db).url(db).clone(),
                                    range: RangeKind::from(other_pou.span(db)).into(),
                                },
                                message: format!("'{duplicate}' is previously declared here"),
                            }])
                            .call()
                        );
                    }
                }
            }
        }
    }
}