use db::WorkspaceDataBase;
use hir::{
    hir_def::{
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
use rustc_hash::{FxHashMap, FxHashSet};

use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    expr::{MirArgKind, MirConstant, MirExpr},
    function::{
        MirFunction, MirLinkage, MirLocal, MirParam, MirParamKind, MirStorage, MirVariableStorage,
    },
    lower::{
        lower_expr::ExprLowerCtx,
        lower_type::{LowerTypeError, lower_type},
    },
    memory::{MirAllocKind, MirMemoryLayout},
    stmt::MirStmt,
    types::MirType,
};

/// Pin a codegen failure to a POU declaration, the fallback for
/// declaration-level failures (an expression carries its own span).
fn at_node<'db, T>(
    db: &'db dyn WorkspaceDataBase,
    node: impl hir::HirNodeInfo<'db>,
    result: Result<T, LowerTypeError>,
) -> Result<T, LowerTypeError> {
    result.map_err(|e| e.with_location(node.get_scope_id(db).file(db), node.get_span(db)))
}

/// Every method an instance of `pou` responds to: its own, plus the
/// inherited ones it does not override. An inherited method is emitted as
/// a copy on each inheritor, so `THIS.m()` inside it resolves against the
/// inheritor. Which override wins is HIR's answer (`class_members`).
/// Sorted by name for a reproducible artifact.
fn emittable_methods<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: hir::hir_def::pous::pou::Pou<'db>,
    own: &[hir::hir_def::pous::class::MethodDecl<'db>],
) -> Vec<hir::hir_def::pous::class::MethodDecl<'db>> {
    use hir::hir_ty::oop::MethodRef;

    let mut inherited: Vec<_> = hir::hir_ty::oop::class_members(db, pou)
        .inherited(pou)
        .filter_map(|(_, member)| match member.method {
            MethodRef::Declared(d) => Some(d),
            MethodRef::Prototype(_) => None,
        })
        .collect();
    inherited.sort_by_key(|m| m.name(db).text(db).to_string());

    let mut out = own.to_vec();
    out.extend(inherited);
    out
}

/// What is emitted for the instance type `pou`: every method it answers to
/// ([`emittable_methods`]), then each method and FB body of a base its code
/// reaches through `SUPER.m()` and `SUPER()`, followed transitively. Each is
/// emitted on `pou`, so `THIS` inside it is `pou` and an override wins there,
/// as it does in an inherited method. Base-most bodies last.
pub(crate) struct InstanceCopies<'db> {
    pub methods: Vec<hir::hir_def::pous::class::MethodDecl<'db>>,
    pub bodies: Vec<FunctionBlock<'db>>,
}

pub(crate) fn instance_copies<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: hir::hir_def::pous::pou::Pou<'db>,
) -> InstanceCopies<'db> {
    use hir::hir_def::expressions::expression::PathExprKind;
    use hir::hir_def::expressions::invocation::InvocationKind;
    use hir::hir_def::pous::pou::Pou;
    use hir::hir_def::scope::ScopeKind;
    use hir::hir_ty::oop::MethodRef;
    use hir::hir_ty::ty::CallableType;

    let own: &[hir::hir_def::pous::class::MethodDecl<'db>] = match pou {
        Pou::FunctionBlock(fb) => fb.methods(db),
        Pou::Class(c) => c.methods(db),
        _ => &[],
    };
    let mut methods = emittable_methods(db, pou, own);
    let mut bodies: Vec<FunctionBlock<'db>> = Vec::new();

    let mut work: Vec<hir::hir_def::scope::ScopeId<'db>> =
        methods.iter().map(|m| m.scope_id(db)).collect();
    if let Pou::FunctionBlock(fb) = pou {
        work.push(fb.scope_id(db));
    }
    while let Some(scope) = work.pop() {
        let body = hir::hir_ty::body::infer_body(db, scope);
        for fc in &body.calls {
            let path = fc.path(db);
            if path.invocation(db).map(|i| i.kind(db)) != Some(InvocationKind::Super) {
                continue;
            }
            // `SUPER.inner.m()` is the member's method, not a base's.
            if matches!(
                path.expr(db).map(|pe| pe.expr(db)),
                Some(PathExprKind::Field(_))
            ) {
                continue;
            }
            let method = match path.infer(db) {
                Type::MethodDecl(MethodRef::Declared(m))
                | Type::CallableType(CallableType::MethodDecl(MethodRef::Declared(m))) => m,
                _ => continue,
            };
            if !methods.contains(&method) {
                methods.push(method);
                work.push(method.scope_id(db));
            }
        }
        // `SUPER()` runs the base of the block whose body holds it.
        if body.first_super_body.is_some()
            && let ScopeKind::Pou(holder) = hir::hir_def::semantic_index::get_scope(db, scope).kind
            && let Some(Pou::FunctionBlock(base)) =
                hir::hir_ty::oop::explicit_bases(db, holder).extends
            && !bodies.contains(&base)
        {
            bodies.push(base);
            work.push(base.scope_id(db));
        }
    }
    InstanceCopies { methods, bodies }
}

#[allow(clippy::too_many_arguments)]
pub fn lower_function<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_subs: Option<&super::mono_iface::IfaceSubs<'db>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    // Phase C: `Some(n)` when this is the arity specialization `f$n` of a
    // variadic function; `None` for every ordinary function.
    variadic_arity: Option<usize>,
) -> Result<MirFunction, LowerTypeError> {
    let r = lower_function_inner(
        db,
        func,
        index,
        memory_layout,
        string_pool,
        iface_subs,
        iface_call_rewrites,
        variadic_arity,
    );
    at_node(db, func, r)
}

#[allow(clippy::too_many_arguments)]
pub fn lower_function_block<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    iface_method_instances: &[&super::mono_iface::IfaceInstance<'db>],
    arity_method_instances: &[&super::mono_arity::ArityInstance<'db>],
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let r = lower_function_block_inner(
        db,
        fb,
        start_index,
        memory_layout,
        string_pool,
        iface_call_rewrites,
        iface_method_instances,
        arity_method_instances,
    );
    at_node(db, fb, r)
}

#[allow(clippy::too_many_arguments)]
pub fn lower_class<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    iface_method_instances: &[&super::mono_iface::IfaceInstance<'db>],
    arity_method_instances: &[&super::mono_arity::ArityInstance<'db>],
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let r = lower_class_inner(
        db,
        class,
        start_index,
        memory_layout,
        string_pool,
        iface_call_rewrites,
        iface_method_instances,
        arity_method_instances,
    );
    at_node(db, class, r)
}

pub fn lower_program<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<(MirFunction, MirType), LowerTypeError> {
    let r = lower_program_inner(
        db,
        program,
        index,
        memory_layout,
        string_pool,
        iface_call_rewrites,
    );
    at_node(db, program, r)
}

#[allow(clippy::too_many_arguments)]
fn lower_function_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    func: Function<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    // Phase B: for a specialized copy, each interface param's concrete
    // implementer.
    iface_subs: Option<&super::mono_iface::IfaceSubs<'db>>,
    // Phase B: module-global call-site -> mangled specialization rewrites, so
    // calls in this body route to the right specialization.
    iface_call_rewrites: &FxHashMap<
        hir::hir_def::expressions::expression::FuncCall<'db>,
        hir::hir_def::interned::identifier::Ident,
    >,
    // Phase C: the argument count this copy is specialized for.
    variadic_arity: Option<usize>,
) -> Result<MirFunction, LowerTypeError> {
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 0;
    open_frame(
        db,
        hir::hir_ty::calls::CallNode::Function(func),
        memory_layout,
    );

    // Collect address-taken variables for storage decisions
    let mut address_taken = collect_address_taken_vars(db, func.scope_id(db));
    address_taken.extend(collect_address_taken_in_inits(db, func.variables(db)));

    // 1. Parameters (Input, InOut, Output). VAR_OUTPUT is a pointer at the
    // wasm level. `next_local_idx` advances by the slots each param consumes
    // (`param_wasm_width`): a STRING flattens to two i32s. Phase C: the pack
    // becomes `arity` by-value parameters `pack$0..pack$n-1`.
    let mut variadic_expansion: Option<Rc<super::lower_expr::VariadicExpansion>> = None;
    for var in func.variables(db) {
        if let Some(arity) = variadic_arity
            && var.variadic(db)
        {
            let (pack, expansion) = pack_params(db, *var, arity)?;
            for param in pack {
                next_local_idx += param_wasm_width(&param.ty, param.kind);
                params.push(param);
            }
            variadic_expansion = Some(expansion);
            continue;
        }
        if let Some(param) = param_for_var(db, var, iface_subs)? {
            next_local_idx += param_wasm_width(&param.ty, param.kind);
            params.push(param);
        }
    }
    let entry_copies = shadow_inputs(
        db,
        &mut params,
        &address_taken,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );

    // 2. Return type
    let return_type = func
        .return_type(db)
        // The declared spec, not the inferred type: a `STRING[n]` result keeps
        // its capacity, which `Type::normalize` drops.
        .map(|spec| super::lower_type::lower_spec(db, *spec))
        .transpose()?;

    // The return slot, named after the function: scalars in a wasm local,
    // STRING, aggregates and a slot `REF()` or a VAR_IN_OUT takes in linear
    // memory.
    if let Some(ref ret_ty) = return_type {
        let storage = allocate_local_storage(
            func.name(db),
            ret_ty,
            address_taken.contains(&func.name(db)),
            &mut next_local_idx,
            memory_layout,
        );
        locals.push(MirLocal {
            name: func.name(db),
            ty: ret_ty.clone(),
            init: None,
            storage,
            // Synthetic return slot in a stateless FUNCTION.
            var_storage: MirVariableStorage::Automatic,
        });
    }

    // 3. Local variables (Var, Temp - Output is a parameter now)
    for var in func.variables(db) {
        match var.kind(db) {
            // VAR_EXTERNAL resolves to a global's address, not a function local.
            VariableKind::Input
            | VariableKind::InOut
            | VariableKind::Output
            | VariableKind::External => continue,
            _ => {}
        }

        let ty = lower_var_type(db, *var)?;
        let storage = allocate_local_storage(
            var.name(db),
            &ty,
            address_taken.contains(&var.name(db)),
            &mut next_local_idx,
            memory_layout,
        );

        locals.push(MirLocal {
            name: var.name(db),
            ty,
            init: None, // TODO: lower initializers
            storage,
            // FUNCTION locals are stateless — automatic regardless of section.
            var_storage: MirVariableStorage::Automatic,
        });
    }

    // The context the body lowers in; a local's own initializer, written in
    // the same declarations, lowers in it too. Interface specialization
    // threads `iface_subs` and `iface_call_rewrites` into it.
    let ctx = crate::lower::lower_stmt::body_ctx(
        db,
        None,
        None,
        string_pool.clone(),
        iface_subs,
        iface_call_rewrites,
        variadic_expansion.clone(),
    );

    // 4. Starting values: the shadowed inputs, the result's type defaults,
    // then every local's.
    let mut init_stmts = entry_copies;
    if let (Some(spec), Some(ret_ty)) = (func.return_type(db), &return_type) {
        init_stmts.extend(lower_result_init_stmts(
            db,
            func.name(db),
            ret_ty,
            spec,
            &string_pool,
        )?);
    }
    init_stmts.extend(lower_local_init_stmts(db, func.variables(db), &ctx)?);

    // 5. Body statements.
    let mut body = crate::lower::lower_stmt::lower_body(&ctx, func.statements(db))?;
    append_call_scratch_locals(
        ctx.call_scratch.take(),
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );
    // Prepend initializers
    if !init_stmts.is_empty() {
        init_stmts.append(&mut body);
        body = init_stmts;
    }
    snapshot_string_arguments(
        db,
        &mut body,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );
    let body = body;

    // Exported when the declaration says so, with `{export}`. A `{test}` is
    // too, for the runner to call; codegen leaves it out of a release build.
    // Everything else is internal: an export is a root, and with every POU
    // exported an optimizer could remove nothing.
    let linkage = {
        use hir::HasPragmas;
        if func.extern_pragma(db).is_some() {
            MirLinkage::Internal // extern functions are imports, handled separately
        } else if func.is_export(db) || func.is_test(db) {
            MirLinkage::Export
        } else {
            MirLinkage::Internal
        }
    };

    Ok(MirFunction {
        name: super::naming::mir_function_symbol(db, func),
        origin_name: func.name(db),
        index,
        params,
        return_type,
        locals,
        body,
        linkage,
        is_test: hir::hir_def::pous::pragma::is_test(db, func.pragmas(db)),
        export_name: None,
        frame: memory_layout.end_frame(),
        host_entry: hir::hir_def::pous::pragma::is_test(db, func.pragmas(db)),
    })
}

/// Lower a FUNCTION_BLOCK to MirFunctions (one per method + instance type).
#[allow(clippy::too_many_arguments)]
fn lower_function_block_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    iface_method_instances: &[&super::mono_iface::IfaceInstance<'db>],
    arity_method_instances: &[&super::mono_arity::ArityInstance<'db>],
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();
    let mut idx = start_index;

    // Each method is emitted once, except interface-param methods, emitted
    // once per specialization (`Owner#Use$@Worker`), and variadic ones, once
    // per argument count called. A base's method or body reached through
    // SUPER is emitted here too, on this instance type.
    let copies = instance_copies(db, hir::hir_def::pous::pou::Pou::FunctionBlock(fb));
    let jobs = method_jobs(
        db,
        hir::hir_def::pous::pou::Pou::FunctionBlock(fb),
        iface_method_instances,
        arity_method_instances,
    );

    // Lower each method as a separate function with 'this' parameter
    for (method, spec, arity) in jobs {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'
        open_frame(
            db,
            hir::hir_ty::calls::CallNode::Method(method),
            memory_layout,
        );
        // A method local whose address is taken must live in memory. Same scan
        // as the other bodies.
        let mut address_taken = collect_address_taken_vars(db, method.scope_id(db));
        address_taken.extend(collect_address_taken_in_inits(db, method.variables(db)));

        // 'this' pointer parameter — the FB's instance struct.
        let fb_type = super::lower_type::lower_fb_type(db, fb)?;
        // The method body resolves bare member access (implicit THIS) against
        // this struct.
        let this_struct = match &fb_type {
            MirType::Struct(s) => s.clone(),
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "FB type is not a struct".into(),
                ));
            }
        };
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(fb_type)),
            kind: MirParamKind::This,
        });

        // Method parameters, then its locals: every wasm parameter's index
        // comes before the first local's, whatever order the sections are
        // declared in.
        let mut local_vars = Vec::new();
        let mut variadic_expansion = None;
        for var in method.variables(db) {
            if let Some(arity) = arity
                && var.variadic(db)
            {
                let (pack, expansion) = pack_params(db, *var, arity)?;
                for param in pack {
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                variadic_expansion = Some(expansion);
                continue;
            }
            match param_for_var(db, var, spec.map(|i| &i.iface_subs))? {
                Some(param) => {
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                None => local_vars.push(var),
            }
        }
        let entry_copies = shadow_inputs(
            db,
            &mut params,
            &address_taken,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        for var in local_vars {
            let ty = lower_var_type(db, *var)?;
            let storage = allocate_local_storage(
                var.name(db),
                &ty,
                address_taken.contains(&var.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: var.name(db),
                ty,
                init: None,
                storage,
                // FB/class method local — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        let return_type = method
            .return_type(db)
            .map(|spec| super::lower_type::lower_spec(db, *spec))
            .transpose()?;

        // The return slot (scalar → wasm local, else linear memory, as when
        // its address is taken).
        if let Some(ref ret_ty) = return_type {
            let storage = allocate_local_storage(
                method.name(db),
                ret_ty,
                address_taken.contains(&method.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                storage,
                // Synthetic method return slot — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        // A specialization lowers its body with its own bindings and call
        // rewrites.
        let (body_subs, body_rewrites) = match spec {
            Some(inst) => (Some(&inst.iface_subs), &inst.call_rewrites),
            None => (None, iface_call_rewrites),
        };
        let ctx = crate::lower::lower_stmt::body_ctx(
            db,
            Some(this_struct),
            Some(hir::hir_def::pous::pou::Pou::FunctionBlock(fb)),
            string_pool.clone(),
            body_subs,
            body_rewrites,
            variadic_expansion,
        );
        let mut body = crate::lower::lower_stmt::lower_body(&ctx, method.stmts(db))?;

        // A method's result and locals are per call: their starting values
        // are stores at entry.
        let mut init_stmts = entry_copies;
        if let (Some(spec), Some(ret_ty)) = (method.return_type(db), &return_type) {
            init_stmts.extend(lower_result_init_stmts(
                db,
                method.name(db),
                ret_ty,
                spec,
                &string_pool,
            )?);
        }
        init_stmts.extend(lower_local_init_stmts(db, method.variables(db), &ctx)?);
        append_call_scratch_locals(
            ctx.call_scratch.take(),
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        if !init_stmts.is_empty() {
            init_stmts.append(&mut body);
            body = init_stmts;
        }
        snapshot_string_arguments(
            db,
            &mut body,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        let body = body;

        // Method symbol: `<FB>#<method>` (`NsA.Counter#inc`), a base's reached
        // through SUPER `<FB>#<Base>.<method>`; a specialization uses its
        // pre-mangled name.
        let qualified_name = match spec {
            Some(inst) => inst.mangled_name,
            None => super::naming::method_copy_symbol(
                db,
                hir::hir_def::pous::pou::Pou::FunctionBlock(fb),
                method,
            ),
        };
        // A variadic method's copy for one argument count, as the call site
        // names it: `Acc#Sum$3`.
        let qualified_name = match arity {
            Some(arity) => {
                super::naming::mangle_generic_name(db, qualified_name, &[&arity.to_string()])
            }
            None => qualified_name,
        };

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            // A method needs an instance the host does not have.
            linkage: MirLinkage::Internal,
            is_test: false,
            export_name: None,
            frame: memory_layout.end_frame(),
            host_entry: false,
        });
        idx += 1;
    }

    // The FB body as `__body__`, every variable through `this`. Lowered even
    // when empty: call sites emit `call FB$__body__` regardless. Then each
    // base's body its `SUPER()` reaches, on this instance.
    for body_of in std::iter::once(fb).chain(copies.bodies.iter().copied()) {
        functions.push(lower_fb_body(
            db,
            fb,
            body_of,
            idx,
            memory_layout,
            string_pool.clone(),
            iface_call_rewrites,
        )?);
        idx += 1;
    }

    Ok(functions)
}

/// The body of `body_of` emitted for the instance type `instance`: its own,
/// or a base's that `SUPER()` reaches, where `THIS` is still `instance`. A
/// derived instance is layout-compatible with its base, so the base's
/// statements address the same members.
#[allow(clippy::too_many_arguments)]
fn lower_fb_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    instance: FunctionBlock<'db>,
    body_of: FunctionBlock<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<MirFunction, LowerTypeError> {
    let fb_type = super::lower_type::lower_fb_type(db, instance)?;
    let body_params = vec![MirParam {
        name: Ident::new(db, compact_str::CompactString::from("this")),
        ty: MirType::Pointer(Box::new(fb_type.clone())),
        kind: MirParamKind::This,
    }];

    let mut body_locals = Vec::new();
    let mut next_local_idx: u32 = 1; // 0 is 'this'
    open_frame(
        db,
        hir::hir_ty::calls::CallNode::Body(body_of),
        memory_layout,
    );

    let mut address_taken = collect_address_taken_vars(db, body_of.scope_id(db));
    address_taken.extend(collect_address_taken_in_inits(db, body_of.variables(db)));

    for var in body_of.variables(db) {
        if var.kind(db) == VariableKind::Temp {
            let ty = lower_var_type(db, *var)?;
            let storage = allocate_local_storage(
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
                // Only VAR_TEMP reaches here, marked Temp so codegen resets aggregate
                // temps on entry.
                storage,
                var_storage: MirVariableStorage::Automatic,
            });
        }
    }

    // Body lowering with the `this` struct context.
    let this_struct = match &fb_type {
        MirType::Struct(s) => s.clone(),
        _ => {
            return Err(LowerTypeError::UnsupportedType(
                "FB type is not a struct".into(),
            ));
        }
    };
    let (body_stmts, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
        db,
        body_of.statements(db),
        this_struct,
        Some(hir::hir_def::pous::pou::Pou::FunctionBlock(instance)),
        string_pool.clone(),
        None,
        iface_call_rewrites,
    )?;
    // VAR_TEMP starts over at every call, from its declared values.
    let mut body_stmts = with_temp_inits(db, body_of.variables(db), body_stmts, &string_pool)?;
    append_call_scratch_locals(
        call_scratch,
        &mut body_locals,
        &mut next_local_idx,
        memory_layout,
    );
    snapshot_string_arguments(
        db,
        &mut body_stmts,
        &mut body_locals,
        &mut next_local_idx,
        memory_layout,
    );

    Ok(MirFunction {
        name: super::naming::body_symbol(
            db,
            hir::hir_def::pous::pou::Pou::FunctionBlock(instance),
            hir::hir_def::pous::pou::Pou::FunctionBlock(body_of),
        ),
        origin_name: body_of.name(db),
        index,
        params: body_params,
        return_type: None,
        locals: body_locals,
        body: body_stmts,
        // Called by whoever holds an instance, never by the host.
        linkage: MirLinkage::Internal,
        is_test: false,
        export_name: None,
        frame: memory_layout.end_frame(),
        // A task may run it, and so may any code holding an instance.
        host_entry: false,
    })
}

/// Lower a CLASS to MirFunctions (one per method + instance type).
#[allow(clippy::too_many_arguments)]
fn lower_class_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
    start_index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
    iface_method_instances: &[&super::mono_iface::IfaceInstance<'db>],
    arity_method_instances: &[&super::mono_arity::ArityInstance<'db>],
) -> Result<Vec<MirFunction>, LowerTypeError> {
    let mut functions = Vec::new();

    // Interface-param and variadic methods are emitted once per
    // specialization (see the FB-method site), and a base's method reached
    // through SUPER too.
    let jobs = method_jobs(
        db,
        hir::hir_def::pous::pou::Pou::Class(class),
        iface_method_instances,
        arity_method_instances,
    );

    for (idx, (method, spec, arity)) in (start_index..).zip(jobs) {
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut next_local_idx: u32 = 1; // 0 is 'this'
        open_frame(
            db,
            hir::hir_ty::calls::CallNode::Method(method),
            memory_layout,
        );

        // 'this' pointer parameter
        let class_type = lower_type(db, Type::Class(class))?;
        // The method body resolves bare member access (implicit THIS) against
        // this struct.
        let this_struct = match &class_type {
            MirType::Struct(s) => s.clone(),
            _ => {
                return Err(LowerTypeError::UnsupportedType(
                    "class type is not a struct".into(),
                ));
            }
        };
        params.push(MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(class_type)),
            kind: MirParamKind::This,
        });

        // Same address-taken rule as the FB method loop above.
        let mut address_taken = collect_address_taken_vars(db, method.scope_id(db));
        address_taken.extend(collect_address_taken_in_inits(db, method.variables(db)));

        // Method parameters, then its locals: every wasm parameter's index
        // comes before the first local's, whatever order the sections are
        // declared in.
        let mut local_vars = Vec::new();
        let mut variadic_expansion = None;
        for var in method.variables(db) {
            if let Some(arity) = arity
                && var.variadic(db)
            {
                let (pack, expansion) = pack_params(db, *var, arity)?;
                for param in pack {
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                variadic_expansion = Some(expansion);
                continue;
            }
            match param_for_var(db, var, spec.map(|i| &i.iface_subs))? {
                Some(param) => {
                    next_local_idx += param_wasm_width(&param.ty, param.kind);
                    params.push(param);
                }
                None => local_vars.push(var),
            }
        }
        let entry_copies = shadow_inputs(
            db,
            &mut params,
            &address_taken,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        for var in local_vars {
            let ty = lower_var_type(db, *var)?;
            let storage = allocate_local_storage(
                var.name(db),
                &ty,
                address_taken.contains(&var.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: var.name(db),
                ty,
                init: None,
                storage,
                // FB/class method local — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        let return_type = method
            .return_type(db)
            .map(|spec| super::lower_type::lower_spec(db, *spec))
            .transpose()?;

        // Return local: scalar → WASM local, non-scalar or address-taken →
        // linear memory.
        if let Some(ref ret_ty) = return_type {
            let storage = allocate_local_storage(
                method.name(db),
                ret_ty,
                address_taken.contains(&method.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: method.name(db),
                ty: ret_ty.clone(),
                init: None,
                storage,
                // Synthetic method return slot — stateless per call.
                var_storage: MirVariableStorage::Automatic,
            });
        }

        // Specializations lower with their own bindings + rewrites (see the
        // FB-method site).
        let (body_subs, body_rewrites) = match spec {
            Some(inst) => (Some(&inst.iface_subs), &inst.call_rewrites),
            None => (None, iface_call_rewrites),
        };
        let ctx = crate::lower::lower_stmt::body_ctx(
            db,
            Some(this_struct),
            Some(hir::hir_def::pous::pou::Pou::Class(class)),
            string_pool.clone(),
            body_subs,
            body_rewrites,
            variadic_expansion,
        );
        let mut body = crate::lower::lower_stmt::lower_body(&ctx, method.stmts(db))?;

        // A method's result and locals are per call: their starting values
        // are stores at entry.
        let mut init_stmts = entry_copies;
        if let (Some(spec), Some(ret_ty)) = (method.return_type(db), &return_type) {
            init_stmts.extend(lower_result_init_stmts(
                db,
                method.name(db),
                ret_ty,
                spec,
                &string_pool,
            )?);
        }
        init_stmts.extend(lower_local_init_stmts(db, method.variables(db), &ctx)?);
        append_call_scratch_locals(
            ctx.call_scratch.take(),
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        if !init_stmts.is_empty() {
            init_stmts.append(&mut body);
            body = init_stmts;
        }
        snapshot_string_arguments(
            db,
            &mut body,
            &mut locals,
            &mut next_local_idx,
            memory_layout,
        );
        let body = body;

        // Method symbol: `<NsPath.>Class#Method` (see the FB-method site).
        let qualified_name = match spec {
            Some(inst) => inst.mangled_name,
            None => super::naming::method_copy_symbol(
                db,
                hir::hir_def::pous::pou::Pou::Class(class),
                method,
            ),
        };
        // A variadic method's copy for one argument count, as the call site
        // names it: `Acc#Sum$3`.
        let qualified_name = match arity {
            Some(arity) => {
                super::naming::mangle_generic_name(db, qualified_name, &[&arity.to_string()])
            }
            None => qualified_name,
        };

        functions.push(MirFunction {
            name: qualified_name,
            origin_name: method.name(db),
            index: idx,
            params,
            return_type,
            locals,
            body,
            // A method needs an instance the host does not have.
            linkage: MirLinkage::Internal,
            is_test: false,
            export_name: None,
            frame: memory_layout.end_frame(),
            host_entry: false,
        });
    }

    Ok(functions)
}

/// Lower a PROGRAM's body to `<Prog>$__body__(this)` plus its instance
/// struct: a PROGRAM is compiled like a FUNCTION_BLOCK, with only
/// `VAR_TEMP` body-local. Instances are allocated per program
/// configuration in `lower_module`.
fn lower_program_inner<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: Rc<RefCell<super::lower_expr::StringPool>>,
    iface_call_rewrites: &super::mono_iface::IfaceCallRewrites<'db>,
) -> Result<(MirFunction, MirType), LowerTypeError> {
    let prog_type = super::lower_type::lower_program_type(db, program)?;
    let this_struct = match &prog_type {
        MirType::Struct(s) => s.clone(),
        _ => {
            return Err(LowerTypeError::UnsupportedType(
                "program type is not a struct".into(),
            ));
        }
    };

    let params = vec![MirParam {
        name: Ident::new(db, compact_str::CompactString::from("this")),
        ty: MirType::Pointer(Box::new(prog_type.clone())),
        kind: MirParamKind::This,
    }];

    // VAR_TEMP become body locals (automatic); persistent vars are instance
    // fields accessed through `this`.
    let mut locals = Vec::new();
    let mut next_local_idx: u32 = 1; // 0 is 'this'
    let mut address_taken = collect_address_taken_vars(db, program.scope_id(db));
    address_taken.extend(collect_address_taken_in_inits(db, program.variables(db)));
    for var in program.variables(db) {
        if var.kind(db) == VariableKind::Temp {
            let ty = lower_var_type(db, *var)?;
            let storage = allocate_local_storage(
                var.name(db),
                &ty,
                address_taken.contains(&var.name(db)),
                &mut next_local_idx,
                memory_layout,
            );
            locals.push(MirLocal {
                name: var.name(db),
                ty,
                init: None,
                // Guarded by `VariableKind::Temp` above.
                storage,
                var_storage: MirVariableStorage::Automatic,
            });
        }
    }

    let (body, call_scratch) = crate::lower::lower_stmt::lower_stmts_fb_body(
        db,
        program.statements(db),
        this_struct,
        None,
        string_pool.clone(),
        None,
        iface_call_rewrites,
    )?;
    // VAR_TEMP starts over at every scan, from its declared values.
    let mut body = with_temp_inits(db, program.variables(db), body, &string_pool)?;
    append_call_scratch_locals(
        call_scratch,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );
    snapshot_string_arguments(
        db,
        &mut body,
        &mut locals,
        &mut next_local_idx,
        memory_layout,
    );

    let body_name = Ident::new(
        db,
        compact_str::CompactString::from(format!(
            "{}$__body__",
            program.name_with_case(db).text(db)
        )),
    );

    let func = MirFunction {
        name: body_name,
        origin_name: program.name(db),
        index,
        params,
        return_type: None,
        locals,
        body,
        // The schedule names this export, and the host calls it every scan.
        linkage: MirLinkage::Export,
        is_test: false,
        export_name: None,
        // Nothing calls a PROGRAM but the host.
        frame: None,
        host_entry: true,
    };
    Ok((func, prog_type))
}

/// A variadic pack at `arity`: that many by-value parameters `pack$0..`,
/// and the expansion its folds read them through.
fn pack_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    arity: usize,
) -> Result<(Vec<MirParam>, Rc<super::lower_expr::VariadicExpansion>), LowerTypeError> {
    let elem_ty = lower_var_type(db, var)?;
    let elem = match &elem_ty {
        MirType::Elementary(e) => *e,
        other => {
            return Err(LowerTypeError::UnsupportedType(format!(
                "a variadic parameter must be elementary, got {other:?}"
            )));
        }
    };
    let mut names = Vec::with_capacity(arity);
    let mut params = Vec::with_capacity(arity);
    for i in 0..arity {
        let name = Ident::new(
            db,
            compact_str::CompactString::from(format!("{}${i}", var.name(db).text(db))),
        );
        names.push(name);
        params.push(MirParam {
            name,
            ty: input_param_type(elem_ty.clone()),
            kind: MirParamKind::Input,
        });
    }
    let expansion = Rc::new(super::lower_expr::VariadicExpansion {
        pack: var.name(db),
        params: names,
        elem,
    });
    Ok((params, expansion))
}

/// The copies of an instance type's methods to emit, each with its
/// interface specialization and its argument count: a method with an
/// interface parameter once per specialization, a variadic one once per
/// argument count called (an uncalled one not at all), any other once. A
/// base's method or body reached through SUPER is emitted on this instance
/// type too.
#[allow(clippy::type_complexity)]
fn method_jobs<'a, 'db>(
    db: &'db dyn WorkspaceDataBase,
    owner: hir::hir_def::pous::pou::Pou<'db>,
    iface_method_instances: &[&'a super::mono_iface::IfaceInstance<'db>],
    arity_method_instances: &[&'a super::mono_arity::ArityInstance<'db>],
) -> Vec<(
    hir::hir_def::pous::class::MethodDecl<'db>,
    Option<&'a super::mono_iface::IfaceInstance<'db>>,
    Option<usize>,
)> {
    let mut jobs = Vec::new();
    for method in instance_copies(db, owner).methods.iter().copied() {
        let specs: Vec<_> = if method
            .variables(db)
            .iter()
            .any(|v| super::mono_iface::is_interface_param(db, v))
        {
            iface_method_instances
                .iter()
                .filter(|inst| {
                    matches!(
                        inst.target,
                        super::mono_iface::IfaceTarget::Method { method: m, .. } if m == method
                    )
                })
                .map(|inst| Some(*inst))
                .collect()
        } else {
            vec![None]
        };
        let arities: Vec<_> = if method.variables(db).iter().any(|v| v.variadic(db)) {
            arity_method_instances
                .iter()
                .filter(|inst| {
                    matches!(
                        inst.target,
                        super::mono_arity::ArityTarget::Method { method: m, .. } if m == method
                    )
                })
                .map(|inst| Some(inst.arity))
                .collect()
        } else {
            vec![None]
        };
        for spec in &specs {
            for arity in &arities {
                jobs.push((method, *spec, *arity));
            }
        }
    }
    jobs
}

/// The wasm-level parameter a declared variable becomes, or `None` when it
/// is not part of the calling convention. The one encoding of the
/// convention: input by value, `VAR_IN_OUT`/`VAR_OUTPUT` by pointer, a
/// specialized interface param as a pointer to the concrete instance.
fn param_for_var<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: &hir::hir_def::pous::variable::VariableDecl<'db>,
    iface_subs: Option<&super::mono_iface::IfaceSubs<'db>>,
) -> Result<Option<MirParam>, LowerTypeError> {
    if let Some(concrete) = iface_subs.and_then(|m| m.get(var)) {
        let ty = lower_type(db, hir::hir_ty::ty::Type::new_pou(db, *concrete))?;
        return Ok(Some(MirParam {
            name: var.name(db),
            ty: MirType::Pointer(Box::new(ty)),
            kind: MirParamKind::InOut,
        }));
    }

    Ok(match var.kind(db) {
        VariableKind::Input => Some(MirParam {
            name: var.name(db),
            ty: input_param_type(lower_var_type(db, *var)?),
            kind: MirParamKind::Input,
        }),
        VariableKind::InOut | VariableKind::Output => Some(MirParam {
            name: var.name(db),
            ty: MirType::Pointer(Box::new(lower_var_type(db, *var)?)),
            kind: if var.kind(db) == VariableKind::InOut {
                MirParamKind::InOut
            } else {
                MirParamKind::Output
            },
        }),
        _ => None,
    })
}

/// Number of wasm i32 locals a parameter consumes in the signature; must
/// match `build_local_map` in `wasm_codegen`, since lowering advances the
/// wasm-local index by it. `VAR_INPUT STRING` → `(ptr, len)`, a STRING
/// pointer → `(addr, cap)`, everything else one slot.
pub fn param_wasm_width(ty: &MirType, kind: MirParamKind) -> u32 {
    match (kind, ty) {
        (MirParamKind::Input, MirType::String { .. }) => 2,
        (MirParamKind::InOut | MirParamKind::Output, MirType::Pointer(inner))
            if matches!(inner.as_ref(), MirType::String { .. }) =>
        {
            2
        }
        _ => 1,
    }
}

/// Aggregate `VAR_INPUT`s are received as a pointer to the caller's
/// call-entry snapshot (`MirExpr::CopyIntoScratch`); everything else is a
/// value param.
pub(crate) fn input_param_type(ty: MirType) -> MirType {
    match ty {
        MirType::Struct(_) | MirType::Array(_) => MirType::Pointer(Box::new(ty)),
        other => other,
    }
}

/// Drain the scratch locals a body's lowering synthesized into the
/// function's locals; memory-forced, since the call site takes their
/// address.
pub(crate) fn append_call_scratch_locals(
    scratch: crate::lower::lower_expr::CallScratch,
    locals: &mut Vec<MirLocal>,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) {
    // Aggregate snapshots are memory-forced (their address is taken); extern
    // result scratches are plain wasm locals (they only ever LocalSet/Get).
    for (force_memory, list) in [(true, scratch.memory), (false, scratch.scalar)] {
        for (name, ty) in list {
            let storage =
                allocate_local_storage(name, &ty, force_memory, next_local_idx, memory_layout);
            locals.push(MirLocal {
                name,
                ty,
                init: None,
                storage,
                var_storage: MirVariableStorage::Automatic,
            });
        }
    }
}

/// Copy each STRING a call returns and passes by value to another call into
/// a local of its own ([`MirExpr::StringSnapshot`]). The result sits in its
/// callee's slot until the call it is passed to copies it at entry, and
/// before that a later argument may call the callee again, or the call be
/// the callee's own and zero the slot, `F(F(x))`. One local per such
/// argument, in the frame when the function has one.
fn snapshot_string_arguments(
    db: &dyn WorkspaceDataBase,
    body: &mut [MirStmt],
    locals: &mut Vec<MirLocal>,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) {
    let mut copies = crate::lower::lower_expr::CallScratch::default();
    crate::stmt::for_each_call_mut(body, &mut |call| {
        for arg in &mut call.args {
            let (MirArgKind::ByValue, MirExpr::Call(inner)) = (arg.kind, &arg.value) else {
                continue;
            };
            let MirType::String { capacity } = inner.return_type else {
                continue;
            };
            let scratch = Ident::new(
                db,
                compact_str::CompactString::from(format!("$strcopy${}", copies.memory.len())),
            );
            copies.memory.push((scratch, MirType::String { capacity }));
            let src = std::mem::replace(&mut arg.value, MirExpr::Constant(MirConstant::Null));
            arg.value = MirExpr::StringSnapshot {
                scratch,
                src: Box::new(src),
            };
        }
    });
    append_call_scratch_locals(copies, locals, next_local_idx, memory_layout);
}

pub fn allocate_local_storage(
    name: Ident,
    ty: &MirType,
    force_memory: bool,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) -> MirStorage {
    if ty.is_scalar() && !force_memory {
        let idx = *next_local_idx;
        *next_local_idx += 1;
        MirStorage::Scalar { local_index: idx }
    } else {
        let size = ty.size_bytes();
        let align = ty.alignment();
        if let Some(offset) = memory_layout.allocate_in_frame(size, align) {
            return MirStorage::Frame {
                offset,
                size,
                align,
            };
        }
        let address = memory_layout.allocate(name, size, align, MirAllocKind::Variable);
        MirStorage::Memory {
            address,
            size,
            align,
        }
    }
}

/// Lay out `node`'s storage in a frame from here on when HIR's call graph
/// says it may call itself: each of its calls then has storage of its own.
/// The function's `frame` closes it.
fn open_frame<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: hir::hir_ty::calls::CallNode<'db>,
    memory_layout: &mut MirMemoryLayout,
) {
    if hir::hir_ty::calls::is_recursive(db, node) {
        memory_layout.begin_frame();
    }
}

/// A VAR_INPUT that needs storage of its own has none as a wasm parameter:
/// the parameter becomes `x$arg`, and `x` a local in linear memory, which
/// the returned statements fill from it at entry. That is an input whose
/// address the body takes (`REF(x)`, `x` as a VAR_IN_OUT argument or an
/// output's destination), and every STRING input: it arrives as the
/// caller's `(ptr, len)`, and an input is a copy, which a change to the
/// caller's variable during the call, or an assignment to the input, must
/// not reach. An aggregate input already arrives as the address of the
/// caller's snapshot.
fn shadow_inputs(
    db: &dyn WorkspaceDataBase,
    params: &mut [MirParam],
    address_taken: &FxHashSet<Ident>,
    locals: &mut Vec<MirLocal>,
    next_local_idx: &mut u32,
    memory_layout: &mut MirMemoryLayout,
) -> Vec<MirStmt> {
    let mut copies = Vec::new();
    for param in params {
        let own_storage =
            matches!(param.ty, MirType::String { .. }) || address_taken.contains(&param.name);
        if !matches!(param.kind, MirParamKind::Input)
            || matches!(param.ty, MirType::Pointer(_))
            || !own_storage
        {
            continue;
        }
        let name = param.name;
        param.name = Ident::new(
            db,
            compact_str::CompactString::from(format!("{}$arg", name.text(db))),
        );
        let storage = allocate_local_storage(name, &param.ty, true, next_local_idx, memory_layout);
        locals.push(MirLocal {
            name,
            ty: param.ty.clone(),
            init: None,
            storage,
            var_storage: MirVariableStorage::Automatic,
        });
        copies.push(MirStmt::Assign {
            target: crate::expr::MirPlace::Local(name),
            value: crate::expr::MirExpr::Load(
                crate::expr::MirPlace::Local(param.name),
                param.ty.clone(),
            ),
        });
    }
    copies
}

/// Lower a variable's type spec, recovering a declared `STRING[N]`
/// capacity that `Type::normalize` collapses.
pub(crate) fn lower_var_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    super::lower_type::lower_spec(db, var.spec(db))
}

/// The variables a declaration initializer takes the address of:
/// `q : REF_TO INT := REF(x)` marks `x` like the statement `q := REF(x)`
/// does.
fn collect_address_taken_in_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[hir::hir_def::pous::variable::VariableDecl<'db>],
) -> FxHashSet<Ident> {
    use hir::hir_def::expressions::expression::{ExprKind, InitExprKind, PrimaryExpr, RefValue};

    fn walk_init<'db>(
        db: &'db dyn WorkspaceDataBase,
        init: &hir::hir_def::expressions::expression::InitExpr<'db>,
        result: &mut FxHashSet<Ident>,
    ) {
        match init.kind(db) {
            InitExprKind::ConstantExpr(expr) => {
                if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                    value: RefValue::Address(begin_path),
                }) = expr.expr(db)
                    && let Some(path_expr) = begin_path.expr(db)
                {
                    result.insert(path_expr.ident(db).ident(db));
                }
            }
            InitExprKind::ArrayInit { values }
            | InitExprKind::ArrayIndexedElement { values, .. }
            | InitExprKind::StructInit { values } => {
                for v in values {
                    walk_init(db, &v, result);
                }
            }
            InitExprKind::StructElement { value, .. } => walk_init(db, &value, result),
        }
    }

    let mut result = FxHashSet::default();
    for var in vars {
        if let Some(init) = var.init(db) {
            walk_init(db, &init, &mut result);
        }
    }
    result
}

/// The variables whose address the body of `scope` takes, which linear
/// memory holds even when they are scalars: the root of every `REF()`,
/// VAR_IN_OUT argument and output destination, wherever it stands (a
/// subscript, an assignment target, a nested call). Read off the body's
/// inference, which visited every expression and bound every call.
fn collect_address_taken_vars<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: hir::hir_def::scope::ScopeId<'db>,
) -> FxHashSet<Ident> {
    use hir::hir_def::expressions::expression::{
        BeginPathExpr, ExprKind, PrimaryExpr, RefValue, VariableAccessKind,
    };
    use hir::hir_ty::body::ParamBinding;

    fn root<'db>(db: &'db dyn WorkspaceDataBase, path: &BeginPathExpr<'db>) -> Option<Ident> {
        let root = path.expr(db)?.flatten(db).first()?.get_expr(db);
        Some(root.ident(db).ident(db))
    }

    let body = hir::hir_ty::body::infer_body(db, scope);
    let mut result = FxHashSet::default();
    for expr in body.type_of_expr.keys() {
        if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
            value: RefValue::Address(path),
        }) = expr.expr(db)
        {
            result.extend(root(db, path));
        }
    }
    for call in body.resolved_calls.values() {
        for (var, binding) in &call.params {
            let access = match binding {
                ParamBinding::Values(values) if var.is_in_out(db) => values
                    .iter()
                    .filter_map(|value| match value.expr(db) {
                        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => Some(*access),
                        _ => None,
                    })
                    .collect(),
                ParamBinding::Output(access) => vec![*access],
                _ => continue,
            };
            for access in access {
                if let VariableAccessKind::Symbolic(path) = access.kind(db) {
                    result.extend(root(db, &path));
                }
            }
        }
    }
    result
}

/// The statements that give a POU's own variables their starting values at
/// entry: the type's defaults first (an FB's or CLASS's member defaults
/// included), then the declaration's own initializer, which overlays them by
/// store order. A FUNCTION's or METHOD's VAR_OUTPUT starts over too: it is
/// written through its pointer into the caller's variable. A VAR_INPUT or
/// VAR_IN_OUT holds what the caller passed, and a VAR_EXTERNAL is the
/// global's own storage, initialized once in `__init`.
fn lower_local_init_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[hir::hir_def::pous::variable::VariableDecl<'db>],
    // The context the body lowers in, where a declaration's own initializer
    // lowers too.
    body: &ExprLowerCtx<'db>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let string_pool = &body.string_pool;
    let mut init_stmts = Vec::new();
    for var in vars {
        match var.kind(db) {
            VariableKind::Input | VariableKind::InOut | VariableKind::External => continue,
            _ => {}
        }
        let var_ty = lower_var_type(db, *var)?;
        lower_type_default_inits(
            db,
            InitTarget::Local {
                name: var.name(db),
                base: 0,
                whole: scalar_shaped(&var_ty),
            },
            &var_ty,
            var.spec(db).infer(db),
            string_pool,
            &mut init_stmts,
        )?;
        if let Some(init_expr) = var.init(db) {
            lower_var_init(db, var.name(db), &var_ty, init_expr, body, &mut init_stmts)?;
        }
    }
    Ok(init_stmts)
}

/// The starting value of a FUNCTION's or METHOD's result: its type's
/// defaults, written at entry like a local's.
fn lower_result_init_stmts<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    ret_ty: &MirType,
    spec: &hir::hir_def::expressions::spec::Spec<'db>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let mut stmts = Vec::new();
    lower_type_default_inits(
        db,
        InitTarget::Local {
            name,
            base: 0,
            whole: scalar_shaped(ret_ty),
        },
        ret_ty,
        spec.infer(db),
        string_pool,
        &mut stmts,
    )?;
    Ok(stmts)
}

/// `body` preceded by the starting values of the VAR_TEMPs among `vars`: an
/// FB's or PROGRAM's other variables are instance state, initialized once.
fn with_temp_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[hir::hir_def::pous::variable::VariableDecl<'db>],
    body: Vec<MirStmt>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<Vec<MirStmt>, LowerTypeError> {
    let temps: Vec<_> = vars
        .iter()
        .filter(|v| v.kind(db) == VariableKind::Temp)
        .copied()
        .collect();
    // A VAR_TEMP has no initializer of its own (E0004), only its type's.
    let mut stmts =
        lower_local_init_stmts(db, &temps, &ExprLowerCtx::new(db, string_pool.clone()))?;
    stmts.extend(body);
    Ok(stmts)
}

/// Every scalar-shaped variable is the whole target of its initializer,
/// subrange and enum aliases included; an aggregate is reached through.
fn scalar_shaped(ty: &MirType) -> bool {
    matches!(
        ty,
        MirType::Elementary(_) | MirType::Subrange(_) | MirType::Enum(_) | MirType::Pointer(_)
    )
}

fn lower_var_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
    var_ty: &crate::types::MirType,
    init_expr: hir::hir_def::expressions::expression::InitExpr<'db>,
    body: &ExprLowerCtx<'db>,
    out: &mut Vec<MirStmt>,
) -> Result<(), LowerTypeError> {
    lower_init_leaves(
        db,
        InitTarget::Local {
            name: var_name,
            base: 0,
            whole: true,
        },
        var_ty,
        init_expr,
        None,
        Some(body),
        &body.string_pool,
        out,
    )
}

/// Where an instance's member initializers are written; the member walk
/// itself is shared.
#[derive(Clone, Copy)]
pub(crate) enum InitTarget {
    /// A statically allocated instance at an absolute address (PROGRAM
    /// instances, config globals), baked into `__init`: only constant leaves
    /// qualify.
    Static { base: u32 },
    /// A local instance, addressed over the local's own base, emitted into
    /// the owning function's prologue. `whole` says the target is the named
    /// local itself.
    Local { name: Ident, base: u32, whole: bool },
}

/// The instance a member's initializer is part of: a `REF()` in it names
/// that instance's members (`p : REF_TO INT := REF(x)` points at the `x`
/// beside this `p`).
pub(crate) struct InitOwner {
    pub target: InitTarget,
    pub layout: crate::types::MirStructType,
}

impl InitOwner {
    fn place(&self) -> crate::expr::MirPlace {
        use crate::expr::MirPlace;
        let ty = crate::types::MirType::Struct(self.layout.clone());
        match self.target {
            InitTarget::Static { base } => MirPlace::Global {
                name: None,
                address: base,
                ty,
            },
            InitTarget::Local { name, base, .. } => MirPlace::Field {
                base: Box::new(MirPlace::Local(name)),
                field_name: name,
                field_offset: base,
                field_type: ty,
            },
        }
    }
}

/// `expr` with every `this` root replaced by `instance`, subscripts and
/// call arguments included: a member's `REF()` default is lowered as a
/// method of the instance would lower it, then pointed at the instance
/// being initialized.
fn rebase_this(expr: &mut crate::expr::MirExpr, instance: &crate::expr::MirPlace) {
    use crate::expr::MirExpr;
    match expr {
        MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => {}
        MirExpr::Load(place, _) | MirExpr::AddrOf(place) | MirExpr::StringCapacity(place) => {
            rebase_this_place(place, instance)
        }
        MirExpr::BinOp { lhs, rhs, .. } => {
            rebase_this(lhs, instance);
            rebase_this(rhs, instance);
        }
        MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => rebase_this(expr, instance),
        MirExpr::CopyIntoScratch { src, .. } | MirExpr::StringSnapshot { src, .. } => {
            rebase_this(src, instance)
        }
        MirExpr::Call(call) => {
            for arg in &mut call.args {
                rebase_this(&mut arg.value, instance);
            }
            for binding in &mut call.output_bindings {
                rebase_this_place(&mut binding.target, instance);
                rebase_this(&mut binding.value, instance);
            }
            for result in &mut call.extern_results {
                if let Some(dest) = &mut result.dest {
                    rebase_this_place(dest, instance);
                }
            }
        }
    }
}

fn rebase_this_place(place: &mut crate::expr::MirPlace, instance: &crate::expr::MirPlace) {
    use crate::expr::MirPlace;
    match place {
        MirPlace::ThisField {
            field_name,
            field_offset,
            field_type,
        } => {
            *place = MirPlace::Field {
                base: Box::new(instance.clone()),
                field_name: *field_name,
                field_offset: *field_offset,
                field_type: field_type.clone(),
            }
        }
        MirPlace::Field { base, .. } => rebase_this_place(base, instance),
        MirPlace::Index { base, index, .. } => {
            rebase_this_place(base, instance);
            rebase_this(index, instance);
        }
        MirPlace::Deref { base, capacity, .. } => {
            rebase_this_place(base, instance);
            if let Some(capacity) = capacity {
                rebase_this_place(capacity, instance);
            }
        }
        MirPlace::Local(_) | MirPlace::Global { .. } => {}
    }
}

impl InitTarget {
    fn offset_by(self, delta: u32) -> Self {
        match self {
            InitTarget::Static { base } => InitTarget::Static { base: base + delta },
            InitTarget::Local { name, base, whole } => InitTarget::Local {
                name,
                base: base + delta,
                whole,
            },
        }
    }
}

/// Lower one initializer's resolved leaves into `out` at `target`;
/// `infer_initialization` already flattened and validated them. `owner` is
/// the instance a member's initializer is part of.
#[allow(clippy::too_many_arguments)]
fn lower_init_leaves<'db>(
    db: &'db dyn WorkspaceDataBase,
    target: InitTarget,
    ty: &crate::types::MirType,
    init: hir::hir_def::expressions::expression::InitExpr<'db>,
    owner: Option<&InitOwner>,
    // The body's context, for a local's own initializer; a type's or a
    // static host's leaves are constants, which need none.
    body: Option<&ExprLowerCtx<'db>>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
    out: &mut Vec<MirStmt>,
) -> Result<(), LowerTypeError> {
    use hir::hir_ty::head::init_inference::infer_initialization;

    let inference = infer_initialization(db, init.scope_id(db));
    let Some(leaves) = inference.init_expr_result.resolved.get(&init) else {
        return Ok(()); // HIR produced no resolved leaves (non-flattenable init)
    };
    let fresh;
    let ctx = match body {
        Some(body) => body,
        None => {
            fresh = ExprLowerCtx::new(db, string_pool.clone());
            &fresh
        }
    };
    for leaf in leaves {
        let (offset, leaf_ty) = if leaf.path.is_empty() {
            (0, ty.clone())
        } else {
            let Some(found) = walk_init_path(db, ty, &leaf.path) else {
                continue;
            };
            found
        };
        // The value itself, or the end of the CONSTANT chain it names.
        let end = hir::hir_ty::infer::const_eval::resolve_constant_ref(db, leaf.value)
            .unwrap_or(leaf.value);
        let value = match owner {
            // A member named in a REF() is the owner's.
            Some(owner)
                if matches!(
                    end.expr(db),
                    hir::hir_def::expressions::expression::ExprKind::PrimaryExpr(
                        hir::hir_def::expressions::expression::PrimaryExpr::RefValue { .. }
                    )
                ) =>
            {
                let ctx =
                    ExprLowerCtx::with_this_struct(db, owner.layout.clone(), string_pool.clone());
                let mut value = ctx.lower_leaf_value(leaf.value, &leaf_ty)?;
                rebase_this(&mut value, &owner.place());
                value
            }
            _ => ctx.lower_leaf_value(leaf.value, &leaf_ty)?,
        };
        // E0401 refuses every non-constant static leaf, so reaching this arm
        // means check and lowering disagree. A REF() is an address, which
        // `__init` stores once the layout has placed what it names.
        if let InitTarget::Static { .. } = target
            && !is_const_value(&value)
            && !matches!(value, crate::expr::MirExpr::AddrOf(_))
        {
            return Err(LowerTypeError::UnsupportedType(format!(
                "a static initializer leaf survived E0401 without \
                 being constant: {value:?}"
            )));
        }
        out.push(MirStmt::Assign {
            target: init_place(target.offset_by(offset), leaf_ty, leaf.path.is_empty()),
            value,
        });
    }
    Ok(())
}

/// Where a leaf initializer of type `leaf_ty` is stored. `whole_leaf` says the
/// leaf is the target itself rather than a part of it.
fn init_place(
    target: InitTarget,
    leaf_ty: crate::types::MirType,
    whole_leaf: bool,
) -> crate::expr::MirPlace {
    use crate::expr::MirPlace;
    match target {
        InitTarget::Static { base } => MirPlace::Global {
            name: None,
            address: base,
            ty: leaf_ty,
        },
        // Only a local that is the whole scalar-shaped target is addressed
        // directly; everything else takes the Field arm, since a bare `Local`
        // hides the leaf's type from codegen.
        InitTarget::Local { name, base, whole }
            if whole
                && base == 0
                && whole_leaf
                && !matches!(
                    leaf_ty,
                    crate::types::MirType::String { .. }
                        | crate::types::MirType::Struct(_)
                        | crate::types::MirType::Array(_)
                ) =>
        {
            MirPlace::Local(name)
        }
        InitTarget::Local { name, base, .. } => MirPlace::Field {
            base: Box::new(MirPlace::Local(name)),
            // `field_name` is metadata only — addressing uses `field_offset`.
            field_name: name,
            field_offset: base,
            field_type: leaf_ty,
        },
    }
}

/// Lower one initial value at `target`: an initializer the source writes,
/// or the value its type starts at (`InitValue::Implied`), a constant in the
/// slot's lane.
fn lower_init_value<'db>(
    db: &'db dyn WorkspaceDataBase,
    target: InitTarget,
    ty: &crate::types::MirType,
    value: hir::hir_ty::head::instances::InitValue<'db>,
    owner: Option<&InitOwner>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
    out: &mut Vec<MirStmt>,
) -> Result<(), LowerTypeError> {
    use hir::hir_ty::head::instances::InitValue;
    let implied = match value {
        InitValue::Written(init) => {
            return lower_init_leaves(db, target, ty, init, owner, None, string_pool, out);
        }
        InitValue::Implied(v) => v,
    };
    let to = match ty {
        crate::types::MirType::Elementary(e) => *e,
        crate::types::MirType::Subrange(sub) => sub.base,
        crate::types::MirType::Enum(en) => en.storage,
        other => {
            return Err(LowerTypeError::UnsupportedType(format!(
                "a type's starting value landed on {other:?}"
            )));
        }
    };
    out.push(MirStmt::Assign {
        target: init_place(target, ty.clone(), true),
        value: crate::expr::MirExpr::Cast {
            expr: Box::new(crate::expr::MirExpr::Constant(
                crate::expr::MirConstant::I64(implied),
            )),
            from: crate::types::MirElementary::LInt,
            to,
        },
    });
    Ok(())
}

use hir::hir_ty::head::instances::InstanceInitStep;

/// Map an initializer's member path (from [`instance_initializers`]) onto
/// the layout, one `(byte offset, type)` per slot; an `AllElements` step
/// fans out over an array.
///
/// [`instance_initializers`]: hir::hir_ty::head::instances::instance_initializers
fn member_path_slots(
    db: &dyn WorkspaceDataBase,
    ty: &crate::types::MirType,
    path: &[InstanceInitStep],
    base: u32,
    out: &mut Vec<(u32, crate::types::MirType)>,
) {
    let Some((step, rest)) = path.split_first() else {
        out.push((base, ty.clone()));
        return;
    };
    match (step, ty) {
        (InstanceInitStep::Field(name), crate::types::MirType::Struct(s)) => {
            if let Some(field) = s.fields.iter().find(|f| f.name(db) == *name) {
                member_path_slots(db, &field.ty, rest, base + field.offset, out);
            }
        }
        (InstanceInitStep::AllElements, crate::types::MirType::Array(a)) => {
            for i in 0..a.total_elements {
                member_path_slots(db, &a.element_type, rest, base + i * a.element_size, out);
            }
        }
        // HIR and the layout disagree about this member's shape; the caller
        // reports the miss.
        _ => {}
    }
}

/// Emit the initial values a variable's TYPE contributes (an alias's `:= 5`,
/// STRUCT field defaults, an FB's or CLASS's member defaults, an enum's first
/// value) before the declaration's own; HIR's [`type_default_inits`] decides
/// what applies, MIR turns each path into byte offsets.
///
/// [`type_default_inits`]: hir::hir_ty::head::instances::type_default_inits
pub(crate) fn lower_type_default_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    target: InitTarget,
    mir_ty: &crate::types::MirType,
    hir_ty: hir::hir_ty::ty::Type<'db>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
    out: &mut Vec<MirStmt>,
) -> Result<(), LowerTypeError> {
    for entry in hir::hir_ty::head::instances::type_default_inits(db, hir_ty) {
        // The slots of the instances the member is part of, then the member
        // in each: the instance is the owner a REF() in its default names.
        let (owner_path, member) = match entry.path.split_last() {
            Some((InstanceInitStep::Field(name), owner_path)) => (owner_path, Some(*name)),
            _ => (&entry.path[..], None),
        };
        let mut owners = Vec::new();
        member_path_slots(db, mir_ty, owner_path, 0, &mut owners);
        for (owner_offset, owner_ty) in owners {
            let (offset, slot_ty, owner) = match (member, owner_ty) {
                (None, slot_ty) => (owner_offset, slot_ty, None),
                (Some(name), crate::types::MirType::Struct(layout)) => {
                    // HIR and the layout disagree about this member; the
                    // slot walk skipped it too.
                    let Some(field) = layout.fields.iter().find(|f| f.name(db) == name) else {
                        continue;
                    };
                    let slot = (owner_offset + field.offset, field.ty.clone());
                    let owner = InitOwner {
                        target: target.offset_by(owner_offset),
                        layout,
                    };
                    (slot.0, slot.1, Some(owner))
                }
                (Some(_), _) => continue,
            };
            lower_init_value(
                db,
                target.offset_by(offset),
                &slot_ty,
                entry.init,
                owner.as_ref(),
                string_pool,
                out,
            )?;
        }
    }
    Ok(())
}

/// Emit one `Assign { Global, value }` per resolved initializer leaf; MIR
/// walks each leaf's path over the layout and never re-walks the
/// `InitExpr` tree.
pub(crate) fn lower_resolved_init_into<'db>(
    db: &'db dyn WorkspaceDataBase,
    base: u32,
    ty: &crate::types::MirType,
    init: hir::hir_def::expressions::expression::InitExpr<'db>,
    owner: Option<&InitOwner>,
    out: &mut Vec<crate::stmt::MirStmt>,
    string_pool: &Rc<RefCell<super::lower_expr::StringPool>>,
) -> Result<(), LowerTypeError> {
    lower_init_leaves(
        db,
        InitTarget::Static { base },
        ty,
        init,
        owner,
        None,
        string_pool,
        out,
    )
}

/// Walk a resolved leaf's path over the layout to its byte offset and
/// type.
fn walk_init_path(
    db: &dyn WorkspaceDataBase,
    root: &crate::types::MirType,
    path: &[hir::hir_ty::head::init_inference::InitPathStep],
) -> Option<(u32, crate::types::MirType)> {
    use crate::types::MirType;
    use hir::hir_ty::head::init_inference::InitPathStep;
    let mut offset = 0u32;
    let mut cur = root;
    for step in path {
        match (cur, step) {
            (MirType::Struct(s), InitPathStep::Field(name)) => {
                let f = s.fields.iter().find(|f| f.name(db) == *name)?;
                offset += f.offset;
                cur = &f.ty;
            }
            (MirType::Array(a), InitPathStep::ArrayElem(idx)) => {
                offset += *idx * a.element_size;
                cur = a.element_type.as_ref();
            }
            _ => return None,
        }
    }
    Some((offset, cur.clone()))
}

/// A value that can be baked into `__init`: a literal, a string literal,
/// or arithmetic over literals. No variable loads or calls.
fn is_const_value(e: &crate::expr::MirExpr) -> bool {
    use crate::expr::MirExpr;
    match e {
        MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => true,
        MirExpr::BinOp { lhs, rhs, .. } => is_const_value(lhs) && is_const_value(rhs),
        MirExpr::UnaryOp { expr, .. } => is_const_value(expr),
        MirExpr::Cast { expr, .. } => is_const_value(expr),
        MirExpr::Load(..)
        | MirExpr::Call(_)
        | MirExpr::AddrOf(_)
        | MirExpr::StringCapacity(_)
        | MirExpr::CopyIntoScratch { .. }
        | MirExpr::StringSnapshot { .. } => false,
    }
}
