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

/// The context a POU's statements lower in: the instance a METHOD runs on,
/// an interface specialization's bindings, an arity specialization's pack.
/// A local's own initializer lowers in it too, since it is written in the
/// same declarations as the body.
#[allow(clippy::too_many_arguments)]
pub fn body_ctx<'db>(
    db: &'db dyn WorkspaceDataBase,
    this_struct: Option<crate::types::MirStructType>,
    // The POU the body belongs to: the inheritor for a copied inherited
    // method.
    this_pou: Option<hir::hir_def::pous::pou::Pou<'db>>,
    string_pool: std::rc::Rc<std::cell::RefCell<super::lower_expr::StringPool>>,
    iface_subs: Option<&super::mono_iface::IfaceSubs<'db>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    variadic_expansion: Option<std::rc::Rc<super::lower_expr::VariadicExpansion>>,
) -> ExprLowerCtx<'db> {
    let mut ctx = match this_struct {
        Some(this_struct) => ExprLowerCtx::with_this_struct(db, this_struct, string_pool),
        None => ExprLowerCtx::new(db, string_pool),
    };
    ctx.this_pou = this_pou;
    ctx.variadic_expansion = variadic_expansion;
    if let Some(subs) = iface_subs
        && !subs.is_empty()
    {
        ctx.iface_subs = Some(std::rc::Rc::new(subs.clone()));
    }
    if !iface_call_rewrites.is_empty() {
        ctx.iface_call_rewrites = Some(std::rc::Rc::new(iface_call_rewrites.clone()));
    }
    ctx
}

/// Lower a body in the context [`body_ctx`] built.
pub fn lower_body<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmts: &[Stmt<'db>],
) -> Result<Vec<MirStmt>, LowerTypeError> {
    lower_stmts_inner(ctx, stmts)
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
    iface_subs: Option<&super::mono_iface::IfaceSubs<'db>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<(Vec<MirStmt>, super::lower_expr::CallScratch), LowerTypeError> {
    // A specialized METHOD body carries its interface-param bindings, like
    // a specialized function body.
    let ctx = body_ctx(
        db,
        Some(this_struct),
        this_pou,
        string_pool,
        iface_subs,
        iface_call_rewrites,
        None,
    );
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

/// The MIR a statement lowers to: none for one that has no MIR equivalent
/// (e.g., ExternPragma), and a scratch's store before one that must evaluate
/// something once (a CASE selector, a place both read and stored).
fn lower_stmt<'db>(
    ctx: &ExprLowerCtx<'db>,
    stmt: Stmt<'db>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
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
                None => var.infer(ctx.db).normalize(ctx.db),
            };
            // ADJUSTED: `a[i]` infers as the array, but the value read is the
            // element.
            let target_type = target.infer_adjusted(ctx.db).normalize(ctx.db);
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
            // whole of `b` with only those bits replaced. A view is the same
            // store into its owner: `%QX0.3 := x` beside a `%QW0` rewrites the
            // word with bit 3 replaced.
            let mut pin = None;
            let (place, value) = if let Some(view) = ctx.view(*var, var.infer(ctx.db))? {
                view.write(value)
            } else if var.multibits(ctx.db).is_some() {
                // The store reads the cell it rewrites: a call in its subscript
                // runs once, for both.
                let (store, place) = ctx.pin_place(place);
                pin = store;
                let value =
                    ctx.lower_multibit_write(place.clone(), *var, var.infer(ctx.db), value)?;
                (place, value)
            } else {
                (place, value)
            };
            let mut stmts: Vec<MirStmt> = pin.into_iter().collect();
            stmts.push(MirStmt::Assign {
                target: place,
                value,
            });
            Ok(stmts)
        }

        StmtKind::FuncCall(func_call) => {
            // Check if this is a FB invocation (callee is a variable of FB type).
            // A FUNCTION or METHOD normalizes to its result, which may be a
            // FUNCTION_BLOCK: the callee is still the FUNCTION or METHOD.
            let path = func_call.path(ctx.db);
            let callee = path.infer(ctx.db);
            let fb = match callee {
                hir::hir_ty::ty::Type::Function(_)
                | hir::hir_ty::ty::Type::MethodDecl(_)
                | hir::hir_ty::ty::Type::CallableType(
                    hir::hir_ty::ty::CallableType::Function(_)
                    | hir::hir_ty::ty::CallableType::MethodDecl(_),
                ) => None,
                _ => match callee.normalize(ctx.db) {
                    hir::hir_ty::ty::Type::FunctionBlock(fb)
                    | hir::hir_ty::ty::Type::CallableType(
                        hir::hir_ty::ty::CallableType::FunctionBlock(fb),
                    ) => Some(fb),
                    _ => None,
                },
            };
            if let Some(fb) = fb {
                Ok(ctx
                    .lower_fb_invocation(*func_call, fb)?
                    .into_iter()
                    .collect())
            } else {
                let call_expr = ctx.lower_func_call(*func_call, None)?;
                match call_expr {
                    crate::expr::MirExpr::Call(call) => Ok(vec![MirStmt::Call(call)]),
                    // A call statement that lowered to anything else must not vanish.
                    other => Err(LowerTypeError::UnsupportedType(format!(
                        "a call statement lowered to a non-call expression: {other:?}"
                    ))),
                }
            }
        }

        StmtKind::Return => Ok(vec![MirStmt::Return]),

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

            Ok(vec![MirStmt::If {
                condition: cond,
                then_body,
                else_ifs,
                else_body,
            }])
        }

        StmtKind::Case {
            condition,
            cases,
            else_,
        } => {
            // The selector runs once, before any label is tested.
            let (store, selector) = ctx.case_selector(*condition)?;
            // Labels compare at the selector's width and signedness.
            let lane = ctx.expr_to_mir_elementary(*condition).ok();
            let mut arms = Vec::new();
            for (case_kinds, body) in cases {
                let mut patterns = Vec::new();
                for ck in case_kinds {
                    patterns.push(ctx.lower_case_kind(ck, &selector, lane)?);
                }
                let body = lower_stmts_inner(ctx, body)?;
                arms.push(crate::stmt::MirCaseArm { patterns, body });
            }
            let else_body = match else_ {
                Some(stmts) => Some(lower_stmts_inner(ctx, stmts)?),
                None => None,
            };

            let mut stmts: Vec<MirStmt> = store.into_iter().collect();
            stmts.push(MirStmt::Case {
                selector,
                arms,
                else_body,
            });
            Ok(stmts)
        }

        StmtKind::For {
            control_variable,
            start,
            end,
            step,
            body,
        } => {
            // Any place can be a counter: a FUNCTION local or a PROGRAM/FB member
            // (HIR rejected the shapes IEC forbids), or whole bytes of a wider
            // address's cell. A bit of one has no place to count in.
            let control_place = match ctx.view(*control_variable, control_variable.infer(ctx.db))? {
                Some(super::multibit::View::Bytes(place)) => place,
                Some(super::multibit::View::Slice { .. }) => {
                    return Err(LowerTypeError::UnsupportedType(
                            "a FOR counter cannot be a bit of a wider address; `rk check` refuses a BOOL counter (E1206)"
                                .to_string(),
                        ));
                }
                None => ctx.lower_variable_access(*control_variable)?,
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

            // A subrange counter is checked at the initial store and at the body's
            // top: the range may overshoot the subrange, the observed values may
            // not. `FOR i := 0 TO 12 BY 7` on a (0..10) counter is legal.
            let control_sub = control_type.as_subrange(ctx.db).and_then(|sr| {
                match hir::hir_ty::infer::const_eval::subrange_bounds(ctx.db, sr) {
                    (Some(lower), Some(upper)) => Some(crate::types::MirSubrangeType {
                        base: control_elem,
                        lower,
                        upper,
                    }),
                    _ => None,
                }
            });

            // The bounds convert to the counter's width, as an assignment's
            // value does.
            let start_mir = ctx.lower_expr_with_cast(*start, control_elem)?;
            let start_mir = match &control_sub {
                Some(sub) => ctx.checked_range_mir(start_mir, sub),
                None => start_mir,
            };
            let end_mir = ctx.lower_expr_with_cast(*end, control_elem)?;
            // The step is the folded value inference recorded (E1204), emitted as a
            // constant at the counter's width: its sign picks the exit comparison.
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

            Ok(vec![MirStmt::For {
                control: control_place,
                control_type: control_elem,
                start: start_mir,
                end: end_mir,
                step: Box::new(step_mir),
                body,
            }])
        }

        StmtKind::While { condition, body } => {
            let cond = ctx.lower_expr(*condition)?;
            let body = lower_stmts_inner(ctx, body)?;
            Ok(vec![MirStmt::While {
                condition: cond,
                body,
            }])
        }

        StmtKind::Repeat { condition, body } => {
            let cond = ctx.lower_expr(*condition)?;
            let body = lower_stmts_inner(ctx, body)?;
            Ok(vec![MirStmt::Repeat {
                condition: cond,
                body,
            }])
        }

        StmtKind::Exit => Ok(vec![MirStmt::Exit]),

        StmtKind::Continue => Ok(vec![MirStmt::Continue]),

        StmtKind::Raise { message } => {
            let mir_msg = ctx.lower_expr(*message)?;
            Ok(vec![MirStmt::Raise { message: mir_msg }])
        }

        StmtKind::WasmPragma(decl) => Ok(super::lower_wasm::lower_wasm_pragma(ctx, stmt, decl)?
            .into_iter()
            .collect()),

        // Linter-only marker: no code.
        StmtKind::AllowPragma(_) => Ok(Vec::new()),

        StmtKind::EmptyPathExpression(begin_path) => {
            // `SUPER()` parses as a bare begin-path statement, so it lands here; HIR
            // validated it (E1108/E1107).
            if begin_path.invocation(ctx.db).map(|i| i.kind(ctx.db))
                == Some(hir::hir_def::expressions::invocation::InvocationKind::SuperBody)
            {
                return Ok(ctx
                    .lower_super_body_call(*begin_path)?
                    .into_iter()
                    .collect());
            }
            Ok(Vec::new())
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
        let mut lowered = lower_stmt(ctx, *stmt)?;
        if !lowered.is_empty() {
            result.push(MirStmt::DebugTrap {
                location: stmt_location(ctx.db, *stmt),
            });
            result.append(&mut lowered);
        }
        result.append(&mut ctx.after_stmt.borrow_mut());
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
