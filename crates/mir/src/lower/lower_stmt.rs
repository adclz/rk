use db::WorkspaceDataBase;
use hir::hir_def::expressions::statement::{Stmt, StmtKind};
use hir::hir_ty::infer::Infer;

use crate::{
    expr::MirConstant,
    lower::lower_expr::ExprLowerCtx,
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir},
    stmt::MirStmt,
    types::MirElementary,
};

/// Check if we need an implicit cast between two HIR types.
fn needs_cast<'db>(
    db: &'db dyn WorkspaceDataBase,
    from: hir::hir_ty::ty::Type<'db>,
    to: hir::hir_ty::ty::Type<'db>,
) -> bool {
    let from = from.normalize(db);
    let to = to.normalize(db);
    match (&from, &to) {
        (hir::hir_ty::ty::Type::Elementary(f), hir::hir_ty::ty::Type::Elementary(t)) => f != t,
        _ => false,
    }
}

/// Lower a slice of HIR statements to MIR statements.
pub fn lower_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
) -> Result<Vec<MirStmt>, LowerTypeError> {
    lower_stmts_with_ctx(db, stmts, None)
}

/// Lower a slice of HIR statements with an optional ANY type override for monomorphization.
pub fn lower_stmts_with_ctx<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    any_override: Option<hir::hir_def::expressions::spec::ElementarySpec>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let ctx = match any_override {
        Some(concrete) => ExprLowerCtx::with_any_override(db, concrete),
        None => ExprLowerCtx::new(db),
    };
    let mut result = Vec::new();
    for stmt in stmts {
        if let Some(mir_stmt) = lower_stmt(&ctx, *stmt)? {
            result.push(mir_stmt);
        }
    }
    Ok(result)
}

/// Lower a single HIR statement to a MIR statement.
/// Returns None for statements that have no MIR equivalent (e.g., ExternPragma).
fn lower_stmt<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmt: Stmt<'db>,
) -> Result<Option<MirStmt>, LowerTypeError> {
    match stmt.stmt(ctx.db) {
        StmtKind::Assignment { var, target } => {
            let place = ctx.lower_variable_access(*var)?;
            // Get target variable type for implicit cast insertion
            let var_type = var.infer(ctx.db);
            let target_type = target.infer(ctx.db);
            let value = if needs_cast(ctx.db, target_type, var_type) {
                // Insert cast from expression type to variable type
                let from = ctx.type_to_mir_elementary_pub(target_type)?;
                let to = ctx.type_to_mir_elementary_pub(var_type)?;
                let inner = ctx.lower_expr(*target)?;
                crate::expr::MirExpr::Cast {
                    expr: Box::new(inner),
                    from,
                    to,
                }
            } else {
                ctx.lower_expr(*target)?
            };
            Ok(Some(MirStmt::Assign {
                target: place,
                value,
            }))
        }

        StmtKind::FuncCall(func_call) => {
            // Lower the FuncCall directly without creating a new Expr (which would
            // create a Salsa tracked struct outside a tracked function).
            let call_expr = ctx.lower_func_call(*func_call, None)?;
            match call_expr {
                crate::expr::MirExpr::Call(call) => Ok(Some(MirStmt::Call(call))),
                _ => Ok(None),
            }
        }

        StmtKind::Return => Ok(Some(MirStmt::Return)),

        StmtKind::If {
            condition,
            then,
            else_if,
            else_,
        } => {
            let cond = ctx.lower_expr(*condition)?;
            let then_body = match then {
                Some(stmts) => lower_stmts_inner(ctx, stmts)?,
                None => Vec::new(),
            };
            let mut else_ifs = Vec::new();
            for (cond_expr, body) in else_if {
                let eif_cond = ctx.lower_expr(*cond_expr)?;
                let eif_body = lower_stmts_inner(ctx, body)?;
                else_ifs.push((eif_cond, eif_body));
            }
            let else_body = match else_ {
                Some(stmts) => Some(lower_stmts_inner(ctx, stmts)?),
                None => None,
            };

            Ok(Some(MirStmt::If {
                condition: cond,
                then_body,
                else_ifs,
                else_body,
            }))
        }

        StmtKind::Case {
            condition,
            cases,
            else_,
        } => {
            let selector = ctx.lower_expr(*condition)?;
            let mut arms = Vec::new();
            for (case_kinds, body) in cases {
                let mut patterns = Vec::new();
                for ck in case_kinds {
                    patterns.push(ctx.lower_case_kind(ck)?);
                }
                let body = lower_stmts_inner(ctx, body)?;
                arms.push(crate::stmt::MirCaseArm { patterns, body });
            }
            let else_body = match else_ {
                Some(stmts) => Some(lower_stmts_inner(ctx, stmts)?),
                None => None,
            };

            Ok(Some(MirStmt::Case {
                selector,
                arms,
                else_body,
            }))
        }

        StmtKind::For {
            control_variable,
            start,
            end,
            step,
            body,
        } => {
            let control_place = ctx.lower_variable_access(*control_variable)?;
            let control_ident = match &control_place {
                crate::expr::MirPlace::Local(ident) => *ident,
                _ => {
                    return Err(LowerTypeError::UnsupportedType(
                        "FOR control variable must be a simple local".to_string(),
                    ));
                }
            };

            // Determine the control variable type
            let control_type = control_variable.infer(ctx.db);
            let control_elem = match control_type.normalize(ctx.db) {
                hir::hir_ty::ty::Type::Elementary(spec) => elementary_spec_to_mir(spec)?,
                _ => {
                    return Err(LowerTypeError::UnsupportedType(
                        "FOR control variable must be elementary".to_string(),
                    ));
                }
            };

            let start_mir = ctx.lower_expr(*start)?;
            let end_mir = ctx.lower_expr(*end)?;
            let step_mir = match step {
                Some(s) => ctx.lower_expr(*s)?,
                None => {
                    // Default step is 1
                    if control_elem.is_64bit() {
                        crate::expr::MirExpr::Constant(MirConstant::I64(1))
                    } else {
                        crate::expr::MirExpr::Constant(MirConstant::I32(1))
                    }
                }
            };

            let body = lower_stmts_inner(ctx, body)?;

            Ok(Some(MirStmt::For {
                control_var: control_ident,
                control_type: control_elem,
                start: start_mir,
                end: end_mir,
                step: step_mir,
                body,
            }))
        }

        StmtKind::While { condition, body } => {
            let cond = ctx.lower_expr(*condition)?;
            let body = lower_stmts_inner(ctx, body)?;
            Ok(Some(MirStmt::While {
                condition: cond,
                body,
            }))
        }

        StmtKind::Repeat { condition, body } => {
            let cond = ctx.lower_expr(*condition)?;
            let body = lower_stmts_inner(ctx, body)?;
            Ok(Some(MirStmt::Repeat {
                condition: cond,
                body,
            }))
        }

        StmtKind::Exit => Ok(Some(MirStmt::Exit)),

        StmtKind::Continue => Ok(Some(MirStmt::Continue)),

        StmtKind::ExternPragma(_) => Ok(None), // No MIR equivalent
        StmtKind::WasmPragma(_) => Ok(None),   // Handled at function level, not statement level

        StmtKind::AssignmentAttempt { .. } => Err(LowerTypeError::UnsupportedType(
            "AssignmentAttempt not yet supported".to_string(),
        )),

        StmtKind::EmptyPathExpression(_) => Ok(None),
    }
}

/// Helper: lower a slice of statements using an existing context.
fn lower_stmts_inner<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmts: &[Stmt<'db>],
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut result = Vec::new();
    for stmt in stmts {
        if let Some(mir_stmt) = lower_stmt(ctx, *stmt)? {
            result.push(mir_stmt);
        }
    }
    Ok(result)
}
