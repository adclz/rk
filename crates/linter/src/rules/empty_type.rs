use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{expressions::spec::SpecKind, pous::data_type::DataType},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "empty-type";

/// L0213: a STRUCT or ENUM type declaration has no members.
struct EmptyType;

impl ErrorCode for EmptyType {
    fn code(&self) -> &'static str {
        "L0214"
    }

    fn description(&self) -> &'static str {
        "empty type declaration"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    dt: DataType<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match dt.spec(db).kind(db) {
        SpecKind::Struct(s) => {
            if s.elements(db).is_empty() {
                let name = dt.get_name_ident(db).text(db);
                diagnostics.push(
                    diag()
                        .message(format!("STRUCT '{name}' has no fields"))
                        .desc(&EmptyType)
                        .range(
                            hir::denormalize(
                                db,
                                dt.get_scope_id(db).file(db),
                                &dt.get_name_span(db),
                            )
                            .unwrap_or_default(),
                        )
                        .severity(DiagnosticSeverity::HINT)
                        .call(),
                );
            }
        }
        SpecKind::Enum(e) => {
            if e.variants(db).is_empty() {
                let name = dt.get_name_ident(db).text(db);
                diagnostics.push(
                    diag()
                        .message(format!("ENUM '{name}' has no variants"))
                        .desc(&EmptyType)
                        .range(
                            hir::denormalize(
                                db,
                                dt.get_scope_id(db).file(db),
                                &dt.get_name_span(db),
                            )
                            .unwrap_or_default(),
                        )
                        .severity(DiagnosticSeverity::HINT)
                        .call(),
                );
            }
        }
        _ => {}
    }
}
