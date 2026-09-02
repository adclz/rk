use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_def::expressions::statement::{Stmt, StmtKind};
use hir::hir_ty::infer::Infer;

use crate::{
    expr::MirConstant,
    lower::lower_expr::ExprLowerCtx,
    lower::lower_type::{LowerTypeError, elementary_spec_to_mir},
    stmt::{MirSourceLocation, MirStmt},
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

/// Lower a slice of HIR statements to MIR statements.
pub fn lower_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
) -> Result<(Vec<MirStmt>, super::lower_expr::CallScratch), LowerTypeError> {
    lower_stmts_with_ctx(
        db,
        stmts,
        None,
        &super::mono_iface::IfaceCallRewrites::default(),
        string_pool,
        None,
    )
}

/// Lower a free-function body carrying interface-specialization context
/// (empty for ordinary functions).
pub fn lower_stmts_with_ctx<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    iface_subs: Option<
        &rustc_hash::FxHashMap<
            hir::hir_def::interned::identifier::Ident,
            hir::hir_def::pous::pou::Pou<'db>,
        >,
    >,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
    // Phase C: inside an arity specialization, how this body's pack expanded.
    variadic_expansion: Option<std::rc::Rc<super::lower_expr::VariadicExpansion>>,
) -> Result<(Vec<MirStmt>, super::lower_expr::CallScratch), LowerTypeError> {
    let mut ctx = ExprLowerCtx::new(db, string_pool);
    ctx.variadic_expansion = variadic_expansion;
    if let Some(is) = iface_subs
        && !is.is_empty()
    {
        ctx.iface_subs = Some(std::rc::Rc::new(is.clone()));
    }
    if !iface_call_rewrites.is_empty() {
        ctx.iface_call_rewrites = Some(std::rc::Rc::new(iface_call_rewrites.clone()));
    }
    let stmts = lower_stmts_inner(&ctx, stmts)?;
    Ok((stmts, ctx.call_scratch.take()))
}

/// Lower a slice of HIR statements in a FB body context where variables are struct fields.
pub fn lower_stmts_fb_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    this_struct: crate::types::MirStructType,
    // The POU this body belongs to: the inheritor for a copied inherited
    // method.
    this_pou: Option<hir::hir_def::pous::pou::Pou<'db>>,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
    iface_subs: Option<
        &rustc_hash::FxHashMap<
            hir::hir_def::interned::identifier::Ident,
            hir::hir_def::pous::pou::Pou<'db>,
        >,
    >,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<(Vec<MirStmt>, super::lower_expr::CallScratch), LowerTypeError> {
    let mut ctx = ExprLowerCtx::with_this_struct(db, this_struct, string_pool);
    ctx.this_pou = this_pou;
    // A specialized METHOD body carries its interface-param bindings, like
    // a specialized function body.
    if let Some(subs) = iface_subs
        && !subs.is_empty()
    {
        ctx.iface_subs = Some(std::rc::Rc::new(subs.clone()));
    }
    if !iface_call_rewrites.is_empty() {
        ctx.iface_call_rewrites = Some(std::rc::Rc::new(iface_call_rewrites.clone()));
    }
    let stmts = lower_stmts_inner(&ctx, stmts)?;
    Ok((stmts, ctx.call_scratch.take()))
}

/// The subrange a store through `place` must respect, when the place
/// carries the slot's type (an element, a field, a VAR_IN_OUT write); a
/// plain local's declared type answers instead.
fn place_subrange(place: &crate::expr::MirPlace) -> Option<crate::types::MirSubrangeType> {
    use crate::expr::MirPlace;
    let ty = match place {
        MirPlace::Index { element_type, .. } => element_type,
        MirPlace::Field { field_type, .. } => field_type,
        MirPlace::Deref { pointee_type, .. } => pointee_type,
        _ => return None,
    };
    match ty {
        crate::types::MirType::Subrange(sub) => Some(sub.clone()),
        _ => None,
    }
}

/// Returns None for statements that have no MIR equivalent (e.g., ExternPragma).
fn lower_stmt<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmt: Stmt<'db>,
) -> Result<Option<MirStmt>, LowerTypeError> {
    match stmt.stmt(ctx.db) {
        StmtKind::Assignment { var, target } => {
            let place = ctx.lower_variable_access(*var)?;
            // The conversion lane is inference's decision (`coercion_target`); the
            // fallback covers what HIR does not record.
            let var_type = match hir::hir_ty::body::infer_body(ctx.db, target.scope_id(ctx.db))
                .coercion_target
                .get(target)
            {
                Some(ty) => *ty,
                None => resolve_for_cast(ctx.db, var.infer(ctx.db)),
            };
            // ADJUSTED: `a[i]` infers as the array, but the value read is the
            // element.
            let target_type = resolve_for_cast(ctx.db, target.infer_adjusted(ctx.db));
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
            // A subrange target checks the value at the door; the place knows the
            // slot's type where the access is an adjustment.
            let value = match place_subrange(&place) {
                Some(sub) => ctx.checked_range_mir(value, &sub),
                None => ctx.checked_range(value, var.infer(ctx.db)),
            };
            // `b.1 := x` names a slice of `b`; the store has to put back the
            // whole of `b` with only those bits replaced.
            let value = if var.multibits(ctx.db).is_some() {
                ctx.lower_multibit_write(place.clone(), *var, var.infer(ctx.db), value)?
            } else {
                value
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
                    // A call statement that lowered to anything else must not vanish.
                    other => Err(LowerTypeError::UnsupportedType(format!(
                        "a call statement lowered to a non-call expression: {other:?}"
                    ))),
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
                    patterns.push(ctx.lower_case_kind(ck, *condition)?);
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
            // Any PLACE can be a counter: a FUNCTION local, a PROGRAM/FB
            // member (other toolchains accepts both). HIR has already rejected the
            // shapes IEC's grammar forbids — `control_variable ::= identifier`
            // — so a Field/Index/Deref place cannot reach here from valid
            // source; the codegen addresses whatever arrives.
            let control_place = ctx.lower_variable_access(*control_variable)?;

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

            // A subrange counter is checked at the initial store and at the body's
            // top: the range may overshoot the subrange, the observed values may
            // not. `FOR i := 0 TO 12 BY 7` on a (0..10) counter is legal.
            let control_sub = control_type
                .as_subrange(ctx.db)
                .and_then(|sr| {
                    match hir::hir_ty::infer::const_eval::subrange_bounds(ctx.db, sr) {
                        (Some(lower), Some(upper)) => Some(crate::types::MirSubrangeType {
                            base: control_elem,
                            lower,
                            upper,
                        }),
                        _ => None,
                    }
                });

            let start_mir = ctx.lower_expr(*start)?;
            let start_mir = match &control_sub {
                Some(sub) => ctx.checked_range_mir(start_mir, sub),
                None => start_mir,
            };
            let end_mir = ctx.lower_expr(*end)?;
            // The step is the VALUE inference folded and recorded (E1007
            // refused anything that does not fold), emitted as a constant at
            // the counter's width — its sign is what picks the exit
            // comparison. Lowering the expression instead left a CONSTANT
            // variable as a Load, whose sign codegen could not see.
            let step_mir = match step {
                Some(s) => {
                    let v = hir::hir_ty::body::infer_body(ctx.db, s.scope_id(ctx.db))
                        .for_step_value
                        .get(s)
                        .copied()
                        .ok_or_else(|| {
                            LowerTypeError::UnsupportedType(
                                "FOR step was not folded to a constant".to_string(),
                            )
                        })?;
                    if control_elem.is_64bit() {
                        crate::expr::MirExpr::Constant(MirConstant::I64(v))
                    } else {
                        crate::expr::MirExpr::Constant(MirConstant::I32(v as i32))
                    }
                }
                None => {
                    // Default step is 1
                    if control_elem.is_64bit() {
                        crate::expr::MirExpr::Constant(MirConstant::I64(1))
                    } else {
                        crate::expr::MirExpr::Constant(MirConstant::I32(1))
                    }
                }
            };

            let mut body = lower_stmts_inner(ctx, body)?;
            if let Some(sub) = &control_sub {
                body.insert(
                    0,
                    MirStmt::Assign {
                        target: control_place.clone(),
                        value: ctx.checked_range_mir(
                            crate::expr::MirExpr::Load(
                                control_place.clone(),
                                crate::types::MirType::Elementary(control_elem),
                            ),
                            sub,
                        ),
                    },
                );
            }

            Ok(Some(MirStmt::For {
                control: control_place,
                control_type: control_elem,
                start: start_mir,
                end: end_mir,
                step: Box::new(step_mir),
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

        StmtKind::WasmPragma(_) => Ok(None),   // Handled at function level, not statement level

        // Linter-only marker: no code.
        StmtKind::AllowPragma(_) => Ok(None),

        StmtKind::EmptyPathExpression(begin_path) => {
            // `SUPER()` — call the immediate base FB's cyclic body on the current
            // `this`. It parses as a bare begin-path statement (the `()` belongs to
            // the `SuperBody` invocation, not a param list), so it lands here, not
            // in `FuncCall`. HIR (`resolve_invocation`) validated it (E0501/E0513).
            if begin_path.invocation(ctx.db).map(|i| i.kind(ctx.db))
                == Some(hir::hir_def::expressions::invocation::InvocationKind::SuperBody)
            {
                return ctx.lower_super_body_call(*begin_path);
            }
            Ok(None)
        }
    }
}

/// The chokepoint every statement sequence funnels through; it inserts a
/// [`MirStmt::DebugTrap`] marker before each statement, which emits no
/// wasm and records the code offset for the `debug-lines` table.
fn lower_stmts_inner<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmts: &[Stmt<'db>],
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut result = Vec::new();
    for stmt in stmts {
        if let Some(mir_stmt) = lower_stmt(ctx, *stmt)? {
            result.push(MirStmt::DebugTrap {
                location: stmt_location(ctx.db, *stmt),
            });
            result.push(mir_stmt);
        }
    }
    Ok(result)
}

/// The source position (file/line/column) of a HIR statement, for the line
/// table. Lines and columns are 0-based (tree-sitter rows/columns).
fn stmt_location<'db>(db: &'db dyn WorkspaceDataBase, stmt: Stmt<'db>) -> MirSourceLocation {
    let range = stmt.get_span(db);
    // Only the file is recorded; its index into the module's file table is
    // resolved at codegen, once every module is lowered.
    let file_url = stmt.get_scope_id(db).file(db).url(db).as_str().into();
    MirSourceLocation {
        file_url,
        line: range.start_point.row as u32,
        column: range.start_point.column as u32,
    }
}
