// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Evaluating an expression to a value known before the program runs.
//!
//! "Constant" in IEC is a semantic property, not a shape: `Constant_Expr :
//! Expression`, with the constraint that it evaluate at compile time. A CASE
//! label, an array bound and a TASK period all state that same requirement,
//! so they ask the same question here rather than each deciding for itself
//! what counts — which is how they came to disagree, a label MIR could not
//! fold aborting codegen on source `rk check` had called clean.
//!
//! One evaluator computes the value as the program would: each operation at
//! the type inference gives it, wrapping at that type's width, so
//! `200 * 200` is the INT -25536 wherever it is written. Its two entry
//! points differ in how a name binds:
//!
//! - [`const_int`] reads the binding the body's inference recorded, for an
//!   expression in a body: a CASE label, a FOR step, a subscript.
//! - [`spec_value`] binds a name through the scope chain's declarations, for
//!   an expression no body infers: a bound, a length, an initializer.
//!
//! A `CONSTANT`'s own initializer always folds the second way, in the scope
//! that declares it.

use db::WorkspaceDataBase;

use crate::{
    Qualifier,
    hir_def::{
        expressions::{
            expression::{
                AddOperatorKind, BooleanOperatorKind, Elementary, Expr, ExprKind, InitExprKind,
                MultOperatorKind, PrimaryExpr, UnaryOperatorKind, VariableAccess,
            },
            spec::{ElementarySpec, Spec, SpecKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};

/// The expression a `CONSTANT` declaration is fixed to, if it is one.
///
/// `VAR_EXTERNAL CONSTANT K : INT;` names a global and holds no value itself,
/// so the link is followed to the declaration that does. Anything not marked
/// `CONSTANT` yields `None` — an ordinary variable may be written between now
/// and whenever the value would be needed, so it is not knowable here.
pub fn constant_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    decl: VariableDecl<'db>,
) -> Option<Expr<'db>> {
    if !decl.qualifier(db).contains(Qualifier::CONSTANT) {
        return None;
    }
    let decl = if decl.is_external(db) {
        crate::hir_ty::index_graphs::external_var_lookup(db, decl.name(db))?
    } else {
        decl
    };
    match decl.init(db)?.kind(db) {
        InitExprKind::ConstantExpr(init) => Some(init),
        _ => None,
    }
}

/// The compile-time integer value of `expr` in a body, or `None` if it has
/// none. A name in it is the declaration the body bound it to.
pub fn const_int<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    body: &BodyInferenceResult<'db>,
) -> Option<i128> {
    // The binding HIR resolved for the access. NOT normalized: normalize peels
    // the `Variable` wrapper down to the underlying type, and the binding is
    // exactly what is needed.
    let bind = |va| match body.type_of_variable_access_with_adjustments(db, va) {
        Type::Variable((decl, None)) => Some(decl),
        _ => None,
    };
    fold(db, expr, &bind, &mut Folds::default()).map(|folded| folded.value)
}

/// An operation on constants whose result the type it runs at cannot hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Overflow {
    /// The result as written: `32767 + 1` is 32768.
    pub exact: i128,
    /// What the program computes, the result wrapped: -32768.
    pub held: i128,
    /// The type the operation runs at.
    pub ty: ElementarySpec,
}

/// The overflow of `expr` itself, when it is a `+`, `-`, `*`, `/`, `MOD` or
/// a minus sign on constants: `32767 + 1` at INT, `200 * 200` too. Its
/// operands fold as the program computes them. An operand in which an
/// operation wrapped already is that operation's overflow, and what follows
/// is not another; an untyped literal its type cannot hold is reported as a
/// literal (E0306).
///
/// `bind` is the declaration a name in it refers to: a body's binding, or
/// [`spec_name_binding`] in a declaration.
pub fn overflow<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    bind: &dyn Fn(VariableAccess<'db>) -> Option<VariableDecl<'db>>,
) -> Option<Overflow> {
    // An operand, or `None` when it does not fold or wrapped inside.
    let operand = |e| {
        let mut wrapped = false;
        let folded = fold_tracking(db, e, bind, &mut Folds::default(), &mut wrapped)?;
        (!wrapped).then_some(folded)
    };
    let fits = |operand: Folded, ty| {
        operand.ty.is_some() || integer_holds(operand.value, ty) == Some(true)
    };
    let (exact, ty) = match expr.expr(db) {
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (operand(*left)?, operand(*right)?);
            let ty = l.join(r);
            if !fits(l, ty) || !fits(r, ty) {
                return None;
            }
            let exact = match operator {
                AddOperatorKind::Plus => l.value.checked_add(r.value)?,
                AddOperatorKind::Minus => l.value.checked_sub(r.value)?,
            };
            (exact, ty)
        }
        ExprKind::MultOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (operand(*left)?, operand(*right)?);
            let ty = l.join(r);
            if !fits(l, ty) || !fits(r, ty) {
                return None;
            }
            let exact = match operator {
                MultOperatorKind::Mul => l.value.checked_mul(r.value)?,
                MultOperatorKind::Div => l.value.checked_div(r.value)?,
                MultOperatorKind::Mod => l.value.checked_rem(r.value)?,
            };
            (exact, ty)
        }
        // A minus on an untyped literal is part of the literal: `-32768`.
        ExprKind::UnaryOperator {
            expr,
            operator: UnaryOperatorKind::Minus,
        } => {
            let operand = operand(*expr)?;
            (operand.value.checked_neg()?, operand.ty?)
        }
        _ => return None,
    };
    let held = Folded::at(exact, ty)?.value;
    (held != exact).then_some(Overflow { exact, held, ty })
}

/// The compile-time integer value of a SPEC-context expression — an array or
/// subrange bound, an enum value, a STRING length, an initializer. These are
/// typed by INIT inference, not body inference, so a name binds through the
/// scope chain's declarations ([`spec_name_binding`]). It asks no inference
/// query, so this is callable from anywhere — a diagnostic message rendered
/// inside `infer_initialization` included.
pub fn spec_value<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> Option<i128> {
    fold(
        db,
        expr,
        &|va| spec_name_binding(db, va),
        &mut Folds::default(),
    )
    .map(|folded| folded.value)
}

/// [`spec_value`] in the 64 bits a bound is counted in. A ULINT above
/// `i64::MAX` is its bit pattern, as an unsigned subrange compares it and as
/// a radix literal always read; a value wider than 64 bits does not fold.
pub fn spec_bound<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> Option<i64> {
    let value = spec_value(db, expr)?;
    (i128::from(i64::MIN)..=i128::from(u64::MAX))
        .contains(&value)
        .then_some(value as i64)
}

/// `value` as a slot of type `ty` holds it, when `ty` is an integer type: a
/// CASE label as the selector compares it, a FOR step as the counter adds it
/// (`16#FFFF` on an INT is -1). Any other type keeps the value.
pub fn held_as<'db>(db: &'db dyn WorkspaceDataBase, value: i128, ty: Type<'db>) -> i128 {
    match ty.normalize(db) {
        Type::Elementary(spec) => Folded::at(value, spec).map_or(value, |folded| folded.value),
        _ => value,
    }
}

/// A folded integer and the type it is computed at.
#[derive(Clone, Copy)]
struct Folded {
    /// Exact: every integer type's values fit in 128 bits.
    value: i128,
    /// `None` for an untyped literal, which takes the type of what it meets.
    ty: Option<ElementarySpec>,
}

impl Folded {
    /// `value` computed at `ty`: wrapped to its width and read signed or
    /// unsigned, as the program holds it. `None` when `ty` is not an integer
    /// type.
    fn at(value: i128, ty: ElementarySpec) -> Option<Folded> {
        let (bits, signed) = integer_layout(ty)?;
        let low = value & ((1 << bits) - 1);
        let value = if signed && low >> (bits - 1) == 1 {
            low - (1 << bits)
        } else {
            low
        };
        Some(Folded {
            value,
            ty: Some(ty),
        })
    }

    /// Another value of the same type.
    fn with(self, value: i128) -> Option<Folded> {
        match self.ty {
            Some(ty) => Folded::at(value, ty),
            None => Some(Folded { value, ty: None }),
        }
    }

    /// The type an operation on two operands runs at, by the rule inference
    /// types it with (`resolve_expr_expecting`): two untyped literals make an
    /// INT, an untyped literal takes the other operand's type, and two types
    /// meet at the wider, the left one when neither widens to the other.
    fn join(self, other: Folded) -> ElementarySpec {
        match (self.ty, other.ty) {
            (None, None) => ElementarySpec::Int,
            (Some(ty), None) | (None, Some(ty)) => ty,
            (Some(l), Some(r)) => l.wider(r).unwrap_or(l),
        }
    }
}

/// Whether `ty` holds `value`, when `ty` is an integer type.
pub fn integer_holds(value: i128, ty: ElementarySpec) -> Option<bool> {
    let (min, max) = integer_range(ty)?;
    Some((min..=max).contains(&value))
}

/// The smallest and the largest value of an integer type, a bit string read
/// unsigned. `None` for any other type.
pub fn integer_range(ty: ElementarySpec) -> Option<(i128, i128)> {
    let (bits, signed) = integer_layout(ty)?;
    Some(if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    })
}

/// Width and signedness of an integer type; a bit string reads unsigned.
fn integer_layout(ty: ElementarySpec) -> Option<(u32, bool)> {
    use ElementarySpec::*;
    Some(match ty {
        SInt => (8, true),
        Int => (16, true),
        DInt => (32, true),
        LInt => (64, true),
        USInt | Byte => (8, false),
        UInt | Word => (16, false),
        UDInt | DWord => (32, false),
        ULInt | LWord => (64, false),
        _ => return None,
    })
}

/// A CONSTANT, or the value written for a named value (`Two := Step#One + 1`):
/// what a fold folds where it is written, wherever it is named.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Folding<'db> {
    Constant(VariableDecl<'db>),
    Value(Expr<'db>),
}

/// What one fold has met so far.
#[derive(Default)]
struct Folds<'db> {
    /// Those it is in the middle of. One reached again is a cycle, which does
    /// not fold: `k1 := k2; k2 := k1` recursed to a stack overflow.
    pending: Vec<Folding<'db>>,
    /// Those it has folded, each once however often it is named. Folded again
    /// at each name, a chain of values that each name the two before them took
    /// exponential time.
    done: rustc_hash::FxHashMap<Folding<'db>, Option<Folded>>,
}

impl<'db> Folds<'db> {
    /// What `what` is written as, folded. `None` when it does not fold, or
    /// when it is a cycle: the fold is already in the middle of it.
    fn written(&mut self, db: &'db dyn WorkspaceDataBase, what: Folding<'db>) -> Option<Folded> {
        if let Some(done) = self.done.get(&what) {
            return *done;
        }
        if self.pending.contains(&what) {
            return None;
        }
        let written = match what {
            Folding::Constant(decl) => constant_init(db, decl)?,
            Folding::Value(value) => value,
        };
        self.pending.push(what);
        // Its names bind where it is written, wherever it is named.
        let folded = fold(db, written, &|va| spec_name_binding(db, va), self);
        self.pending.pop();
        // Kept when it is `None` too. A value that came back to one in progress
        // is part of a cycle, or names one, and does not fold whichever is
        // folded first.
        self.done.insert(what, folded);
        folded
    }
}

fn fold<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    bind: &dyn Fn(VariableAccess<'db>) -> Option<VariableDecl<'db>>,
    folds: &mut Folds<'db>,
) -> Option<Folded> {
    fold_tracking(db, expr, bind, folds, &mut false)
}

fn fold_tracking<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    bind: &dyn Fn(VariableAccess<'db>) -> Option<VariableDecl<'db>>,
    folds: &mut Folds<'db>,
    // Set when an operation's exact result differs from what it holds.
    wrapped: &mut bool,
) -> Option<Folded> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(literal)) => {
            let (integer, ty) = match literal {
                Elementary::InferInteger(i) => (i, None),
                Elementary::SInt(i) => (i, Some(ElementarySpec::SInt)),
                Elementary::Int(i) => (i, Some(ElementarySpec::Int)),
                Elementary::DInt(i) => (i, Some(ElementarySpec::DInt)),
                Elementary::LInt(i) => (i, Some(ElementarySpec::LInt)),
                Elementary::USInt(i) => (i, Some(ElementarySpec::USInt)),
                Elementary::UInt(i) => (i, Some(ElementarySpec::UInt)),
                Elementary::UDInt(i) => (i, Some(ElementarySpec::UDInt)),
                Elementary::ULInt(i) => (i, Some(ElementarySpec::ULInt)),
                Elementary::Byte(i) => (i, Some(ElementarySpec::Byte)),
                Elementary::Word(i) => (i, Some(ElementarySpec::Word)),
                Elementary::DWord(i) => (i, Some(ElementarySpec::DWord)),
                Elementary::LWord(i) => (i, Some(ElementarySpec::LWord)),
                _ => return None,
            };
            let value = integer.as_i128(db).ok()?;
            match ty {
                Some(ty) => Folded::at(value, ty),
                None => Some(Folded { value, ty: None }),
            }
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            fold_tracking(db, *expr, bind, folds, wrapped)
        }
        ExprKind::UnaryOperator { expr, operator } => {
            let operand = fold_tracking(db, *expr, bind, folds, wrapped)?;
            match operator {
                UnaryOperatorKind::Plus => Some(operand),
                UnaryOperatorKind::Minus => noted(
                    operand.with(operand.value.wrapping_neg())?,
                    operand.value.checked_neg(),
                    wrapped,
                ),
                // Each bit flipped at the operand's type: `NOT WORD#16#0F0F` is
                // 16#F0F0. An untyped literal flips in 128 bits, which is its
                // pattern at whatever width it meets.
                UnaryOperatorKind::Not => operand.with(!operand.value),
            }
        }
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
            let decl = bind(*va)?;
            let value = folds.written(db, Folding::Constant(decl))?;
            // At its declared type, as the program stores it.
            Folded::at(value.value, declared_integer(db, decl.spec(db), 0)?)
        }
        // A name of a type with named values is its number, at the type's base.
        // A plain enum's has none.
        ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { name, variant }) => {
            let (values, base) = named_values_of(db, *name)?;
            let value = named_value(db, values, variant.ident(db), folds)?;
            Folded::at(i128::from(value), base)
        }
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (
                fold_tracking(db, *left, bind, folds, wrapped)?,
                fold_tracking(db, *right, bind, folds, wrapped)?,
            );
            let value = match operator {
                AddOperatorKind::Plus => l.value.wrapping_add(r.value),
                AddOperatorKind::Minus => l.value.wrapping_sub(r.value),
            };
            noted(Folded::at(value, l.join(r))?, Some(value), wrapped)
        }
        ExprKind::MultOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (
                fold_tracking(db, *left, bind, folds, wrapped)?,
                fold_tracking(db, *right, bind, folds, wrapped)?,
            );
            let value = match operator {
                MultOperatorKind::Mul => l.value.wrapping_mul(r.value),
                MultOperatorKind::Div => l.value.checked_div(r.value)?,
                MultOperatorKind::Mod => l.value.checked_rem(r.value)?,
            };
            noted(Folded::at(value, l.join(r))?, Some(value), wrapped)
        }
        // At the type the two meet at, as arithmetic is. Each bit of the result
        // comes from the two bits at its place, with no carry, so nothing wraps.
        ExprKind::BooleanOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (
                fold_tracking(db, *left, bind, folds, wrapped)?,
                fold_tracking(db, *right, bind, folds, wrapped)?,
            );
            let value = match operator {
                BooleanOperatorKind::And => l.value & r.value,
                BooleanOperatorKind::Or => l.value | r.value,
                BooleanOperatorKind::Xor => l.value ^ r.value,
            };
            Folded::at(value, l.join(r))
        }
        _ => None,
    }
}

/// `folded`, noting in `wrapped` when it is not `exact`, the result before
/// wrapping. An exact result past 128 bits is noted too.
fn noted(folded: Folded, exact: Option<i128>, wrapped: &mut bool) -> Option<Folded> {
    if exact != Some(folded.value) {
        *wrapped = true;
    }
    Some(folded)
}

/// The integer type a declaration holds: a subrange's base, a named type's
/// own spec. The name resolves as the signature resolves it, without asking
/// inference, so a fold stays callable while a signature is inferred.
fn declared_integer<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: Spec<'db>,
    depth: u32,
) -> Option<ElementarySpec> {
    use crate::hir_ty::resolver::name::{NameResolution, resolve_name};
    // A cyclic alias is refused elsewhere; stop regardless.
    if depth > 16 {
        return None;
    }
    match spec.kind(db) {
        SpecKind::Simple(ty) => integer_layout(*ty).map(|_| *ty),
        SpecKind::Subrange(sub) => declared_integer(db, sub._type(db), depth + 1),
        // A type with named values holds its base's.
        SpecKind::Enum(values) => declared_integer(db, values.typ(db)?, depth + 1),
        SpecKind::Target(target) => match resolve_name(db, &target.path, spec.scope_id(db)) {
            NameResolution::Pou(Pou::DataType(dt), _) => {
                declared_integer(db, dt.spec(db), depth + 1)
            }
            _ => None,
        },
        _ => None,
    }
}

/// The values a name like `Light#Red` is one of, and their base, when the type it
/// names was written with one. Resolved as the signature resolves it, as
/// [`declared_integer`] does.
fn named_values_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: crate::hir_def::expressions::expression::BeginPathExpr<'db>,
) -> Option<(crate::hir_def::expressions::spec::Enum<'db>, ElementarySpec)> {
    use crate::HirNodeInfo;
    use crate::hir_ty::resolver::name::{NameResolution, resolve_name};
    fn of_spec<'db>(
        db: &'db dyn WorkspaceDataBase,
        spec: Spec<'db>,
        depth: u32,
    ) -> Option<(crate::hir_def::expressions::spec::Enum<'db>, ElementarySpec)> {
        // A cyclic alias is refused elsewhere; stop regardless.
        if depth > 16 {
            return None;
        }
        match spec.kind(db) {
            SpecKind::Enum(values) => {
                Some((*values, declared_integer(db, values.typ(db)?, depth + 1)?))
            }
            SpecKind::Target(target) => match resolve_name(db, &target.path, spec.scope_id(db)) {
                NameResolution::Pou(Pou::DataType(dt), _) => of_spec(db, dt.spec(db), depth + 1),
                _ => None,
            },
            _ => None,
        }
    }
    if name.invocation(db).is_some() {
        return None;
    }
    let path = name.expr(db)?;
    let (access, _) = path.to_namespace_access(db).as_ref()?;
    match resolve_name(db, access, path.get_scope_id(db)) {
        NameResolution::Pou(Pou::DataType(dt), _) => of_spec(db, dt.spec(db), 0),
        _ => None,
    }
}

/// The value `variant` of `values` stands for, as [`enum_ordinals`] gives it: its
/// declared value, or the nearest one declared before it plus how far it comes
/// after that one. Only that value is folded, so a value may name the others of
/// its type. One that comes back to itself, through a value counted from it or
/// through another type, is a cycle.
fn named_value<'db>(
    db: &'db dyn WorkspaceDataBase,
    values: crate::hir_def::expressions::spec::Enum<'db>,
    variant: crate::hir_def::interned::identifier::Ident,
    folds: &mut Folds<'db>,
) -> Option<i64> {
    use crate::hir_ty::infer::Infer;
    let &(written, after) = counted_from(db, values).get(&variant)?;
    let after = i64::try_from(after).ok()?;
    let Some(value) = written else {
        // None written up to it: counted from 0.
        return Some(after);
    };
    let folded = folds.written(db, Folding::Value(value))?.value;
    let storage = values.typ(db).map(|spec| spec.infer(db));
    Some(ordinal(db, storage, folded)?.wrapping_add(after))
}

/// Each of a type's named values by its name: the value written nearest at or
/// before it, `None` when none is, and how far after that one it comes, from 0
/// when none is. The first of two values of one name is the one kept. A fold
/// asks it at each name: finding the value among the others each time made a
/// chain of values cubic to check.
#[salsa::tracked(returns(ref))]
fn counted_from<'db>(
    db: &'db dyn WorkspaceDataBase,
    values: crate::hir_def::expressions::spec::Enum<'db>,
) -> rustc_hash::FxHashMap<crate::hir_def::interned::identifier::Ident, (Option<Expr<'db>>, usize)>
{
    let mut counted = rustc_hash::FxHashMap::default();
    let mut written = None;
    for (at, declared) in values.variants(db).iter().enumerate() {
        if let Some(value) = declared.value {
            written = Some((value, at));
        }
        let from = match written {
            Some((value, start)) => (Some(value), at - start),
            None => (None, at),
        };
        counted.entry(declared.name.ident(db)).or_insert(from);
    }
    counted
}

/// The declaration a bare name in a SPEC bound refers to, resolved through
/// the scope chain's declaration maps, and an FB's or a CLASS's inherited
/// members after its own, as body resolution finds them — no inference
/// query, so this is callable from anywhere, a diagnostic message being
/// rendered inside `infer_initialization` included. A namespaced or
/// otherwise non-bare name yields `None` and the bound is refused as
/// non-constant.
pub fn spec_name_binding<'db>(
    db: &'db dyn WorkspaceDataBase,
    va: crate::hir_def::expressions::expression::VariableAccess<'db>,
) -> Option<crate::hir_def::pous::variable::VariableDecl<'db>> {
    use crate::HirNodeInfo;
    use crate::hir_def::expressions::expression::{PathExprKind, VarAccess, VariableAccessKind};
    use crate::hir_def::{scope::ScopeKind, semantic_index::get_scope};

    if va.multibits(db).is_some() {
        return None;
    }
    let VariableAccessKind::Symbolic(begin) = va.kind(db) else {
        return None;
    };
    if begin.invocation(db).is_some() {
        return None;
    }
    let path = begin.expr(db)?;
    let PathExprKind::VarAccess(VarAccess::Simple(span_ident)) = path.expr(db) else {
        return None;
    };
    let ident = span_ident.ident(db);

    let mut scope = Some(va.get_scope_id(db));
    while let Some(sc) = scope {
        if let Some(decl) = sc.def_map(db).global_variables.get(&ident) {
            return Some(*decl);
        }
        let scope_data = get_scope(db, sc);
        if let ScopeKind::Pou(pou @ (Pou::FunctionBlock(_) | Pou::Class(_))) = scope_data.kind
            && let Some(member) = crate::hir_ty::oop::instance_members(db, pou)
                .iter()
                .find(|member| member.var.name(db) == ident)
        {
            return Some(member.var);
        }
        scope = scope_data.parent;
    }
    None
}

/// The bare identifier a simple access names, if it is one — the same shape
/// [`spec_name_binding`] resolves, exposed for diagnostics that want to look
/// the name up elsewhere (the app-level global map) to say WHY it did not
/// resolve here.
pub(crate) fn bare_access_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    va: crate::hir_def::expressions::expression::VariableAccess<'db>,
) -> Option<crate::hir_def::interned::identifier::Ident> {
    use crate::hir_def::expressions::expression::{PathExprKind, VarAccess, VariableAccessKind};
    if va.multibits(db).is_some() {
        return None;
    }
    let VariableAccessKind::Symbolic(begin) = va.kind(db) else {
        return None;
    };
    if begin.invocation(db).is_some() {
        return None;
    }
    let path = begin.expr(db)?;
    let PathExprKind::VarAccess(VarAccess::Simple(span_ident)) = path.expr(db) else {
        return None;
    };
    Some(span_ident.ident(db))
}

/// Each variant of an enum with its ordinal: the declared value where one is
/// written, the previous ordinal plus one where not. `None` marks a declared
/// value that does not fold — the declaration check refuses it (E0604), so a
/// consumer reading ordinals afterwards may treat `None` as unreachable.
pub fn enum_ordinals<'db>(
    db: &'db dyn WorkspaceDataBase,
    enm: crate::hir_def::expressions::spec::Enum<'db>,
) -> Vec<(
    crate::hir_def::expressions::spec::EnumVariant<'db>,
    Option<i64>,
)> {
    use crate::hir_ty::infer::Infer;
    let storage = enm.typ(db).map(|spec| spec.infer(db));
    enum_ordinals_by(db, enm, |e| ordinal(db, storage, spec_value(db, e)?))
}

/// A value written for a named value, as its ordinal: as a declared storage
/// holds it, so `16#FF` on a SINT is -1, the bit pattern its check accepted,
/// and a ULINT above `i64::MAX` is its bit pattern too. Under the default DINT
/// a value stays as written, so E0605 sees one past the storage.
fn ordinal<'db>(
    db: &'db dyn WorkspaceDataBase,
    storage: Option<Type<'db>>,
    value: i128,
) -> Option<i64> {
    let value = storage.map_or(value, |ty| held_as(db, value, ty));
    (i128::from(i64::MIN)..=i128::from(u64::MAX))
        .contains(&value)
        .then_some(value as i64)
}

fn enum_ordinals_by<'db>(
    db: &'db dyn WorkspaceDataBase,
    enm: crate::hir_def::expressions::spec::Enum<'db>,
    fold: impl Fn(Expr<'db>) -> Option<i64>,
) -> Vec<(
    crate::hir_def::expressions::spec::EnumVariant<'db>,
    Option<i64>,
)> {
    let mut out = Vec::new();
    let mut next: i64 = 0;
    for variant in enm.variants(db).iter() {
        let value = match variant.value {
            Some(expr) => fold(expr),
            None => Some(next),
        };
        if let Some(v) = value {
            next = v.wrapping_add(1);
        }
        out.push((*variant, value));
    }
    out
}

/// A subrange's bounds, folded. `None` marks a bound that does not fold —
/// refused at the declaration (E0703), so a consumer reading bounds
/// afterwards may treat it as unreachable.
pub fn subrange_bounds<'db>(
    db: &'db dyn WorkspaceDataBase,
    subrange: crate::hir_def::expressions::spec::SubRange<'db>,
) -> (Option<i64>, Option<i64>) {
    (
        spec_bound(db, subrange.lower(db)),
        spec_bound(db, subrange.upper(db)),
    )
}

/// An array's dimensions, folded — `(lower, upper)` per dimension. `None`
/// marks a bound that does not fold, refused at the declaration
/// (E0501/E0502).
pub fn array_dimensions<'db>(
    db: &'db dyn WorkspaceDataBase,
    array: crate::hir_def::expressions::spec::Array<'db>,
) -> Vec<(Option<i64>, Option<i64>)> {
    array
        .subranges(db)
        .iter()
        .map(|(lo, hi)| (spec_bound(db, *lo), spec_bound(db, *hi)))
        .collect()
}

/// Follow a pure chain of `CONSTANT` references to the expression at its
/// end: `:= K` where `K : REAL := 2.5` resolves to the `2.5`. Resolution is
/// the SPEC rule — the scope chain's declaration maps, `VAR_EXTERNAL
/// CONSTANT` links followed — so a constant visible only through app-level
/// linkage must be declared external where it is used, the ordinary ST
/// idiom. Only BARE accesses substitute (arithmetic over a reference is
/// [`spec_value`]'s job), and a cycle answers `None` rather than looping.
pub fn resolve_constant_ref<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
) -> Option<Expr<'db>> {
    fn walk<'db>(
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
        visited: &mut Vec<VariableDecl<'db>>,
    ) -> Option<Expr<'db>> {
        let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = expr.expr(db) else {
            return Some(expr);
        };
        let decl = spec_name_binding(db, *va)?;
        if visited.contains(&decl) {
            return None;
        }
        visited.push(decl);
        walk(db, constant_init(db, decl)?, visited)
    }
    let out = walk(db, expr, &mut Vec::new())?;
    (out != expr).then_some(out)
}

/// Whether `expr` is a literal-shaped constant: something MIR lowers to a
/// constant with no name resolution — a numeric/bool/time literal (signed,
/// parenthesized), an enum value, a string literal.
fn is_literal_shaped<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(_)) => true,
        ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { .. }) => true,
        // References are exempt from the once-per-type rule on purpose: an
        // address is per-instance BY NATURE (`:= REF(member)` points into
        // whichever instance is being initialized), and `:= NULL` is as
        // constant as it gets. The nullability analysis is built on these
        // being legal member defaults.
        ExprKind::PrimaryExpr(PrimaryExpr::RefValue { .. }) => true,
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_literal_shaped(db, *expr)
        }
        ExprKind::UnaryOperator { expr, .. } => is_literal_shaped(db, *expr),
        _ => false,
    }
}

/// Whether `expr` is REAL arithmetic that folds: `+ - * / MOD **`, a sign
/// and parentheses over number literals and CONSTANTs that fold, with a REAL
/// or LREAL literal or CONSTANT in it (`1.5 * 2.0`, `KR / 4`). Integer
/// arithmetic is [`spec_value`]'s. The compiler lowers it as the program
/// would compute it, at the expression's own type, each CONSTANT replaced
/// by its value.
pub fn real_folds<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    let mut real = false;
    real_folds_guarded(db, expr, &mut Vec::new(), &mut real) && real
}

fn real_folds_guarded<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    visited: &mut Vec<VariableDecl<'db>>,
    real: &mut bool,
) -> bool {
    use crate::hir_def::expressions::expression::{Elementary, UnaryOperatorKind};
    use crate::hir_def::expressions::spec::ElementarySpec;
    use crate::hir_ty::infer::Infer;
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(literal)) => match literal {
            Elementary::Real(_) | Elementary::LReal(_) | Elementary::InferFloat(_) => {
                *real = true;
                true
            }
            _ => expr.as_const_int(db).is_some(),
        },
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            real_folds_guarded(db, *expr, visited, real)
        }
        ExprKind::UnaryOperator {
            expr,
            operator: UnaryOperatorKind::Minus | UnaryOperatorKind::Plus,
        } => real_folds_guarded(db, *expr, visited, real),
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            real_folds_guarded(db, *left, visited, real)
                && real_folds_guarded(db, *right, visited, real)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
            let Some(decl) = spec_name_binding(db, *va) else {
                return false;
            };
            if visited.contains(&decl) {
                return false;
            }
            let Some(init) = constant_init(db, decl) else {
                return false;
            };
            if matches!(
                decl.spec(db).infer(db).normalize(db),
                Type::Elementary(ElementarySpec::Real | ElementarySpec::LReal)
            ) {
                *real = true;
            }
            visited.push(decl);
            let folds = real_folds_guarded(db, init, visited, real);
            visited.pop();
            folds
        }
        _ => false,
    }
}

/// The first part of `expr` that keeps it from being a constant: a name
/// that does not fold, or a call. `None` when every name and call in it is
/// constant, or it has none.
pub fn non_constant_part<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
) -> Option<Expr<'db>> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_) | PrimaryExpr::FuncCall(_)) => {
            (!init_leaf_is_constant(db, expr)).then_some(expr)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr })
        | ExprKind::UnaryOperator { expr, .. } => non_constant_part(db, *expr),
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            non_constant_part(db, *left).or_else(|| non_constant_part(db, *right))
        }
        _ => None,
    }
}

/// The ONE acceptance test for a once-per-type initializer leaf (TYPE
/// defaults, FB/CLASS member defaults, static-host initializers): the check
/// refuses what this rejects, and MIR folds what it accepts — the two agree
/// BY CONSTRUCTION because they both call this.
///
/// Accepted: anything [`spec_value`] folds (integer arithmetic and `AND`,
/// `OR`, `XOR`, `NOT` over literals and CONSTANTs), REAL arithmetic
/// [`real_folds`] accepts, any literal-shaped value, and a pure
/// CONSTANT-reference chain ending in one.
pub fn init_leaf_is_constant<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    if spec_value(db, expr).is_some() || is_literal_shaped(db, expr) || real_folds(db, expr) {
        return true;
    }
    matches!(resolve_constant_ref(db, expr), Some(end) if is_literal_shaped(db, end))
}
