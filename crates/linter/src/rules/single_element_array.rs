use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::spec::SpecKind};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "single-element-array";

/// L0110: array dimension with equal lower and upper bounds has only one element.
struct SingleElementArray;

impl ErrorCode for SingleElementArray {
    fn code(&self) -> &'static str {
        "L0110"
    }

    fn description(&self) -> &'static str {
        "single-element array"
    }
}

/// Check a variable's spec for single-element array dimensions.
pub fn check_spec<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: &SpecKind<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if let SpecKind::Array(array) = spec {
        let dims = hir::hir_ty::infer::const_eval::array_dimensions(db, *array);
        for (dim_idx, ((lower, _upper), (lo, hi))) in
            array.subranges(db).iter().zip(dims).enumerate()
        {
            if let (Some(lo), Some(hi)) = (lo, hi)
                && lo == hi
            {
                diagnostics.push(
                        diag()
                            .message(format!(
                                "array dimension {} has equal bounds ({lo}..{hi}), contains only one element",
                                dim_idx + 1
                            ))
                            .desc(&SingleElementArray)
                            .range(hir::denormalize(db, lower.get_scope_id(db).file(db), &lower.get_span(db)).unwrap_or_default())
                            .severity(DiagnosticSeverity::INFORMATION)
                            .call(),
                    );
            }
        }
        // Recurse into nested array types
        check_spec(db, array.of_type(db).kind(db), diagnostics);
    }
}
