pub mod date;
pub mod dt;
pub mod floats;
pub mod integers;
pub mod invalid_literals;
pub mod strings;
pub mod time;
pub mod tod;

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HasName;
use hir::hir_def::expressions::expression::{Elementary, ExprKind, InitExprKind, PrimaryExpr};
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::variable::VariableDecl;

use crate::tests::utils::{add_sources, find_pou_with_name};

/// Build a single-variable test source: `VAR x : <ty> := <literal>;`
/// inside a function block called `fb1`. Returns the literal's `Ident`
/// so each per-type integer-encoding test can call the matching
/// `Ident::as_*` helper directly.
pub(super) fn parse_literal<'db>(
    db: &'db mut RootDatabase,
    ty: &str,
    literal: &str,
) -> Ident {
    let source = format!(
        "FUNCTION_BLOCK fb1\n\
         VAR\n\
             x : {ty} := {literal};\n\
         END_VAR\n\
         END_FUNCTION_BLOCK\n"
    );
    add_sources(db, &[&source]);
    let file = *db.get_files().iter().last().unwrap();
    let pou = find_pou_with_name(db, file, "fb1").expect("fb1 missing");
    let fb = match pou {
        Pou::FunctionBlock(fb) => fb,
        _ => panic!("expected FunctionBlock"),
    };
    let var: VariableDecl = *fb
        .variables(db)
        .iter()
        .find(|v| v.name(db).text(db).as_str() == "x")
        .expect("var x missing");
    let init = var.init(db).expect("var x has no init");
    let expr = match init.kind(db) {
        InitExprKind::ConstantExpr(e) => e,
        other => panic!("unexpected init kind: {other:?}"),
    };
    let prim = match expr.expr(db) {
        ExprKind::PrimaryExpr(p) => p,
        other => panic!("unexpected expr kind: {other:?}"),
    };
    let elem = match prim {
        PrimaryExpr::Literal(e) => e,
        other => panic!("unexpected primary expr: {other:?}"),
    };
    match elem {
        Elementary::Time(i)
        | Elementary::LTime(i)
        | Elementary::Date(i)
        | Elementary::LDate(i)
        | Elementary::TimeOfDay(i)
        | Elementary::LTod(i)
        | Elementary::DateAndTime(i)
        | Elementary::LDateTime(i) => *i,
        other => panic!("not a time/date/tod literal: {other:?}"),
    }
}
