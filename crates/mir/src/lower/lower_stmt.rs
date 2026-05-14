use db::WorkspaceDataBase;
use hir::hir_def::expressions::statement::{Stmt, StmtKind};
use hir::hir_ty::infer::Infer;

use crate::{
    expr::MirConstant,
    lower::lower_expr::ExprLowerCtx,
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir},
    stmt::MirStmt,
};

/// Resolve a HIR type for cast checks: `normalize`, plus the extra hop
/// from a bare `Type::Function`/`Type::MethodDecl` (a function name used
/// as its own return slot) to its return type.
fn resolve_for_cast<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: hir::hir_ty::ty::Type<'db>,
) -> hir::hir_ty::ty::Type<'db> {
    let ty = ty.normalize(db);
    match ty {
        hir::hir_ty::ty::Type::Function(_) | hir::hir_ty::ty::Type::MethodDecl(_) => {
            ty.with_return_type(db).map_or(ty, |rt| rt.normalize(db))
        }
        _ => ty,
    }
}

/// Check if we need an implicit cast between two HIR types.
fn needs_cast<'db>(
    db: &'db dyn WorkspaceDataBase,
    from: hir::hir_ty::ty::Type<'db>,
    to: hir::hir_ty::ty::Type<'db>,
) -> bool {
    let from = resolve_for_cast(db, from);
    let to = resolve_for_cast(db, to);
    match (&from, &to) {
        (hir::hir_ty::ty::Type::Elementary(f), hir::hir_ty::ty::Type::Elementary(t)) => f != t,
        _ => false,
    }
}

type FbSubsMap = rustc_hash::FxHashMap<
    hir::hir_def::interned::identifier::Ident,
    rustc_hash::FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        hir::hir_def::expressions::spec::ElementarySpec,
    >,
>;

/// Lower a slice of HIR statements to MIR statements.
pub fn lower_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    lower_stmts_with_ctx(db, stmts, None, None, string_pool)
}

/// Lower with FB substitutions (for functions that use generic FBs).
pub fn lower_stmts_with_fb_subs<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    fb_subs: &FbSubsMap,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    lower_stmts_with_ctx(db, stmts, None, Some(fb_subs), string_pool)
}

/// Lower with both legacy `fb_subs` and a per-function var-name → mangled
/// FB name map. The latter is consulted when constructing FbCall body
/// names so generic FB instantiations route to their per-T `__body__`.
pub fn lower_stmts_with_fb_subs_and_mangling<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    fb_subs: &FbSubsMap,
    local_fb_mangling: &rustc_hash::FxHashMap<
        hir::hir_def::interned::identifier::Ident,
        hir::hir_def::interned::identifier::Ident,
    >,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut ctx = super::lower_expr::ExprLowerCtx::new(db, string_pool);
    if !fb_subs.is_empty() {
        ctx.fb_subs = Some(std::rc::Rc::new(fb_subs.clone()));
    }
    if !local_fb_mangling.is_empty() {
        ctx.local_fb_mangling = Some(std::rc::Rc::new(local_fb_mangling.clone()));
    }
    let mut result = Vec::new();
    for stmt in stmts {
        if let Some(mir_stmt) = lower_stmt(&ctx, *stmt)? {
            result.push(mir_stmt);
        }
    }
    Ok(result)
}

/// Lower a slice of HIR statements with an optional ANY type override for monomorphization.
pub fn lower_stmts_with_ctx<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    any_override: Option<hir::hir_def::expressions::spec::ElementarySpec>,
    fb_subs: Option<&FbSubsMap>,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut ctx = match any_override {
        Some(concrete) => ExprLowerCtx::with_any_override(db, concrete, string_pool),
        None => ExprLowerCtx::new(db, string_pool),
    };
    if let Some(subs) = fb_subs {
        ctx.fb_subs = Some(std::rc::Rc::new(subs.clone()));
    }
    let mut result = Vec::new();
    for stmt in stmts {
        if let Some(mir_stmt) = lower_stmt(&ctx, *stmt)? {
            result.push(mir_stmt);
        }
    }
    Ok(result)
}

/// Lower a slice of HIR statements in a FB body context where variables are struct fields.
pub fn lower_stmts_fb_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    this_struct: crate::types::MirStructType,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
    fb_subs: Option<&FbSubsMap>,
    any_override: Option<hir::hir_def::expressions::spec::ElementarySpec>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut ctx = ExprLowerCtx::with_this_struct(db, this_struct, string_pool);
    ctx.any_override = any_override;
    if let Some(subs) = fb_subs {
        ctx.fb_subs = Some(std::rc::Rc::new(subs.clone()));
    }
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
            // Get target variable type for implicit cast insertion. Resolve
            // through `Type::Function` / `Type::MethodDecl` so assigning to
            // the function-name return slot uses the declared return type.
            let var_type = resolve_for_cast(ctx.db, var.infer(ctx.db));
            let target_type = resolve_for_cast(ctx.db, target.infer(ctx.db));
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
            // Check if this is a FB invocation (callee is a variable of FB type)
            let path = func_call.path(ctx.db);
            let callee_type = path.infer(ctx.db).normalize(ctx.db);

            let fb = match callee_type {
                hir::hir_ty::ty::Type::FunctionBlock(fb)
                | hir::hir_ty::ty::Type::CallableType(
                    hir::hir_ty::ty::CallableType::FunctionBlock(fb),
                ) => Some(fb),
                _ => None,
            };
            if let Some(fb) = fb {
                ctx.lower_fb_invocation(*func_call, fb)
            } else {
                let call_expr = ctx.lower_func_call(*func_call, None)?;
                match call_expr {
                    crate::expr::MirExpr::Call(call) => Ok(Some(MirStmt::Call(call))),
                    _ => Ok(None),
                }
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

        StmtKind::Raise { message } => {
            let mir_msg = ctx.lower_expr(*message)?;
            Ok(Some(MirStmt::Raise { message: mir_msg }))
        }

        StmtKind::ExternPragma(_) => Ok(None), // No MIR equivalent
        StmtKind::WasmPragma(_) => Ok(None),   // Handled at function level, not statement level
        StmtKind::PreprocessIf { .. } => Ok(None), // Resolved at monomorphization, not lowered as a statement

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
