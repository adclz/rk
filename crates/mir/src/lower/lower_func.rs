use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::{
            expression::InitExprKind,
            statement::StmtKind,
        },
        interned::identifier::Ident,
        pous::{
            class::Class,
            function::Function,
            function_block::FunctionBlock,
            variable::{VariableDecl, VariableKind},
        },
        program::ProgramDecl,
    },
    hir_ty::{infer::Infer, ty::Type},
};
use rustc_hash::FxHashSet;

use crate::{
    expr::{MirExpr, MirPlace},
    function::{
        MirFunction, MirLinkage, MirLocal, MirLocalKind, MirParam,
        MirParamKind, MirStorage,
    },
    lower::{
        lower_expr::ExprLowerCtx,
        lower_stmt::lower_stmts,
        lower_type::{LowerTypeError, lower_type},
    },
    memory::{MirAllocKind, MirMemoryLayout},
    stmt::MirStmt,
    types::MirType,
};

/// Lower a FUNCTION to a MirFunction.
pub fn lower_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
) -> Result<MirFunction, LowerTypeError> {
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;

    // Collect address-taken variables for storage decisions
    let address_taken = collect_address_taken_vars(db, func.statements(db));

    // 1. Build parameters (Input, InOut)
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input => {
                let ty = lower_var_type(db, *var)?;
                params.push(MirParam {
                    name: var.name(db),
                    ty: ty.clone(),
                    kind: MirParamKind::Input,
                });
                next_local_idx += 1;
            }
            VariableKind::InOut => {
                let ty = lower_var_type(db, *var)?;
                params.push(MirParam {
                    name: var.name(db),
                    ty: MirType::Pointer(Box::new(ty)),
                    kind: MirParamKind::InOut,
                });
                next_local_idx += 1;
            }
            _ => {}
        }
    }

    // 2. Return type
    let return_type = func
        .return_type(db)
        .map(|spec| {
            let ty = spec.infer(db);
            lower_type(db, ty)
        })
        .transpose()?;

    // If there's a return type, allocate a local for it (named after the function)
    if let Some(ref ret_ty) = return_type {
        locals.push(MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            kind: MirLocalKind::Var,
            storage: MirStorage::Scalar { local_index: next_local_idx },
        });
        next_local_idx += 1;
    }

    // 3. Local variables (Var, Temp, Output)
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input | VariableKind::InOut => continue,
            _ => {}
        }

        let ty = lower_var_type(db, *var)?;
        let storage = compute_storage(
            var.name(db),
            &ty,
            address_taken.contains(&var.name(db)),
            &mut next_local_idx,
            memory_layout,
        );

        let kind = match var.kind(db) {
            VariableKind::Output => MirLocalKind::Output,
            VariableKind::Temp => MirLocalKind::Temp,
            _ => MirLocalKind::Var,
        };

        locals.push(MirLocal {
            name: var.name(db),
            ty,
            init: None, // TODO: lower initializers
            kind,
            storage,
        });
    }

    // 4. Generate initializer statements for variables with init expressions
    let mut init_stmts = Vec::new();
    for var in func.variables(db) {
        match var.kind(db) {
            VariableKind::Input | VariableKind::InOut => continue,
            _ => {}
        }
        if let Some(init_expr) = var.init(db) {
            if let Some(stmt) = lower_var_init(db, var.name(db), init_expr)? {
                init_stmts.push(stmt);
            }
        }
    }

    // 5. Lower body statements
    let mut body = lower_stmts(db, func.statements(db))?;
    // Prepend initializers
    if !init_stmts.is_empty() {
        init_stmts.append(&mut body);
        body = init_stmts;
    }
    let body = body;

    // Determine linkage — check if there's an extern pragma
    let is_extern = func.statements(db).iter().any(|s| {
        matches!(s.stmt(db), StmtKind::ExternPragma(_))
    });

    let linkage = if is_extern {
        MirLinkage::Internal // extern functions are imports, handled separately
    } else {
        MirLinkage::Export
    };

    Ok(MirFunction {
        name: func.name(db),
        origin_name: func.name(db),
        index,
        params,
        return_type,
        locals,
        body,
        linkage,
    })
}

/// Lower a FUNCTION_BLOCK to MirFunctions (one per method + instance type).
pub fn lower_function_block<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    // Lower each method as a separate function with 'this' parameter
    for method in fb.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter
        let fb_type = lower_type(db, Type::FunctionBlock(fb))?;
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = lower_var_type(db, *var)?;
                    params.push(MirParam {
                        name: var.name(db),
                        ty,
                        kind: MirParamKind::Input,
                    });
                    next_local_idx += 1;
                }
                VariableKind::InOut => {
                    let ty = lower_var_type(db, *var)?;
                    params.push(MirParam {
                        name: var.name(db),
                        ty: MirType::Pointer(Box::new(ty)),
                        kind: MirParamKind::InOut,
                    });
                    next_local_idx += 1;
                }
                _ => {
                    let ty = lower_var_type(db, *var)?;
                    let storage = compute_storage(
                        var.name(db),
                        &ty,
                        false,
                        &mut next_local_idx,
                        memory_layout,
                    );
                    locals.push(MirLocal {
                        name: var.name(db),
                        ty,
                        init: None,
                        kind: MirLocalKind::Var,
                        storage,
                    });
                }
            }
        }

        let return_type = method
            .return_type(db)
            .map(|spec| {
                let ty = spec.infer(db);
                lower_type(db, ty)
            })
            .transpose()?;

        // If there's a return type, allocate a local for it (named after the method)
        if let Some(ref ret_ty) = return_type {
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                kind: MirLocalKind::Var,
                storage: MirStorage::Scalar { local_index: next_local_idx },
            });
            next_local_idx += 1;
        }

        let body = lower_stmts(db, method.stmts(db))?;

        // Qualified name: "FBName$MethodName"
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}${}",
                fb.name(db).text(db),
                method.name(db).text(db)
            )),
        );

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            linkage: MirLinkage::Export,
        });
        idx += 1;
    }

    // Lower FB body as __body__ function
    // All variables (input, output, var) are accessed through the 'this' pointer.
    if !fb.statements(db).is_empty() {
        let fb_type = lower_type(db, Type::FunctionBlock(fb))?;
        let body_params = vec![MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type.clone())),
            kind: MirParamKind::This,
        }];

        let mut body_locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        let address_taken = collect_address_taken_vars(db, fb.statements(db));

        for var in fb.variables(db) {
            if var.kind(db) == VariableKind::Temp {
                let ty = lower_var_type(db, *var)?;
                let storage = compute_storage(
                    var.name(db),
                    &ty,
                    address_taken.contains(&var.name(db)),
                    &mut next_local_idx,
                    memory_layout,
                );
                body_locals.push(MirLocal {
                    name: var.name(db),
                    ty,
                    init: None,
                    kind: MirLocalKind::Var,
                    storage,
                });
            }
        }

        // Body lowering with the `this` struct context.
        let this_struct = match &fb_type {
            MirType::Struct(s) => s.clone(),
            _ => return Err(LowerTypeError::UnsupportedType("FB type is not a struct".into())),
        };
        let body_stmts = crate::lower::lower_stmt::lower_stmts_fb_body(db, fb.statements(db), this_struct)?;

        let body_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}$__body__",
                fb.name(db).text(db)
            )),
        );

        functions.push(MirFunction {
            name: body_name,
            origin_name: fb.name(db),
            index: idx,
            params: body_params,
            return_type: None,
            locals: body_locals,
            body: body_stmts,
            linkage: MirLinkage::Export,
        });
    }

    Ok(functions)
}

/// Lower a CLASS to MirFunctions (one per method + instance type).
pub fn lower_class<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    for method in class.methods(db) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'

        // 'this' pointer parameter
        let class_type = lower_type(db, Type::Class(class))?;
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(class_type)),
            kind: MirParamKind::This,
        });

        // Method parameters
        for var in method.variables(db) {
            match var.kind(db) {
                VariableKind::Input => {
                    let ty = lower_var_type(db, *var)?;
                    params.push(MirParam {
                        name: var.name(db),
                        ty,
                        kind: MirParamKind::Input,
                    });
                    next_local_idx += 1;
                }
                VariableKind::InOut => {
                    let ty = lower_var_type(db, *var)?;
                    params.push(MirParam {
                        name: var.name(db),
                        ty: MirType::Pointer(Box::new(ty)),
                        kind: MirParamKind::InOut,
                    });
                    next_local_idx += 1;
                }
                _ => {
                    let ty = lower_var_type(db, *var)?;
                    let storage = compute_storage(
                        var.name(db),
                        &ty,
                        false,
                        &mut next_local_idx,
                        memory_layout,
                    );
                    locals.push(MirLocal {
                        name: var.name(db),
                        ty,
                        init: None,
                        kind: MirLocalKind::Var,
                        storage,
                    });
                }
            }
        }

        let return_type = method
            .return_type(db)
            .map(|spec| {
                let ty = spec.infer(db);
                lower_type(db, ty)
            })
            .transpose()?;

        // Return local
        if let Some(ref ret_ty) = return_type {
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                kind: MirLocalKind::Var,
                storage: MirStorage::Scalar { local_index: next_local_idx },
            });
            next_local_idx += 1;
        }

        let body = lower_stmts(db, method.stmts(db))?;

        // Qualified name: "ClassName$MethodName"
        let qualified_name = Ident::new(
            db,
            compact_str::CompactString::from(format!(
                "{}${}",
                class.name(db).text(db),
                method.name(db).text(db)
            )),
        );

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            linkage: MirLinkage::Export,
        });
        idx += 1;
    }

    Ok(functions)
}

/// Lower a PROGRAM to a MirFunction (zero-param exported function).
pub fn lower_program<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
) -> Result<MirFunction, LowerTypeError> {
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;

    for var in program.variables(db) {
        let ty = lower_var_type(db, *var)?;
        let storage = compute_storage(
            var.name(db),
            &ty,
            false,
            &mut next_local_idx,
            memory_layout,
        );
        locals.push(MirLocal {
            name: var.name(db),
            ty,
            init: None,
            kind: MirLocalKind::Var,
            storage,
        });
    }

    let body = lower_stmts(db, program.statements(db))?;

    Ok(MirFunction {
        name: program.name(db),
        origin_name: program.name(db),
        index,
        params: Vec::new(),
        return_type: None,
        locals,
        body,
        linkage: MirLinkage::Export,
    })
}

/// Determine storage for a variable (WASM local vs linear memory).
fn compute_storage(
    name: Ident,
    ty: &MirType,
    is_address_taken: bool,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) -> MirStorage {
    if ty.is_scalar() && !is_address_taken {
        let idx = *next_local_idx;
        *next_local_idx += 1;
        MirStorage::Scalar { local_index: idx }
    } else {
        let size = ty.size_bytes();
        let align = ty.alignment();
        let address = memory_layout.allocate(name, size, align, MirAllocKind::Variable);
        MirStorage::Memory {
            address,
            size,
            align,
        }
    }
}

/// Lower a variable's type spec via HIR inference.
fn lower_var_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    let ty = var.spec(db).infer(db);
    lower_type(db, ty)
}

/// Collect identifiers of variables whose address is taken (via REF()).
/// These must be allocated in linear memory even if they're scalars.
fn collect_address_taken_vars<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
) -> FxHashSet<Ident> {
    use hir::hir_def::expressions::expression::{
        ExprKind, PrimaryExpr, RefValue, PathExprKind, VarAccess,
    };

    let mut result = FxHashSet::default();

    fn walk_expr<'db>(
        db: &'db dyn WorkspaceDataBase,
        expr: hir::hir_def::expressions::expression::Expr<'db>,
        result: &mut FxHashSet<Ident>,
    ) {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(begin_path),
            }) => {
                // REF(var) — extract the variable name
                if let Some(path_expr) = begin_path.expr(db) {
                    let ident = path_expr.ident(db).ident;
                    result.insert(ident);
                }
            }
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_)) => {}
            ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(fc)) => {
                for param in fc.params(db) {
                    match param.kind(db) {
                        hir::hir_def::expressions::expression::ParamAssignKind::NonFormal { value }
                        | hir::hir_def::expressions::expression::ParamAssignKind::FormalInput { value, .. } => {
                            walk_expr(db, value, result);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => {
                walk_expr(db, *inner, result);
            }
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right } => {
                walk_expr(db, *left, result);
                walk_expr(db, *right, result);
            }
            ExprKind::UnaryOperator { expr: inner, .. } => {
                walk_expr(db, *inner, result);
            }
            _ => {}
        }
    }

    fn walk_stmts<'db>(
        db: &'db dyn WorkspaceDataBase,
        stmts: &[hir::hir_def::expressions::statement::Stmt<'db>],
        result: &mut FxHashSet<Ident>,
    ) {
        use hir::hir_def::expressions::statement::StmtKind;
        for stmt in stmts {
            match stmt.stmt(db) {
                StmtKind::Assignment { target: _, var: _ } => {
                    // `var` is the target and `target` the value expression, as HIR
                    // names them.
                }
                _ => {}
            }
            // Walk all expressions in the statement
            walk_stmt_exprs(db, *stmt, result);
        }
    }

    fn walk_stmt_exprs<'db>(
        db: &'db dyn WorkspaceDataBase,
        stmt: hir::hir_def::expressions::statement::Stmt<'db>,
        result: &mut FxHashSet<Ident>,
    ) {
        use hir::hir_def::expressions::statement::StmtKind;
        match stmt.stmt(db) {
            StmtKind::Assignment { var: _, target } => {
                walk_expr(db, *target, result);
            }
            StmtKind::If { condition, then, else_if, else_ } => {
                walk_expr(db, *condition, result);
                if let Some(stmts) = then {
                    walk_stmts(db, stmts, result);
                }
                for (cond, body) in else_if {
                    walk_expr(db, *cond, result);
                    walk_stmts(db, body, result);
                }
                if let Some(stmts) = else_ {
                    walk_stmts(db, stmts, result);
                }
            }
            StmtKind::For { start, end, step, body, .. } => {
                walk_expr(db, *start, result);
                walk_expr(db, *end, result);
                if let Some(s) = step {
                    walk_expr(db, *s, result);
                }
                walk_stmts(db, body, result);
            }
            StmtKind::While { condition, body } => {
                walk_expr(db, *condition, result);
                walk_stmts(db, body, result);
            }
            StmtKind::Repeat { condition, body } => {
                walk_expr(db, *condition, result);
                walk_stmts(db, body, result);
            }
            StmtKind::Case { condition, cases, else_ } => {
                walk_expr(db, *condition, result);
                for (_, body) in cases {
                    walk_stmts(db, body, result);
                }
                if let Some(stmts) = else_ {
                    walk_stmts(db, stmts, result);
                }
            }
            StmtKind::FuncCall(fc) => {
                for param in fc.params(db) {
                    match param.kind(db) {
                        hir::hir_def::expressions::expression::ParamAssignKind::NonFormal { value }
                        | hir::hir_def::expressions::expression::ParamAssignKind::FormalInput { value, .. } => {
                            walk_expr(db, value, result);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    walk_stmts(db, stmts, &mut result);
    result
}

/// Lower a variable initializer to an assignment statement.
/// Returns None if the init expression can't be lowered (e.g., complex struct inits).
fn lower_var_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
    init_expr: hir::hir_def::expressions::expression::InitExpr<'db>,
) -> Result<Option<MirStmt>, LowerTypeError> {
    match init_expr.kind(db) {
        InitExprKind::ConstantExpr(expr) => {
            let ctx = ExprLowerCtx::new(db);
            let value = ctx.lower_expr(expr)?;
            Ok(Some(MirStmt::Assign {
                target: MirPlace::Local(var_name),
                value,
            }))
        }
        // TODO: ArrayInit, StructInit
        _ => Ok(None),
    }
}
