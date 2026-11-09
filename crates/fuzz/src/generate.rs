// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Well-typed programs, written with their expected results.
//!
//! Every other oracle checks that the compiler behaves; none checks that
//! the program it emits computes the right values. This one builds a
//! program from the fuzzer's bytes, evaluates it here with the semantics
//! the docs give, and writes the value of every variable, array element,
//! struct field and FB member after two scans into a header comment.
//! [`crate::semantics`] compiles the text, runs it, reads each one back
//! through the debug symbols and compares.
//!
//! What a program may hold, and the rule the evaluator follows for it:
//!
//! - Integers at every width: every result wraps at its type's width,
//!   division truncates, MOD takes the dividend's sign
//!   (docs/math-operations.md).
//! - REAL and LREAL: IEEE arithmetic in the type's own precision. Literals
//!   are dyadic (`n / 2^m`), written out exactly, so no parser can round
//!   them differently.
//! - STRING[n], ASCII only: a variable longer than its destination is cut
//!   to its capacity, a literal never is (that is a compile error), and
//!   comparison is byte-lexicographic (docs/strings.md).
//! - BOOL, arrays, and STRUCTs with defaults in their TYPE. An array's
//!   initializer writes its elements in order, at their index (`[i] := v`)
//!   or not at all, left at the default (docs/arrays.md).
//! - IF, CASE (integers and strings), FOR with EXIT and CONTINUE, RETURN.
//! - FUNCTIONs, some taking STRUCTs, some overloaded: an overload takes an
//!   earlier FUNCTION's name and parameters, with one retyped (a STRUCT for
//!   another STRUCT), added or dropped. Each argument has its parameter's
//!   exact type, which picks that overload (docs/overloading.md).
//! - FUNCTION_BLOCKs with inputs, outputs, state, a VAR_IN_OUT and METHODs
//!   that change their state; INTERFACEs, reached the way rk allows them,
//!   as a FUNCTION's VAR_IN_OUT (E1121).
//! - CLASSes with EXTENDS and OVERRIDE: `THIS.m()` reaches the instance's
//!   own version, `SUPER.m()` the base's (docs/monomorphized-oop.md). A
//!   method only calls methods declared before it, or its own base version,
//!   so no call chain loops.
//! - References: `REF()` of a variable, an array element or a field, and
//!   reads and writes through `^`. The evaluator keeps where each points,
//!   its index fixed when `REF()` was taken. A reference is set at the top
//!   of the body, before any use; not in its declaration, where rk leaves a
//!   PROGRAM variable NULL.
//! - `ARRAY[*]` parameters of FUNCTIONs, METHODs and FUNCTION_BLOCK bodies,
//!   bound to a program array, a two-dimensional one or one of its rows.
//!   Each walks its array between the bounds `LOWER_BOUND` and
//!   `UPPER_BOUND` read, forward or backward. It folds the elements, or
//!   rewrites them with a running total. A VAR_INPUT walks its own copy.
//!   The bound functions are the standard library's, written into the
//!   program, which is one file.
//!
//! - Recursive FUNCTIONs, METHODs and FUNCTION_BLOCK bodies, called at a
//!   literal depth of 0 to 4, which close their cycle directly, through a
//!   second FUNCTION, a local's initializer, an interface, a VAR_IN_OUT, a
//!   reference, `SUPER.m()` or `SUPER()`. Each call keeps a value of its own
//!   in memory, an array element, a STRING or a scalar it takes the address
//!   of, before calling itself, and reads it back after: calls sharing it
//!   compute another value.
//!
//! - Partial access of integers: a bit read as a BOOL (`p.%X3`, or bare
//!   `p.3`) or written (`p.%X3 := cond`), and a byte, word, double word or
//!   long word copied from one place into another (`a.%B1 := b.%B0`), on a
//!   variable, an array element, a field, a reference's target or an FB's
//!   member. A FUNCTION writes part of its own result after assigning it.
//!   The evaluator works on the two's-complement bits of the place's width.
//!
//! - A member of a FUNCTION_BLOCK or a CLASS named through `THIS` in its body
//!   and its methods, for reads, writes and partial access, one per block or
//!   class. `THIS.s.f0.%X3` is a bit of `f0`.
//!
//! - References compared, by where they point: `r = REF(x)`, `REF(x) <> r`,
//!   `REF(x) = REF(y)`, `r = q`, `r <> NULL`, a `REF()` in parentheses, on any
//!   type a program reference has.
//!
//! - Types that come back to themselves through a reference, by each route a
//!   name gives them, in a FUNCTION that goes round and says so
//!   ([`CYCLES`]).
//!
//! - `__TRY … __CATCH(e) … __FINALLY … __END_TRY`, the body raising a
//!   literal message after some statements: the ones after the raise are
//!   left out, the message is cut to the capacity of the STRING the
//!   `__CATCH` names, when it names one, and the handler runs. A jump out
//!   of the body skips the raise and the handler. The `__FINALLY`, when
//!   there is one, runs last either way, and holds no jump.
//!
//! Nothing may stop the module: an integer divisor is a literal other than
//! 0 and -1 (`DINT#-2147483648 / -1` traps), an index is wrapped into its
//! array's range by the expression itself, and loops have literal bounds.
//! A call that changes state (an FB, a method) is a statement of its own,
//! so no evaluation order is assumed. A `VAR_IN_OUT` is modelled as
//! copy-in, copy-out, the same thing here: nothing else runs meanwhile.

use std::fmt::Write as _;

/// The scans the header's values are taken after: two, so the second
/// starts from the state the first left.
pub const SCANS: u64 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ty {
    Bool,
    Sint,
    Int,
    Dint,
    Lint,
    Usint,
    Uint,
    Udint,
    Ulint,
    Real,
    Lreal,
    /// STRING[n].
    Str(u8),
    /// The STRUCT declared as `S{k}`.
    Struct(usize),
    /// The type with named values declared as `E{k}`, held as its base,
    /// `INTEGERS[base]`.
    Named(usize, u8),
}

const INTEGERS: [Ty; 8] = [
    Ty::Sint,
    Ty::Int,
    Ty::Dint,
    Ty::Lint,
    Ty::Usint,
    Ty::Uint,
    Ty::Udint,
    Ty::Ulint,
];

impl Ty {
    fn name(self) -> String {
        match self {
            Ty::Bool => "BOOL".into(),
            Ty::Sint => "SINT".into(),
            Ty::Int => "INT".into(),
            Ty::Dint => "DINT".into(),
            Ty::Lint => "LINT".into(),
            Ty::Usint => "USINT".into(),
            Ty::Uint => "UINT".into(),
            Ty::Udint => "UDINT".into(),
            Ty::Ulint => "ULINT".into(),
            Ty::Real => "REAL".into(),
            Ty::Lreal => "LREAL".into(),
            Ty::Str(n) => format!("STRING[{n}]"),
            Ty::Struct(k) => format!("S{k}"),
            Ty::Named(k, _) => format!("E{k}"),
        }
    }

    fn is_int(self) -> bool {
        INTEGERS.contains(&self)
    }

    fn is_float(self) -> bool {
        matches!(self, Ty::Real | Ty::Lreal)
    }

    /// The integer type a value of this one is held as: its own, or a named
    /// values type's base.
    fn held(self) -> Ty {
        match self {
            Ty::Named(_, base) => INTEGERS[base as usize],
            t => t,
        }
    }

    /// Whether `-x` is allowed.
    fn signed(self) -> bool {
        matches!(
            self.held(),
            Ty::Sint | Ty::Int | Ty::Dint | Ty::Lint | Ty::Real | Ty::Lreal
        )
    }

    fn bits(self) -> u32 {
        match self.held() {
            Ty::Sint | Ty::Usint => 8,
            Ty::Int | Ty::Uint => 16,
            Ty::Dint | Ty::Udint => 32,
            _ => 64,
        }
    }

    /// `v` wrapped into this integer type's range, as two's complement does.
    fn wrap(self, v: i128) -> i128 {
        if self == Ty::Bool {
            return (v != 0) as i128;
        }
        let modulus = 1i128 << self.bits();
        let low = v.rem_euclid(modulus);
        match self.signed() && low >= modulus / 2 {
            true => low - modulus,
            false => low,
        }
    }

    fn min(self) -> i128 {
        match self.signed() {
            true => -(1i128 << (self.bits() - 1)),
            false => 0,
        }
    }

    fn max(self) -> i128 {
        match self.signed() {
            true => (1i128 << (self.bits() - 1)) - 1,
            false => (1i128 << self.bits()) - 1,
        }
    }
}

/// A value of any type the generator writes.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    /// Every integer, and BOOL as 0 or 1.
    Int(i128),
    F32(f32),
    F64(f64),
    Str(Vec<u8>),
    Struct(Vec<Val>),
}

impl Val {
    fn int(&self) -> i128 {
        match self {
            Val::Int(v) => *v,
            _ => 0,
        }
    }

    fn truthy(&self) -> bool {
        self.int() != 0
    }
}

#[derive(Clone, Copy, Debug)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Clone, Copy, Debug)]
enum Cmp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Clone, Copy, Debug)]
enum Logic {
    And,
    Or,
    Xor,
}

/// Where a value lives.
#[derive(Clone, Debug)]
enum Place {
    /// A variable of the current frame: the program's, a FUNCTION's input,
    /// a FUNCTION_BLOCK's member or a METHOD's parameter.
    Var(usize),
    /// An element of a program array.
    Index(usize, Box<Expr>),
    /// A member of a program's FB instance, read from outside.
    Member(usize, usize),
    /// A field of a STRUCT-typed place.
    Field(Box<Place>, usize),
    /// What a program reference points to: `r^`.
    Deref(usize),
}

#[derive(Clone, Debug)]
enum Expr {
    Lit(Ty, Val),
    Read(Place),
    Arith(Arith, Ty, Box<Expr>, Box<Expr>),
    Neg(Ty, Box<Expr>),
    Cmp(Cmp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Logic(Logic, Box<Expr>, Box<Expr>),
    /// A FUNCTION, which has no state: callable anywhere.
    Call(usize, Vec<Expr>),
    /// A variadic FUNCTION's body: its pack, of this type, folded.
    Fold(Fold, Ty),
    /// A variadic FUNCTION's body: the argument of its pack, its second input,
    /// that its first selects.
    Element,
    /// Bit `n` of an integer place of type `ty`, as a BOOL: `p.%Xn`, or
    /// `p.n` when bare.
    Bit(Box<Place>, Ty, u32, bool),
    /// A FUNCTION of the library every program is read with
    /// ([`crate::session::LIBRARY`]), spelled one of its two ways.
    LibCall(LibFn, Vec<Expr>, u8),
    /// `a = b`, or `a <> b` when not set: two references, by where they point.
    SameRef(bool, RefSide, RefSide),
    /// `cycle{k}(n)`: goes round the type and answers `n + 1` ([`CYCLES`]).
    Cycle(usize, Box<Expr>),
    /// A CONSTANT, by its place in [`Decls::constants`].
    Const(usize),
    /// `E{k}#n{j}`: a named value, its number at the type's base.
    Named(usize, usize),
    /// `AND`, `OR` or `XOR` of two integers of type `ty`, bit by bit.
    Bits(Logic, Ty, Box<Expr>, Box<Expr>),
    /// `a ** b`, of two REALs or two LREALs.
    Pow(Box<Expr>, Box<Expr>),
}

/// One side of a comparison of references.
#[derive(Clone, Debug)]
enum RefSide {
    /// A program reference.
    Held(usize),
    /// `REF(place)`, in parentheses when set.
    Of(Place, bool),
    Null,
}

/// The FUNCTIONs of the library, each with its semantics here.
#[derive(Clone, Copy, Debug)]
enum LibFn {
    /// `Lib.Half(n)`: `n / 2`.
    Half,
    /// `Lib.Math.Clamp0(n)`: `n`, or 0 below it.
    Clamp0,
    /// `Lib.Flags.Both(a, b)`: `a AND b`.
    Both,
}

impl LibFn {
    fn params(self) -> &'static [Ty] {
        match self {
            LibFn::Half | LibFn::Clamp0 => &[Ty::Dint],
            LibFn::Both => &[Ty::Bool, Ty::Bool],
        }
    }

    /// How a call writes the name: under the library's name, or without
    /// it, which the root import resolves, a nested namespace under the
    /// root included.
    fn spelled(self, form: u8) -> &'static str {
        match (self, form % 2) {
            (LibFn::Half, 0) => "Lib.Half",
            (LibFn::Half, _) => "Half",
            (LibFn::Clamp0, 0) => "Lib.Math.Clamp0",
            (LibFn::Clamp0, _) => "Math.Clamp0",
            (LibFn::Both, 0) => "Lib.Flags.Both",
            (LibFn::Both, _) => "Flags.Both",
        }
    }
}

/// What a variadic FUNCTION folds its pack with: `+`, `-`, `*`, `/` and `%` from
/// left to right, `&`, `|` and `^` over BOOLs, a comparison of each
/// adjacent pair, the smallest or the largest integer, `**` from the left over
/// REALs and LREALs (docs/variadics.md).
#[derive(Clone, Copy, Debug)]
enum Fold {
    Arith(Arith),
    Logic(Logic),
    Cmp(Cmp),
    Min,
    Max,
    Power,
}

impl Fold {
    fn symbol(self) -> &'static str {
        match self {
            Fold::Arith(Arith::Add) => "+",
            Fold::Arith(Arith::Sub) => "-",
            Fold::Arith(Arith::Mul) => "*",
            Fold::Arith(Arith::Div) => "/",
            Fold::Arith(Arith::Mod) => "%",
            Fold::Logic(Logic::And) => "&",
            Fold::Logic(Logic::Or) => "|",
            Fold::Logic(Logic::Xor) => "^",
            Fold::Cmp(Cmp::Eq) => "=",
            Fold::Cmp(Cmp::Ne) => "<>",
            Fold::Cmp(Cmp::Lt) => "<",
            Fold::Cmp(Cmp::Gt) => ">",
            Fold::Cmp(Cmp::Le) => "<=",
            Fold::Cmp(Cmp::Ge) => ">=",
            Fold::Min => " MIN",
            Fold::Max => " MAX",
            Fold::Power => "**",
        }
    }
}

#[derive(Clone, Debug)]
enum Label {
    /// A number, written as itself or as the constant expression that folds
    /// to it.
    Int(i128, Option<Expr>),
    /// `a..b`, each end written as itself or as a constant expression.
    Range(i128, i128, Option<(Expr, Expr)>),
    Str(Vec<u8>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Assign(Place, Expr),
    /// `a := b += e`, the places as written, each with its operator, `None`
    /// for `:=`: the last is written the value, then each other what the
    /// place after it holds, or its operation over that.
    Chain(Vec<(Place, Option<Arith>)>, Expr),
    /// `x += e`, `x -= e`, `x *= e`, `x /= e`: the place read and written.
    Compound(Place, Arith, Expr),
    /// An FB instance called with its inputs, and the variable bound to
    /// its `VAR_IN_OUT` if it has one.
    CallFb(usize, Vec<Expr>, Option<usize>),
    /// `target := instance.method(args)`.
    CallMethod(Place, usize, usize, Vec<Expr>),
    /// `target := user(dev := instance, args)`: a FUNCTION that calls a
    /// method through its interface parameter.
    CallUser(Place, usize, usize, Vec<Expr>),
    /// `target := object.m{j}(args)`.
    CallObject(Place, usize, usize, Vec<Expr>),
    /// `target := THIS.m{i}(args)`, or `SUPER.m{i}` when set, in a class
    /// method.
    CallThis(Place, usize, Vec<Expr>, bool),
    /// `r := REF(place)`.
    SetRef(usize, Place),
    /// A walker called on an array.
    Walk(WalkCall),
    /// A recursive body called with a depth.
    Recurse(Recurse),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    Case(Expr, Vec<(Vec<Label>, Vec<Stmt>)>, Option<Vec<Stmt>>),
    /// `FOR v := from TO to BY step`, the step written as itself or as the
    /// constant expression that folds to it.
    For(usize, i128, i128, i128, Option<Expr>, Vec<Stmt>),
    /// `p.%Xn := cond`, or `p.n := cond` when bare, on an integer place of
    /// type `ty`.
    SetBit(Place, Ty, u32, bool, Expr),
    /// `to.%B1 := from.%B0`: the slice of `width` bits at offset `from_off`
    /// of `from` copied over the one at `to_off` of `to`, each offset counted
    /// in widths.
    MoveSlice(Slice, Slice, u32),
    Exit,
    Continue,
    Return,
    /// `__TRY body; __RAISE('message'); __CATCH(variable) handler __END_TRY`.
    Try(Try),
}

/// A `__TRY` whose body ends on a raise.
#[derive(Clone, Debug)]
struct Try {
    body: Vec<Stmt>,
    message: Vec<u8>,
    /// The STRING variable of the frame the `__CATCH` names, when it does.
    variable: Option<usize>,
    handler: Vec<Stmt>,
    finally: Option<Vec<Stmt>>,
}

/// An integer place, its type and a slice offset of it, in widths.
#[derive(Clone, Debug)]
struct Slice {
    place: Place,
    ty: Ty,
    offset: u32,
}

/// What a FUNCTION writes into part of its result, after `f := body`.
#[derive(Clone, Debug)]
enum ResultSlice {
    /// `f.%Xn := cond`, or `f.n := cond` when bare: a BOOL over the inputs.
    Bit(u32, bool, Expr),
    /// `f.%B1 := a0.%B0`: a slice of an integer input, `width` bits.
    Move {
        to_off: u32,
        input: usize,
        from_off: u32,
        width: u32,
    },
}

/// The letter a slice of `width` bits is written with: `%B1`, `%W0`.
fn width_letter(width: u32) -> char {
    match width {
        8 => 'B',
        16 => 'W',
        32 => 'D',
        _ => 'L',
    }
}

/// The bits of an integer value of type `ty`, as two's complement holds
/// them in the type's width.
fn raw(ty: Ty, v: &Val) -> u128 {
    v.int().rem_euclid(1i128 << ty.bits()) as u128
}

/// The value of type `ty` whose bits are `bits`.
fn from_raw(ty: Ty, bits: u128) -> Val {
    Val::Int(ty.wrap(bits as i128))
}

/// `width` bits set.
fn mask(width: u32) -> u128 {
    (1u128 << width) - 1
}

/// `value` with the slice of `width` bits at `offset` (in widths) replaced
/// by `slice`.
fn put_slice(value: u128, offset: u32, width: u32, slice: u128) -> u128 {
    let shift = offset * width;
    (value & !(mask(width) << shift)) | ((slice & mask(width)) << shift)
}

/// The slice of `width` bits at `offset` (in widths) of `value`.
fn get_slice(value: u128, offset: u32, width: u32) -> u128 {
    (value >> (offset * width)) & mask(width)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// A program variable, or an FB's state.
    Plain,
    /// Out of reach: a FOR counter outside its loop (its value after the
    /// loop is the implementation's, so it is not compared either), or an
    /// FB's in-out seen from a METHOD (bound only during the block's own
    /// call).
    Hidden,
    /// An input: read, never assigned.
    Input,
    Output,
    InOut,
}

#[derive(Clone)]
struct Var {
    name: String,
    ty: Ty,
    init: Val,
    role: Role,
}

/// How an element of a program array is written in its initializer.
#[derive(Clone, Copy)]
enum Written {
    /// In order, after the one before it.
    Next,
    /// At its index, `[i] := v`.
    AtIndex,
    /// Not at all: it keeps the type's default, and the next one written
    /// is at its index.
    Default,
}

struct Array {
    name: String,
    ty: Ty,
    lo: i128,
    /// The bounds written as constant expressions that fold to them, or as
    /// themselves when not set.
    bounds: Option<(Expr, Expr)>,
    init: Vec<Val>,
    /// Each element, as the initializer writes it.
    written: Vec<Written>,
}

struct Function {
    name: String,
    ty: Ty,
    inputs: Vec<Var>,
    body: Expr,
    /// Parts of an integer result written after `body` is assigned.
    slices: Vec<ResultSlice>,
    /// The last input is a pack, which `body` reads: a call passes it any
    /// number of arguments, positionally.
    variadic: bool,
}

#[derive(Clone)]
struct Signature {
    name: String,
    ty: Ty,
    params: Vec<Var>,
}

struct Interface {
    name: String,
    methods: Vec<Signature>,
}

struct Method {
    sig: Signature,
    body: Vec<Stmt>,
    result: Expr,
}

struct Block {
    name: String,
    /// Inputs first, then outputs, the in-out, and the state.
    members: Vec<Var>,
    body: Vec<Stmt>,
    implements: Option<usize>,
    /// Its body and methods name its members through `THIS`.
    through_this: bool,
    /// In the interface's order when it implements one.
    methods: Vec<Method>,
}

impl Block {
    fn role(&self, role: Role) -> Vec<usize> {
        (0..self.members.len())
            .filter(|&m| self.members[m].role == role)
            .collect()
    }

    fn inputs(&self) -> Vec<usize> {
        self.role(Role::Input)
    }

    fn inout(&self) -> Option<usize> {
        self.role(Role::InOut).first().copied()
    }
}

/// A FUNCTION that takes an interface as its `VAR_IN_OUT dev` and returns
/// what one of its methods returns.
struct User {
    name: String,
    interface: usize,
    method: usize,
    inputs: Vec<Var>,
    /// The method's arguments, over the inputs.
    args: Vec<Expr>,
}

struct Instance {
    name: String,
    block: usize,
}

struct Class {
    name: String,
    parent: Option<usize>,
    /// Its methods name the variables they see through `THIS`.
    through_this: bool,
    /// Its own variables; the inherited ones come first in an instance.
    vars: Vec<Var>,
    /// Every method it has, `m{j}`: its parent's, then its own.
    families: Vec<Signature>,
    /// The ones it defines, a new method or an OVERRIDE.
    defs: Vec<Option<ClassDef>>,
}

struct ClassDef {
    overrides: bool,
    body: Vec<Stmt>,
    result: Expr,
}

struct Object {
    name: String,
    class: usize,
}

struct Reference {
    name: String,
    target: Ty,
}

/// A program array of two dimensions. Only `ARRAY[*]` parameters reach
/// it, whole or one row at a time.
struct Matrix {
    name: String,
    ty: Ty,
    /// Each dimension's lower bound.
    lo: [i128; 2],
    /// How many rows, and how many elements in a row.
    len: [usize; 2],
    /// Row by row.
    init: Vec<Val>,
    /// Each element, as the initializer writes it.
    written: Vec<Written>,
}

/// What a call binds to an `ARRAY[*]` parameter.
#[derive(Clone, Copy, Debug)]
enum Bound {
    Array(usize),
    Matrix(usize),
    /// A row of a matrix: `mat0[2]`.
    Row(usize, i128),
}

/// Where a walker's code is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Host {
    /// A FUNCTION, whose result starts from its default at each call.
    Function,
    /// A FUNCTION_BLOCK's body, its `ARRAY[*]` a VAR_IN_OUT and its result
    /// the output `acc`, kept between calls.
    Block,
    /// A CLASS's METHOD `run`, which returns the class variable `acc`,
    /// kept between calls.
    Method,
}

/// Code over an `ARRAY[*] OF T` parameter `a`, which visits every element
/// between the bounds `LOWER_BOUND` and `UPPER_BOUND` read. It folds the
/// elements into `acc`, or rewrites them with a running total that starts
/// from its input `k` and counts them in `acc`.
struct Walker {
    name: String,
    host: Host,
    elem: Ty,
    rank: usize,
    /// A VAR_IN_OUT; a VAR_INPUT walks its own copy.
    in_out: bool,
    /// The field of a STRUCT element it reads and writes.
    field: Option<usize>,
    writes: bool,
    /// From each upper bound down.
    backward: bool,
    /// With two dimensions, the second in the outer loop.
    transposed: bool,
    /// An earlier FUNCTION walker that `a` is passed on to first, whose
    /// result `acc` starts from.
    pass: Option<usize>,
}

impl Walker {
    /// The type of what it reads and writes in each element.
    fn view(&self, d: &Decls) -> Ty {
        match (self.elem, self.field) {
            (Ty::Struct(k), Some(f)) => d.structs[k][f].ty,
            (t, _) => t,
        }
    }

    /// The type of `acc`: what the walk folds, or a DINT count.
    fn result(&self, d: &Decls) -> Ty {
        match self.writes {
            true => Ty::Dint,
            false => self.view(d),
        }
    }
}

/// A program's instance of a FUNCTION_BLOCK or CLASS walker.
struct WalkerInstance {
    name: String,
    walker: usize,
}

/// A walker called on an array: `target := walk0(a := arr0, k := 1)` for a
/// FUNCTION, `w0(a := arr0)` for a block instance, `target :=
/// w1.run(a := mat0[2])` for an object. It writes the array it is bound
/// to, so it is a statement of its own.
#[derive(Clone, Debug)]
struct WalkCall {
    target: Option<Place>,
    walker: usize,
    /// The block instance or object, for a walker that is not a FUNCTION.
    instance: Option<usize>,
    bound: Bound,
    /// The argument for `k`, when the walker writes.
    k: Option<Expr>,
}

/// How a recursive body reaches itself again: the call the call graph has
/// to find for each call's storage to be its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Route {
    /// `rec{k}` calls itself.
    Direct,
    /// `rec{k}` calls `hop{k}`, which calls it back.
    Hop,
    /// The call is the initializer of `rec{k}`'s local `below`, through
    /// `hop{k}`, which stops at depth 0.
    Initializer,
    /// `climb` of CLASS `rb{k}` calls `THIS.climb`, which `rd{k}` overrides
    /// with `SUPER.climb`: on an `rd{k}`, the base's method is on a cycle.
    Super,
    /// `rec{k}` calls `dev.climb` through its interface VAR_IN_OUT, and the
    /// CLASS `rc{k}` implementing it calls `rec{k}(dev := THIS)`.
    Interface,
    /// The body of FB `node{k}` calls `visit{k}`, which calls the instance
    /// bound to its VAR_IN_OUT, bound to itself: what each call keeps is a
    /// VAR_TEMP.
    InOut,
    /// The body of FB `node{k}` calls `next^()`, a reference to itself.
    Ref,
    /// The body of FB `base{k}` calls `kick{k}`, whose `derived{k}`
    /// instance's body calls `SUPER()`.
    SuperBody,
}

/// What a recursive body writes before it calls itself and reads after:
/// shared between the calls, each would read the one below's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Keep {
    /// Element `at` of `keep : ARRAY[0..len - 1] OF DINT`.
    Array { len: usize, at: usize },
    /// `keep : STRING[cap]`, 'even' or 'odd' after the call's value.
    Text { cap: u8 },
    /// `own : DINT`, read back through `r := REF(own)`.
    Addressed,
}

/// A recursive body `rec{k}`, called with a depth `n` and a `seed`. Each
/// call keeps `seed + n`, calls itself with `n - 1` and `seed * mul + add`
/// while `n > 0`, and returns what came back times `factor`, plus what it
/// kept.
struct Recursion {
    route: Route,
    keep: Keep,
    factor: i128,
    mul: i128,
    add: i128,
}

/// `target := rec{k}(n := depth, seed := ...)`, or `ro{k}.climb(...)`. The
/// depth is a literal, so every recursion ends.
#[derive(Clone, Debug)]
struct Recurse {
    target: Place,
    recursion: usize,
    depth: i128,
    seed: Expr,
}

/// A type with named values, `E{k} : base (n0 := ..., n1, ...)`.
struct Named {
    base: Ty,
    /// What is written for each value, when anything is: a value with
    /// nothing written is the one before it plus one, or 0 when it is first.
    written: Vec<Option<Expr>>,
    /// Each value's number, as the base holds it.
    numbers: Vec<i128>,
}

/// A CONSTANT: `k{n}` of the PROGRAM, or `gk{n}` of the CONFIGURATION,
/// which a POU naming it declares `VAR_EXTERNAL CONSTANT`.
struct Constant {
    name: String,
    ty: Ty,
    expr: Expr,
    value: Val,
    global: bool,
}

/// A `VAR_GLOBAL` of the CONFIGURATION, `gv{n}`, started by a constant
/// expression. Nothing writes it, so it ends as it starts.
struct Global {
    name: String,
    ty: Ty,
    expr: Expr,
    value: Val,
}

/// A program's declarations, which the evaluator and the printer read.
#[derive(Default)]
struct Decls {
    /// The types with named values, `E{k}`.
    named: Vec<Named>,
    /// The CONSTANTs, the CONFIGURATION's and the PROGRAM's.
    constants: Vec<Constant>,
    globals: Vec<Global>,
    /// Initial values written as constant expressions: a program variable's,
    /// by its place in the program's variables; a field's of `S{k}`, by
    /// `(k, field)`; a member's of `fb{b}`, by `(b, member)`.
    var_inits: Vec<(usize, Expr)>,
    field_inits: Vec<((usize, usize), Expr)>,
    member_inits: Vec<((usize, usize), Expr)>,
    /// Which of [`CYCLES`] each `cycle{k}` goes round.
    cycles: Vec<usize>,
    /// The fields of `S{k}`.
    structs: Vec<Vec<Var>>,
    functions: Vec<Function>,
    interfaces: Vec<Interface>,
    blocks: Vec<Block>,
    users: Vec<User>,
    arrays: Vec<Array>,
    instances: Vec<Instance>,
    classes: Vec<Class>,
    objects: Vec<Object>,
    refs: Vec<Reference>,
    matrices: Vec<Matrix>,
    walkers: Vec<Walker>,
    walker_instances: Vec<WalkerInstance>,
    recursions: Vec<Recursion>,
}

impl Decls {
    /// A class's variables, the inherited ones first.
    fn class_vars(&self, c: usize) -> Vec<Var> {
        let class = &self.classes[c];
        let mut vars = class.parent.map(|p| self.class_vars(p)).unwrap_or_default();
        vars.extend(class.vars.iter().cloned());
        vars
    }

    /// The class whose definition of `m{j}` an instance of `c` runs.
    fn resolve(&self, mut c: usize, j: usize) -> usize {
        loop {
            if self.classes[c].defs[j].is_some() {
                return c;
            }
            c = self.classes[c]
                .parent
                .expect("a method is defined where it is declared");
        }
    }
}

/// The fuzzer's bytes as a stream of choices; past the end, every choice
/// is the first one, so any input makes a finite program.
struct Choices<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Choices<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.bytes.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        b
    }

    fn below(&mut self, n: usize) -> usize {
        match n {
            0 | 1 => 0,
            _ => (u16::from_le_bytes([self.byte(), self.byte()]) as usize) % n,
        }
    }

    fn percent(&mut self, p: u8) -> bool {
        self.byte() % 100 < p
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(std::array::from_fn(|_| self.byte()))
    }
}

struct Generator<'a> {
    choices: Choices<'a>,
    /// The variables of the frame being written: the program's, or a
    /// FUNCTION's, FB's or METHOD's while its body is.
    vars: Vec<Var>,
    /// False while a FUNCTION, FB or METHOD body is written: those see
    /// neither the program's arrays, its instances nor other FUNCTIONs.
    in_program: bool,
    d: Decls,
    /// Statements left to write, so a program stays small.
    budget: usize,
    /// How many indexes are being written, one inside the other: an index
    /// reads no array element past the first level.
    indexing: u32,
    /// The class method being written, for its THIS and SUPER calls.
    this: Option<ThisCtx>,
}

/// A class method being written: the class's methods, which one this is,
/// and how many its parent has.
#[derive(Clone)]
struct ThisCtx {
    families: Vec<Signature>,
    family: usize,
    parent_families: usize,
}

impl Generator<'_> {
    fn integer(&mut self) -> Ty {
        INTEGERS[self.choices.below(INTEGERS.len())]
    }

    /// A type a variable can have: a number, a BOOL, a string, and when
    /// `structs` is set, a declared STRUCT.
    fn var_type(&mut self, structs: bool) -> Ty {
        match self.choices.below(10) {
            0 => Ty::Bool,
            1 => [Ty::Real, Ty::Lreal][self.choices.below(2)],
            2 => Ty::Str(1 + self.choices.below(8) as u8),
            3 if structs && !self.d.structs.is_empty() => {
                Ty::Struct(self.choices.below(self.d.structs.len()))
            }
            _ => self.integer(),
        }
    }

    /// A type an expression can have: anything but a STRUCT.
    fn scalar_type(&mut self) -> Ty {
        self.var_type(false)
    }

    fn literal(&mut self, ty: Ty) -> Val {
        match ty {
            Ty::Bool => Val::Int((self.choices.byte() & 1) as i128),
            Ty::Real => Val::F32(self.dyadic() as f32),
            Ty::Lreal => Val::F64(self.dyadic()),
            Ty::Str(cap) => {
                let len = self.choices.below(cap as usize + 1);
                Val::Str((0..len).map(|_| b"abcde"[self.choices.below(5)]).collect())
            }
            Ty::Struct(_) => self.default_of(ty),
            Ty::Named(k, _) => {
                let numbers = &self.d.named[k].numbers;
                Val::Int(numbers[self.choices.below(numbers.len())])
            }
            _ => Val::Int(match self.choices.below(8) {
                0 => ty.min(),
                1 => ty.max(),
                2 => 0,
                3 => 1,
                4 if ty.signed() => -1,
                5 | 6 => ty.wrap((self.choices.byte() % 20) as i128 - 10),
                _ => ty.wrap(self.choices.u64() as i128),
            }),
        }
    }

    /// `n / 2^m` with `|n| <= 2^15` and `m <= 8`: exact in a REAL, and in
    /// the decimal it is written as.
    fn dyadic(&mut self) -> f64 {
        let n = (self.choices.below(1 << 16) as i64) - (1 << 15);
        let m = self.choices.below(9) as i32;
        n as f64 / 2f64.powi(m)
    }

    fn default_of(&self, ty: Ty) -> Val {
        match ty {
            Ty::Struct(k) => {
                Val::Struct(self.d.structs[k].iter().map(|f| f.init.clone()).collect())
            }
            // A type with named values starts at its first.
            Ty::Named(k, _) => Val::Int(self.d.named[k].numbers[0]),
            t => zero(t),
        }
    }

    /// A constant expression of type `ty`, an integer type, REAL or LREAL:
    /// typed literals, the CONSTANTs among `visible` of that type and the
    /// named values held as `ty`, under arithmetic, `AND`, `OR` and `XOR` on
    /// integers, and `**` on reals. An integer divisor is a literal other
    /// than 0.
    fn constant(&mut self, ty: Ty, depth: u32, visible: &[usize]) -> Expr {
        if depth == 0 || self.choices.percent(35) {
            let constants: Vec<usize> = visible
                .iter()
                .copied()
                .filter(|&c| self.d.constants[c].ty == ty)
                .collect();
            let named = self.named_values(ty);
            return match self.choices.below(3) {
                0 if !constants.is_empty() => {
                    Expr::Const(constants[self.choices.below(constants.len())])
                }
                1 if !named.is_empty() => {
                    let (k, j) = named[self.choices.below(named.len())];
                    Expr::Named(k, j)
                }
                _ => Expr::Lit(ty, self.literal(ty)),
            };
        }
        let d = depth - 1;
        let operand = |g: &mut Self| Box::new(g.constant(ty, d, visible));
        if ty.is_float() {
            return match self.choices.below(6) {
                0 => Expr::Pow(operand(self), operand(self)),
                1 => Expr::Neg(ty, operand(self)),
                _ => {
                    let op =
                        [Arith::Add, Arith::Sub, Arith::Mul, Arith::Div][self.choices.below(4)];
                    Expr::Arith(op, ty, operand(self), operand(self))
                }
            };
        }
        match self.choices.below(8) {
            0 if ty.signed() => Expr::Neg(ty, operand(self)),
            1 | 2 => {
                let op = [Logic::And, Logic::Or, Logic::Xor][self.choices.below(3)];
                Expr::Bits(op, ty, operand(self), operand(self))
            }
            3 => {
                let op = [Arith::Div, Arith::Mod][self.choices.below(2)];
                let divisor = self.divisor(ty);
                Expr::Arith(op, ty, operand(self), Box::new(Expr::Lit(ty, divisor)))
            }
            _ => {
                let op = [Arith::Add, Arith::Sub, Arith::Mul][self.choices.below(3)];
                Expr::Arith(op, ty, operand(self), operand(self))
            }
        }
    }

    /// Every named value held as `ty`, as `(type, value)`: what a constant of
    /// that type may name.
    fn named_values(&self, ty: Ty) -> Vec<(usize, usize)> {
        self.d
            .named
            .iter()
            .enumerate()
            .filter(|(_, n)| n.base == ty)
            .flat_map(|(k, n)| (0..n.numbers.len()).map(move |j| (k, j)))
            .collect()
    }

    /// The value of a constant expression.
    fn folded(&self, ty: Ty, e: &Expr) -> Val {
        store(ty, eval(e, &State::frame(Vec::new()), &self.d))
    }

    /// `E{k}`: two to five values held as an integer base. A value is
    /// counted from the one before it, or written: a constant expression of
    /// the base over literals, its type's values before it and the values
    /// of the types before it of the same base.
    fn named_type(&mut self) {
        let base = self.choices.below(INTEGERS.len());
        let ty = INTEGERS[base];
        let k = self.d.named.len();
        self.d.named.push(Named {
            base: ty,
            written: Vec::new(),
            numbers: Vec::new(),
        });
        for _ in 0..2 + self.choices.below(4) {
            let before = self.d.named[k].numbers.last().copied();
            // A counted value the base does not hold is refused (E0605).
            let counted = self.choices.percent(40) && before.is_none_or(|b| b < ty.max());
            let (written, number) = match counted {
                true => (None, before.map_or(0, |b| b + 1)),
                false => {
                    let e = self.constant(ty, 2, &[]);
                    let number = self.folded(ty, &e).int();
                    (Some(e), number)
                }
            };
            let named = &mut self.d.named[k];
            named.written.push(written);
            named.numbers.push(number);
        }
    }

    /// Up to three CONSTANTs, the CONFIGURATION's when `global`. Each names
    /// literals, named values, the CONSTANTs of its kind before it and, in
    /// the PROGRAM, the CONFIGURATION's. One in three is a DINT, for the
    /// bounds, labels and steps that name them.
    fn constants(&mut self, global: bool) {
        for n in 0..self.choices.below(4) {
            let ty = match self.choices.below(4) {
                0 => [Ty::Real, Ty::Lreal][self.choices.below(2)],
                1 => Ty::Dint,
                _ => self.integer(),
            };
            let visible: Vec<usize> = (0..self.d.constants.len())
                .filter(|&c| !global || self.d.constants[c].global)
                .collect();
            let expr = self.constant(ty, 3, &visible);
            let value = self.folded(ty, &expr);
            let name = match global {
                true => format!("gk{n}"),
                false => format!("k{n}"),
            };
            self.d.constants.push(Constant {
                name,
                ty,
                expr,
                value,
                global,
            });
        }
    }

    /// Up to two `VAR_GLOBAL`s, each started by a constant expression over
    /// the CONFIGURATION's CONSTANTs: a value `__init` computes.
    fn globals(&mut self) {
        let visible: Vec<usize> = (0..self.d.constants.len())
            .filter(|&c| self.d.constants[c].global)
            .collect();
        for n in 0..self.choices.below(3) {
            let ty = match self.choices.below(3) {
                0 => [Ty::Real, Ty::Lreal][self.choices.below(2)],
                _ => self.integer(),
            };
            let expr = self.constant(ty, 3, &visible);
            let value = self.folded(ty, &expr);
            self.d.globals.push(Global {
                name: format!("gv{n}"),
                ty,
                expr,
                value,
            });
        }
    }

    /// `value` as a DINT constant expression that folds to it: a DINT
    /// CONSTANT `visible` lists, or a named value held as DINT, plus what
    /// is left. `None` when there is neither, or by choice.
    fn const_for(&mut self, value: i128, visible: &[usize]) -> Option<Expr> {
        if !self.choices.percent(50) {
            return None;
        }
        let mut sources: Vec<Expr> = visible
            .iter()
            .copied()
            .filter(|&c| self.d.constants[c].ty == Ty::Dint)
            .map(Expr::Const)
            .collect();
        sources.extend(
            self.named_values(Ty::Dint)
                .into_iter()
                .map(|(k, j)| Expr::Named(k, j)),
        );
        let source = self.pick_expr(sources)?;
        let at = self.folded(Ty::Dint, &source).int();
        // Wrapping at DINT, as the fold does: any value has its rest.
        let rest = Expr::Lit(Ty::Dint, Val::Int(Ty::Dint.wrap(value - at)));
        Some(Expr::Arith(
            Arith::Add,
            Ty::Dint,
            Box::new(source),
            Box::new(rest),
        ))
    }

    fn pick_expr(&mut self, mut exprs: Vec<Expr>) -> Option<Expr> {
        match exprs.is_empty() {
            true => None,
            false => Some(exprs.swap_remove(self.choices.below(exprs.len()))),
        }
    }

    /// The CONSTANTs a body may name: in the PROGRAM, all of them; anywhere
    /// else, none, its named values only.
    fn visible_constants(&self) -> Vec<usize> {
        match self.in_program {
            true => (0..self.d.constants.len()).collect(),
            false => Vec::new(),
        }
    }

    /// `count` elements of a program array, each as the initializer writes
    /// it: a value in order, one at its index, or none, left at the default.
    fn elements(&mut self, ty: Ty, count: usize) -> (Vec<Val>, Vec<Written>) {
        let mut init = Vec::with_capacity(count);
        let mut written = Vec::with_capacity(count);
        let mut gap = false;
        for _ in 0..count {
            let how = match self.choices.below(4) {
                0 => Written::Default,
                1 => Written::AtIndex,
                _ if gap => Written::AtIndex,
                _ => Written::Next,
            };
            gap = matches!(how, Written::Default);
            init.push(match how {
                Written::Default => self.default_of(ty),
                _ => self.literal(ty),
            });
            written.push(how);
        }
        (init, written)
    }

    /// A divisor: never 0, which traps. -1 is one: the minimum divided by it
    /// wraps, as `eval` has it.
    fn divisor(&mut self, ty: Ty) -> Val {
        match self.literal(ty) {
            Val::Int(0) => Val::Int(3),
            v => v,
        }
    }

    /// An index into `array`: any DINT expression, wrapped into the range
    /// by `((e MOD n) + n) MOD n + lo`, which cannot leave it.
    fn index(&mut self, array: usize, readable: &[usize]) -> Expr {
        let (lo, n) = (
            self.d.arrays[array].lo,
            self.d.arrays[array].init.len() as i128,
        );
        self.indexing += 1;
        let e = self.expr(Ty::Dint, 1, readable);
        self.indexing -= 1;
        let lit = |v| Box::new(Expr::Lit(Ty::Dint, Val::Int(v)));
        let arith = |op, a, b| Box::new(Expr::Arith(op, Ty::Dint, a, b));
        let wrapped = arith(
            Arith::Mod,
            arith(Arith::Add, arith(Arith::Mod, Box::new(e), lit(n)), lit(n)),
            lit(n),
        );
        Expr::Arith(Arith::Add, Ty::Dint, wrapped, lit(lo))
    }

    /// Every place of type `ty`: the frame variables among `vars` and the
    /// fields of STRUCT ones, and in the program array elements (and their
    /// fields) and, when `outputs` is set, FB outputs.
    fn places(&mut self, ty: Ty, vars: &[usize], outputs: bool) -> Vec<Place> {
        let mut out = Vec::new();
        for &v in vars {
            match self.vars[v].ty {
                t if t == ty => out.push(Place::Var(v)),
                Ty::Struct(k) => {
                    for (f, field) in self.d.structs[k].iter().enumerate() {
                        if field.ty == ty {
                            out.push(Place::Field(Box::new(Place::Var(v)), f));
                        }
                    }
                }
                _ => {}
            }
        }
        if !self.in_program || self.indexing > 1 {
            return out;
        }
        let arrays: Vec<(usize, Ty)> = self.d.arrays.iter().map(|a| a.ty).enumerate().collect();
        for (a, elem) in arrays {
            if elem == ty {
                let index = self.index(a, vars);
                out.push(Place::Index(a, Box::new(index)));
            } else if let Ty::Struct(k) = elem {
                let fields: Vec<usize> = (0..self.d.structs[k].len())
                    .filter(|&f| self.d.structs[k][f].ty == ty)
                    .collect();
                for f in fields {
                    let index = self.index(a, vars);
                    out.push(Place::Field(Box::new(Place::Index(a, Box::new(index))), f));
                }
            }
        }
        for (r, reference) in self.d.refs.iter().enumerate() {
            match reference.target {
                t if t == ty => out.push(Place::Deref(r)),
                Ty::Struct(k) => {
                    for (f, field) in self.d.structs[k].iter().enumerate() {
                        if field.ty == ty {
                            out.push(Place::Field(Box::new(Place::Deref(r)), f));
                        }
                    }
                }
                _ => {}
            }
        }
        if outputs {
            for (i, inst) in self.d.instances.iter().enumerate() {
                let block = &self.d.blocks[inst.block];
                for m in block.role(Role::Output) {
                    if block.members[m].ty == ty {
                        out.push(Place::Member(i, m));
                    }
                }
            }
        }
        out
    }

    /// What `REF()` may take, of exactly type `ty`: a program variable, an
    /// array element or a field, not a dereference.
    fn addressable(&mut self, ty: Ty) -> Vec<Place> {
        let plain: Vec<usize> = (0..self.vars.len())
            .filter(|&v| self.vars[v].role == Role::Plain)
            .collect();
        fn through_ref(p: &Place) -> bool {
            match p {
                Place::Deref(_) => true,
                Place::Field(base, _) => through_ref(base),
                _ => false,
            }
        }
        let mut places = self.places(ty, &plain, false);
        places.retain(|p| !through_ref(p));
        places
    }

    fn pick(&mut self, mut places: Vec<Place>) -> Option<Place> {
        match places.is_empty() {
            true => None,
            false => Some(places.swap_remove(self.choices.below(places.len()))),
        }
    }

    /// Where a value of type `ty` may be written. A string goes into a
    /// string of any capacity, cut to it.
    fn target(&mut self, ty: Ty) -> Option<Place> {
        let assignable: Vec<usize> = (0..self.vars.len())
            .filter(|&v| matches!(self.vars[v].role, Role::Plain | Role::Output | Role::InOut))
            .collect();
        let tys: Vec<Ty> = match ty {
            Ty::Str(_) => (1..=8).map(Ty::Str).collect(),
            t => vec![t],
        };
        let mut places = Vec::new();
        for t in tys {
            places.extend(self.places(t, &assignable, false));
        }
        self.pick(places)
    }

    /// A type with named values, when one is declared.
    fn named_ty(&mut self) -> Option<Ty> {
        match self.d.named.is_empty() {
            true => None,
            false => {
                let k = self.choices.below(self.d.named.len());
                let base = INTEGERS.iter().position(|&t| t == self.d.named[k].base)?;
                Some(Ty::Named(k, base as u8))
            }
        }
    }

    /// A CONSTANT the frame sees, or a named value, of type `ty`.
    fn constant_leaf(&mut self, ty: Ty) -> Option<Expr> {
        let mut leaves: Vec<Expr> = self
            .visible_constants()
            .into_iter()
            .filter(|&c| self.d.constants[c].ty == ty)
            .map(Expr::Const)
            .collect();
        leaves.extend(
            self.named_values(ty)
                .into_iter()
                .map(|(k, j)| Expr::Named(k, j)),
        );
        self.pick_expr(leaves)
    }

    /// An expression of type `ty` over the variables in `readable`. A
    /// string expression reads a string of any capacity; a string literal
    /// fits the capacity asked for. A type with named values is read or
    /// named: no operation keeps it.
    fn expr(&mut self, ty: Ty, depth: u32, readable: &[usize]) -> Expr {
        if let Ty::Named(k, _) = ty {
            let places = self.places(ty, readable, true);
            if !self.choices.percent(40)
                && let Some(place) = self.pick(places)
            {
                return Expr::Read(place);
            }
            let values = self.d.named[k].numbers.len();
            return Expr::Named(k, self.choices.below(values));
        }
        if depth == 0 || self.choices.percent(30) || matches!(ty, Ty::Str(_)) {
            if !self.choices.percent(35) {
                let wanted = match ty {
                    Ty::Str(_) => Ty::Str(1 + self.choices.below(8) as u8),
                    t => t,
                };
                let places = self.places(wanted, readable, true);
                if let Some(place) = self.pick(places) {
                    return Expr::Read(place);
                }
            }
            if self.choices.percent(20)
                && let Some(leaf) = self.constant_leaf(ty)
            {
                return leaf;
            }
            if let Some(call) = self.call(ty, depth, readable) {
                return call;
            }
            return Expr::Lit(ty, self.literal(ty));
        }
        let d = depth - 1;
        if ty == Ty::Bool {
            if self.choices.percent(15)
                && let Some(same) = self.reference_comparison()
            {
                return same;
            }
            let pick = self.choices.below(5);
            if pick == 4
                && let Some(bit) = self.bit_read(readable)
            {
                return bit;
            }
            return match pick {
                0 => Expr::Not(Box::new(self.expr(ty, d, readable))),
                1 => {
                    let op = [Logic::And, Logic::Or, Logic::Xor][self.choices.below(3)];
                    Expr::Logic(
                        op,
                        Box::new(self.expr(ty, d, readable)),
                        Box::new(self.expr(ty, d, readable)),
                    )
                }
                _ => {
                    let named = match self.choices.percent(15) {
                        true => self.named_ty(),
                        false => None,
                    };
                    let operands = match named {
                        Some(named) => named,
                        None => match self.scalar_type() {
                            Ty::Bool => Ty::Dint,
                            t => t,
                        },
                    };
                    let op = [Cmp::Eq, Cmp::Ne, Cmp::Lt, Cmp::Gt, Cmp::Le, Cmp::Ge]
                        [self.choices.below(6)];
                    Expr::Cmp(
                        op,
                        Box::new(self.expr(operands, d, readable)),
                        Box::new(self.expr(operands, d, readable)),
                    )
                }
            };
        }
        match self.choices.below(12) {
            0 if ty.signed() => Expr::Neg(ty, Box::new(self.expr(ty, d, readable))),
            1 => match self.call(ty, depth, readable) {
                Some(call) => call,
                None => Expr::Lit(ty, self.literal(ty)),
            },
            10 if ty.is_int() => {
                let op = [Logic::And, Logic::Or, Logic::Xor][self.choices.below(3)];
                Expr::Bits(
                    op,
                    ty,
                    Box::new(self.expr(ty, d, readable)),
                    Box::new(self.expr(ty, d, readable)),
                )
            }
            11 if ty.is_float() => Expr::Pow(
                Box::new(self.expr(ty, d, readable)),
                Box::new(self.expr(ty, d, readable)),
            ),
            2 | 3 if ty.is_int() => {
                let op = [Arith::Div, Arith::Mod][self.choices.below(2)];
                let divisor = self.divisor(ty);
                Expr::Arith(
                    op,
                    ty,
                    Box::new(self.expr(ty, d, readable)),
                    Box::new(Expr::Lit(ty, divisor)),
                )
            }
            _ => {
                // A float may divide by anything: 0.0 makes an infinity
                // or a NaN, never a trap.
                let ops: &[Arith] = match ty.is_float() {
                    true => &[Arith::Add, Arith::Sub, Arith::Mul, Arith::Div],
                    false => &[Arith::Add, Arith::Sub, Arith::Mul],
                };
                let op = ops[self.choices.below(ops.len())];
                Expr::Arith(
                    op,
                    ty,
                    Box::new(self.expr(ty, d, readable)),
                    Box::new(self.expr(ty, d, readable)),
                )
            }
        }
    }

    /// A bit of an integer place, read as a BOOL.
    fn bit_read(&mut self, readable: &[usize]) -> Option<Expr> {
        let ty = INTEGERS[self.choices.below(INTEGERS.len())];
        let places = self.places(ty, readable, true);
        let place = self.pick(places)?;
        let bit = self.choices.below(ty.bits() as usize) as u32;
        Some(Expr::Bit(
            Box::new(place),
            ty,
            bit,
            self.choices.percent(50),
        ))
    }

    /// A bit of an integer place written: `p.%X3 := cond`.
    fn set_bit(&mut self, readable: &[usize]) -> Option<Stmt> {
        let ty = INTEGERS[self.choices.below(INTEGERS.len())];
        let target = self.target(ty)?;
        let bit = self.choices.below(ty.bits() as usize) as u32;
        let bare = self.choices.percent(50);
        let value = self.expr(Ty::Bool, 3, readable);
        Some(Stmt::SetBit(target, ty, bit, bare, value))
    }

    /// A slice of one integer place copied over one of another, of a width
    /// both have: `a.%W1 := b.%W0`.
    fn move_slice(&mut self, readable: &[usize]) -> Option<Stmt> {
        let to_ty = INTEGERS[self.choices.below(INTEGERS.len())];
        let from_ty = INTEGERS[self.choices.below(INTEGERS.len())];
        let widths: Vec<u32> = [8, 16, 32, 64]
            .into_iter()
            .filter(|w| *w <= to_ty.bits() && *w <= from_ty.bits())
            .collect();
        let width = widths[self.choices.below(widths.len())];
        let to = self.target(to_ty)?;
        let sources = self.places(from_ty, readable, true);
        let from = self.pick(sources)?;
        let to_off = self.choices.below((to_ty.bits() / width) as usize) as u32;
        let from_off = self.choices.below((from_ty.bits() / width) as usize) as u32;
        Some(Stmt::MoveSlice(
            Slice {
                place: to,
                ty: to_ty,
                offset: to_off,
            },
            Slice {
                place: from,
                ty: from_ty,
                offset: from_off,
            },
            width,
        ))
    }

    /// What a FUNCTION of type `ty` writes into parts of its result, over
    /// its inputs: nothing unless its result is an integer.
    fn result_slices(&mut self, ty: Ty, readable: &[usize]) -> Vec<ResultSlice> {
        if !ty.is_int() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for _ in 0..self.choices.below(3) {
            let integers: Vec<usize> = readable
                .iter()
                .copied()
                .filter(|&i| self.vars[i].ty.is_int())
                .collect();
            if self.choices.percent(50) || integers.is_empty() {
                let bit = self.choices.below(ty.bits() as usize) as u32;
                let bare = self.choices.percent(50);
                out.push(ResultSlice::Bit(
                    bit,
                    bare,
                    self.expr(Ty::Bool, 2, readable),
                ));
                continue;
            }
            let input = integers[self.choices.below(integers.len())];
            let from_ty = self.vars[input].ty;
            let widths: Vec<u32> = [8, 16, 32, 64]
                .into_iter()
                .filter(|w| *w <= ty.bits() && *w <= from_ty.bits())
                .collect();
            let width = widths[self.choices.below(widths.len())];
            out.push(ResultSlice::Move {
                to_off: self.choices.below((ty.bits() / width) as usize) as u32,
                input,
                from_off: self.choices.below((from_ty.bits() / width) as usize) as u32,
                width,
            });
        }
        out
    }

    /// A fold, the type of the pack it folds and the type it gives: integers
    /// added, subtracted, multiplied or divided (wrapping at each step), BOOLs
    /// combined, integers compared pair by pair into a BOOL, the smallest or
    /// the largest integer, floats raised to a power.
    fn fold(&mut self) -> (Fold, Ty, Ty) {
        let integer = INTEGERS[self.choices.below(INTEGERS.len())];
        match self.choices.below(5) {
            0 => {
                let ops = [Arith::Add, Arith::Sub, Arith::Mul, Arith::Div, Arith::Mod];
                let op = ops[self.choices.below(ops.len())];
                (Fold::Arith(op), integer, integer)
            }
            1 => {
                let op = [Logic::And, Logic::Or, Logic::Xor][self.choices.below(3)];
                (Fold::Logic(op), Ty::Bool, Ty::Bool)
            }
            2 => {
                let op =
                    [Cmp::Eq, Cmp::Ne, Cmp::Lt, Cmp::Gt, Cmp::Le, Cmp::Ge][self.choices.below(6)];
                (Fold::Cmp(op), integer, Ty::Bool)
            }
            3 => {
                let op = [Fold::Min, Fold::Max][self.choices.below(2)];
                (op, integer, integer)
            }
            _ => {
                let float = [Ty::Real, Ty::Lreal][self.choices.below(2)];
                (Fold::Power, float, float)
            }
        }
    }

    /// Two references of one type compared. Only a program holds them, and each
    /// is set before anything can read it.
    fn reference_comparison(&mut self) -> Option<Expr> {
        if !self.in_program || self.d.refs.is_empty() {
            return None;
        }
        let r = self.choices.below(self.d.refs.len());
        let mut a = self.ref_side(r);
        let b = self.ref_side(r);
        // No reference is on either side of this one: `NULL = NULL` is refused.
        if matches!((&a, &b), (RefSide::Null, RefSide::Null)) {
            a = RefSide::Held(r);
        }
        Some(Expr::SameRef(self.choices.percent(50), a, b))
    }

    /// A side that compares with the program reference `r`.
    fn ref_side(&mut self, r: usize) -> RefSide {
        let target = self.d.refs[r].target;
        match self.choices.below(5) {
            0 => RefSide::Null,
            1 => {
                let same: Vec<usize> = (0..self.d.refs.len())
                    .filter(|&q| self.d.refs[q].target == target)
                    .collect();
                RefSide::Held(same[self.choices.below(same.len())])
            }
            2 => RefSide::Held(r),
            _ => {
                let places = self.addressable(target);
                match self.pick(places) {
                    Some(place) => RefSide::Of(place, self.choices.percent(25)),
                    None => RefSide::Held(r),
                }
            }
        }
    }

    /// A call to a FUNCTION returning `ty` (any string, for a string), if
    /// there is one and calls are allowed here.
    fn call(&mut self, ty: Ty, depth: u32, readable: &[usize]) -> Option<Expr> {
        if !self.in_program || depth == 0 {
            return None;
        }
        if ty == Ty::Dint && !self.d.cycles.is_empty() && self.choices.percent(25) {
            let k = self.choices.below(self.d.cycles.len());
            let n = self.expr(Ty::Dint, depth - 1, readable);
            return Some(Expr::Cycle(k, Box::new(n)));
        }
        // Some calls go to the library every program is read with.
        let library = match ty {
            Ty::Dint => Some([LibFn::Half, LibFn::Clamp0][self.choices.below(2)]),
            Ty::Bool => Some(LibFn::Both),
            _ => None,
        };
        if let Some(f) = library
            && self.choices.percent(15)
        {
            let form = self.choices.below(2) as u8;
            let args = f
                .params()
                .iter()
                .map(|&t| self.expr(t, depth - 1, readable))
                .collect();
            return Some(Expr::LibCall(f, args, form));
        }
        let callable: Vec<usize> = (0..self.d.functions.len())
            .filter(|&f| match (self.d.functions[f].ty, ty) {
                (Ty::Str(_), Ty::Str(_)) => true,
                (a, b) => a == b,
            })
            .collect();
        if callable.is_empty() {
            return None;
        }
        let f = callable[self.choices.below(callable.len())];
        let args = self.args(f, depth - 1, readable)?;
        Some(Expr::Call(f, args))
    }

    /// The arguments of a call to FUNCTION `f`, each of its parameter's
    /// exact type, if there is a STRUCT for each STRUCT parameter.
    fn args(&mut self, f: usize, depth: u32, readable: &[usize]) -> Option<Vec<Expr>> {
        if self.d.functions[f].variadic {
            // One for each input before the pack, then one to four, each of the
            // pack's type. An integer divided by zero faults, so what a fold divides
            // by is a literal, as for `/`.
            let inputs = self.d.functions[f].inputs.clone();
            let (pack, before) = inputs.split_last().expect("a variadic FUNCTION's pack");
            let pack = pack.ty;
            let divides = matches!(
                self.d.functions[f].body,
                Expr::Fold(Fold::Arith(Arith::Div | Arith::Mod), _)
            );
            let mut args: Vec<Expr> = before
                .iter()
                .map(|v| self.expr(v.ty, depth, readable))
                .collect();
            let n = 1 + self.choices.below(4);
            for i in 0..n {
                args.push(match divides && i > 0 {
                    true => Expr::Lit(pack, self.divisor(pack)),
                    false => self.expr(pack, depth, readable),
                });
            }
            return Some(args);
        }
        let inputs: Vec<Ty> = self.d.functions[f].inputs.iter().map(|v| v.ty).collect();
        let mut args = Vec::new();
        for t in inputs {
            args.push(match t {
                // No expression makes a STRUCT: one is read from a place.
                Ty::Struct(_) => {
                    let places = self.places(t, readable, true);
                    Expr::Read(self.pick(places)?)
                }
                t => self.expr(t, depth, readable),
            });
        }
        Some(args)
    }

    /// `target := f(args)`: a FUNCTION called for its value, so the
    /// overloads of a name meet at call sites and not only deep inside an
    /// expression.
    fn function_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let overloaded: Vec<usize> = (0..self.d.functions.len())
            .filter(|&f| {
                let name = &self.d.functions[f].name;
                self.d.functions.iter().filter(|g| &g.name == name).count() > 1
            })
            .collect();
        let f = match overloaded.is_empty() || self.choices.percent(30) {
            true => self.choices.below(self.d.functions.len()),
            false => overloaded[self.choices.below(overloaded.len())],
        };
        let target = self.target(self.d.functions[f].ty)?;
        let args = self.args(f, 2, readable)?;
        Some(Stmt::Assign(target, Expr::Call(f, args)))
    }

    /// An overload of an earlier FUNCTION: its name and return type, and its
    /// parameters with one retyped, one added or one dropped, the way
    /// siblings are written. `None` when the name already has that
    /// parameter list.
    ///
    /// Each argument is written with its parameter's exact type, so the
    /// overload rk picks is the one exact on every argument, the one the
    /// evaluator runs (docs/overloading.md).
    fn overload(&mut self) -> Option<(String, Ty, Vec<Var>)> {
        let sibling = self.choices.below(self.d.functions.len());
        // A variadic FUNCTION keeps its name to itself.
        if self.d.functions[sibling].variadic {
            return None;
        }
        let name = self.d.functions[sibling].name.clone();
        let ty = self.d.functions[sibling].ty;
        let mut inputs = self.d.functions[sibling].inputs.clone();
        match self.choices.below(5) {
            0 if inputs.len() > 1 => {
                inputs.pop();
            }
            1 if inputs.len() < 3 => {
                let ty = self.param_type(None);
                inputs.push(input(inputs.len(), ty));
            }
            _ => {
                let i = self.choices.below(inputs.len());
                inputs[i].ty = self.param_type(Some(inputs[i].ty));
            }
        }
        let taken = self
            .d
            .functions
            .iter()
            .any(|f| f.name == name && param_key(&f.inputs) == param_key(&inputs));
        (!taken).then_some((name, ty, inputs))
    }

    /// A parameter type for an overload: for a STRUCT another STRUCT when
    /// there is one, so two named types meet in one overload set.
    fn param_type(&mut self, instead_of: Option<Ty>) -> Ty {
        let structs = self.d.structs.len();
        match instead_of {
            Some(Ty::Struct(k)) if structs > 1 => {
                Ty::Struct((k + 1 + self.choices.below(structs - 1)) % structs)
            }
            _ if structs > 0 && self.choices.percent(30) => Ty::Struct(self.choices.below(structs)),
            _ => self.scalar_type(),
        }
    }

    /// Up to four statements. `readable` holds the variables and the
    /// counters of the enclosing loops; `free` the counters not in use.
    fn block(
        &mut self,
        depth: u32,
        readable: &[usize],
        free: &[usize],
        in_loop: bool,
        may_return: bool,
    ) -> Vec<Stmt> {
        let mut out = Vec::new();
        for _ in 0..1 + self.choices.below(4) {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            let nested = |g: &mut Self| g.block(depth - 1, readable, free, in_loop, may_return);
            let stmt = match self.choices.below(24) {
                0 | 1 if depth > 0 => {
                    let mut arms = Vec::new();
                    for _ in 0..1 + self.choices.below(3) {
                        let cond = self.expr(Ty::Bool, 3, readable);
                        arms.push((cond, nested(self)));
                    }
                    let otherwise = match self.choices.percent(50) {
                        true => Some(nested(self)),
                        false => None,
                    };
                    Stmt::If(arms, otherwise)
                }
                2 if depth > 0 => match self.case(readable, nested) {
                    Some(case) => case,
                    None => continue,
                },
                3 if depth > 0 && !free.is_empty() => {
                    let counter = free[0];
                    let from = self.choices.below(9) as i128 - 4;
                    let to = self.choices.below(9) as i128 - 4;
                    let by = [1, -1, 2, -2][self.choices.below(4)];
                    let visible = self.visible_constants();
                    let step = self.const_for(by, &visible);
                    let mut inner = readable.to_vec();
                    inner.push(counter);
                    let body = self.block(depth - 1, &inner, &free[1..], true, may_return);
                    Stmt::For(counter, from, to, by, step, body)
                }
                4 if in_loop => match self.choices.percent(50) {
                    true => Stmt::Exit,
                    false => Stmt::Continue,
                },
                5 if may_return && self.choices.percent(20) => Stmt::Return,
                6 | 7 if self.in_program && !self.d.instances.is_empty() => {
                    match self.fb_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                8 if self.in_program && !self.d.instances.is_empty() => {
                    match self.method_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                9 if self.in_program && !self.d.users.is_empty() => {
                    match self.user_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                10 | 11 if self.in_program && !self.d.objects.is_empty() => {
                    match self.object_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                12 if self.in_program && !self.d.refs.is_empty() => {
                    let r = self.choices.below(self.d.refs.len());
                    let places = self.addressable(self.d.refs[r].target);
                    match self.pick(places) {
                        Some(place) => Stmt::SetRef(r, place),
                        None => continue,
                    }
                }
                13 | 14 if self.this.is_some() => match self.this_call(readable) {
                    Some(call) => call,
                    None => continue,
                },
                15 | 16 if self.in_program && !self.d.functions.is_empty() => {
                    match self.function_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                17 | 18 if self.in_program && !self.d.recursions.is_empty() => {
                    match self.recurse(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                19 | 20 if self.in_program && !self.d.walkers.is_empty() => {
                    match self.walk_call(readable) {
                        Some(call) => call,
                        None => continue,
                    }
                }
                21 => match self.set_bit(readable) {
                    Some(set) => set,
                    None => continue,
                },
                22 => match self.move_slice(readable) {
                    Some(copy) => copy,
                    None => continue,
                },
                23 if depth > 0 => {
                    let body = nested(self);
                    let len = self.choices.below(10);
                    let message = (0..len).map(|_| b"abcde"[self.choices.below(5)]).collect();
                    // Any STRING of the frame, cut to its capacity.
                    let strings: Vec<usize> = (0..self.vars.len())
                        .filter(|&v| {
                            matches!(self.vars[v].ty, Ty::Str(_))
                                && matches!(self.vars[v].role, Role::Plain | Role::Output)
                        })
                        .collect();
                    let variable = match strings.is_empty() || self.choices.percent(25) {
                        true => None,
                        false => Some(strings[self.choices.below(strings.len())]),
                    };
                    let handler = nested(self);
                    // No jump in a __FINALLY (E1210): neither a loop's nor a
                    // RETURN, whatever the __TRY is in.
                    let finally = match self.choices.percent(50) {
                        true => Some(self.block(depth - 1, readable, free, false, false)),
                        false => None,
                    };
                    Stmt::Try(Try {
                        body,
                        message,
                        variable,
                        handler,
                        finally,
                    })
                }
                _ => match self.assign(readable) {
                    Some(assign) => assign,
                    None => continue,
                },
            };
            out.push(stmt);
        }
        out
    }

    fn case(
        &mut self,
        readable: &[usize],
        nested: impl Fn(&mut Self) -> Vec<Stmt>,
    ) -> Option<Stmt> {
        // A selector of a type with named values, labelled by its names.
        if self.choices.percent(15)
            && let Some(Ty::Named(k, base)) = self.named_ty()
        {
            let places = self.places(Ty::Named(k, base), readable, true);
            let selector = Expr::Read(self.pick(places)?);
            let mut used: Vec<i128> = Vec::new();
            let mut arms = Vec::new();
            for _ in 0..1 + self.choices.below(3) {
                let j = self.choices.below(self.d.named[k].numbers.len());
                let number = self.d.named[k].numbers[j];
                // Two names of one number are one label twice.
                if used.contains(&number) {
                    continue;
                }
                used.push(number);
                arms.push((
                    vec![Label::Int(number, Some(Expr::Named(k, j)))],
                    nested(self),
                ));
            }
            let otherwise = match self.choices.percent(50) {
                true => Some(nested(self)),
                false => None,
            };
            return Some(Stmt::Case(selector, arms, otherwise));
        }
        // A string selector reads a string: rk refuses a literal one.
        let cap = 1 + self.choices.below(8) as u8;
        let strings = self.choices.percent(30);
        let places = match strings {
            true => self.places(Ty::Str(cap), readable, true),
            false => Vec::new(),
        };
        let (strings, selector) = match self.pick(places) {
            Some(place) => (true, Expr::Read(place)),
            None => (false, self.expr(Ty::Dint, 2, readable)),
        };
        let mut ints: Vec<(i128, i128)> = Vec::new();
        let mut texts: Vec<Vec<u8>> = Vec::new();
        let mut arms = Vec::new();
        for _ in 0..1 + self.choices.below(4) {
            let mut labels = Vec::new();
            for _ in 0..1 + self.choices.below(2) {
                if strings {
                    if let Val::Str(s) = self.literal(Ty::Str(3))
                        && !texts.contains(&s)
                    {
                        texts.push(s.clone());
                        labels.push(Label::Str(s));
                    }
                    continue;
                }
                let lo = self.choices.below(12) as i128 - 4;
                let hi = lo + self.choices.below(3) as i128;
                // Labels may not overlap.
                if ints.iter().any(|&(a, b)| lo <= b && a <= hi) {
                    continue;
                }
                ints.push((lo, hi));
                // Written as constant expressions where the frame sees some.
                let visible = self.visible_constants();
                labels.push(match lo == hi {
                    true => Label::Int(lo, self.const_for(lo, &visible)),
                    false => {
                        let ends = self
                            .const_for(lo, &visible)
                            .zip(self.const_for(hi, &visible));
                        Label::Range(lo, hi, ends)
                    }
                });
            }
            if !labels.is_empty() {
                arms.push((labels, nested(self)));
            }
        }
        let otherwise = match self.choices.percent(50) {
            true => Some(nested(self)),
            false => None,
        };
        match arms.is_empty() {
            true => None,
            false => Some(Stmt::Case(selector, arms, otherwise)),
        }
    }

    fn assign(&mut self, readable: &[usize]) -> Option<Stmt> {
        let ty = match self.choices.percent(15) {
            true => self.named_ty()?,
            false => self.var_type(true),
        };
        let target = self.target(ty)?;
        if self.choices.percent(12)
            && let Some(compound) = self.compound(&target, readable)
        {
            return Some(compound);
        }
        let value = match ty {
            // A whole STRUCT copies from another of its type.
            Ty::Struct(_) => {
                let sources = self.places(ty, readable, false);
                Expr::Read(self.pick(sources)?)
            }
            _ => {
                let ty = self.place_ty(&target);
                self.expr(ty, 4, readable)
            }
        };
        // Now and then a chain: one place or two more of the type, a STRING
        // of any capacity, each written what the place to its right holds,
        // or with an operator its operation over that.
        if self.choices.percent(15) {
            let mut places = Vec::new();
            for _ in 0..=self.choices.below(2) {
                let place = self.target(ty)?;
                let operator = self.chain_operator(&place);
                places.push((place, operator));
            }
            let operator = self.chain_operator(&target);
            places.push((target, operator));
            return Some(Stmt::Chain(places, value));
        }
        Some(Stmt::Assign(target, value))
    }

    /// The operator a place of a chain is written with: `:=` mostly, and now
    /// and then one its arithmetic takes. An integer is not divided: what it
    /// would be divided by is a place or a value, which may be 0.
    fn chain_operator(&mut self, place: &Place) -> Option<Arith> {
        let ty = self.place_ty(place);
        if !(ty.is_int() || ty.is_float()) || !self.choices.percent(25) {
            return None;
        }
        let ops: &[Arith] = match ty.is_float() {
            true => &[Arith::Add, Arith::Sub, Arith::Mul, Arith::Div],
            false => &[Arith::Add, Arith::Sub, Arith::Mul],
        };
        Some(ops[self.choices.below(ops.len())])
    }

    /// `x += e` on a number: an operator its arithmetic takes, an integer
    /// divided by a literal that is not 0, as `x / e` is written.
    fn compound(&mut self, target: &Place, readable: &[usize]) -> Option<Stmt> {
        let ty = self.place_ty(target);
        if !ty.is_int() && !ty.is_float() {
            return None;
        }
        let (op, value) = match ty.is_int() && self.choices.percent(25) {
            true => (Arith::Div, Expr::Lit(ty, self.divisor(ty))),
            false => {
                let ops: &[Arith] = match ty.is_float() {
                    true => &[Arith::Add, Arith::Sub, Arith::Mul, Arith::Div],
                    false => &[Arith::Add, Arith::Sub, Arith::Mul],
                };
                let op = ops[self.choices.below(ops.len())];
                (op, self.expr(ty, 3, readable))
            }
        };
        Some(Stmt::Compound(target.clone(), op, value))
    }

    fn fb_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let inst = self.choices.below(self.d.instances.len());
        let block = &self.d.blocks[self.d.instances[inst].block];
        let inputs: Vec<Ty> = block
            .inputs()
            .into_iter()
            .map(|m| block.members[m].ty)
            .collect();
        let bound = match block.inout().map(|m| block.members[m].ty) {
            None => None,
            Some(ty) => {
                let fits: Vec<usize> = (0..self.vars.len())
                    .filter(|&v| self.vars[v].role == Role::Plain && self.vars[v].ty == ty)
                    .collect();
                if fits.is_empty() {
                    return None;
                }
                Some(fits[self.choices.below(fits.len())])
            }
        };
        let args = inputs
            .into_iter()
            .map(|ty| self.expr(ty, 3, readable))
            .collect();
        Some(Stmt::CallFb(inst, args, bound))
    }

    fn method_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let inst = self.choices.below(self.d.instances.len());
        let block = &self.d.blocks[self.d.instances[inst].block];
        if block.methods.is_empty() {
            return None;
        }
        let m = self.choices.below(block.methods.len());
        let sig = block.methods[m].sig.clone();
        let target = self.target(sig.ty)?;
        let args = sig
            .params
            .iter()
            .map(|p| self.expr(p.ty, 3, readable))
            .collect();
        Some(Stmt::CallMethod(target, inst, m, args))
    }

    fn user_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let u = self.choices.below(self.d.users.len());
        let interface = self.d.users[u].interface;
        let implementers: Vec<usize> = (0..self.d.instances.len())
            .filter(|&i| self.d.blocks[self.d.instances[i].block].implements == Some(interface))
            .collect();
        if implementers.is_empty() {
            return None;
        }
        let inst = implementers[self.choices.below(implementers.len())];
        let user = &self.d.users[u];
        let ty = self.d.interfaces[interface].methods[user.method].ty;
        let inputs: Vec<Ty> = user.inputs.iter().map(|v| v.ty).collect();
        let target = self.target(ty)?;
        let args = inputs
            .into_iter()
            .map(|t| self.expr(t, 3, readable))
            .collect();
        Some(Stmt::CallUser(target, u, inst, args))
    }

    fn object_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let o = self.choices.below(self.d.objects.len());
        let class = &self.d.classes[self.d.objects[o].class];
        let j = self.choices.below(class.families.len());
        let sig = class.families[j].clone();
        let target = self.target(sig.ty)?;
        let args = sig
            .params
            .iter()
            .map(|p| self.expr(p.ty, 3, readable))
            .collect();
        Some(Stmt::CallObject(target, o, j, args))
    }

    /// `THIS.m{i}` for a method declared before this one, or `SUPER.m{i}`
    /// for one the parent has, this one's own base version included.
    fn this_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let ctx = self.this.clone()?;
        let supers = ctx.parent_families.min(ctx.family + 1);
        let via_super = supers > 0 && (ctx.family == 0 || self.choices.percent(50));
        let i = match via_super {
            true => self.choices.below(supers),
            false if ctx.family > 0 => self.choices.below(ctx.family),
            false => return None,
        };
        let sig = ctx.families[i].clone();
        let target = self.target(sig.ty)?;
        let args = sig
            .params
            .iter()
            .map(|p| self.expr(p.ty, 2, readable))
            .collect();
        Some(Stmt::CallThis(target, i, args, via_super))
    }

    /// A walker over elements of a type some program array or matrix has,
    /// so a call can bind one.
    fn walker(&mut self, n: usize) -> Option<Walker> {
        let mut shapes: Vec<(Ty, usize)> = self.d.arrays.iter().map(|a| (a.ty, 1)).collect();
        for m in &self.d.matrices {
            shapes.extend([(m.ty, 1), (m.ty, 2)]);
        }
        let (elem, rank) = *shapes.get(self.choices.below(shapes.len()))?;
        let host = match self.choices.below(5) {
            0 => Host::Block,
            1 => Host::Method,
            _ => Host::Function,
        };
        let field = match elem {
            Ty::Struct(k) => Some(self.choices.below(self.d.structs[k].len())),
            _ => None,
        };
        let mut walker = Walker {
            name: format!("walk{n}"),
            host,
            elem,
            rank,
            in_out: host == Host::Block || self.choices.percent(50),
            field,
            writes: self.choices.percent(50),
            backward: self.choices.percent(30),
            transposed: rank == 2 && self.choices.percent(30),
            pass: None,
        };
        // A FUNCTION walking the same elements the same way. A VAR_INPUT
        // is passed on to a VAR_INPUT only.
        let passes: Vec<usize> = (0..self.d.walkers.len())
            .filter(|&j| {
                let other = &self.d.walkers[j];
                other.host == Host::Function
                    && (other.elem, other.rank, other.field, other.writes)
                        == (elem, rank, field, walker.writes)
                    && (walker.in_out || !other.in_out)
            })
            .collect();
        if !passes.is_empty() && self.choices.percent(50) {
            walker.pass = Some(passes[self.choices.below(passes.len())]);
        }
        Some(walker)
    }

    /// A walker called on an array of its element type and rank, or on a
    /// row of a matrix for one dimension.
    fn walk_call(&mut self, readable: &[usize]) -> Option<Stmt> {
        let w = self.choices.below(self.d.walkers.len());
        let walker = &self.d.walkers[w];
        let (elem, rank, host, writes) = (walker.elem, walker.rank, walker.host, walker.writes);
        let (view, result) = (walker.view(&self.d), walker.result(&self.d));
        let mut bounds: Vec<Bound> = Vec::new();
        for (a, array) in self.d.arrays.iter().enumerate() {
            if rank == 1 && array.ty == elem {
                bounds.push(Bound::Array(a));
            }
        }
        for (m, matrix) in self.d.matrices.iter().enumerate() {
            match (matrix.ty == elem, rank) {
                (false, _) => {}
                (true, 2) => bounds.push(Bound::Matrix(m)),
                (true, _) => bounds
                    .extend((0..matrix.len[0]).map(|r| Bound::Row(m, matrix.lo[0] + r as i128))),
            }
        }
        let bound = *bounds.get(self.choices.below(bounds.len()))?;
        let instance = match host {
            Host::Function => None,
            _ => {
                let held: Vec<usize> = (0..self.d.walker_instances.len())
                    .filter(|&i| self.d.walker_instances[i].walker == w)
                    .collect();
                Some(held[self.choices.below(held.len())])
            }
        };
        let target = match host {
            Host::Block => None,
            _ => Some(self.target(result)?),
        };
        let k = writes.then(|| self.expr(view, 2, readable));
        Some(Stmt::Walk(WalkCall {
            target,
            walker: w,
            instance,
            bound,
            k,
        }))
    }

    /// A recursive body, which closes its cycle one of the ways the call
    /// graph knows, and keeps one kind of storage across its own call.
    fn recursion(&mut self) -> Recursion {
        let route = [
            Route::Direct,
            Route::Hop,
            Route::Initializer,
            Route::Super,
            Route::Interface,
            Route::InOut,
            Route::Ref,
            Route::SuperBody,
        ][self.choices.below(8)];
        let keep = match (route, self.choices.below(3)) {
            // An initializer writes no array element and no STRING.
            (Route::Initializer, _) | (_, 2) => Keep::Addressed,
            (_, 0) => {
                let len = 1 + self.choices.below(4);
                Keep::Array {
                    len,
                    at: self.choices.below(len),
                }
            }
            _ => Keep::Text {
                cap: 4 + self.choices.below(5) as u8,
            },
        };
        Recursion {
            route,
            keep,
            factor: 2 + self.choices.below(8) as i128,
            mul: 1 + self.choices.below(5) as i128,
            add: self.choices.below(7) as i128 - 3,
        }
    }

    /// A recursive body called at a depth of 0 to 4, with any seed.
    fn recurse(&mut self, readable: &[usize]) -> Option<Stmt> {
        let recursion = self.choices.below(self.d.recursions.len());
        let target = self.target(Ty::Dint)?;
        let depth = self.choices.below(5) as i128;
        let seed = self.expr(Ty::Dint, 2, readable);
        Some(Stmt::Recurse(Recurse {
            target,
            recursion,
            depth,
            seed,
        }))
    }

    /// Make sure a program variable can take a value of `ty` (a string of
    /// any capacity, for a string), declaring one when none can.
    fn holder(&mut self, ty: Ty) {
        let present = self.vars.iter().any(|v| match (v.ty, ty) {
            (Ty::Str(_), Ty::Str(_)) => true,
            (a, b) => a == b,
        });
        if !present {
            let init = self.literal(ty);
            self.vars.push(Var {
                name: format!("v{}", self.vars.len()),
                ty,
                init,
                role: Role::Plain,
            });
        }
    }

    fn place_ty(&self, place: &Place) -> Ty {
        let tys: Vec<Ty> = self.vars.iter().map(|v| v.ty).collect();
        place_ty(place, &tys, &self.d)
    }

    /// Scalar variables for a signature or a declaration: numbers, BOOLs
    /// and strings, each with a literal to start from.
    /// A STRUCT a block or a class may hold in its state, for a path of several
    /// steps from `THIS`: `THIS.t0.f1.%X3`.
    fn struct_member(&mut self) -> Option<Var> {
        if self.d.structs.is_empty() || !self.choices.percent(50) {
            return None;
        }
        let ty = Ty::Struct(self.choices.below(self.d.structs.len()));
        Some(Var {
            name: "t0".to_string(),
            ty,
            init: self.default_of(ty),
            role: Role::Plain,
        })
    }

    fn vars_named(&mut self, prefix: &str, count: usize, role: Role) -> Vec<Var> {
        (0..count)
            .map(|i| {
                let ty = self.scalar_type();
                let init = self.literal(ty);
                Var {
                    name: format!("{prefix}{i}"),
                    ty,
                    init,
                    role,
                }
            })
            .collect()
    }

    fn signatures(&mut self, count: usize) -> Vec<Signature> {
        (0..count)
            .map(|j| {
                let ty = self.scalar_type();
                let n = self.choices.below(3);
                Signature {
                    name: format!("m{j}"),
                    ty,
                    params: self.vars_named("p", n, Role::Input),
                }
            })
            .collect()
    }

    /// Write a body over `frame`, which sees nothing of the program's.
    fn with_frame<T>(
        &mut self,
        frame: Vec<Var>,
        write: impl FnOnce(&mut Self) -> T,
    ) -> (Vec<Var>, T) {
        let saved = std::mem::replace(&mut self.vars, frame);
        let was = std::mem::replace(&mut self.in_program, false);
        let out = write(self);
        self.in_program = was;
        (std::mem::replace(&mut self.vars, saved), out)
    }
}

/// A FUNCTION's input `a{i}`.
fn input(i: usize, ty: Ty) -> Var {
    Var {
        name: format!("a{i}"),
        ty,
        init: Val::Int(0),
        role: Role::Input,
    }
}

/// What tells two parameter lists apart: their types, a STRING's length
/// aside, which is no part of its type.
fn param_key(inputs: &[Var]) -> Vec<Ty> {
    inputs
        .iter()
        .map(|v| match v.ty {
            Ty::Str(_) => Ty::Str(0),
            t => t,
        })
        .collect()
}

/// A program from `bytes`: ST text whose header comment holds, after
/// [`SCANS`] scans, the value of every variable, array element, STRUCT
/// field and FB member.
pub fn program(bytes: &[u8]) -> String {
    let mut g = Generator {
        choices: Choices { bytes, at: 0 },
        vars: Vec::new(),
        in_program: true,
        d: Decls::default(),
        budget: 28,
        indexing: 0,
        this: None,
    };

    // Types with named values, then the CONFIGURATION's CONSTANTs and the
    // VAR_GLOBALs they start: everything below may name them.
    for _ in 0..g.choices.below(3) {
        g.named_type();
    }
    g.constants(true);
    g.globals();

    for k in 0..g.choices.below(3) {
        let n = 1 + g.choices.below(3);
        let mut fields = g.vars_named("f", n, Role::Plain);
        // A field's default may be a constant expression: a TYPE sees no
        // CONSTANT, only literals and named values.
        for (f, field) in fields.iter_mut().enumerate() {
            if (field.ty.is_int() || field.ty.is_float()) && g.choices.percent(25) {
                let e = g.constant(field.ty, 2, &[]);
                field.init = g.folded(field.ty, &e);
                g.d.field_inits.push(((k, f), e));
            }
        }
        g.d.structs.push(fields);
    }

    for _ in 0..g.choices.below(3) {
        let cycle = g.choices.below(CYCLES.len());
        g.d.cycles.push(cycle);
    }

    for k in 0..g.choices.below(5) {
        // One in four is variadic: a name of its own, never an overload, and
        // not a program variable's (`v0`).
        if g.choices.percent(25) {
            // One in four of those reads an argument by position, at an index of
            // any integer type, so past either end as well.
            if g.choices.percent(25) {
                let index = INTEGERS[g.choices.below(INTEGERS.len())];
                let pack = match g.choices.below(4) {
                    0 => Ty::Bool,
                    1 => [Ty::Real, Ty::Lreal][g.choices.below(2)],
                    _ => INTEGERS[g.choices.below(INTEGERS.len())],
                };
                g.d.functions.push(Function {
                    name: format!("pick{k}"),
                    ty: pack,
                    inputs: vec![input(0, index), input(1, pack)],
                    body: Expr::Element,
                    slices: Vec::new(),
                    variadic: true,
                });
                continue;
            }
            let (fold, pack, ty) = g.fold();
            g.d.functions.push(Function {
                name: format!("fold{k}"),
                ty,
                inputs: vec![input(0, pack)],
                body: Expr::Fold(fold, pack),
                slices: Vec::new(),
                variadic: true,
            });
            continue;
        }
        let overload = match g.d.functions.is_empty() || !g.choices.percent(40) {
            true => None,
            false => g.overload(),
        };
        let (name, ty, inputs) = match overload {
            Some(overload) => overload,
            None => {
                let ty = g.scalar_type();
                let n = 1 + g.choices.below(3);
                // Some take STRUCTs and compute from their fields; a string
                // FUNCTION takes strings; any other, its own type.
                let structs = !g.d.structs.is_empty() && g.choices.percent(25);
                let inputs = (0..n)
                    .map(|i| {
                        let ty = match ty {
                            _ if structs => Ty::Struct(g.choices.below(g.d.structs.len())),
                            Ty::Str(_) => Ty::Str(1 + g.choices.below(8) as u8),
                            t => t,
                        };
                        input(i, ty)
                    })
                    .collect();
                (format!("g{k}"), ty, inputs)
            }
        };
        let readable: Vec<usize> = (0..inputs.len()).collect();
        let (inputs, (body, slices)) = g.with_frame(inputs, |g| {
            let body = g.expr(ty, 3, &readable);
            (body, g.result_slices(ty, &readable))
        });
        g.d.functions.push(Function {
            name,
            ty,
            inputs,
            body,
            slices,
            variadic: false,
        });
    }

    for k in 0..g.choices.below(3) {
        let n = 1 + g.choices.below(2);
        let methods = g.signatures(n);
        g.d.interfaces.push(Interface {
            name: format!("I{k}"),
            methods,
        });
    }

    for k in 0..g.choices.below(3) {
        let mut members = Vec::new();
        for (count, prefix, role) in [
            (1 + g.choices.below(2), "x", Role::Input),
            (1 + g.choices.below(2), "y", Role::Output),
            (g.choices.below(2), "io", Role::InOut),
            (g.choices.below(3), "s", Role::Plain),
        ] {
            for mut v in g.vars_named(prefix, count, role) {
                if role == Role::InOut {
                    v.name = prefix.to_string();
                }
                members.push(v);
            }
        }
        // A member's initial value may be a constant expression over the
        // CONFIGURATION's CONSTANTs, which the block declares VAR_EXTERNAL,
        // and named values. An instance of the program is `__init`'s to
        // start, so these are values `__init` computes.
        let global: Vec<usize> = (0..g.d.constants.len())
            .filter(|&c| g.d.constants[c].global)
            .collect();
        let b = g.d.blocks.len();
        for (m, member) in members.iter_mut().enumerate() {
            if member.role != Role::InOut
                && (member.ty.is_int() || member.ty.is_float())
                && g.choices.percent(25)
            {
                let e = g.constant(member.ty, 2, &global);
                member.init = g.folded(member.ty, &e);
                g.d.member_inits.push(((b, m), e));
            }
        }
        if g.choices.percent(30)
            && let Some(ty @ Ty::Named(k, _)) = g.named_ty()
        {
            let j = g.choices.below(g.d.named[k].numbers.len());
            g.d.member_inits
                .push(((b, members.len()), Expr::Named(k, j)));
            // Named like a type, another one than its own when there are
            // two: names are caseless, and the block's `E1#n0` still names
            // the type `E1`, not a member `e1 : E0`.
            let types = g.d.named.len();
            let like = match types {
                1 => k,
                _ => (k + 1 + g.choices.below(types - 1)) % types,
            };
            members.push(Var {
                name: format!("e{like}"),
                ty,
                init: Val::Int(g.d.named[k].numbers[j]),
                role: Role::Plain,
            });
        }
        if let Some(held) = g.struct_member() {
            members.push(held);
        }
        let implements = match g.d.interfaces.is_empty() || g.choices.percent(30) {
            true => None,
            false => Some(g.choices.below(g.d.interfaces.len())),
        };
        let signatures = match implements {
            Some(i) => g.d.interfaces[i].methods.clone(),
            None => {
                let n = g.choices.below(2);
                g.signatures(n)
            }
        };
        let mut methods = Vec::new();
        for sig in signatures {
            // A METHOD sees the block's members but its in-out, and its
            // own parameters; it ends by setting its result.
            let mut frame: Vec<Var> = members
                .iter()
                .map(|m| Var {
                    role: match m.role {
                        Role::InOut => Role::Hidden,
                        r => r,
                    },
                    ..m.clone()
                })
                .collect();
            frame.extend(sig.params.iter().cloned());
            let readable: Vec<usize> = (0..frame.len())
                .filter(|&v| frame[v].role != Role::Hidden)
                .collect();
            g.budget = 6;
            let (_, (body, result)) = g.with_frame(frame, |g| {
                let body = g.block(1, &readable, &[], false, false);
                (body, g.expr(sig.ty, 3, &readable))
            });
            methods.push(Method { sig, body, result });
        }
        let readable: Vec<usize> = (0..members.len()).collect();
        g.budget = 10;
        let (members, body) = g.with_frame(members, |g| g.block(2, &readable, &[], false, true));
        let through_this = g.choices.percent(50);
        g.d.blocks.push(Block {
            through_this,
            name: format!("fb{k}"),
            members,
            body,
            implements,
            methods,
        });
    }

    for i in 0..g.d.interfaces.len() {
        let method = g.choices.below(g.d.interfaces[i].methods.len());
        let n = g.choices.below(3);
        let inputs = g.vars_named("q", n, Role::Input);
        let params: Vec<Ty> = g.d.interfaces[i].methods[method]
            .params
            .iter()
            .map(|p| p.ty)
            .collect();
        let readable: Vec<usize> = (0..n).collect();
        let (inputs, args) = g.with_frame(inputs, |g| {
            params
                .into_iter()
                .map(|t| g.expr(t, 2, &readable))
                .collect::<Vec<Expr>>()
        });
        let k = g.d.users.len();
        g.d.users.push(User {
            name: format!("use{k}"),
            interface: i,
            method,
            inputs,
            args,
        });
    }

    // The PROGRAM's CONSTANTs, then its variables: one may start at a
    // constant expression, one of a type with named values at a name.
    g.constants(false);
    let constants: Vec<usize> = (0..g.d.constants.len()).collect();
    for k in 0..1 + g.choices.below(8) {
        let named = match g.choices.percent(15) {
            true => g.named_ty(),
            false => None,
        };
        let ty = match named {
            Some(ty) => ty,
            None => g.var_type(true),
        };
        let (init, written) = match ty {
            Ty::Named(n, _) => {
                let j = g.choices.below(g.d.named[n].numbers.len());
                (Val::Int(g.d.named[n].numbers[j]), Some(Expr::Named(n, j)))
            }
            t if (t.is_int() || t.is_float()) && g.choices.percent(30) => {
                let e = g.constant(t, 3, &constants);
                (g.folded(t, &e), Some(e))
            }
            t => (g.literal(t), None),
        };
        if let Some(e) = written {
            g.d.var_inits.push((g.vars.len(), e));
        }
        g.vars.push(Var {
            name: format!("v{k}"),
            ty,
            init,
            role: Role::Plain,
        });
    }
    for k in 0..g.choices.below(3) {
        let parent = match k > 0 && g.choices.percent(60) {
            true => Some(g.choices.below(k)),
            false => None,
        };
        let mut families = parent
            .map(|p| g.d.classes[p].families.clone())
            .unwrap_or_default();
        let inherited = families.len();
        let n = g.choices.below(3);
        let taken = g.d.classes.iter().map(|c| c.vars.len()).sum::<usize>();
        let mut vars: Vec<Var> = g
            .vars_named("z", n, Role::Plain)
            .into_iter()
            .enumerate()
            .map(|(i, v)| Var {
                name: format!("z{}", taken + i),
                ..v
            })
            .collect();
        if let Some(mut held) = g.struct_member() {
            held.name = format!("z{}", taken + vars.len());
            vars.push(held);
        }
        let new = 1 + g.choices.below(2);
        for j in inherited..inherited + new {
            let mut sig = g.signatures(1).remove(0);
            sig.name = format!("m{j}");
            families.push(sig);
        }
        let mut frame: Vec<Var> = parent.map(|p| g.d.class_vars(p)).unwrap_or_default();
        frame.extend(vars.iter().cloned());
        let mut defs = Vec::new();
        for j in 0..families.len() {
            if j < inherited && !g.choices.percent(40) {
                defs.push(None);
                continue;
            }
            let mut method_frame = frame.clone();
            method_frame.extend(families[j].params.iter().cloned());
            let readable: Vec<usize> = (0..method_frame.len()).collect();
            let ty = families[j].ty;
            g.this = Some(ThisCtx {
                families: families.clone(),
                family: j,
                parent_families: inherited,
            });
            g.budget = 6;
            let (_, (body, result)) = g.with_frame(method_frame, |g| {
                let body = g.block(1, &readable, &[], false, false);
                (body, g.expr(ty, 3, &readable))
            });
            g.this = None;
            defs.push(Some(ClassDef {
                overrides: j < inherited,
                body,
                result,
            }));
        }
        let through_this = g.choices.percent(50);
        g.d.classes.push(Class {
            through_this,
            name: format!("C{k}"),
            parent,
            vars,
            families,
            defs,
        });
    }

    // Somewhere to put what each method returns.
    let results: Vec<Ty> =
        g.d.blocks
            .iter()
            .flat_map(|b| b.methods.iter().map(|m| m.sig.ty))
            .chain(
                g.d.classes
                    .iter()
                    .flat_map(|c| c.families.iter().map(|s| s.ty)),
            )
            .collect();
    for ty in results {
        g.holder(ty);
    }
    for k in 0..g.choices.below(3) {
        let ty = g.var_type(true);
        let lo = g.choices.below(6) as i128 - 3;
        let count = 1 + g.choices.below(5);
        let (init, written) = g.elements(ty, count);
        let hi = lo + count as i128 - 1;
        let bounds = g.const_for(lo, &constants).zip(g.const_for(hi, &constants));
        g.d.arrays.push(Array {
            name: format!("arr{k}"),
            ty,
            lo,
            bounds,
            init,
            written,
        });
    }
    if !g.d.blocks.is_empty() {
        for k in 0..1 + g.choices.below(3) {
            let block = g.choices.below(g.d.blocks.len());
            g.d.instances.push(Instance {
                name: format!("f{k}"),
                block,
            });
        }
    }
    if !g.d.classes.is_empty() {
        for k in 0..1 + g.choices.below(3) {
            let class = g.choices.below(g.d.classes.len());
            g.d.objects.push(Object {
                name: format!("o{k}"),
                class,
            });
        }
    }
    for k in 0..g.choices.below(4) {
        let target = g.var_type(true);
        // Something it can point to.
        if !g.vars.iter().any(|v| v.ty == target) {
            let init = g.literal(target);
            g.vars.push(Var {
                name: format!("v{}", g.vars.len()),
                ty: target,
                init,
                role: Role::Plain,
            });
        }
        g.d.refs.push(Reference {
            name: format!("r{k}"),
            target,
        });
    }
    for k in 0..g.choices.below(3) {
        let ty = g.var_type(true);
        let lo = [0, 1].map(|_| g.choices.below(6) as i128 - 3);
        let len = [1 + g.choices.below(3), 1 + g.choices.below(4)];
        let (init, written) = g.elements(ty, len[0] * len[1]);
        g.d.matrices.push(Matrix {
            name: format!("mat{k}"),
            ty,
            lo,
            len,
            init,
            written,
        });
    }
    for n in 0..g.choices.below(4) {
        if let Some(walker) = g.walker(n) {
            g.d.walkers.push(walker);
        }
    }
    for w in 0..g.d.walkers.len() {
        if g.d.walkers[w].host != Host::Function {
            for _ in 0..1 + g.choices.below(2) {
                g.d.walker_instances.push(WalkerInstance {
                    name: format!("w{}", g.d.walker_instances.len()),
                    walker: w,
                });
            }
        }
        if g.d.walkers[w].host != Host::Block {
            g.holder(g.d.walkers[w].result(&g.d));
        }
    }
    for _ in 0..g.choices.below(3) {
        let recursion = g.recursion();
        g.d.recursions.push(recursion);
    }
    if !g.d.recursions.is_empty() {
        g.holder(Ty::Dint);
    }
    let readable: Vec<usize> = (0..g.vars.len()).collect();
    let counters: Vec<usize> = (0..2)
        .map(|k| {
            g.vars.push(Var {
                name: format!("i{k}"),
                ty: Ty::Dint,
                init: Val::Int(0),
                role: Role::Hidden,
            });
            g.vars.len() - 1
        })
        .collect();
    // Every reference is set before anything can read it, and nothing in
    // these first statements reads through one.
    let mut body = Vec::new();
    let refs = std::mem::take(&mut g.d.refs);
    for (r, reference) in refs.iter().enumerate() {
        let places = g.addressable(reference.target);
        let place = g.pick(places).expect("a variable of the reference's type");
        body.push(Stmt::SetRef(r, place));
    }
    g.d.refs = refs;
    // The program's own budget, whatever the blocks spent.
    g.budget = 28;
    body.extend(g.block(3, &readable, &counters, false, true));

    // What the program must leave behind.
    let mut state = State {
        vars: g.vars.iter().map(|v| v.init.clone()).collect(),
        arrays: g.d.arrays.iter().map(|a| a.init.clone()).collect(),
        instances: g
            .d
            .instances
            .iter()
            .map(|i| {
                g.d.blocks[i.block]
                    .members
                    .iter()
                    .map(|m| m.init.clone())
                    .collect()
            })
            .collect(),
        objects: g
            .d
            .objects
            .iter()
            .map(|o| {
                g.d.class_vars(o.class)
                    .iter()
                    .map(|v| v.init.clone())
                    .collect()
            })
            .collect(),
        refs: vec![Loc::Var(0); g.d.refs.len()],
        this: None,
        matrices: g.d.matrices.iter().map(|m| m.init.clone()).collect(),
        walkers: g
            .d
            .walker_instances
            .iter()
            .map(|i| zero(g.d.walkers[i.walker].result(&g.d)))
            .collect(),
    };
    let tys: Vec<Ty> = g.vars.iter().map(|v| v.ty).collect();
    for _ in 0..SCANS {
        let _ = run_block(&body, &mut state, &tys, &g.d);
    }

    write_program(&g.vars, &g.d, &body, &state)
}

fn write_program(vars: &[Var], d: &Decls, body: &[Stmt], state: &State) -> String {
    let mut out = format!("(* rk-fuzz expects, after {SCANS} scans:\n");
    for (v, value) in vars.iter().zip(&state.vars) {
        if v.role != Role::Hidden {
            expect(&mut out, d, &format!("Run.{}", v.name), v.ty, value);
        }
    }
    for (a, values) in d.arrays.iter().zip(&state.arrays) {
        for (k, value) in values.iter().enumerate() {
            let path = format!("Run.{}[{}]", a.name, a.lo + k as i128);
            expect(&mut out, d, &path, a.ty, value);
        }
    }
    for (inst, values) in d.instances.iter().zip(&state.instances) {
        for (m, value) in d.blocks[inst.block].members.iter().zip(values) {
            // An in-out is a reference into the caller, not a value.
            if m.role != Role::InOut {
                let path = format!("Run.{}.{}", inst.name, m.name);
                expect(&mut out, d, &path, m.ty, value);
            }
        }
    }
    for (o, values) in d.objects.iter().zip(&state.objects) {
        for (v, value) in d.class_vars(o.class).iter().zip(values) {
            let path = format!("Run.{}.{}", o.name, v.name);
            expect(&mut out, d, &path, v.ty, value);
        }
    }
    for (m, values) in d.matrices.iter().zip(&state.matrices) {
        for (k, value) in values.iter().enumerate() {
            let (row, column) = (k / m.len[1], k % m.len[1]);
            let path = format!(
                "Run.{}[{}, {}]",
                m.name,
                m.lo[0] + row as i128,
                m.lo[1] + column as i128
            );
            expect(&mut out, d, &path, m.ty, value);
        }
    }
    for (i, value) in d.walker_instances.iter().zip(&state.walkers) {
        let ty = d.walkers[i.walker].result(d);
        expect(&mut out, d, &format!("Run.{}.acc", i.name), ty, value);
    }
    for g in &d.globals {
        expect(&mut out, d, &g.name, g.ty, &g.value);
    }
    out.push_str("*)\n\n");

    let names = |vars: &[Var]| Names {
        vars: vars.iter().map(|v| v.name.clone()).collect(),
        d,
    };
    // A method's frame: what its instance holds, then its own parameters.
    let framed = |members: &[Var], through_this: bool, params: &[Var]| Names {
        vars: members
            .iter()
            .map(|v| match through_this {
                true => format!("THIS.{}", v.name),
                false => v.name.clone(),
            })
            .chain(params.iter().map(|v| v.name.clone()))
            .collect(),
        d,
    };
    let decl = |out: &mut String, v: &Var, with_init: bool| {
        let _ = match with_init {
            true => writeln!(
                out,
                "    {} : {} := {};",
                v.name,
                v.ty.name(),
                literal(v.ty, &v.init)
            ),
            false => writeln!(out, "    {} : {};", v.name, v.ty.name()),
        };
    };
    let signature = |out: &mut String, s: &Signature| {
        let _ = writeln!(out, "    METHOD {} : {}", s.name, s.ty.name());
        if !s.params.is_empty() {
            out.push_str("    VAR_INPUT\n");
            for p in &s.params {
                let _ = writeln!(out, "        {} : {};", p.name, p.ty.name());
            }
            out.push_str("    END_VAR\n");
        }
    };

    // What a constant expression names is no variable: any frame writes it.
    let constants = names(&[]);
    // A declaration whose initial value is the constant expression `init`.
    let start = |out: &mut String, v: &Var, init: &Expr| {
        let _ = writeln!(
            out,
            "    {} : {} := {};",
            v.name,
            v.ty.name(),
            constants.expr(init)
        );
    };
    for (k, named) in d.named.iter().enumerate() {
        let values: Vec<String> = named
            .written
            .iter()
            .enumerate()
            .map(|(j, written)| match written {
                Some(e) => format!("n{j} := {}", constants.expr(e)),
                None => format!("n{j}"),
            })
            .collect();
        let _ = writeln!(
            out,
            "TYPE E{k} : {} ({});\nEND_TYPE\n",
            named.base.name(),
            values.join(", ")
        );
    }
    for (k, fields) in d.structs.iter().enumerate() {
        let _ = writeln!(out, "TYPE S{k} : STRUCT");
        for (f, field) in fields.iter().enumerate() {
            match d.field_inits.iter().find(|(at, _)| *at == (k, f)) {
                Some((_, init)) => start(&mut out, field, init),
                None => decl(&mut out, field, true),
            }
        }
        out.push_str("END_STRUCT\nEND_TYPE\n\n");
    }
    // Every CONSTANT of the CONFIGURATION, as a POU that names one declares
    // them: when the block's starting values name one, and always in the
    // PROGRAM. Written last first, so a CONSTANT names one declared after it.
    let externals = |out: &mut String| {
        let global: Vec<&Constant> = d.constants.iter().filter(|c| c.global).collect();
        if !global.is_empty() {
            out.push_str("VAR_EXTERNAL CONSTANT\n");
            for c in global.iter().rev() {
                let _ = writeln!(out, "    {} : {};", c.name, c.ty.name());
            }
            out.push_str("END_VAR\n");
        }
    };
    for f in &d.functions {
        let _ = writeln!(out, "FUNCTION {} : {}\nVAR_INPUT", f.name, f.ty.name());
        for (i, v) in f.inputs.iter().enumerate() {
            match f.variadic && i + 1 == f.inputs.len() {
                true => {
                    let _ = writeln!(out, "    {} : {}...;", v.name, v.ty.name());
                }
                false => decl(&mut out, v, false),
            }
        }
        let n = names(&f.inputs);
        let _ = writeln!(out, "END_VAR\n    {} := {};", f.name, n.expr(&f.body));
        for slice in &f.slices {
            match slice {
                ResultSlice::Bit(bit, bare, cond) => {
                    let part = match bare {
                        true => format!("{bit}"),
                        false => format!("%X{bit}"),
                    };
                    let _ = writeln!(out, "    {}.{part} := {};", f.name, n.expr(cond));
                }
                ResultSlice::Move {
                    to_off,
                    input,
                    from_off,
                    width,
                } => {
                    let w = width_letter(*width);
                    let _ = writeln!(
                        out,
                        "    {}.%{w}{to_off} := {}.%{w}{from_off};",
                        f.name, f.inputs[*input].name
                    );
                }
            }
        }
        out.push_str("END_FUNCTION\n\n");
    }
    for i in &d.interfaces {
        let _ = writeln!(out, "INTERFACE {}", i.name);
        for s in &i.methods {
            signature(&mut out, s);
            out.push_str("    END_METHOD\n");
        }
        out.push_str("END_INTERFACE\n\n");
    }
    for u in &d.users {
        let interface = &d.interfaces[u.interface];
        let method = &interface.methods[u.method];
        let _ = writeln!(
            out,
            "FUNCTION {} : {}\nVAR_IN_OUT\n    dev : {};\nEND_VAR",
            u.name,
            method.ty.name(),
            interface.name
        );
        if !u.inputs.is_empty() {
            out.push_str("VAR_INPUT\n");
            for v in &u.inputs {
                decl(&mut out, v, false);
            }
            out.push_str("END_VAR\n");
        }
        let _ = writeln!(
            out,
            "    {} := dev.{}({});\nEND_FUNCTION\n",
            u.name,
            method.name,
            names(&u.inputs).args(&method.params, &u.args)
        );
    }
    for (k, b) in d.blocks.iter().enumerate() {
        match b.implements {
            Some(i) => {
                let _ = writeln!(
                    out,
                    "FUNCTION_BLOCK {} IMPLEMENTS {}",
                    b.name, d.interfaces[i].name
                );
            }
            None => {
                let _ = writeln!(out, "FUNCTION_BLOCK {}", b.name);
            }
        }
        let inits: Vec<&((usize, usize), Expr)> = d
            .member_inits
            .iter()
            .filter(|((block, _), _)| *block == k)
            .collect();
        if inits.iter().any(|(_, e)| names_constant(e)) {
            externals(&mut out);
        }
        for (section, role) in [
            ("VAR_INPUT", Role::Input),
            ("VAR_OUTPUT", Role::Output),
            ("VAR_IN_OUT", Role::InOut),
            ("VAR", Role::Plain),
        ] {
            let members = b.role(role);
            if members.is_empty() {
                continue;
            }
            let _ = writeln!(out, "{section}");
            for m in members {
                // A reference has no initial value of its own, and a STRUCT
                // has its TYPE's.
                let member = &b.members[m];
                match inits.iter().find(|((_, at), _)| *at == m) {
                    Some((_, init)) => start(&mut out, member, init),
                    None => decl(
                        &mut out,
                        member,
                        role != Role::InOut && !matches!(member.ty, Ty::Struct(_)),
                    ),
                }
            }
            out.push_str("END_VAR\n");
        }
        for m in &b.methods {
            signature(&mut out, &m.sig);
            let n = framed(&b.members, b.through_this, &m.sig.params);
            n.block(&mut out, &m.body, 2);
            let _ = writeln!(
                out,
                "        {} := {};\n    END_METHOD",
                m.sig.name,
                n.expr(&m.result)
            );
        }
        framed(&b.members, b.through_this, &[]).block(&mut out, &b.body, 1);
        out.push_str("END_FUNCTION_BLOCK\n\n");
    }
    for (k, c) in d.classes.iter().enumerate() {
        match c.parent {
            Some(p) => {
                let _ = writeln!(out, "CLASS {} EXTENDS {}", c.name, d.classes[p].name);
            }
            None => {
                let _ = writeln!(out, "CLASS {}", c.name);
            }
        }
        if !c.vars.is_empty() {
            out.push_str("VAR\n");
            for v in &c.vars {
                decl(&mut out, v, !matches!(v.ty, Ty::Struct(_)));
            }
            out.push_str("END_VAR\n");
        }
        let visible = d.class_vars(k);
        for (sig, def) in c.families.iter().zip(&c.defs) {
            let Some(def) = def else {
                continue;
            };
            let head = match def.overrides {
                true => Signature {
                    name: format!("OVERRIDE {}", sig.name),
                    ..sig.clone()
                },
                false => sig.clone(),
            };
            signature(&mut out, &head);
            let n = framed(&visible, c.through_this, &sig.params);
            n.block(&mut out, &def.body, 2);
            let _ = writeln!(
                out,
                "        {} := {};\n    END_METHOD",
                sig.name,
                n.expr(&def.result)
            );
        }
        out.push_str("END_CLASS\n\n");
    }
    if !d.walkers.is_empty() {
        out.push_str(BOUNDS);
    }
    for w in &d.walkers {
        write_walker(&mut out, d, w);
    }
    for (k, r) in d.recursions.iter().enumerate() {
        write_recursion(&mut out, k, r);
    }
    for (k, cycle) in d.cycles.iter().enumerate() {
        write_cycle(&mut out, k, *cycle);
    }
    out.push_str("PROGRAM P\n");
    externals(&mut out);
    let program: Vec<&Constant> = d.constants.iter().filter(|c| !c.global).collect();
    if !program.is_empty() {
        out.push_str("VAR CONSTANT\n");
        for c in program.iter().rev() {
            let _ = writeln!(
                out,
                "    {} : {} := {};",
                c.name,
                c.ty.name(),
                constants.expr(&c.expr)
            );
        }
        out.push_str("END_VAR\n");
    }
    out.push_str("VAR\n");
    for (i, v) in vars.iter().enumerate() {
        match d.var_inits.iter().find(|(at, _)| *at == i) {
            Some((_, init)) => start(&mut out, v, init),
            None => decl(&mut out, v, !matches!(v.ty, Ty::Struct(_))),
        }
    }
    for a in &d.arrays {
        let range = match &a.bounds {
            Some((lo, hi)) => format!("{}..{}", constants.expr(lo), constants.expr(hi)),
            None => format!("{}..{}", a.lo, a.lo + a.init.len() as i128 - 1),
        };
        match a.ty {
            // Elements of a STRUCT type take the TYPE's defaults.
            Ty::Struct(_) => {
                let _ = writeln!(out, "    {} : ARRAY[{range}] OF {};", a.name, a.ty.name());
            }
            _ => {
                let values = written_elements(a.ty, &a.init, &a.written, |i| {
                    (a.lo + i as i128).to_string()
                });
                let _ = writeln!(
                    out,
                    "    {} : ARRAY[{range}] OF {}{};",
                    a.name,
                    a.ty.name(),
                    values
                );
            }
        }
    }
    for i in &d.instances {
        let _ = writeln!(out, "    {} : {};", i.name, d.blocks[i.block].name);
    }
    for o in &d.objects {
        let _ = writeln!(out, "    {} : {};", o.name, d.classes[o.class].name);
    }
    // Declared unset: rk leaves a PROGRAM reference's REF() initializer NULL.
    for r in &d.refs {
        let _ = writeln!(out, "    {} : REF_TO {};", r.name, r.target.name());
    }
    for m in &d.matrices {
        let ranges = format!(
            "{}..{}, {}..{}",
            m.lo[0],
            m.lo[0] + m.len[0] as i128 - 1,
            m.lo[1],
            m.lo[1] + m.len[1] as i128 - 1
        );
        let _ = match m.ty {
            Ty::Struct(_) => writeln!(out, "    {} : ARRAY[{ranges}] OF {};", m.name, m.ty.name()),
            _ => {
                let values = written_elements(m.ty, &m.init, &m.written, |i| {
                    format!(
                        "{}, {}",
                        m.lo[0] + (i / m.len[1]) as i128,
                        m.lo[1] + (i % m.len[1]) as i128
                    )
                });
                writeln!(
                    out,
                    "    {} : ARRAY[{ranges}] OF {}{};",
                    m.name,
                    m.ty.name(),
                    values
                )
            }
        };
    }
    for i in &d.walker_instances {
        let _ = writeln!(out, "    {} : {};", i.name, d.walkers[i.walker].name);
    }
    for (k, r) in d.recursions.iter().enumerate() {
        let _ = match r.route {
            Route::Super => writeln!(out, "    ro{k} : rd{k};"),
            Route::Interface => writeln!(out, "    ro{k} : rc{k};"),
            Route::InOut => writeln!(out, "    na{k} : node{k};\n    nb{k} : node{k};"),
            Route::Ref => writeln!(out, "    nn{k} : node{k};"),
            _ => Ok(()),
        };
    }
    out.push_str("END_VAR\n");
    names(vars).block(&mut out, body, 1);
    out.push_str("END_PROGRAM\n\nCONFIGURATION C\n");
    let global: Vec<&Constant> = d.constants.iter().filter(|c| c.global).collect();
    if !global.is_empty() {
        out.push_str("    VAR_GLOBAL CONSTANT\n");
        for c in global.iter().rev() {
            let _ = writeln!(
                out,
                "        {} : {} := {};",
                c.name,
                c.ty.name(),
                constants.expr(&c.expr)
            );
        }
        out.push_str("    END_VAR\n");
    }
    if !d.globals.is_empty() {
        out.push_str("    VAR_GLOBAL\n");
        for g in &d.globals {
            let _ = writeln!(
                out,
                "        {} : {} := {};",
                g.name,
                g.ty.name(),
                constants.expr(&g.expr)
            );
        }
        out.push_str("    END_VAR\n");
    }
    out.push_str(
        "    RESOURCE R ON CPU\n        TASK T(INTERVAL := T#10ms, PRIORITY := 1);\n        PROGRAM Run WITH T : P;\n    END_RESOURCE\nEND_CONFIGURATION\n",
    );
    out
}

/// Whether `e` names a CONSTANT, which its POU then declares.
fn names_constant(e: &Expr) -> bool {
    match e {
        Expr::Const(_) => true,
        Expr::Arith(_, _, a, b) | Expr::Bits(_, _, a, b) | Expr::Pow(a, b) => {
            names_constant(a) || names_constant(b)
        }
        Expr::Neg(_, a) => names_constant(a),
        _ => false,
    }
}

/// The bounds every walker reads, as the standard library's `Std.Arrays`
/// declares them.
const BOUNDS: &str = "\
FUNCTION LOWER_BOUND : DINT
VAR_INPUT
    ARR : ARRAY[*];
    DIM : DINT;
END_VAR
VAR
    dimensions : DINT;
END_VAR
    {wasm 'array.dimensions' (params ARR) (result dimensions)}
    IF DIM < 1 OR DIM > dimensions THEN
        __RAISE('array dimension out of range');
    END_IF;
    {wasm 'array.lower_bound' (params ARR DIM) (result LOWER_BOUND)}
END_FUNCTION

FUNCTION UPPER_BOUND : DINT
VAR_INPUT
    ARR : ARRAY[*];
    DIM : DINT;
END_VAR
VAR
    dimensions : DINT;
END_VAR
    {wasm 'array.dimensions' (params ARR) (result dimensions)}
    IF DIM < 1 OR DIM > dimensions THEN
        __RAISE('array dimension out of range');
    END_IF;
    {wasm 'array.upper_bound' (params ARR DIM) (result UPPER_BOUND)}
END_FUNCTION

";

/// A walker's declaration: `a`, `k` when it writes, its counters, then
/// the walk over `acc`.
fn write_walker(out: &mut String, d: &Decls, w: &Walker) {
    let (view, result) = (w.view(d), w.result(d));
    // A METHOD's sections sit one level in.
    let pad = match w.host {
        Host::Method => "    ",
        _ => "",
    };
    let acc = match w.host {
        Host::Function => w.name.as_str(),
        _ => "acc",
    };
    let _ = match w.host {
        Host::Function => writeln!(out, "FUNCTION {} : {}", w.name, result.name()),
        Host::Block => writeln!(out, "FUNCTION_BLOCK {}", w.name),
        Host::Method => writeln!(
            out,
            "CLASS {}\nVAR\n    acc : {};\nEND_VAR\n    METHOD run : {}",
            w.name,
            result.name(),
            result.name()
        ),
    };
    let section = |out: &mut String, name: &str, lines: &[String]| {
        let _ = writeln!(out, "{pad}{name}");
        for line in lines {
            let _ = writeln!(out, "{pad}    {line}");
        }
        let _ = writeln!(out, "{pad}END_VAR");
    };
    let dims = vec!["*"; w.rank].join(", ");
    let array = format!("a : ARRAY[{dims}] OF {};", w.elem.name());
    section(
        out,
        if w.in_out { "VAR_IN_OUT" } else { "VAR_INPUT" },
        &[array],
    );
    if w.writes {
        section(out, "VAR_INPUT", &[format!("k : {};", view.name())]);
    }
    if w.host == Host::Block {
        section(out, "VAR_OUTPUT", &[format!("acc : {};", result.name())]);
    }
    let mut locals = vec!["i : DINT;".to_string()];
    if w.rank == 2 {
        locals.push("j : DINT;".into());
    }
    if w.writes {
        locals.push(format!("prev : {};", view.name()));
        if let Ty::Str(_) = view {
            locals.push(format!("t : {};", view.name()));
        }
    }
    section(out, "VAR", &locals);

    let mut level = format!("{pad}    ");
    if let Some(j) = w.pass {
        let callee = &d.walkers[j];
        let k = if callee.writes { ", k := k" } else { "" };
        let _ = writeln!(out, "{level}{acc} := {}(a := a{k});", callee.name);
    }
    if w.writes {
        let _ = writeln!(out, "{level}prev := k;");
    }
    let loops: &[(&str, usize)] = match (w.rank, w.transposed) {
        (1, _) => &[("i", 1)],
        (_, false) => &[("i", 1), ("j", 2)],
        (_, true) => &[("j", 2), ("i", 1)],
    };
    for (counter, dim) in loops {
        let range = match w.backward {
            false => format!("LOWER_BOUND(a, {dim}) TO UPPER_BOUND(a, {dim})"),
            true => format!("UPPER_BOUND(a, {dim}) TO LOWER_BOUND(a, {dim}) BY -1"),
        };
        let _ = writeln!(out, "{level}FOR {counter} := {range} DO");
        level.push_str("    ");
    }
    let element = match w.rank {
        1 => "a[i]".to_string(),
        _ => "a[i, j]".to_string(),
    };
    let v = match w.field {
        Some(f) => format!("{element}.f{f}"),
        None => element,
    };
    let count = format!("{acc} := {acc} + 1;");
    let steps: Vec<String> = match (w.writes, view) {
        (false, Ty::Bool) => vec![format!("{acc} := (NOT {acc}) OR {v};")],
        (false, Ty::Str(_)) => vec![
            format!("IF {v} > {acc} THEN"),
            format!("    {acc} := {v};"),
            "END_IF;".into(),
        ],
        (false, t) => {
            let factor = match t {
                Ty::Real => Val::F32(2.0),
                Ty::Lreal => Val::F64(2.0),
                _ => Val::Int(3),
            };
            vec![format!("{acc} := {acc} * {} + {v};", literal(t, &factor))]
        }
        (true, Ty::Bool) => vec![
            format!("prev := prev XOR {v};"),
            format!("{v} := prev;"),
            count,
        ],
        (true, Ty::Str(_)) => vec![
            format!("t := {v};"),
            format!("{v} := prev;"),
            "prev := t;".into(),
            count,
        ],
        (true, _) => vec![
            format!("prev := prev + {v};"),
            format!("{v} := prev;"),
            count,
        ],
    };
    for step in steps {
        let _ = writeln!(out, "{level}{step}");
    }
    for _ in loops {
        level.truncate(level.len() - 4);
        let _ = writeln!(out, "{level}END_FOR;");
    }
    match w.host {
        Host::Function => out.push_str("END_FUNCTION\n\n"),
        Host::Block => out.push_str("END_FUNCTION_BLOCK\n\n"),
        Host::Method => {
            let _ = writeln!(out, "{level}run := acc;\n    END_METHOD\nEND_CLASS\n");
        }
    }
}

/// Recursion `k`: `rec{k}` and the `hop{k}` it goes through, or the
/// CLASSes `rb{k}` and `rd{k}`. Each call writes what it keeps before it
/// calls itself, and reads it after.
fn write_recursion(out: &mut String, k: usize, r: &Recursion) {
    let (decls, keep, read): (Vec<String>, Vec<String>, String) = match r.keep {
        Keep::Array { len, at } => (
            vec![format!("keep : ARRAY[0..{}] OF DINT;", len - 1)],
            vec![format!("keep[{at}] := seed + n;")],
            format!("keep[{at}]"),
        ),
        Keep::Text { cap } => (
            vec![format!("keep : STRING[{cap}];"), "own : DINT;".into()],
            [
                "IF (seed + n) MOD 2 = 0 THEN",
                "    keep := 'even';",
                "ELSE",
                "    keep := 'odd';",
                "END_IF;",
            ]
            .map(String::from)
            .to_vec(),
            "own".into(),
        ),
        Keep::Addressed => (
            vec!["own : DINT;".into(), "r : REF_TO DINT;".into()],
            vec!["r := REF(own);".into(), "own := seed + n;".into()],
            "r^".into(),
        ),
    };
    // A STRING is read back as a number once the call below has returned.
    let reread: &[&str] = match r.keep {
        Keep::Text { .. } => &[
            "IF keep = 'even' THEN",
            "    own := 1;",
            "ELSE",
            "    own := 2;",
            "END_IF;",
        ],
        _ => &[],
    };
    let next = match r.add < 0 {
        true => format!("n := n - 1, seed := seed * {} - {}", r.mul, -r.add),
        false => format!("n := n - 1, seed := seed * {} + {}", r.mul, r.add),
    };
    let inputs = "VAR_INPUT\n    n : DINT;\n    seed : DINT;\nEND_VAR\n";
    let factor = r.factor;
    // The body that keeps, calls `call` while `n > 0`, and returns into
    // `result`, its lines indented by `pad`. A block's body keeps in its
    // VAR_TEMP, which starts over at each call.
    let body = |out: &mut String, pad: &str, section: &str, call: &str, result: &str| {
        let _ = writeln!(out, "{pad}{section}");
        for decl in &decls {
            let _ = writeln!(out, "{pad}    {decl}");
        }
        let _ = writeln!(out, "{pad}    below : DINT;\n{pad}END_VAR");
        for line in &keep {
            let _ = writeln!(out, "{pad}    {line}");
        }
        let _ = writeln!(out, "{pad}    IF n > 0 THEN");
        for line in call.replace("{next}", &next).lines() {
            let _ = writeln!(out, "{pad}        {line}");
        }
        let _ = writeln!(out, "{pad}    END_IF;");
        for line in reread {
            let _ = writeln!(out, "{pad}    {line}");
        }
        let _ = writeln!(out, "{pad}    {result} := below * {factor} + {read};");
    };
    match r.route {
        Route::Direct | Route::Hop => {
            let _ = write!(out, "FUNCTION rec{k} : DINT\n{inputs}");
            let call = match r.route {
                Route::Direct => format!("rec{k}"),
                _ => format!("hop{k}"),
            };
            body(
                out,
                "",
                "VAR",
                &format!("below := {call}({{next}});"),
                &format!("rec{k}"),
            );
            out.push_str("END_FUNCTION\n\n");
            if r.route == Route::Hop {
                let _ = writeln!(
                    out,
                    "FUNCTION hop{k} : DINT\n{inputs}    hop{k} := rec{k}(n := n, seed := seed);\nEND_FUNCTION\n"
                );
            }
        }
        Route::Initializer => {
            let _ = writeln!(
                out,
                "FUNCTION rec{k} : DINT\n{inputs}VAR\n    own : DINT := seed + n;\n    r : REF_TO DINT := REF(own);\n    below : DINT := hop{k}(n := n, seed := seed);\nEND_VAR\n    rec{k} := below * {factor} + r^;\nEND_FUNCTION\n"
            );
            let _ = writeln!(
                out,
                "FUNCTION hop{k} : DINT\n{inputs}    IF n > 0 THEN\n        hop{k} := rec{k}({next});\n    END_IF;\nEND_FUNCTION\n"
            );
        }
        Route::Super => {
            let nested: String = inputs.lines().map(|line| format!("    {line}\n")).collect();
            let _ = write!(out, "CLASS rb{k}\n    METHOD PUBLIC climb : DINT\n{nested}");
            body(out, "    ", "VAR", "below := THIS.climb({next});", "climb");
            out.push_str("    END_METHOD\nEND_CLASS\n\n");
            let _ = writeln!(
                out,
                "CLASS rd{k} EXTENDS rb{k}\n    METHOD PUBLIC OVERRIDE climb : DINT\n{nested}        climb := SUPER.climb(n := n, seed := seed) + 1;\n    END_METHOD\nEND_CLASS\n"
            );
        }
        Route::Interface => {
            let nested: String = inputs.lines().map(|line| format!("    {line}\n")).collect();
            let _ = writeln!(
                out,
                "INTERFACE IR{k}\n    METHOD climb : DINT\n{nested}    END_METHOD\nEND_INTERFACE\n"
            );
            let _ = write!(
                out,
                "FUNCTION rec{k} : DINT\nVAR_IN_OUT\n    dev : IR{k};\nEND_VAR\n{inputs}"
            );
            body(
                out,
                "",
                "VAR",
                "below := dev.climb({next});",
                &format!("rec{k}"),
            );
            out.push_str("END_FUNCTION\n\n");
            let _ = writeln!(
                out,
                "CLASS rc{k} IMPLEMENTS IR{k}\n    METHOD PUBLIC climb : DINT\n{nested}        climb := rec{k}(dev := THIS, n := n, seed := seed);\n    END_METHOD\nEND_CLASS\n"
            );
        }
        Route::InOut | Route::Ref => {
            let _ = write!(
                out,
                "FUNCTION_BLOCK node{k}\n{inputs}VAR_OUTPUT\n    total : DINT;\nEND_VAR\n"
            );
            let call = match r.route {
                Route::InOut => {
                    out.push_str("VAR_IN_OUT\n    peer : node");
                    let _ = writeln!(out, "{k};\nEND_VAR");
                    format!("below := visit{k}(c := peer, {{next}});")
                }
                _ => {
                    out.push_str("VAR\n    next : REF_TO node");
                    let _ = writeln!(out, "{k};\nEND_VAR");
                    // Set from outside, so the body checks it (E0902).
                    "IF next <> NULL THEN\n    next^({next});\n    below := next^.total;\nEND_IF;"
                        .to_string()
                }
            };
            body(out, "", "VAR_TEMP", &call, "total");
            out.push_str("END_FUNCTION_BLOCK\n\n");
            if r.route == Route::InOut {
                let _ = writeln!(
                    out,
                    "FUNCTION visit{k} : DINT\nVAR_IN_OUT\n    c : node{k};\nEND_VAR\n{inputs}    c(n := n, seed := seed, peer := c);\n    visit{k} := c.total;\nEND_FUNCTION\n"
                );
            }
        }
        Route::SuperBody => {
            let _ = write!(
                out,
                "FUNCTION_BLOCK base{k}\n{inputs}VAR_OUTPUT\n    total : DINT;\nEND_VAR\n"
            );
            body(
                out,
                "",
                "VAR_TEMP",
                &format!("below := kick{k}({{next}});"),
                "total",
            );
            out.push_str("END_FUNCTION_BLOCK\n\n");
            let _ = writeln!(
                out,
                "FUNCTION_BLOCK derived{k} EXTENDS base{k}\n    SUPER();\nEND_FUNCTION_BLOCK\n"
            );
            let _ = writeln!(
                out,
                "FUNCTION kick{k} : DINT\n{inputs}VAR\n    d : derived{k};\nEND_VAR\n    d(n := n, seed := seed);\n    kick{k} := d.total;\nEND_FUNCTION\n"
            );
        }
    }
}

/// One header line per scalar; a STRUCT writes one per field.
fn expect(out: &mut String, d: &Decls, path: &str, ty: Ty, value: &Val) {
    match (ty, value) {
        (Ty::Struct(k), Val::Struct(fields)) => {
            for (f, v) in d.structs[k].iter().zip(fields) {
                expect(out, d, &format!("{path}.{}", f.name), f.ty, v);
            }
        }
        _ => {
            let _ = writeln!(out, "{path} = {}", shown(ty, value));
        }
    }
}

/// A value as the header writes it, and as [`crate::semantics`] writes
/// the debugger's value back.
fn shown(ty: Ty, v: &Val) -> String {
    match (ty, v) {
        (Ty::Bool, v) => (if v.truthy() { "TRUE" } else { "FALSE" }).to_string(),
        (_, Val::Int(v)) => v.to_string(),
        (_, Val::F32(v)) => format!("{v:?}"),
        (_, Val::F64(v)) => format!("{v:?}"),
        (_, Val::Str(s)) => format!("'{}'", String::from_utf8_lossy(s)),
        (_, Val::Struct(_)) => unreachable!("a STRUCT is shown by field"),
    }
}

/// The initializer of a program array, ` := [...]` with each element as
/// it is written: in order, at its index (`index` names an element's), or
/// left out. No initializer at all when every element is left out.
fn written_elements(
    ty: Ty,
    init: &[Val],
    written: &[Written],
    index: impl Fn(usize) -> String,
) -> String {
    let values: Vec<String> = init
        .iter()
        .zip(written)
        .enumerate()
        .filter_map(|(i, (v, how))| match how {
            Written::Default => None,
            Written::Next => Some(literal(ty, v)),
            Written::AtIndex => Some(format!("[{}] := {}", index(i), literal(ty, v))),
        })
        .collect();
    if values.is_empty() {
        String::new()
    } else {
        format!(" := [{}]", values.join(", "))
    }
}

fn literal(ty: Ty, v: &Val) -> String {
    match (ty, v) {
        (Ty::Bool, _) => shown(ty, v),
        (Ty::Named(..), _) => unreachable!("a named value is written by its name"),
        (Ty::Real, Val::F32(x)) => format!("REAL#{}", exact(*x as f64)),
        (Ty::Lreal, Val::F64(x)) => format!("LREAL#{}", exact(*x)),
        (_, Val::Str(s)) => format!("'{}'", String::from_utf8_lossy(s)),
        (_, Val::Int(x)) => format!("{}#{x}", ty.name()),
        _ => unreachable!("no literal for {ty:?}"),
    }
}

/// The exact decimal of a dyadic `n / 2^m` with `m <= 8`, as the
/// generator makes them.
fn exact(x: f64) -> String {
    let scaled = (x * 256.0) as i128;
    let sign = if scaled < 0 { "-" } else { "" };
    let digits = format!("{:0>9}", scaled.unsigned_abs() * 5u128.pow(8));
    let (int, frac) = digits.split_at(digits.len() - 8);
    match frac.trim_end_matches('0') {
        "" => format!("{sign}{int}.0"),
        frac => format!("{sign}{int}.{frac}"),
    }
}

/// Types that come back to themselves through a reference, and what a FUNCTION
/// does with them: `K` is its number. It answers `n + 1` when the statements hold
/// what they should, and `n` when not, so a wrong read shows in a value.
const CYCLES: [(&str, &str, &str, &str); 7] = [
    (
        "ca_K : ARRAY[0..1] OF REF_TO ca_K;",
        "a, b : ca_K; r : REF_TO ca_K;",
        "a[0] := REF(b); a[0]^[1] := REF(a); r := REF(a);",
        "b[1] = r AND a[0]^[1]^[0] = a[0]",
    ),
    (
        "cp_K : REF_TO cp_K;",
        "a, b, c : cp_K; r : REF_TO cp_K;",
        "a := REF(b); b := REF(a); c := a; r := REF(b);",
        "c = r AND a^ = REF(a) AND a^^ = r",
    ),
    (
        "cp_K : REF_TO cq_K; cq_K : REF_TO cp_K;",
        "a : cp_K; b : cq_K;",
        "a := REF(b); b := REF(a);",
        "a^ = b AND a^^ = a",
    ),
    (
        "ca_K : cb_K; cb_K : ARRAY[0..1] OF REF_TO ca_K;",
        "a : ca_K; b : cb_K; r : REF_TO ca_K;",
        "a[0] := REF(a); b := a; r := REF(a);",
        "b[0] = r",
    ),
    (
        "ca_K : ARRAY[0..1] OF cr_K; cr_K : REF_TO ca_K;",
        "a, b : ca_K; r : cr_K;",
        "a[0] := REF(b); a[0]^[1] := REF(a); r := REF(a);",
        "b[1] = r",
    ),
    (
        "ca_K : ARRAY[0..1] OF cb_K; cb_K : ARRAY[0..1] OF REF_TO ca_K;",
        "a, b : ca_K; r : REF_TO ca_K;",
        "a[0][0] := REF(b); a[0][0]^[1][1] := REF(a); r := REF(a);",
        "b[1][1] = r",
    ),
    (
        "ca_K : ARRAY[0..1] OF cs_K; cs_K : STRUCT again : REF_TO ca_K; m : DINT; END_STRUCT;",
        "a, b : ca_K;",
        "a[0].again := REF(b); a[0].again^[1].m := n;",
        "b[1].m = n",
    ),
];

fn write_cycle(out: &mut String, k: usize, cycle: usize) {
    let (types, vars, statements, holds) = CYCLES[cycle];
    let text = format!(
        "TYPE\n    {types}\nEND_TYPE\n\nFUNCTION cycle{k} : DINT\nVAR_INPUT\n    n : DINT;\nEND_VAR\nVAR\n    {vars}\nEND_VAR\n    {statements}\n    IF {holds} THEN\n        cycle{k} := n + 1;\n    ELSE\n        cycle{k} := n;\n    END_IF;\nEND_FUNCTION\n\n"
    );
    out.push_str(&text.replace("_K", &k.to_string()));
}

/// What the names in a frame's code refer to.
struct Names<'a> {
    vars: Vec<String>,
    d: &'a Decls,
}

impl Names<'_> {
    /// A variable's name where only a name goes: a FOR's counter, a `__CATCH`.
    fn bare(&self, v: usize) -> &str {
        let name = &self.vars[v];
        name.strip_prefix("THIS.").unwrap_or(name)
    }

    fn place(&self, p: &Place) -> String {
        match p {
            Place::Var(v) => self.vars[*v].clone(),
            Place::Index(a, index) => {
                format!("{}[{}]", self.d.arrays[*a].name, self.expr(index))
            }
            Place::Member(i, m) => {
                let inst = &self.d.instances[*i];
                format!(
                    "{}.{}",
                    inst.name, self.d.blocks[inst.block].members[*m].name
                )
            }
            Place::Field(base, f) => format!("{}.f{f}", self.place(base)),
            Place::Deref(r) => format!("{}^", self.d.refs[*r].name),
        }
    }

    fn expr(&self, e: &Expr) -> String {
        match e {
            Expr::Lit(ty, v) => literal(*ty, v),
            Expr::Read(p) => self.place(p),
            Expr::Arith(op, _, a, b) => {
                let op = match op {
                    Arith::Add => "+",
                    Arith::Sub => "-",
                    Arith::Mul => "*",
                    Arith::Div => "/",
                    Arith::Mod => "MOD",
                };
                format!("({} {op} {})", self.expr(a), self.expr(b))
            }
            Expr::Neg(_, a) => format!("(-{})", self.expr(a)),
            Expr::Cmp(op, a, b) => {
                let op = match op {
                    Cmp::Eq => "=",
                    Cmp::Ne => "<>",
                    Cmp::Lt => "<",
                    Cmp::Gt => ">",
                    Cmp::Le => "<=",
                    Cmp::Ge => ">=",
                };
                format!("({} {op} {})", self.expr(a), self.expr(b))
            }
            Expr::Not(a) => format!("(NOT {})", self.expr(a)),
            Expr::Logic(op, a, b) => {
                let op = match op {
                    Logic::And => "AND",
                    Logic::Or => "OR",
                    Logic::Xor => "XOR",
                };
                format!("({} {op} {})", self.expr(a), self.expr(b))
            }
            Expr::Call(f, args) => {
                let f = &self.d.functions[*f];
                if f.variadic {
                    let args: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                    format!("{}({})", f.name, args.join(", "))
                } else {
                    format!("{}({})", f.name, self.args(&f.inputs, args))
                }
            }
            Expr::LibCall(f, args, form) => {
                let args: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}({})", f.spelled(*form), args.join(", "))
            }
            Expr::SameRef(equal, a, b) => {
                let side = |s: &RefSide| match s {
                    RefSide::Held(r) => self.d.refs[*r].name.clone(),
                    RefSide::Of(p, false) => format!("REF({})", self.place(p)),
                    RefSide::Of(p, true) => format!("(REF({}))", self.place(p)),
                    RefSide::Null => "NULL".to_string(),
                };
                let op = if *equal { "=" } else { "<>" };
                format!("({} {op} {})", side(a), side(b))
            }
            Expr::Cycle(k, n) => format!("cycle{k}({})", self.expr(n)),
            Expr::Const(c) => self.d.constants[*c].name.clone(),
            Expr::Named(k, j) => format!("E{k}#n{j}"),
            Expr::Bits(op, _, a, b) => {
                let op = match op {
                    Logic::And => "AND",
                    Logic::Or => "OR",
                    Logic::Xor => "XOR",
                };
                format!("({} {op} {})", self.expr(a), self.expr(b))
            }
            Expr::Pow(a, b) => format!("({} ** {})", self.expr(a), self.expr(b)),
            Expr::Fold(op, _) => format!("...{}{}", self.vars[0], op.symbol()),
            Expr::Element => format!("...{}[{}]", self.vars[1], self.vars[0]),
            Expr::Bit(p, _, bit, bare) => match bare {
                true => format!("{}.{bit}", self.place(p)),
                false => format!("{}.%X{bit}", self.place(p)),
            },
        }
    }

    fn args(&self, params: &[Var], args: &[Expr]) -> String {
        params
            .iter()
            .zip(args)
            .map(|(p, a)| format!("{} := {}", p.name, self.expr(a)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn bound(&self, bound: Bound) -> String {
        match bound {
            Bound::Array(a) => self.d.arrays[a].name.clone(),
            Bound::Matrix(m) => self.d.matrices[m].name.clone(),
            Bound::Row(m, row) => format!("{}[{row}]", self.d.matrices[m].name),
        }
    }

    fn block(&self, out: &mut String, block: &[Stmt], level: usize) {
        let pad = "    ".repeat(level);
        for stmt in block {
            match stmt {
                Stmt::Assign(p, e) => {
                    let _ = writeln!(out, "{pad}{} := {};", self.place(p), self.expr(e));
                }
                Stmt::Chain(places, e) => {
                    let _ = write!(out, "{pad}");
                    for (p, op) in places {
                        let op = op.map_or(":=", compound_operator);
                        let _ = write!(out, "{} {op} ", self.place(p));
                    }
                    let _ = writeln!(out, "{};", self.expr(e));
                }
                Stmt::Compound(p, op, e) => {
                    let op = compound_operator(*op);
                    let _ = writeln!(out, "{pad}{} {op} {};", self.place(p), self.expr(e));
                }
                Stmt::SetBit(p, _, bit, bare, e) => {
                    let part = match bare {
                        true => format!("{bit}"),
                        false => format!("%X{bit}"),
                    };
                    let _ = writeln!(out, "{pad}{}.{part} := {};", self.place(p), self.expr(e));
                }
                Stmt::MoveSlice(to, from, width) => {
                    let w = width_letter(*width);
                    let _ = writeln!(
                        out,
                        "{pad}{}.%{w}{} := {}.%{w}{};",
                        self.place(&to.place),
                        to.offset,
                        self.place(&from.place),
                        from.offset
                    );
                }
                Stmt::CallFb(i, args, bound) => {
                    let inst = &self.d.instances[*i];
                    let block = &self.d.blocks[inst.block];
                    let inputs: Vec<Var> = block
                        .inputs()
                        .into_iter()
                        .map(|m| block.members[m].clone())
                        .collect();
                    let mut parts = self.args(&inputs, args);
                    if let (Some(m), Some(v)) = (block.inout(), bound) {
                        if !parts.is_empty() {
                            parts.push_str(", ");
                        }
                        let _ = write!(parts, "{} := {}", block.members[m].name, self.vars[*v]);
                    }
                    let _ = writeln!(out, "{pad}{}({parts});", inst.name);
                }
                Stmt::CallMethod(target, i, m, args) => {
                    let inst = &self.d.instances[*i];
                    let sig = &self.d.blocks[inst.block].methods[*m].sig;
                    let _ = writeln!(
                        out,
                        "{pad}{} := {}.{}({});",
                        self.place(target),
                        inst.name,
                        sig.name,
                        self.args(&sig.params, args)
                    );
                }
                Stmt::CallUser(target, u, i, args) => {
                    let user = &self.d.users[*u];
                    let mut parts = format!("dev := {}", self.d.instances[*i].name);
                    if !args.is_empty() {
                        let _ = write!(parts, ", {}", self.args(&user.inputs, args));
                    }
                    let _ = writeln!(
                        out,
                        "{pad}{} := {}({parts});",
                        self.place(target),
                        user.name
                    );
                }
                Stmt::CallObject(target, o, j, args) => {
                    let object = &self.d.objects[*o];
                    let sig = &self.d.classes[object.class].families[*j];
                    let _ = writeln!(
                        out,
                        "{pad}{} := {}.{}({});",
                        self.place(target),
                        object.name,
                        sig.name,
                        self.args(&sig.params, args)
                    );
                }
                Stmt::CallThis(target, i, args, via_super) => {
                    // Parameters are `p0`, `p1`… in every signature.
                    let args: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(k, a)| format!("p{k} := {}", self.expr(a)))
                        .collect();
                    let who = if *via_super { "SUPER" } else { "THIS" };
                    let _ = writeln!(
                        out,
                        "{pad}{} := {who}.m{i}({});",
                        self.place(target),
                        args.join(", ")
                    );
                }
                Stmt::SetRef(r, place) => {
                    let _ = writeln!(
                        out,
                        "{pad}{} := REF({});",
                        self.d.refs[*r].name,
                        self.place(place)
                    );
                }
                Stmt::Walk(call) => {
                    let walker = &self.d.walkers[call.walker];
                    let mut args = format!("a := {}", self.bound(call.bound));
                    if let Some(k) = &call.k {
                        let _ = write!(args, ", k := {}", self.expr(k));
                    }
                    let callee = match call.instance {
                        Some(i) => self.d.walker_instances[i].name.as_str(),
                        None => walker.name.as_str(),
                    };
                    let _ = match (&call.target, walker.host) {
                        (Some(target), Host::Method) => {
                            writeln!(out, "{pad}{} := {callee}.run({args});", self.place(target))
                        }
                        (Some(target), _) => {
                            writeln!(out, "{pad}{} := {callee}({args});", self.place(target))
                        }
                        (None, _) => writeln!(out, "{pad}{callee}({args});"),
                    };
                }
                Stmt::Recurse(call) => {
                    let k = call.recursion;
                    let target = self.place(&call.target);
                    let args = format!("n := {}, seed := {}", call.depth, self.expr(&call.seed));
                    let _ = match self.d.recursions[k].route {
                        Route::Super => writeln!(out, "{pad}{target} := ro{k}.climb({args});"),
                        Route::Interface => {
                            writeln!(out, "{pad}{target} := rec{k}(dev := ro{k}, {args});")
                        }
                        Route::InOut => writeln!(
                            out,
                            "{pad}na{k}({args}, peer := nb{k});\n{pad}{target} := na{k}.total;"
                        ),
                        Route::Ref => writeln!(
                            out,
                            "{pad}nn{k}.next := REF(nn{k});\n{pad}nn{k}({args});\n{pad}{target} := nn{k}.total;"
                        ),
                        Route::SuperBody => writeln!(out, "{pad}{target} := kick{k}({args});"),
                        _ => writeln!(out, "{pad}{target} := rec{k}({args});"),
                    };
                }
                Stmt::If(arms, otherwise) => {
                    for (i, (cond, body)) in arms.iter().enumerate() {
                        let kw = if i == 0 { "IF" } else { "ELSIF" };
                        let _ = writeln!(out, "{pad}{kw} {} THEN", self.expr(cond));
                        self.block(out, body, level + 1);
                    }
                    if let Some(body) = otherwise {
                        let _ = writeln!(out, "{pad}ELSE");
                        self.block(out, body, level + 1);
                    }
                    let _ = writeln!(out, "{pad}END_IF;");
                }
                Stmt::Case(selector, arms, otherwise) => {
                    let _ = writeln!(out, "{pad}CASE {} OF", self.expr(selector));
                    for (labels, body) in arms {
                        let labels: Vec<String> = labels
                            .iter()
                            .map(|l| match l {
                                Label::Int(v, None) => v.to_string(),
                                Label::Int(_, Some(e)) => self.expr(e),
                                Label::Range(a, b, None) => format!("{a}..{b}"),
                                Label::Range(_, _, Some((a, b))) => {
                                    format!("{}..{}", self.expr(a), self.expr(b))
                                }
                                Label::Str(s) => format!("'{}'", String::from_utf8_lossy(s)),
                            })
                            .collect();
                        let _ = writeln!(out, "{pad}{}:", labels.join(", "));
                        self.block(out, body, level + 1);
                    }
                    if let Some(body) = otherwise {
                        let _ = writeln!(out, "{pad}ELSE");
                        self.block(out, body, level + 1);
                    }
                    let _ = writeln!(out, "{pad}END_CASE;");
                }
                Stmt::For(v, from, to, by, step, body) => {
                    let by = match step {
                        Some(e) => self.expr(e),
                        None => by.to_string(),
                    };
                    let _ = writeln!(
                        out,
                        "{pad}FOR {} := {from} TO {to} BY {by} DO",
                        self.bare(*v)
                    );
                    self.block(out, body, level + 1);
                    let _ = writeln!(out, "{pad}END_FOR;");
                }
                Stmt::Exit => {
                    let _ = writeln!(out, "{pad}EXIT;");
                }
                Stmt::Continue => {
                    let _ = writeln!(out, "{pad}CONTINUE;");
                }
                Stmt::Return => {
                    let _ = writeln!(out, "{pad}RETURN;");
                }
                Stmt::Try(t) => {
                    let _ = writeln!(out, "{pad}__TRY");
                    self.block(out, &t.body, level + 1);
                    let _ = writeln!(
                        out,
                        "{pad}    __RAISE('{}');",
                        String::from_utf8_lossy(&t.message)
                    );
                    match t.variable {
                        Some(v) => {
                            let _ = writeln!(out, "{pad}__CATCH({})", self.bare(v));
                        }
                        None => {
                            let _ = writeln!(out, "{pad}__CATCH");
                        }
                    }
                    self.block(out, &t.handler, level + 1);
                    if let Some(finally) = &t.finally {
                        let _ = writeln!(out, "{pad}__FINALLY");
                        self.block(out, finally, level + 1);
                    }
                    let _ = writeln!(out, "{pad}__END_TRY;");
                }
            }
        }
    }
}

/// A frame's values: its variables, and for the program its arrays and FB
/// instances (one value per member).
struct State {
    vars: Vec<Val>,
    arrays: Vec<Vec<Val>>,
    instances: Vec<Vec<Val>>,
    /// Each class instance's variables, the inherited ones first.
    objects: Vec<Vec<Val>>,
    /// Where each program reference points.
    refs: Vec<Loc>,
    /// In a class method, the instance it runs on.
    this: Option<This>,
    /// Each matrix's elements, row by row.
    matrices: Vec<Vec<Val>>,
    /// The `acc` of each walker instance.
    walkers: Vec<Val>,
}

impl State {
    fn frame(vars: Vec<Val>) -> Self {
        Self {
            vars,
            arrays: Vec::new(),
            instances: Vec::new(),
            objects: Vec::new(),
            refs: Vec::new(),
            this: None,
            matrices: Vec::new(),
            walkers: Vec::new(),
        }
    }
}

/// A class method's instance. The frame holds the variables the defining
/// class sees, first; `tail` the ones only a derived class declares, which
/// a `THIS` call may reach.
#[derive(Clone)]
struct This {
    dynamic: usize,
    defining: usize,
    tail: Vec<Val>,
}

/// Where a reference points, its indexes fixed.
#[derive(Clone, Debug, PartialEq)]
enum Loc {
    Var(usize),
    Elem(usize, usize),
    Field(Box<Loc>, usize),
}

fn loc_of(p: &Place, st: &State, d: &Decls) -> Loc {
    match p {
        Place::Var(v) => Loc::Var(*v),
        Place::Index(a, index) => {
            Loc::Elem(*a, (eval(index, st, d).int() - d.arrays[*a].lo) as usize)
        }
        Place::Field(base, f) => Loc::Field(Box::new(loc_of(base, st, d)), *f),
        Place::Deref(r) => st.refs[*r].clone(),
        Place::Member(..) => unreachable!("REF() of an FB member is not generated"),
    }
}

fn loc_slot<'s>(l: &Loc, st: &'s mut State) -> &'s mut Val {
    match l {
        Loc::Var(v) => &mut st.vars[*v],
        Loc::Elem(a, k) => &mut st.arrays[*a][*k],
        Loc::Field(base, f) => match loc_slot(base, st) {
            Val::Struct(fields) => &mut fields[*f],
            _ => unreachable!("a field of a non-STRUCT"),
        },
    }
}

fn loc_read(l: &Loc, st: &State) -> Val {
    match l {
        Loc::Var(v) => st.vars[*v].clone(),
        Loc::Elem(a, k) => st.arrays[*a][*k].clone(),
        Loc::Field(base, f) => match loc_read(base, st) {
            Val::Struct(mut fields) => fields.swap_remove(*f),
            _ => unreachable!("a field of a non-STRUCT"),
        },
    }
}

/// How a statement ended.
enum Flow {
    Next,
    Exit,
    Continue,
    Return,
}

/// `v` as a variable of type `ty` holds it: integers wrapped, strings cut
/// to their capacity.
fn store(ty: Ty, v: Val) -> Val {
    match (ty, v) {
        (Ty::Str(cap), Val::Str(mut s)) => {
            s.truncate(cap as usize);
            Val::Str(s)
        }
        (t, Val::Int(x)) if t.is_int() || t == Ty::Bool => Val::Int(t.wrap(x)),
        (_, v) => v,
    }
}

fn place_ty(p: &Place, tys: &[Ty], d: &Decls) -> Ty {
    match p {
        Place::Var(v) => tys[*v],
        Place::Index(a, _) => d.arrays[*a].ty,
        Place::Member(i, m) => d.blocks[d.instances[*i].block].members[*m].ty,
        Place::Field(base, f) => match place_ty(base, tys, d) {
            Ty::Struct(k) => d.structs[k][*f].ty,
            _ => unreachable!("a field of a non-STRUCT"),
        },
        Place::Deref(r) => d.refs[*r].target,
    }
}

fn read(p: &Place, st: &State, d: &Decls) -> Val {
    match p {
        Place::Var(v) => st.vars[*v].clone(),
        Place::Index(a, index) => {
            let k = (eval(index, st, d).int() - d.arrays[*a].lo) as usize;
            st.arrays[*a][k].clone()
        }
        Place::Member(i, m) => st.instances[*i][*m].clone(),
        Place::Field(base, f) => match read(base, st, d) {
            Val::Struct(mut fields) => fields.swap_remove(*f),
            _ => unreachable!("a field of a non-STRUCT"),
        },
        Place::Deref(r) => loc_read(&st.refs[*r], st),
    }
}

/// The slot `p` names. An index has no side effect, so evaluating it
/// before taking the slot changes nothing.
fn slot<'s>(p: &Place, st: &'s mut State, d: &Decls) -> &'s mut Val {
    match p {
        Place::Var(v) => &mut st.vars[*v],
        Place::Index(a, index) => {
            let k = (eval(index, st, d).int() - d.arrays[*a].lo) as usize;
            &mut st.arrays[*a][k]
        }
        Place::Member(i, m) => &mut st.instances[*i][*m],
        Place::Field(base, f) => match slot(base, st, d) {
            Val::Struct(fields) => &mut fields[*f],
            _ => unreachable!("a field of a non-STRUCT"),
        },
        Place::Deref(r) => {
            let loc = st.refs[*r].clone();
            loc_slot(&loc, st)
        }
    }
}

fn write(p: &Place, v: Val, st: &mut State, tys: &[Ty], d: &Decls) {
    let ty = place_ty(p, tys, d);
    *slot(p, st, d) = store(ty, v);
}

/// `p` with its subscripts evaluated: the slot it names now, whatever they
/// read once something is written.
fn found(p: &Place, st: &State, d: &Decls) -> Place {
    match p {
        Place::Index(a, index) => {
            Place::Index(*a, Box::new(Expr::Lit(Ty::Dint, eval(index, st, d))))
        }
        Place::Field(base, f) => Place::Field(Box::new(found(base, st, d)), *f),
        Place::Var(_) | Place::Member(..) | Place::Deref(_) => p.clone(),
    }
}

/// Call `method` of instance `i` with `args`, already evaluated.
fn run_method(i: usize, method: usize, args: Vec<Val>, st: &mut State, d: &Decls) -> Val {
    let block = &d.blocks[d.instances[i].block];
    let m = &block.methods[method];
    let members = block.members.len();
    let mut vars = st.instances[i].clone();
    let mut tys: Vec<Ty> = block.members.iter().map(|v| v.ty).collect();
    for (p, a) in m.sig.params.iter().zip(args) {
        vars.push(store(p.ty, a));
        tys.push(p.ty);
    }
    let mut frame = State::frame(vars);
    let _ = run_block(&m.body, &mut frame, &tys, d);
    let result = store(m.sig.ty, eval(&m.result, &frame, d));
    frame.vars.truncate(members);
    st.instances[i] = frame.vars;
    result
}

/// Run `m{j}` as class `defining` defines it, on an instance of class
/// `dynamic` whose variables are `full`, with `args` already evaluated.
fn call_class(
    d: &Decls,
    dynamic: usize,
    defining: usize,
    j: usize,
    full: &mut [Val],
    args: Vec<Val>,
) -> Val {
    let visible = d.class_vars(defining);
    let n = visible.len();
    let class = &d.classes[defining];
    let def = class.defs[j].as_ref().expect("resolved to a definition");
    let sig = &class.families[j];
    let mut vars = full[..n].to_vec();
    let mut tys: Vec<Ty> = visible.iter().map(|v| v.ty).collect();
    for (p, a) in sig.params.iter().zip(args) {
        vars.push(store(p.ty, a));
        tys.push(p.ty);
    }
    let mut frame = State::frame(vars);
    frame.this = Some(This {
        dynamic,
        defining,
        tail: full[n..].to_vec(),
    });
    let _ = run_block(&def.body, &mut frame, &tys, d);
    let result = store(sig.ty, eval(&def.result, &frame, d));
    full[..n].clone_from_slice(&frame.vars[..n]);
    full[n..].clone_from_slice(&frame.this.expect("set above").tail);
    result
}

/// The token of a compound assignment's operator.
fn compound_operator(op: Arith) -> &'static str {
    match op {
        Arith::Add => "+=",
        Arith::Sub => "-=",
        Arith::Mul => "*=",
        Arith::Div => "/=",
        Arith::Mod => unreachable!("no MOD= is generated"),
    }
}

fn run_block(block: &[Stmt], st: &mut State, tys: &[Ty], d: &Decls) -> Flow {
    for stmt in block {
        let flow = match stmt {
            Stmt::Assign(p, e) => {
                let v = eval(e, st, d);
                write(p, v, st, tys, d);
                Flow::Next
            }
            // From right to left, each place found once, when its link runs,
            // and read back from the slot it wrote: a STRING passes on what its
            // capacity kept. Found again, a place whose subscript reads what
            // the chain wrote names another slot. A place with an operator is
            // written its operation, the place read first.
            Stmt::Chain(places, e) => {
                let mut value = e.clone();
                for (p, op) in places.iter().rev() {
                    let (p, v) = match op {
                        Some(op) => {
                            let p = found(p, st, d);
                            let ty = place_ty(&p, tys, d);
                            let operation = Expr::Arith(
                                *op,
                                ty,
                                Box::new(Expr::Read(p.clone())),
                                Box::new(value),
                            );
                            let v = eval(&operation, st, d);
                            (p, v)
                        }
                        None => {
                            let v = eval(&value, st, d);
                            (found(p, st, d), v)
                        }
                    };
                    write(&p, v, st, tys, d);
                    value = Expr::Read(p);
                }
                Flow::Next
            }
            // The operation written out, at the place's type.
            Stmt::Compound(p, op, e) => {
                let ty = place_ty(p, tys, d);
                let operation = Expr::Arith(
                    *op,
                    ty,
                    Box::new(Expr::Read(p.clone())),
                    Box::new(e.clone()),
                );
                let v = eval(&operation, st, d);
                write(p, v, st, tys, d);
                Flow::Next
            }
            Stmt::SetBit(p, ty, bit, _, e) => {
                let set = eval(e, st, d).truthy() as u128;
                let bits = put_slice(raw(*ty, &read(p, st, d)), *bit, 1, set);
                write(p, from_raw(*ty, bits), st, tys, d);
                Flow::Next
            }
            Stmt::MoveSlice(to, from, width) => {
                let slice = get_slice(raw(from.ty, &read(&from.place, st, d)), from.offset, *width);
                let bits = put_slice(
                    raw(to.ty, &read(&to.place, st, d)),
                    to.offset,
                    *width,
                    slice,
                );
                write(&to.place, from_raw(to.ty, bits), st, tys, d);
                Flow::Next
            }
            Stmt::CallFb(i, args, bound) => {
                let block = &d.blocks[d.instances[*i].block];
                let values: Vec<Val> = args.iter().map(|a| eval(a, st, d)).collect();
                let member_tys: Vec<Ty> = block.members.iter().map(|m| m.ty).collect();
                let mut frame = State::frame(st.instances[*i].clone());
                for (m, v) in block.inputs().into_iter().zip(values) {
                    frame.vars[m] = store(member_tys[m], v);
                }
                let inout = block.inout().zip(*bound);
                if let Some((m, v)) = inout {
                    frame.vars[m] = st.vars[v].clone();
                }
                let _ = run_block(&block.body, &mut frame, &member_tys, d);
                if let Some((m, v)) = inout {
                    st.vars[v] = frame.vars[m].clone();
                }
                st.instances[*i] = frame.vars;
                Flow::Next
            }
            Stmt::CallMethod(target, i, m, args) => {
                let values: Vec<Val> = args.iter().map(|a| eval(a, st, d)).collect();
                let result = run_method(*i, *m, values, st, d);
                write(target, result, st, tys, d);
                Flow::Next
            }
            Stmt::CallUser(target, u, i, args) => {
                let user = &d.users[*u];
                // The FUNCTION's inputs, the method's arguments over them,
                // the method, and its result through the FUNCTION's type.
                let inputs: Vec<Val> = user
                    .inputs
                    .iter()
                    .zip(args)
                    .map(|(v, a)| store(v.ty, eval(a, st, d)))
                    .collect();
                let frame = State::frame(inputs);
                let method_args: Vec<Val> = user.args.iter().map(|a| eval(a, &frame, d)).collect();
                let result = run_method(*i, user.method, method_args, st, d);
                let ty = d.interfaces[user.interface].methods[user.method].ty;
                write(target, store(ty, result), st, tys, d);
                Flow::Next
            }
            Stmt::CallObject(target, o, j, args) => {
                let values: Vec<Val> = args.iter().map(|a| eval(a, st, d)).collect();
                let class = d.objects[*o].class;
                let defining = d.resolve(class, *j);
                let mut full = st.objects[*o].clone();
                let result = call_class(d, class, defining, *j, &mut full, values);
                st.objects[*o] = full;
                write(target, result, st, tys, d);
                Flow::Next
            }
            Stmt::CallThis(target, i, args, via_super) => {
                let values: Vec<Val> = args.iter().map(|a| eval(a, st, d)).collect();
                let this = st.this.clone().expect("THIS in a class method");
                let n = d.class_vars(this.defining).len();
                let defining = match via_super {
                    true => d.resolve(
                        d.classes[this.defining].parent.expect("SUPER has a parent"),
                        *i,
                    ),
                    false => d.resolve(this.dynamic, *i),
                };
                let mut full = st.vars[..n].to_vec();
                full.extend(this.tail.iter().cloned());
                let result = call_class(d, this.dynamic, defining, *i, &mut full, values);
                st.vars[..n].clone_from_slice(&full[..n]);
                if let Some(this) = st.this.as_mut() {
                    this.tail = full[n..].to_vec();
                }
                write(target, result, st, tys, d);
                Flow::Next
            }
            Stmt::SetRef(r, place) => {
                st.refs[*r] = loc_of(place, st, d);
                Flow::Next
            }
            Stmt::Walk(call) => {
                let walker = &d.walkers[call.walker];
                let k = call
                    .k
                    .as_ref()
                    .map(|e| store(walker.view(d), eval(e, st, d)));
                let acc = match call.instance {
                    Some(i) => st.walkers[i].clone(),
                    None => zero(walker.result(d)),
                };
                let (elems, dims) = match call.bound {
                    Bound::Array(a) => {
                        let len = d.arrays[a].init.len();
                        (&mut st.arrays[a][..], vec![(d.arrays[a].lo, len)])
                    }
                    Bound::Matrix(m) => {
                        let [rows, columns] = d.matrices[m].len;
                        let [lo, lo2] = d.matrices[m].lo;
                        (&mut st.matrices[m][..], vec![(lo, rows), (lo2, columns)])
                    }
                    Bound::Row(m, row) => {
                        let columns = d.matrices[m].len[1];
                        let start = (row - d.matrices[m].lo[0]) as usize * columns;
                        let elems = &mut st.matrices[m][start..start + columns];
                        (elems, vec![(d.matrices[m].lo[1], columns)])
                    }
                };
                let acc = walk(d, call.walker, elems, &dims, k.as_ref(), acc);
                if let Some(i) = call.instance {
                    st.walkers[i] = acc.clone();
                }
                if let Some(target) = &call.target {
                    write(target, acc, st, tys, d);
                }
                Flow::Next
            }
            Stmt::Recurse(call) => {
                let r = &d.recursions[call.recursion];
                let seed = Ty::Dint.wrap(eval(&call.seed, st, d).int());
                let value = match r.route {
                    // The override adds 1 to the base's.
                    Route::Super => Ty::Dint.wrap(descend(r, call.depth, seed) + 1),
                    _ => descend(r, call.depth, seed),
                };
                write(&call.target, Val::Int(value), st, tys, d);
                Flow::Next
            }
            Stmt::If(arms, otherwise) => {
                match arms.iter().find(|(cond, _)| eval(cond, st, d).truthy()) {
                    Some((_, body)) => run_block(body, st, tys, d),
                    None => match otherwise {
                        Some(body) => run_block(body, st, tys, d),
                        None => Flow::Next,
                    },
                }
            }
            Stmt::Case(selector, arms, otherwise) => {
                let s = eval(selector, st, d);
                let hit = arms.iter().find(|(labels, _)| {
                    labels.iter().any(|l| match (l, &s) {
                        (Label::Int(v, _), Val::Int(x)) => x == v,
                        (Label::Range(a, b, _), Val::Int(x)) => a <= x && x <= b,
                        (Label::Str(t), Val::Str(x)) => x == t,
                        _ => false,
                    })
                });
                match (hit, otherwise) {
                    (Some((_, body)), _) => run_block(body, st, tys, d),
                    (None, Some(body)) => run_block(body, st, tys, d),
                    (None, None) => Flow::Next,
                }
            }
            Stmt::For(v, from, to, by, _, body) => {
                st.vars[*v] = Val::Int(*from);
                let mut flow = Flow::Next;
                loop {
                    let i = st.vars[*v].int();
                    if !((*by > 0 && i <= *to) || (*by < 0 && i >= *to)) {
                        break;
                    }
                    match run_block(body, st, tys, d) {
                        Flow::Exit => break,
                        Flow::Return => {
                            flow = Flow::Return;
                            break;
                        }
                        Flow::Next | Flow::Continue => {}
                    }
                    st.vars[*v] = Val::Int(st.vars[*v].int() + *by);
                }
                flow
            }
            Stmt::Exit => Flow::Exit,
            Stmt::Continue => Flow::Continue,
            Stmt::Return => Flow::Return,
            Stmt::Try(t) => {
                // The body runs to its raise, unless it jumps out first; the
                // handler then. The __FINALLY runs either way, and jumps
                // nowhere itself.
                let mut flow = run_block(&t.body, st, tys, d);
                if matches!(flow, Flow::Next) {
                    if let Some(v) = t.variable {
                        write(&Place::Var(v), Val::Str(t.message.clone()), st, tys, d);
                    }
                    flow = run_block(&t.handler, st, tys, d);
                }
                if let Some(finally) = &t.finally {
                    run_block(finally, st, tys, d);
                }
                flow
            }
        };
        if !matches!(flow, Flow::Next) {
            return flow;
        }
    }
    Flow::Next
}

/// The default of a type that is not a STRUCT.
fn zero(ty: Ty) -> Val {
    match ty {
        Ty::Real => Val::F32(0.0),
        Ty::Lreal => Val::F64(0.0),
        Ty::Str(_) => Val::Str(Vec::new()),
        _ => Val::Int(0),
    }
}

/// Run walker `w` over `elems`, the array its `a` is bound to, row by row.
/// Dimension `n` starts at `dims[n].0` and holds `dims[n].1` elements. A
/// VAR_INPUT walks its own copy. `acc` is the accumulator as the call
/// finds it, and the result the one the walk leaves.
fn walk(
    d: &Decls,
    w: usize,
    elems: &mut [Val],
    dims: &[(i128, usize)],
    k: Option<&Val>,
    mut acc: Val,
) -> Val {
    let walker = &d.walkers[w];
    let mut copy;
    let elems = match walker.in_out {
        true => elems,
        false => {
            copy = elems.to_vec();
            &mut copy[..]
        }
    };
    if let Some(j) = walker.pass {
        let callee = &d.walkers[j];
        let k = k.filter(|_| callee.writes);
        acc = walk(d, j, elems, dims, k, zero(callee.result(d)));
    }
    let view = walker.view(d);
    let mut prev = k.cloned().unwrap_or(Val::Int(0));
    for flat in order(walker, dims) {
        let slot = match (walker.field, &mut elems[flat]) {
            (Some(f), Val::Struct(fields)) => &mut fields[f],
            (_, slot) => slot,
        };
        if !walker.writes {
            acc = match (view, &acc, &*slot) {
                (Ty::Bool, a, x) => Val::Int((!a.truthy() || x.truthy()) as i128),
                (_, Val::Str(a), Val::Str(x)) => Val::Str(a.max(x).clone()),
                (_, Val::F32(a), Val::F32(x)) => Val::F32(a * 2.0 + x),
                (_, Val::F64(a), Val::F64(x)) => Val::F64(a * 2.0 + x),
                (t, a, x) => Val::Int(t.wrap(a.int() * 3 + x.int())),
            };
            continue;
        }
        // The running total, which the element takes; a string moves one
        // place on instead, the first taking `k`.
        let (next, written) = match (view, &prev, &*slot) {
            (Ty::Bool, p, x) => {
                let v = Val::Int((p.truthy() ^ x.truthy()) as i128);
                (v.clone(), v)
            }
            (Ty::Str(_), p, x) => (x.clone(), p.clone()),
            (_, Val::F32(p), Val::F32(x)) => (Val::F32(p + x), Val::F32(p + x)),
            (_, Val::F64(p), Val::F64(x)) => (Val::F64(p + x), Val::F64(p + x)),
            (t, p, x) => {
                let v = Val::Int(t.wrap(p.int() + x.int()));
                (v.clone(), v)
            }
        };
        *slot = written;
        prev = next;
        acc = Val::Int(Ty::Dint.wrap(acc.int() + 1));
    }
    acc
}

/// What a call of `r` at depth `n` with `seed` returns, each call reading
/// back what it kept itself. Under `Super`, the call below runs the
/// override, which adds 1.
fn descend(r: &Recursion, n: i128, seed: i128) -> i128 {
    let dint = |v| Ty::Dint.wrap(v);
    let kept = dint(seed + n);
    let own = match r.keep {
        Keep::Text { .. } if kept % 2 == 0 => 1,
        Keep::Text { .. } => 2,
        _ => kept,
    };
    let below = match n > 0 {
        false => 0,
        true => {
            let next = dint(dint(seed * r.mul) + r.add);
            match r.route {
                Route::Super => dint(descend(r, n - 1, next) + 1),
                _ => descend(r, n - 1, next),
            }
        }
    };
    dint(dint(below * r.factor) + own)
}

/// The elements a walker visits, in order, as indexes row by row.
fn order(w: &Walker, dims: &[(i128, usize)]) -> Vec<usize> {
    let mut out: Vec<usize> = match (dims, w.transposed) {
        ([(_, rows), (_, columns)], true) => (0..*columns)
            .flat_map(|j| (0..*rows).map(move |i| i * columns + j))
            .collect(),
        _ => (0..dims.iter().map(|(_, n)| n).product::<usize>()).collect(),
    };
    if w.backward {
        out.reverse();
    }
    out
}

/// An expression's value. Every integer operation wraps, not only the
/// assignment: `(a * b) / c` in SINT is SINT arithmetic throughout. A
/// float operation rounds in its own precision.
fn eval(e: &Expr, st: &State, d: &Decls) -> Val {
    let ev = |e: &Expr| eval(e, st, d);
    match e {
        Expr::Lit(_, v) => v.clone(),
        Expr::Read(p) => read(p, st, d),
        Expr::Arith(op, ty, a, b) => match (ev(a), ev(b)) {
            (Val::F32(a), Val::F32(b)) => Val::F32(match op {
                Arith::Add => a + b,
                Arith::Sub => a - b,
                Arith::Mul => a * b,
                Arith::Div => a / b,
                Arith::Mod => unreachable!("no MOD on REAL"),
            }),
            (Val::F64(a), Val::F64(b)) => Val::F64(match op {
                Arith::Add => a + b,
                Arith::Sub => a - b,
                Arith::Mul => a * b,
                Arith::Div => a / b,
                Arith::Mod => unreachable!("no MOD on LREAL"),
            }),
            (a, b) => {
                let (a, b) = (a.int(), b.int());
                Val::Int(ty.wrap(match op {
                    Arith::Add => a.wrapping_add(b),
                    Arith::Sub => a.wrapping_sub(b),
                    Arith::Mul => a.wrapping_mul(b),
                    // Rust's `/` and `%` truncate toward zero, as wasm's do.
                    Arith::Div => a / b,
                    Arith::Mod => a % b,
                }))
            }
        },
        Expr::Neg(ty, a) => match ev(a) {
            Val::F32(x) => Val::F32(-x),
            Val::F64(x) => Val::F64(-x),
            x => Val::Int(ty.wrap(-x.int())),
        },
        Expr::Cmp(op, a, b) => {
            use std::cmp::Ordering;
            let ord = match (ev(a), ev(b)) {
                (Val::F32(a), Val::F32(b)) => a.partial_cmp(&b),
                (Val::F64(a), Val::F64(b)) => a.partial_cmp(&b),
                // Byte-lexicographic, as docs/strings.md says.
                (Val::Str(a), Val::Str(b)) => Some(a.cmp(&b)),
                (a, b) => Some(a.int().cmp(&b.int())),
            };
            // A NaN is unordered: every comparison is false but `<>`.
            let r = match (op, ord) {
                (Cmp::Ne, None) => true,
                (_, None) => false,
                (Cmp::Eq, Some(o)) => o == Ordering::Equal,
                (Cmp::Ne, Some(o)) => o != Ordering::Equal,
                (Cmp::Lt, Some(o)) => o == Ordering::Less,
                (Cmp::Gt, Some(o)) => o == Ordering::Greater,
                (Cmp::Le, Some(o)) => o != Ordering::Greater,
                (Cmp::Ge, Some(o)) => o != Ordering::Less,
            };
            Val::Int(r as i128)
        }
        Expr::Not(a) => Val::Int(!ev(a).truthy() as i128),
        Expr::Logic(op, a, b) => {
            let (a, b) = (ev(a).truthy(), ev(b).truthy());
            Val::Int(match op {
                Logic::And => a && b,
                Logic::Or => a || b,
                Logic::Xor => a ^ b,
            } as i128)
        }
        Expr::Call(f, args) => {
            let f = &d.functions[*f];
            let inputs = match f.variadic {
                // Each input before the pack, then the pack: every argument left,
                // at its type.
                true => {
                    let (pack, before) = f.inputs.split_last().expect("a pack");
                    args.iter()
                        .enumerate()
                        .map(|(i, a)| store(before.get(i).unwrap_or(pack).ty, ev(a)))
                        .collect()
                }
                false => f
                    .inputs
                    .iter()
                    .zip(args)
                    .map(|(v, a)| store(v.ty, ev(a)))
                    .collect(),
            };
            let frame = State::frame(inputs);
            let mut result = store(f.ty, eval(&f.body, &frame, d));
            for slice in &f.slices {
                let bits = match slice {
                    ResultSlice::Bit(bit, _, cond) => {
                        let set = eval(cond, &frame, d).truthy() as u128;
                        put_slice(raw(f.ty, &result), *bit, 1, set)
                    }
                    ResultSlice::Move {
                        to_off,
                        input,
                        from_off,
                        width,
                    } => {
                        let from = raw(f.inputs[*input].ty, &frame.vars[*input]);
                        let slice = get_slice(from, *from_off, *width);
                        put_slice(raw(f.ty, &result), *to_off, *width, slice)
                    }
                };
                result = from_raw(f.ty, bits);
            }
            result
        }
        Expr::SameRef(equal, a, b) => {
            let side = |s: &RefSide| match s {
                RefSide::Held(r) => Some(st.refs[*r].clone()),
                RefSide::Of(p, _) => Some(loc_of(p, st, d)),
                RefSide::Null => None,
            };
            Val::Int(((side(a) == side(b)) == *equal) as i128)
        }
        Expr::Cycle(_, n) => Val::Int(Ty::Dint.wrap(ev(n).int() + 1)),
        Expr::Const(c) => d.constants[*c].value.clone(),
        Expr::Named(k, j) => Val::Int(d.named[*k].numbers[*j]),
        // Bit by bit on two's complement, which is the value's own bits at
        // its type: nothing wraps.
        Expr::Bits(op, ty, a, b) => {
            let (a, b) = (ev(a).int(), ev(b).int());
            Val::Int(ty.wrap(match op {
                Logic::And => a & b,
                Logic::Or => a | b,
                Logic::Xor => a ^ b,
            }))
        }
        // The `libm` a module's `**` runs (crates/wasm_builtins).
        Expr::Pow(a, b) => match (ev(a), ev(b)) {
            (Val::F32(a), Val::F32(b)) => Val::F32(libm::powf(a, b)),
            (Val::F64(a), Val::F64(b)) => Val::F64(libm::pow(a, b)),
            _ => unreachable!("`**` raises a REAL or an LREAL"),
        },
        Expr::Bit(p, ty, bit, _) => Val::Int(get_slice(raw(*ty, &read(p, st, d)), *bit, 1) as i128),
        // The frame holds the pack, one value per argument. An arithmetic
        // fold wraps at the pack's type at each step, a single argument is
        // itself, and a comparison of one argument is TRUE.
        Expr::LibCall(f, args, _) => {
            let args: Vec<Val> = args.iter().map(ev).collect();
            Val::Int(match f {
                LibFn::Half => (args[0].int() as i32 / 2) as i128,
                LibFn::Clamp0 => (args[0].int() as i32).max(0) as i128,
                LibFn::Both => (args[0].truthy() && args[1].truthy()) as i128,
            })
        }
        // The frame holds the index, then the pack, one value per argument. An
        // index below 0 reads the first, and one past the last reads the last.
        Expr::Element => {
            let last = st.vars.len() as i128 - 2;
            st.vars[1 + st.vars[0].int().clamp(0, last) as usize].clone()
        }
        Expr::Fold(op, ty) => match op {
            Fold::Arith(op) => Val::Int(
                st.vars
                    .iter()
                    .map(Val::int)
                    .reduce(|a, b| {
                        ty.wrap(match op {
                            Arith::Add => a.wrapping_add(b),
                            Arith::Sub => a.wrapping_sub(b),
                            Arith::Mul => a.wrapping_mul(b),
                            Arith::Div => a / b,
                            Arith::Mod => a % b,
                        })
                    })
                    .unwrap_or(0),
            ),
            Fold::Logic(op) => Val::Int(
                st.vars
                    .iter()
                    .map(Val::truthy)
                    .reduce(|a, b| match op {
                        Logic::And => a && b,
                        Logic::Or => a || b,
                        Logic::Xor => a ^ b,
                    })
                    .unwrap_or(false) as i128,
            ),
            Fold::Cmp(op) => Val::Int(st.vars.windows(2).all(|pair| {
                let (a, b) = (pair[0].int(), pair[1].int());
                match op {
                    Cmp::Eq => a == b,
                    Cmp::Ne => a != b,
                    Cmp::Lt => a < b,
                    Cmp::Gt => a > b,
                    Cmp::Le => a <= b,
                    Cmp::Ge => a >= b,
                }
            }) as i128),
            Fold::Min => Val::Int(st.vars.iter().map(Val::int).min().unwrap_or(0)),
            Fold::Max => Val::Int(st.vars.iter().map(Val::int).max().unwrap_or(0)),
            // The `libm` a module's `**` runs (crates/wasm_builtins).
            Fold::Power => st
                .vars
                .iter()
                .cloned()
                .reduce(|a, b| match (a, b) {
                    (Val::F32(a), Val::F32(b)) => Val::F32(libm::powf(a, b)),
                    (Val::F64(a), Val::F64(b)) => Val::F64(libm::pow(a, b)),
                    _ => unreachable!("`**` folds REALs and LREALs"),
                })
                .unwrap_or(Val::F32(0.0)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program of random bytes, enough of them for its body to call what it
    /// declares: 96 bytes declare FUNCTIONs and run out before the body.
    fn random_program(seed: &mut u64) -> String {
        let bytes: Vec<u8> = (0..2048)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 7;
                *seed ^= *seed << 17;
                (*seed >> 24) as u8
            })
            .collect();
        program(&bytes)
    }

    /// Whether `text` calls the FUNCTION whose result `line` assigns. One that is
    /// only declared computes nothing the evaluator could disagree with.
    fn calls(text: &str, line: &str) -> bool {
        let call = format!("{}(", line.trim().split(' ').next().unwrap_or_default());
        text.lines()
            .any(|l| !l.starts_with("FUNCTION") && l.contains(&call))
    }

    /// The generator writes every fold, calls it, and for each the module computes
    /// what the evaluator does. `/`, `%` and `**` were written by nothing for a long
    /// time.
    #[test]
    fn every_fold_is_written_and_computes() {
        const FOLDS: [&str; 17] = [
            "+;", "-;", "*;", "/;", "%;", "**;", "&;", "|;", "^;", "=;", "<>;", "<;", ">;", "<=;",
            ">=;", " MIN;", " MAX;",
        ];
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut seen: std::collections::BTreeSet<&str> = Default::default();
        for _ in 0..20_000 {
            let text = random_program(&mut seed);
            let fresh: Vec<&str> = FOLDS
                .iter()
                .copied()
                .filter(|fold| !seen.contains(fold))
                .filter(|fold| {
                    text.lines().any(|line| {
                        line.contains(":= ...")
                            && line.trim_end().ends_with(fold)
                            && !line.trim_end().ends_with(&format!("*{fold}"))
                            && !line.trim_end().ends_with(&format!("<{fold}"))
                            && !line.trim_end().ends_with(&format!(">{fold}"))
                            && calls(&text, line)
                    })
                })
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            seen.extend(fresh);
            if seen.len() == FOLDS.len() {
                return;
            }
        }
        let missing: Vec<_> = FOLDS.iter().filter(|fold| !seen.contains(*fold)).collect();
        panic!("no program folds with {missing:?}");
    }

    /// The generator reads a pack by position and calls it, and the module selects
    /// the argument the evaluator does, past either end included.
    #[test]
    fn a_read_by_position_is_written_and_computes() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut checked = 0;
        for _ in 0..20_000 {
            let text = random_program(&mut seed);
            if !text
                .lines()
                .any(|line| line.contains(":= ...a1[a0];") && calls(&text, line))
            {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            checked += 1;
            if checked == 8 {
                return;
            }
        }
        panic!("{checked} programs read a pack by position");
    }

    /// `**` computes in the evaluator with the `libm` it computes with in a module,
    /// which is crates/wasm_builtins' own lockfile. A release that moves a result by
    /// the last bit would report a bug in nothing.
    #[test]
    fn the_evaluator_powers_with_the_libm_of_a_module() {
        let lock = include_str!("../../wasm_builtins/Cargo.lock");
        let manifest = include_str!("../Cargo.toml");
        let version = lock
            .split("name = \"libm\"\nversion = \"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("crates/wasm_builtins locks libm");
        assert!(
            manifest.contains(&format!("libm = \"={version}\"")),
            "crates/fuzz/Cargo.toml must pin libm to {version}, as crates/wasm_builtins does"
        );
    }

    /// Programs from a spread of seeds: the generator writes a call to the
    /// library in each of its spellings, under the library's name and
    /// without it, a nested namespace included; and for the first few that
    /// call it, the module computes what the library's FUNCTIONs do here.
    #[test]
    fn library_calls_compute_what_the_library_does() {
        const SPELLINGS: [&str; 6] = [
            "Lib.Half(",
            " Half(",
            "Lib.Math.Clamp0(",
            " Math.Clamp0(",
            "Lib.Flags.Both(",
            " Flags.Both(",
        ];
        let mut seed = 0x6C62_272E_07BB_0142u64;
        let mut seen: std::collections::BTreeSet<&str> = Default::default();
        let mut checked = 0;
        // The loop stops once every spelling is seen: the bound is only how
        // far it may look, past the choices CONSTANTs and named values take.
        for _ in 0..2000 {
            // Long enough that the choices are still the input's when the
            // calls are drawn: past the end, every choice is the first one,
            // which is the qualified spelling.
            let bytes: Vec<u8> = (0..1024)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed >> 24) as u8
                })
                .collect();
            let text = program(&bytes);
            let used: Vec<&str> = SPELLINGS
                .into_iter()
                .filter(|spelling| text.contains(spelling))
                .collect();
            if used.is_empty() {
                continue;
            }
            seen.extend(used);
            // Compiling and running a program is the costly part: a few
            // are enough, the spellings are counted on every text.
            if checked < 6 {
                if let Err(finding) = crate::check_generated(&text) {
                    panic!("{finding}\n--- the program ---\n{text}");
                }
                checked += 1;
            }
            if checked == 6 && seen.len() == SPELLINGS.len() {
                break;
            }
        }
        assert_eq!(checked, 6, "only {checked} seeds called the library");
        assert_eq!(
            seen.len(),
            SPELLINGS.len(),
            "every spelling was written: {seen:?}"
        );
    }

    /// Programs from a spread of seeds, kept when the program calls a
    /// walker: the module walks its arrays as the evaluator does.
    #[test]
    fn walkers_compute_their_walks() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut checked = 0;
        for _ in 0..400 {
            let bytes: Vec<u8> = (0..1024)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed >> 24) as u8
                })
                .collect();
            let text = program(&bytes);
            let calls = text
                .split("\nPROGRAM P")
                .nth(1)
                .is_some_and(|body| body.contains("(a := "));
            if !calls {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            checked += 1;
            if checked == 4 {
                break;
            }
        }
        assert!(checked >= 3, "only {checked} seeds called a walker");
    }

    /// A program for each route a recursive body closes its cycle by: each
    /// call reads back what it kept itself.
    #[test]
    fn recursive_bodies_keep_their_own_storage() {
        // Each route by a line only its skeleton writes.
        let markers = [
            "below := rec",
            "below := hop",
            "below : DINT := hop",
            "SUPER.climb",
            "dev.climb",
            "visit",
            "next^(",
            "SUPER();",
        ];
        let mut seed = 0xD1B5_4A32_D192_ED03u64;
        let mut reached: Vec<&str> = Vec::new();
        for _ in 0..600 {
            let bytes: Vec<u8> = (0..1024)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed >> 24) as u8
                })
                .collect();
            let text = program(&bytes);
            let fresh: Vec<&str> = markers
                .into_iter()
                .filter(|m| text.contains(m) && !reached.contains(m))
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == markers.len() {
                break;
            }
        }
        assert_eq!(reached.len(), markers.len(), "routes reached: {reached:?}");
    }

    /// Each form of what compares references, of what goes through `THIS`, and of
    /// a type that comes back through a reference is written, called, and computes.
    #[test]
    fn references_and_this_are_written_and_compute() {
        // A part of a member, or of a field of one: `THIS.v0.f1.%X3`, `THIS.v0.3`.
        let this_part = |text: &str, steps: usize| {
            text.match_indices("THIS.").any(|(at, _)| {
                let rest = &text[at + "THIS.".len()..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || "_.%".contains(c)))
                    .unwrap_or(rest.len());
                let path: Vec<&str> = rest[..end].split('.').collect();
                path.len() == steps + 1
                    && path.iter().take(steps).all(|s| {
                        s.starts_with(|c: char| c.is_ascii_alphabetic()) && !s.starts_with('m')
                    })
                    && path[steps].starts_with(|c: char| c == '%' || c.is_ascii_digit())
            })
        };
        type Form = (String, Box<dyn Fn(&str) -> bool>);
        let mut forms: Vec<Form> = vec![
            ("r = REF(x)".into(), Box::new(|t| t.contains(" = REF("))),
            ("r <> REF(x)".into(), Box::new(|t| t.contains(" <> REF("))),
            (
                "REF(x) = r".into(),
                Box::new(|t| t.contains("(REF(") && t.contains(") = ")),
            ),
            (
                "(REF(x))".into(),
                Box::new(|t| t.contains("((REF(") || t.contains(" (REF(")),
            ),
            ("r <> NULL".into(), Box::new(|t| t.contains(" <> NULL"))),
            ("NULL = r".into(), Box::new(|t| t.contains("(NULL = "))),
            (
                "one step through THIS".into(),
                Box::new(move |t| this_part(t, 1)),
            ),
            (
                "a field through THIS".into(),
                Box::new(move |t| this_part(t, 2)),
            ),
        ];
        for (k, (types, ..)) in CYCLES.iter().enumerate() {
            // By its own declaration, and only when its FUNCTION is called.
            let declared = types.replace("_K", "0");
            forms.push((
                format!("cycle {k}"),
                Box::new(move |t| t.contains(&declared) && t.contains(" cycle0(")),
            ));
        }
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut reached: Vec<String> = Vec::new();
        for _ in 0..4000 {
            let text = random_program(&mut seed);
            let fresh: Vec<String> = forms
                .iter()
                .filter(|(name, has)| has(&text) && !reached.contains(name))
                .map(|(name, _)| name.clone())
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == forms.len() {
                break;
            }
        }
        assert_eq!(reached.len(), forms.len(), "forms reached: {reached:?}");
    }

    /// The generator writes chains, `a := b := e`, of two places and of three,
    /// through a subscript, with a STRING, and with an operator on the last
    /// place and on another, and for each the module computes what the
    /// evaluator does: from right to left, each place taking what the one to
    /// its right holds, or its operation over that.
    #[test]
    fn chains_are_written_and_compute() {
        // The assignment operators a statement line has outside brackets, in
        // order: two or more is a chain, a declaration or a FOR has one.
        let operators = |line: &str| {
            let mut depth = 0i32;
            let mut found = Vec::new();
            for (at, c) in line.char_indices() {
                match c {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    _ if depth == 0 => {
                        if let Some(op) = [":=", "+=", "-=", "*=", "/="]
                            .into_iter()
                            .find(|op| line[at..].starts_with(op))
                        {
                            found.push(op);
                        }
                    }
                    _ => {}
                }
            }
            found
        };
        let chain = move |text: &str, has: &dyn Fn(&str, &[&str]) -> bool| {
            text.lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("FOR "))
                .any(|line| {
                    let ops = operators(line);
                    ops.len() >= 2 && has(line, &ops)
                })
        };
        type Form = (&'static str, Box<dyn Fn(&str) -> bool>);
        let forms: Vec<Form> = vec![
            (
                "two places",
                Box::new(move |t| chain(t, &|_, ops| ops.len() == 2)),
            ),
            (
                "three places",
                Box::new(move |t| chain(t, &|_, ops| ops.len() == 3)),
            ),
            (
                "a subscript",
                Box::new(move |t| chain(t, &|l, _| l.contains('['))),
            ),
            (
                "a STRING",
                Box::new(move |t| chain(t, &|l, _| l.contains('\''))),
            ),
            (
                "an operator on the last place",
                Box::new(move |t| chain(t, &|_, ops| ops.last() != Some(&":="))),
            ),
            (
                "an operator on another place",
                Box::new(move |t| {
                    chain(t, &|_, ops| {
                        ops[..ops.len() - 1].iter().any(|op| *op != ":=")
                    })
                }),
            ),
        ];
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut reached: Vec<&str> = Vec::new();
        for _ in 0..4000 {
            let text = random_program(&mut seed);
            let fresh: Vec<&str> = forms
                .iter()
                .filter(|(name, has)| has(&text) && !reached.contains(name))
                .map(|(name, _)| *name)
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == forms.len() {
                break;
            }
        }
        assert_eq!(reached.len(), forms.len(), "forms reached: {reached:?}");
    }

    /// The generator writes each compound operator, on an integer and on a
    /// REAL, through a subscript, and for each the module computes what the
    /// operation written out does.
    #[test]
    fn compound_assignments_are_written_and_compute() {
        let statement =
            |text: &str, has: &dyn Fn(&str) -> bool| text.lines().map(str::trim_start).any(has);
        type Form = (&'static str, Box<dyn Fn(&str) -> bool>);
        let mut forms: Vec<Form> = ["+=", "-=", "*=", "/="]
            .into_iter()
            .map(|op| {
                let has = move |t: &str| statement(t, &|l| l.contains(&format!(" {op} ")));
                (op, Box::new(has) as Box<dyn Fn(&str) -> bool>)
            })
            .collect();
        forms.push((
            "through a subscript",
            Box::new(move |t| {
                statement(t, &|l| {
                    ["+=", "-=", "*=", "/="].iter().any(|op| {
                        l.find(&format!(" {op} "))
                            .is_some_and(|at| l[..at].contains('['))
                    })
                })
            }),
        ));
        let mut seed = 0x5DEE_CE66_D1CE_4E5Bu64;
        let mut reached: Vec<&str> = Vec::new();
        for _ in 0..4000 {
            let text = random_program(&mut seed);
            let fresh: Vec<&str> = forms
                .iter()
                .filter(|(name, has)| has(&text) && !reached.contains(name))
                .map(|(name, _)| *name)
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == forms.len() {
                break;
            }
        }
        assert_eq!(reached.len(), forms.len(), "forms reached: {reached:?}");
    }

    /// The evaluator runs a chain as the module does: each place takes what
    /// the place to its right holds, so a STRING cut by a shorter place on
    /// the right reaches the longer one cut. A generated chain has one type,
    /// and few cut a STRING on the way, so the programs rarely tell this
    /// apart from every place taking the value.
    #[test]
    fn a_chain_passes_on_what_each_place_holds() {
        let tys = [Ty::Str(8), Ty::Str(3)];
        let mut st = State::frame(vec![Val::Str(Vec::new()), Val::Str(Vec::new())]);
        let chain = Stmt::Chain(
            vec![(Place::Var(0), None), (Place::Var(1), None)],
            Expr::Lit(Ty::Str(8), Val::Str(b"abcdefgh".to_vec())),
        );
        let _ = run_block(&[chain], &mut st, &tys, &Decls::default());
        assert_eq!(
            st.vars,
            [Val::Str(b"abc".to_vec()), Val::Str(b"abc".to_vec())]
        );
    }

    /// The evaluator writes a place with an operator its operation over what
    /// the place to its right holds, the place read first: `a += b := 5`
    /// writes `b` 5 and `a` 1 + 5, then `a := b *= 3` writes `b` 15 and `a`
    /// what `b` holds.
    #[test]
    fn a_place_with_an_operator_is_written_its_operation() {
        let tys = [Ty::Int, Ty::Int];
        let mut st = State::frame(vec![Val::Int(1), Val::Int(0)]);
        let five = Expr::Lit(Ty::Int, Val::Int(5));
        let chain = Stmt::Chain(
            vec![(Place::Var(0), Some(Arith::Add)), (Place::Var(1), None)],
            five,
        );
        let _ = run_block(&[chain], &mut st, &tys, &Decls::default());
        assert_eq!(st.vars, [Val::Int(6), Val::Int(5)]);
        let three = Expr::Lit(Ty::Int, Val::Int(3));
        let chain = Stmt::Chain(
            vec![(Place::Var(0), None), (Place::Var(1), Some(Arith::Mul))],
            three,
        );
        let _ = run_block(&[chain], &mut st, &tys, &Decls::default());
        assert_eq!(st.vars, [Val::Int(15), Val::Int(15)]);
    }

    /// The evaluator finds each place of a chain once, when its link runs,
    /// and passes on what that slot holds: with `a[0]` 0, `v := a[a[0]] := 1`
    /// and `v := a[a[0]] += 1` both write `a[0]` 1 and `v` 1. Found again
    /// after the write, `a[a[0]]` named `a[1]`, and `v` took its 7.
    #[test]
    fn a_chain_reads_back_the_slot_it_wrote() {
        let mut d = Decls::default();
        d.arrays.push(Array {
            name: "a".to_string(),
            ty: Ty::Dint,
            lo: 0,
            bounds: None,
            init: Vec::new(),
            written: Vec::new(),
        });
        let tys = [Ty::Dint];
        let a_of_a0 = || {
            let a0 = Place::Index(0, Box::new(Expr::Lit(Ty::Dint, Val::Int(0))));
            Place::Index(0, Box::new(Expr::Read(a0)))
        };
        for op in [None, Some(Arith::Add)] {
            let mut st = State::frame(vec![Val::Int(0)]);
            st.arrays = vec![vec![Val::Int(0), Val::Int(7)]];
            let chain = Stmt::Chain(
                vec![(Place::Var(0), None), (a_of_a0(), op)],
                Expr::Lit(Ty::Dint, Val::Int(1)),
            );
            let _ = run_block(&[chain], &mut st, &tys, &d);
            assert_eq!(st.vars, [Val::Int(1)], "{op:?}");
            assert_eq!(st.arrays[0], [Val::Int(1), Val::Int(7)], "{op:?}");
        }
    }

    /// The generator writes CONSTANTs and types with named values, in each place
    /// a constant goes, and for each the module computes what the evaluator does.
    /// It wrote neither: a REAL CONSTANT computed from another, and `**` in a
    /// value `__init` stores, stopped the compiler unseen.
    #[test]
    fn constants_and_named_values_are_written_and_compute() {
        // A line of a declaration section, by what it declares.
        let declares = |text: &str, prefix: &str, has: &dyn Fn(&str) -> bool| {
            text.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with(prefix) && has(line)
            })
        };
        // A field of a STRUCT, by its declaration.
        let field = |text: &str, has: &dyn Fn(&str) -> bool| {
            let mut inside = false;
            text.lines().any(|line| {
                if line.starts_with("TYPE S") {
                    inside = true;
                } else if line.starts_with("END_STRUCT") {
                    inside = false;
                }
                inside && has(line)
            })
        };
        // A CONSTANT's name, `k{n}` or `gk{n}`, among the words of `text`.
        let has_constant = |text: &str| {
            text.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| {
                    word.strip_prefix("gk")
                        .or_else(|| word.strip_prefix('k'))
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
        };
        let named_type = |text: &str, has: &dyn Fn(&str) -> bool| {
            text.lines()
                .any(|line| line.starts_with("TYPE E") && line.contains(" (") && has(line))
        };
        // The value of a CONSTANT, `k{n} : T := ...` or `gk{n} : T := ...`.
        let constant = |text: &str, has: &dyn Fn(&str) -> bool| {
            text.lines().any(|line| {
                let line = line.trim_start();
                (line.starts_with('k') || line.starts_with("gk"))
                    && line.split_once(" := ").is_some_and(|(_, value)| has(value))
            })
        };
        // A label written as a constant expression: a sum or a name.
        let label = |text: &str, has: &dyn Fn(&str) -> bool| {
            text.lines().any(|line| {
                let line = line.trim();
                line.ends_with(':') && !line.contains(":=") && has(line)
            })
        };
        // A block member named like a type other than its own, `e1 : E0`,
        // in a block whose statements name that type's values, `E1#n0`.
        let named_like_another_type = |text: &str| {
            text.split("FUNCTION_BLOCK fb").skip(1).any(|block| {
                let block = block.split("END_FUNCTION_BLOCK").next().unwrap_or_default();
                block.lines().any(|line| {
                    let Some((name, ty)) = line.trim().split_once(" : ") else {
                        return false;
                    };
                    let number = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
                    let ty = ty.split([' ', ';']).next().unwrap_or_default();
                    match (name.strip_prefix('e'), ty.strip_prefix('E')) {
                        (Some(a), Some(b)) if number(a) && number(b) && a != b => {
                            let value = format!("E{a}#n");
                            block
                                .lines()
                                .any(|line| line.contains(&value) && !line.contains(" : "))
                        }
                        _ => false,
                    }
                })
            })
        };
        type Form = (&'static str, Box<dyn Fn(&str) -> bool>);
        let forms: Vec<Form> = vec![
            (
                "a CONSTANT of the PROGRAM",
                Box::new(|t| t.contains("VAR CONSTANT\n")),
            ),
            (
                "a CONSTANT of the CONFIGURATION",
                Box::new(|t| t.contains("VAR_GLOBAL CONSTANT\n")),
            ),
            (
                "a VAR_GLOBAL started by a constant",
                Box::new(|t| t.contains("    VAR_GLOBAL\n")),
            ),
            (
                "a CONSTANT naming another",
                Box::new(move |t| constant(t, &has_constant)),
            ),
            (
                "a bit operation in a constant",
                Box::new(move |t| {
                    constant(t, &|v| {
                        v.contains(" AND ") || v.contains(" OR ") || v.contains(" XOR ")
                    })
                }),
            ),
            (
                "** in a constant",
                Box::new(move |t| constant(t, &|v| v.contains(" ** "))),
            ),
            (
                "a counted named value",
                Box::new(move |t| {
                    named_type(t, &|l| {
                        l.split(['(', ',', ')'])
                            .any(|v| v.trim().starts_with('n') && !v.contains(":="))
                    })
                }),
            ),
            (
                "a named value naming another",
                Box::new(move |t| {
                    named_type(t, &|l| {
                        l.split_once(" (").is_some_and(|(_, v)| v.contains("#n"))
                    })
                }),
            ),
            ("a constant bound", Box::new(|t| t.contains("ARRAY[("))),
            (
                "a constant label",
                Box::new(move |t| label(t, &|l| l.starts_with('('))),
            ),
            (
                "a named label",
                Box::new(move |t| label(t, &|l| l.starts_with('E') && l.contains("#n"))),
            ),
            ("a constant step", Box::new(|t| t.contains(" BY ("))),
            (
                "a block member started by a CONSTANT",
                Box::new(|t| {
                    let lines: Vec<&str> = t.lines().collect();
                    lines.windows(2).any(|pair| {
                        pair[0].starts_with("FUNCTION_BLOCK fb")
                            && pair[1] == "VAR_EXTERNAL CONSTANT"
                    })
                }),
            ),
            (
                "a field started by a constant expression",
                Box::new(move |t| field(t, &|l| l.contains(":= (") || l.contains(":= E"))),
            ),
            (
                "a variable of a type with named values",
                Box::new(move |t| declares(t, "v", &|l| l.contains(" : E") && l.contains("#n"))),
            ),
            (
                "a member named like another type, whose values the block names",
                Box::new(named_like_another_type),
            ),
        ];
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut reached: Vec<&str> = Vec::new();
        for _ in 0..4000 {
            let text = random_program(&mut seed);
            let fresh: Vec<&str> = forms
                .iter()
                .filter(|(name, has)| has(&text) && !reached.contains(name))
                .map(|(name, _)| *name)
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == forms.len() {
                break;
            }
        }
        let missing: Vec<&str> = forms
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !reached.contains(name))
            .collect();
        assert!(missing.is_empty(), "no program writes {missing:?}");
    }

    #[test]
    fn partial_access_computes_its_bits() {
        // Each form by what only it writes: a bit through `%X` and bare, a
        // slice of each width, and a FUNCTION's own result written in part.
        // `.3` after a name, a subscript or a dereference: not a REAL
        // literal's decimals, nor anything in a comment.
        let bare = |text: &str| {
            text.lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .any(|line| {
                    let b = line.as_bytes();
                    (1..b.len()).any(|i| {
                        if b[i] != b'.' || !b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                            return false;
                        }
                        let mut start = i;
                        while start > 0
                            && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_')
                        {
                            start -= 1;
                        }
                        match start < i {
                            true => b[start].is_ascii_alphabetic(),
                            false => matches!(b[i - 1], b']' | b'^'),
                        }
                    })
                })
        };
        let result = |text: &str| {
            text.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with('g')
                    && line
                        .split_once('.')
                        .is_some_and(|(name, _)| name[1..].bytes().all(|b| b.is_ascii_digit()))
            })
        };
        type Has = fn(&str) -> bool;
        let forms: [(&str, Has); 7] = [
            ("%X", |t| t.contains(".%X")),
            ("bare", bare),
            ("%B", |t| t.contains(".%B")),
            ("%W", |t| t.contains(".%W")),
            ("%D", |t| t.contains(".%D")),
            ("%L", |t| t.contains(".%L")),
            ("result", result),
        ];
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut reached: Vec<&str> = Vec::new();
        for _ in 0..2000 {
            let bytes: Vec<u8> = (0..1024)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed >> 24) as u8
                })
                .collect();
            let text = program(&bytes);
            let fresh: Vec<&str> = forms
                .iter()
                .filter(|(name, has)| has(&text) && !reached.contains(name))
                .map(|(name, _)| *name)
                .collect();
            if fresh.is_empty() {
                continue;
            }
            if let Err(finding) = crate::check_generated(&text) {
                panic!("{finding}\n--- the program ---\n{text}");
            }
            reached.extend(fresh);
            if reached.len() == forms.len() {
                break;
            }
        }
        assert_eq!(reached.len(), forms.len(), "forms reached: {reached:?}");
    }

    #[test]
    fn slices_are_two_complement_bits() {
        assert_eq!(raw(Ty::Int, &Val::Int(-2)), 0xFFFE);
        assert_eq!(from_raw(Ty::Int, 0xFEFF), Val::Int(-257));
        assert_eq!(get_slice(0x1122_3344, 1, 8), 0x33);
        assert_eq!(put_slice(0x1122_3344, 1, 16, 0xABCD), 0xABCD_3344);
        assert_eq!(
            put_slice(raw(Ty::Lint, &Val::Int(-1)), 1, 32, 0),
            0xFFFF_FFFF
        );
        assert_eq!(get_slice(u64::MAX as u128, 0, 64), u64::MAX as u128);
    }

    #[test]
    fn wrapping_follows_twos_complement() {
        assert_eq!(Ty::Int.wrap(32768), -32768);
        assert_eq!(Ty::Usint.wrap(256), 0);
        assert_eq!(Ty::Usint.wrap(-1), 255);
        assert_eq!(Ty::Sint.wrap(-129), 127);
        assert_eq!(Ty::Ulint.wrap(-1), u64::MAX as i128);
        assert_eq!(Ty::Lint.wrap(i64::MAX as i128 + 1), i64::MIN as i128);
    }

    #[test]
    fn division_truncates_and_mod_takes_the_dividends_sign() {
        let d = Decls::default();
        let st = State::frame(Vec::new());
        let e = |op, a, b| {
            eval(
                &Expr::Arith(
                    op,
                    Ty::Dint,
                    Box::new(Expr::Lit(Ty::Dint, Val::Int(a))),
                    Box::new(Expr::Lit(Ty::Dint, Val::Int(b))),
                ),
                &st,
                &d,
            )
        };
        assert_eq!(e(Arith::Div, -7, 2), Val::Int(-3));
        assert_eq!(e(Arith::Mod, -7, 2), Val::Int(-1));
        assert_eq!(e(Arith::Mod, 7, -2), Val::Int(1));
    }

    #[test]
    fn a_string_is_cut_to_its_capacity() {
        assert_eq!(
            store(Ty::Str(2), Val::Str(b"abc".to_vec())),
            Val::Str(b"ab".to_vec())
        );
    }

    #[test]
    fn dyadic_literals_are_written_exactly() {
        assert_eq!(exact(0.375), "0.375");
        assert_eq!(exact(-8.0), "-8.0");
        assert_eq!(exact(1.0 / 256.0), "0.00390625");
        assert_eq!(exact(-32768.0), "-32768.0");
    }

    /// Any bytes make a program, with a header, a PROGRAM and its instance.
    #[test]
    fn every_input_makes_a_program() {
        for seed in 0u8..=255 {
            let bytes: Vec<u8> = (0..512u32)
                .map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed))
                .collect();
            let p = program(&bytes);
            assert!(p.starts_with("(* rk-fuzz expects, after 2 scans:"), "{p}");
            assert!(p.contains("PROGRAM Run WITH T : P;"), "{p}");
        }
        assert!(program(&[]).contains("END_PROGRAM"));
    }

    /// Some inputs give a FUNCTION name two declarations, and call it.
    #[test]
    fn some_inputs_overload_a_function() {
        let overloaded = (0u8..=255)
            .filter(|&seed| {
                let bytes: Vec<u8> = (0..4096u32)
                    .map(|i| (i as u8).wrapping_mul(seed ^ 0x5b).wrapping_add(seed))
                    .collect();
                let p = program(&bytes);
                let mut names: Vec<&str> = p
                    .lines()
                    .filter_map(|l| l.strip_prefix("FUNCTION "))
                    .filter_map(|l| l.split(' ').next())
                    .collect();
                names.sort();
                names.windows(2).any(|w| {
                    w[0] == w[1]
                        && p.split("\nPROGRAM P")
                            .nth(1)
                            .is_some_and(|body| body.contains(&format!("{}(", w[0])))
                })
            })
            .count();
        assert!(overloaded > 0, "no input made a called overload set");
    }
}
