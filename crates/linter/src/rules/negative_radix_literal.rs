use std::collections::HashMap;

use auto_lsp::lsp_types::{CodeAction, DiagnosticSeverity, TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, IntegerKind, PrimaryExpr, UnaryOperatorKind},
            spec::ElementarySpec,
        },
        scope::ScopeId,
    },
    hir_ty::ty::Type,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "negative-radix-literal";

/// L0123: an untyped radix literal with its top bit set, where a signed
/// integer is expected. A radix literal is a bit pattern of the width its
/// context gives it, so the same text is two values: `16#80` is 128 in an
/// INT and -128 in a SINT. Written for 128 or 255, it is negative with no
/// error. A typed literal, `SINT#16#80`, says the pattern is meant and is not
/// reported. Neither is one under a minus, which reads as a number
/// (`-(16#80)` is minus 128), nor an operand of AND, OR or XOR, where the
/// bits are the point.
struct NegativeRadixLiteral;

impl ErrorCode for NegativeRadixLiteral {
    fn code(&self) -> &'static str {
        "L0123"
    }

    fn description(&self) -> &'static str {
        "radix literal is negative"
    }
}

/// Every untyped radix literal of `scope`, in its body and in its
/// declarations' initializers, read with the type inference gave it.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let typed: rustc_hash::FxHashMap<Expr<'db>, Type<'db>> =
        scope.inference(db).typed_exprs().collect();
    // A minus in front, through parentheses and a plus, makes the literal a
    // number, as the check reads it; AND, OR and XOR read its bits.
    let mut exempt: rustc_hash::FxHashSet<Expr<'db>> = rustc_hash::FxHashSet::default();
    for expr in typed.keys() {
        match expr.expr(db) {
            ExprKind::UnaryOperator {
                operator: UnaryOperatorKind::Minus,
                expr: inner,
            } => {
                exempt.insert(signed_operand(db, *inner));
            }
            ExprKind::BooleanOperator { left, right, .. } => {
                exempt.insert(signed_operand(db, *left));
                exempt.insert(signed_operand(db, *right));
            }
            _ => {}
        }
    }
    // In source order: the maps are not.
    let mut literals: Vec<(Expr<'db>, Type<'db>)> = typed.into_iter().collect();
    literals.sort_by_key(|(expr, _)| expr.get_span(db).start_byte);

    let file = scope.file(db);
    for (expr, ty) in literals {
        let ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(integer))) =
            expr.expr(db)
        else {
            continue;
        };
        if integer.kind(db) == IntegerKind::Signed || exempt.contains(&expr) {
            continue;
        }
        let Type::Elementary(spec) = ty.normalize(db) else {
            continue;
        };
        let bits: u32 = match spec {
            ElementarySpec::SInt => 8,
            ElementarySpec::Int => 16,
            ElementarySpec::DInt => 32,
            ElementarySpec::LInt => 64,
            _ => continue,
        };
        let Ok(written) = integer.as_i128(db) else {
            continue;
        };
        // The patterns whose top bit is set; a wider one is refused (E0306).
        if written < 1 << (bits - 1) || written >= 1 << bits {
            continue;
        }
        let value = written - (1 << bits);
        let type_name = spec.type_name();
        let text = hir::CallSite::from_scoped(db, &expr).to_string(db);
        let replacement = format!("{type_name}#{text}");

        let range = hir::denormalize(db, file, &expr.get_span(db)).unwrap_or_default();
        let mut diagnostic = diag()
            .message(format!(
                "'{text}' is the {type_name} {value}: a radix literal is a bit pattern, and the top bit is the sign"
            ))
            .desc(&NegativeRadixLiteral)
            .range(range)
            .severity(DiagnosticSeverity::WARNING)
            .call();
        diagnostic.with_fix(CodeAction {
            title: format!("write {replacement}"),
            edit: Some(WorkspaceEdit::new(HashMap::from([(
                file.url(db).clone(),
                vec![TextEdit {
                    range,
                    new_text: replacement,
                }],
            )]))),
            is_preferred: Some(true),
            ..Default::default()
        });
        diagnostics.push(diagnostic);
    }
}

/// The operand an operator applies to, through parentheses and a plus.
fn signed_operand<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> Expr<'db> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner })
        | ExprKind::UnaryOperator {
            operator: UnaryOperatorKind::Plus,
            expr: inner,
        } => signed_operand(db, *inner),
        _ => expr,
    }
}
