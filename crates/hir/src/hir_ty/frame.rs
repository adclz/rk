// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The frame a call of a recursive body pushes on the stack: the storage
//! the body keeps in linear memory, which any other body has at a fixed
//! address. MIR lays its frames out to match, and `rk check` measures the
//! stack against them (E1430).
//!
//! A frame holds, for one call:
//! - a STRING input, and an input whose address the body takes, copied in
//!   at entry;
//! - the result, a local and a VAR_TEMP that are no scalar, or whose address
//!   the body takes;
//! - what the calls it makes need while they run: the copy of an aggregate
//!   input, a buffer for an output left unbound, an output received before
//!   it is negated, converted or written into bits of a wider address, a
//!   STRING CASE selector a call computes, and the copy of a STRING one call
//!   returns into the argument of another.
//!
//! Every part starts at a multiple of [`FRAME_ALIGN`], so a frame's size is
//! the sum of its parts, in whatever order they are laid out.

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasPragmas, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                Expr, ExprKind, FuncCall, PathExprKind, PrimaryExpr, VariableAccess,
                VariableAccessKind,
            },
            spec::{ElementarySpec, Enum},
            statement::{Stmt, StmtKind},
        },
        pous::variable::{LocatedAddress, VariableDecl, VariableKind},
    },
    hir_ty::{
        body::{IndexedType, ParamBinding, ResolvedCall, ScopeInference},
        calls::CallNode,
        index_graphs::{effective_location, located_view},
        infer::{Infer, normalize},
        layout::{self, ArrayLayout, Layout},
        oop::MethodRef,
        ty::{CallableType, Type},
    },
};

/// Where every part of a frame starts, and so the alignment of a frame: the
/// largest a value asks for.
pub const FRAME_ALIGN: u32 = 8;

/// What a part of `layout` takes in a frame: its size up to the next
/// multiple of [`FRAME_ALIGN`]. A part of no size, an instance of a block
/// with no variables, still takes one: the frame it is in must push
/// something to have a base.
pub fn slot(layout: Layout) -> u64 {
    layout::align_to(layout.size.max(1), FRAME_ALIGN)
}

/// The array each `ARRAY[*]` parameter of a body is bound to, in the copy of
/// the body emitted for that binding. Empty for any other body.
pub type Shapes<'db> = FxHashMap<VariableDecl<'db>, ArrayLayout>;

/// The bytes a call of `node` pushes, in its copy for `shapes`. A variadic
/// pack is elementary and passed by value, so every argument count has the
/// same frame.
pub fn frame<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: CallNode<'db>,
    shapes: &Shapes<'db>,
) -> u64 {
    parts(db, node, shapes)
        .into_iter()
        .map(slot)
        .fold(0, u64::saturating_add)
}

/// The parts of `node`'s frame, in no particular order.
pub fn parts<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: CallNode<'db>,
    shapes: &Shapes<'db>,
) -> Vec<Layout> {
    let scope = node.scope(db);
    let inference = scope.inference(db);
    let address_taken = layout::address_taken(db, scope);
    let (variables, result, statements) = match node {
        CallNode::Function(f) => (
            f.variables(db),
            f.return_type(db).map(|spec| (f.name(db), *spec)),
            f.statements(db),
        ),
        CallNode::Method(m) => (
            m.variables(db),
            m.return_type(db).map(|spec| (m.name(db), *spec)),
            m.stmts(db),
        ),
        CallNode::Body(fb) => (fb.variables(db), None, fb.statements(db)),
    };
    // An FB's body runs its statements; its members' initializers run once,
    // where the instance starts.
    let block = matches!(node, CallNode::Body(_));

    let mut parts = Vec::new();
    for var in variables {
        let ty = var.spec(db).infer(db);
        let addressed = address_taken.contains(&var.name(db));
        let own = match var.kind(db) {
            // The rest of a block's variables are its instance's.
            _ if block => var.is_temp(db) && (in_memory(db, ty) || addressed),
            // An address, and a pack passed by value.
            VariableKind::Output | VariableKind::InOut => false,
            VariableKind::Input if var.variadic(db) => false,
            // An aggregate arrives as the address of its caller's copy, and an
            // interface as the instance's.
            VariableKind::Input => match ty.normalize(db) {
                Type::Elementary(ElementarySpec::String) => true,
                Type::RefTo(_) | Type::Interface(_) => false,
                ty if aggregate(ty) => false,
                _ => addressed,
            },
            // The global's storage.
            VariableKind::External if matches!(node, CallNode::Function(_)) => false,
            _ => in_memory(db, ty) || addressed,
        };
        if own {
            parts.extend(layout::of_spec(db, var.spec(db)));
        }
    }
    if let Some((name, spec)) = result
        && (in_memory(db, spec.infer(db)) || address_taken.contains(&name))
    {
        parts.extend(layout::of_spec(db, spec));
    }

    for call in body_calls(db, node) {
        call_parts(db, inference, call, shapes, &mut parts);
    }

    // A STRING compared is passed to the comparison like an argument.
    let exprs: Vec<_> = if block {
        inference.statement_exprs().collect()
    } else {
        inference.typed_exprs().collect()
    };
    for (expr, _) in exprs {
        if let ExprKind::ComparisonOperator { left, right, .. } = expr.expr(db)
            && (is_string(inference.type_of_expr_adjusted(*left).normalize(db))
                || is_string(inference.type_of_expr_adjusted(*right).normalize(db)))
        {
            parts.extend(returned_string(db, inference, *left));
            parts.extend(returned_string(db, inference, *right));
        }
    }

    each_case(db, statements, &mut |selector| {
        parts.extend(case_selector(db, inference, selector, shapes));
    });
    parts
}

/// The calls a call of `node` makes: its statements', and a FUNCTION's or a
/// METHOD's initializers', which start its variables at each call. An FB's
/// members start from theirs once, where the instance does.
pub fn body_calls<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: CallNode<'db>,
) -> Vec<&'db ResolvedCall<'db>> {
    let inference = node.scope(db).inference(db);
    match node {
        CallNode::Body(_) => inference.statement_calls().map(|(_, call)| call).collect(),
        _ => inference.resolved_calls().map(|(_, call)| call).collect(),
    }
}

/// The copy of a body `call` creates: the callee, and the array bound to
/// each of its `ARRAY[*]` parameters, a forwarded one as the copy `context`
/// of the caller binds it. `None` for a call that creates no copy, and for
/// one whose arrays `context` does not bind.
pub fn copy_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    call: &ResolvedCall<'db>,
    context: &Shapes<'db>,
) -> Option<(CallNode<'db>, Shapes<'db>)> {
    let node = match call.callable {
        CallableType::Function(f) => CallNode::Function(f),
        CallableType::MethodDecl(MethodRef::Declared(m)) => CallNode::Method(m),
        CallableType::FunctionBlock(fb) => CallNode::Body(fb),
        CallableType::MethodDecl(MethodRef::Prototype(_)) => return None,
    };
    let mut shapes = Shapes::default();
    for (var, binding) in &call.params {
        if var.conformand(db).is_none() {
            continue;
        }
        let shape = match binding {
            ParamBinding::Values(values) => {
                argument_shape(db, inference, *values.first()?, context)
            }
            ParamBinding::Output { variable, .. } => shape_of(
                db,
                inference.type_of_variable_access_adjusted(*variable),
                context,
            ),
            ParamBinding::Default(_) | ParamBinding::Omitted => continue,
        };
        shapes.insert(*var, shape?);
    }
    (!shapes.is_empty()).then_some((node, shapes))
}

/// What one call needs of its caller's frame while it runs.
fn call_parts<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    call: &ResolvedCall<'db>,
    shapes: &Shapes<'db>,
    parts: &mut Vec<Layout>,
) {
    // A block's inputs and outputs are its instance's members, which the call
    // writes and reads.
    let block = matches!(call.callable, CallableType::FunctionBlock(_));
    // An external function's outputs come back on the wasm stack.
    let external =
        matches!(call.callable, CallableType::Function(f) if f.extern_pragma(db).is_some());
    for (var, binding) in &call.params {
        match binding {
            ParamBinding::Values(values) if !block && !by_address(db, *var) => {
                for value in values {
                    if !aggregate(var.spec(db).infer(db).normalize(db)) {
                        parts.extend(returned_string(db, inference, *value));
                        continue;
                    }
                    // The copy the callee reads, for the call to see the value
                    // the input had when it started.
                    parts.extend(match var.conformand(db) {
                        Some(_) => argument_shape(db, inference, *value, shapes).map(|s| s.whole),
                        None => layout::of_spec(db, var.spec(db)),
                    });
                }
            }
            // The output lands there, and its negation, or its bits, are
            // written into the destination once the call returns.
            ParamBinding::Output { variable, not } if block => {
                if *not {
                    parts.extend(layout::of_spec(db, var.spec(db)));
                } else {
                    parts.extend(bits_view(db, inference, *variable));
                }
            }
            ParamBinding::Output { variable, not } if !external => {
                if *not
                    || bits_view(db, inference, *variable).is_some()
                    || widens(db, inference, *var, *variable)
                {
                    parts.extend(layout::of_spec(db, var.spec(db)));
                }
            }
            // An output left unbound still needs somewhere to be written.
            ParamBinding::Omitted
                if !block && !external && var.is_output(db) && !var.variadic(db) =>
            {
                parts.extend(layout::of_spec(db, var.spec(db)));
            }
            _ => {}
        }
    }
}

/// Whether a value of `ty` is kept in linear memory wherever it is declared:
/// a STRING, an aggregate. Any other is a scalar, which a wasm local holds
/// unless its address is taken.
fn in_memory<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> bool {
    let ty = ty.normalize(db);
    is_string(ty) || aggregate(ty)
}

/// A normalized STRUCT, array or instance.
fn aggregate(ty: Type<'_>) -> bool {
    matches!(
        ty,
        Type::Struct(_)
            | Type::Array(_)
            | Type::ArrayConformand(_)
            | Type::FunctionBlock(_)
            | Type::Class(_)
    )
}

/// A normalized STRING.
fn is_string(ty: Type<'_>) -> bool {
    matches!(ty, Type::Elementary(ElementarySpec::String))
}

/// A parameter passed as an address: a VAR_IN_OUT, a VAR_OUTPUT, an
/// interface, an `ARRAY[*]` of any type.
fn by_address<'db>(db: &'db dyn WorkspaceDataBase, var: VariableDecl<'db>) -> bool {
    matches!(var.kind(db), VariableKind::InOut | VariableKind::Output)
        || matches!(var.spec(db).infer(db).normalize(db), Type::Interface(_))
        || var
            .conformand(db)
            .is_some_and(|conformand| conformand.of_type(db).is_none())
}

/// The call `expr` is, through parentheses.
fn direct_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
) -> Option<(Expr<'db>, FuncCall<'db>)> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => direct_call(db, *expr),
        ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(call)) => Some((expr, *call)),
        _ => None,
    }
}

/// The copy of a STRING a call returns straight into a by-value argument:
/// the result waits in its callee's storage, where a later argument, or the
/// call it is passed to, can write over it before it is read, `F(F(x))`.
fn returned_string<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    expr: Expr<'db>,
) -> Option<Layout> {
    let (expr, call) = direct_call(db, expr)?;
    if !is_string(inference.type_of_expr(expr).normalize(db)) {
        return None;
    }
    Some(layout::string(returned_capacity(db, inference, call)))
}

/// The capacity of the STRING `call` returns: what its callee declares.
fn returned_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    call: FuncCall<'db>,
) -> u64 {
    inference
        .resolved_call(call)
        .and_then(|resolved| resolved.callable.return_type(db))
        .and_then(|spec| normalize::string_capacity(db, *spec))
        .map_or(default_capacity(), u64::from)
}

fn default_capacity() -> u64 {
    u64::from(normalize::DEFAULT_STRING_CAPACITY)
}

/// The copy of a STRING CASE selector that a call computes, which every
/// label is compared with: the call runs once. A selector with no call
/// reads the same at every label, and is read there.
fn case_selector<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    selector: Expr<'db>,
    shapes: &Shapes<'db>,
) -> Option<Layout> {
    if !is_string(inference.type_of_expr_adjusted(selector).normalize(db)) {
        return None;
    }
    let span = selector.get_span(db);
    let has_call = inference.calls().any(|call| {
        let call = call.path(db).get_span(db);
        span.start_byte <= call.start_byte && call.end_byte <= span.end_byte
    });
    has_call.then(|| layout::string(read_capacity(db, inference, selector, shapes)))
}

/// The capacity of the STRING `expr` reads: what its callee declares for a
/// call, and for a path the declaration of the element, the field or the
/// target it ends at.
fn read_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    expr: Expr<'db>,
    shapes: &Shapes<'db>,
) -> u64 {
    if let Some((_, call)) = direct_call(db, expr) {
        return returned_capacity(db, inference, call);
    }
    let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) = expr.expr(db) else {
        return default_capacity();
    };
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        return default_capacity();
    };
    let Some(path) = begin.expr(db) else {
        return default_capacity();
    };
    let declared = match path.expr(db) {
        PathExprKind::Index(_) => {
            match inference.indexed_array(path).map(|indexed| indexed.array) {
                Some(IndexedType::Array(array)) => {
                    normalize::string_capacity(db, array.of_type(db)).map(u64::from)
                }
                Some(IndexedType::Conformand(conformand)) => shapes
                    .iter()
                    .find(|(var, _)| var.conformand(db) == Some(conformand))
                    .map(|(_, shape)| shape.element.size.saturating_sub(4)),
                None => None,
            }
        }
        PathExprKind::Field(_) => {
            normalize::declared_capacity_of(db, inference.type_of_path_expr(path))
                .or_else(|| {
                    let var = inference.variable_for_path_expr(path)?;
                    normalize::string_capacity(db, var.spec(db))
                })
                .map(u64::from)
        }
        PathExprKind::Deref(deref) => match inference.type_of_path_expr(deref.path).normalize(db) {
            Type::RefTo(target) => normalize::string_capacity(db, target).map(u64::from),
            _ => None,
        },
        PathExprKind::VarAccess(_) => None,
    };
    declared.unwrap_or_else(default_capacity)
}

/// The array a call binds to an `ARRAY[*]` input, `value` or the row of an
/// array it names (`m[i]`), as the copy for `shapes` sees a forwarded one.
fn argument_shape<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    value: Expr<'db>,
    shapes: &Shapes<'db>,
) -> Option<ArrayLayout> {
    if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) = value.expr(db)
        && let VariableAccessKind::Symbolic(begin) = access.kind(db)
        && let Some(row) = begin
            .expr(db)
            .and_then(|path| inference.indexed_array(path))
            .filter(|indexed| indexed.is_partial(db))
    {
        let whole = match row.array {
            IndexedType::Array(array) => layout::array_layout(db, array).clone()?,
            IndexedType::Conformand(conformand) => shapes
                .iter()
                .find(|(var, _)| var.conformand(db) == Some(conformand))
                .map(|(_, shape)| shape.clone())?,
        };
        return whole.row(row.through);
    }
    shape_of(db, inference.type_of_expr_adjusted(value), shapes)
}

/// The array a value of `ty` is: a forwarded `ARRAY[*]` parameter's, as the
/// copy binds it, or its own.
fn shape_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    shapes: &Shapes<'db>,
) -> Option<ArrayLayout> {
    if let Type::Variable((var, _)) = ty
        && let Some(shape) = shapes.get(&var)
    {
        return Some(shape.clone());
    }
    match ty.normalize(db) {
        Type::Array(array) => layout::array_layout(db, array).clone(),
        _ => None,
    }
}

/// What an output written into `destination` is received as when the
/// destination is bits of a wider address's cell, which have no address of
/// their own: `Q => %QX0.3` beside a `%QW0`. Whole bytes of the cell have
/// one, and are written like any variable.
fn bits_view<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    destination: VariableAccess<'db>,
) -> Option<Layout> {
    let ty = inference.type_of_variable_access(destination);
    let address = match destination.kind(db) {
        VariableAccessKind::Direct(dv) => LocatedAddress::of(db, dv),
        VariableAccessKind::Symbolic(_) => match ty {
            Type::Variable((var, _)) => {
                effective_location(db, var).and_then(|dv| LocatedAddress::of(db, dv))
            }
            _ => None,
        },
    }?;
    located_view(db, &address)?;
    let partial = destination.multibits(db);
    if address.width >= 8 && partial.is_none() {
        return None;
    }
    let spec = match partial {
        Some(multibits) => normalize::multibits_slice(db, multibits)?.spec,
        None => match ty {
            Type::Variable((var, _)) => lane(db, var.spec(db).infer(db))?,
            _ => width_spec(address.width),
        },
    };
    Some(layout::elementary(spec))
}

/// The bit string of an address's width.
fn width_spec(bits: u8) -> ElementarySpec {
    match bits {
        1 => ElementarySpec::Bool,
        8 => ElementarySpec::Byte,
        16 => ElementarySpec::Word,
        32 => ElementarySpec::DWord,
        _ => ElementarySpec::LWord,
    }
}

/// Whether an output is received apart and converted into its destination
/// once the call returns: a destination of another elementary type than the
/// output's, which the callee does not know to write.
fn widens<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    output: VariableDecl<'db>,
    destination: VariableAccess<'db>,
) -> bool {
    let from = match output.spec(db).infer(db).normalize(db) {
        Type::RefTo(_) | Type::Null => None,
        ty => lane(db, ty),
    };
    let to = lane(db, inference.type_of_variable_access_adjusted(destination));
    matches!((from, to), (Some(from), Some(to)) if from != to)
}

/// The elementary type a value of `ty` is held as: its own, an enumeration's
/// storage, a subrange's base, DINT for an address. `None` for a STRING or
/// an aggregate.
fn lane<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<ElementarySpec> {
    match ty.normalize(db) {
        Type::Elementary(ElementarySpec::String) => None,
        Type::Elementary(spec) => Some(spec),
        Type::Enum(e) => enum_lane(db, e),
        Type::EnumVariant(dt, _) => lane(db, Type::DataType(dt)),
        Type::RefTo(_) | Type::Null => Some(ElementarySpec::DInt),
        _ => None,
    }
}

fn enum_lane<'db>(db: &'db dyn WorkspaceDataBase, e: Enum<'db>) -> Option<ElementarySpec> {
    match e.typ(db) {
        None => Some(ElementarySpec::DInt),
        Some(base) => match base.infer(db).normalize(db) {
            Type::Elementary(spec) => Some(spec),
            _ => None,
        },
    }
}

/// The selector of every CASE in `statements`, nested ones included.
fn each_case<'db>(
    db: &'db dyn WorkspaceDataBase,
    statements: &[Stmt<'db>],
    f: &mut impl FnMut(Expr<'db>),
) {
    for stmt in statements {
        match stmt.stmt(db) {
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                f(*condition);
                for (_, body) in cases {
                    each_case(db, body, f);
                }
                each_case(db, else_.as_deref().unwrap_or_default(), f);
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                each_case(db, then.as_deref().unwrap_or_default(), f);
                for (_, body) in else_if {
                    each_case(db, body, f);
                }
                each_case(db, else_.as_deref().unwrap_or_default(), f);
            }
            StmtKind::For { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. } => each_case(db, body, f),
            _ => {}
        }
    }
}
