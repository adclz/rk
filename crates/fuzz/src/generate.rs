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
//! - BOOL, arrays, and STRUCTs with defaults in their TYPE.
//! - IF, CASE (integers and strings), FOR with EXIT and CONTINUE, RETURN.
//! - FUNCTIONs; FUNCTION_BLOCKs with inputs, outputs, state, a VAR_IN_OUT
//!   and METHODs that change their state; INTERFACEs, reached the way rk
//!   allows them, as a FUNCTION's VAR_IN_OUT (E1121).
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
        }
    }

    fn is_int(self) -> bool {
        INTEGERS.contains(&self)
    }

    fn is_float(self) -> bool {
        matches!(self, Ty::Real | Ty::Lreal)
    }

    /// Whether `-x` is allowed.
    fn signed(self) -> bool {
        matches!(
            self,
            Ty::Sint | Ty::Int | Ty::Dint | Ty::Lint | Ty::Real | Ty::Lreal
        )
    }

    fn bits(self) -> u32 {
        match self {
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
}

#[derive(Clone, Debug)]
enum Label {
    Int(i128),
    Range(i128, i128),
    Str(Vec<u8>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Assign(Place, Expr),
    /// An FB instance called with its inputs, and the variable bound to
    /// its `VAR_IN_OUT` if it has one.
    CallFb(usize, Vec<Expr>, Option<usize>),
    /// `target := instance.method(args)`.
    CallMethod(Place, usize, usize, Vec<Expr>),
    /// `target := user(dev := instance, args)`: a FUNCTION that calls a
    /// method through its interface parameter.
    CallUser(Place, usize, usize, Vec<Expr>),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    Case(Expr, Vec<(Vec<Label>, Vec<Stmt>)>, Option<Vec<Stmt>>),
    For(usize, i128, i128, i128, Vec<Stmt>),
    Exit,
    Continue,
    Return,
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

struct Array {
    name: String,
    ty: Ty,
    lo: i128,
    init: Vec<Val>,
}

struct Function {
    name: String,
    ty: Ty,
    inputs: Vec<Var>,
    body: Expr,
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

/// A program's declarations, which the evaluator and the printer read.
#[derive(Default)]
struct Decls {
    /// The fields of `S{k}`.
    structs: Vec<Vec<Var>>,
    functions: Vec<Function>,
    interfaces: Vec<Interface>,
    blocks: Vec<Block>,
    users: Vec<User>,
    arrays: Vec<Array>,
    instances: Vec<Instance>,
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
            Ty::Real => Val::F32(0.0),
            Ty::Lreal => Val::F64(0.0),
            Ty::Str(_) => Val::Str(Vec::new()),
            Ty::Struct(k) => {
                Val::Struct(self.d.structs[k].iter().map(|f| f.init.clone()).collect())
            }
            _ => Val::Int(0),
        }
    }

    /// A divisor: never 0, and never -1, which traps at the minimum.
    fn divisor(&mut self, ty: Ty) -> Val {
        match self.literal(ty) {
            Val::Int(0 | -1) => Val::Int(3),
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

    /// An expression of type `ty` over the variables in `readable`. A
    /// string expression reads a string of any capacity; a string literal
    /// fits the capacity asked for.
    fn expr(&mut self, ty: Ty, depth: u32, readable: &[usize]) -> Expr {
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
            if let Some(call) = self.call(ty, depth, readable) {
                return call;
            }
            return Expr::Lit(ty, self.literal(ty));
        }
        let d = depth - 1;
        if ty == Ty::Bool {
            return match self.choices.below(4) {
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
                    let operands = match self.scalar_type() {
                        Ty::Bool => Ty::Dint,
                        t => t,
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
        match self.choices.below(10) {
            0 if ty.signed() => Expr::Neg(ty, Box::new(self.expr(ty, d, readable))),
            1 => match self.call(ty, depth, readable) {
                Some(call) => call,
                None => Expr::Lit(ty, self.literal(ty)),
            },
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

    /// A call to a FUNCTION returning `ty` (any string, for a string), if
    /// there is one and calls are allowed here.
    fn call(&mut self, ty: Ty, depth: u32, readable: &[usize]) -> Option<Expr> {
        if !self.in_program || depth == 0 {
            return None;
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
        let inputs: Vec<Ty> = self.d.functions[f].inputs.iter().map(|v| v.ty).collect();
        let args = inputs
            .into_iter()
            .map(|t| self.expr(t, depth - 1, readable))
            .collect();
        Some(Expr::Call(f, args))
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
            let stmt = match self.choices.below(16) {
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
                    let mut inner = readable.to_vec();
                    inner.push(counter);
                    let body = self.block(depth - 1, &inner, &free[1..], true, may_return);
                    Stmt::For(counter, from, to, by, body)
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
                labels.push(match lo == hi {
                    true => Label::Int(lo),
                    false => Label::Range(lo, hi),
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
        let ty = self.var_type(true);
        let target = self.target(ty)?;
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
        Some(Stmt::Assign(target, value))
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

    fn place_ty(&self, place: &Place) -> Ty {
        let tys: Vec<Ty> = self.vars.iter().map(|v| v.ty).collect();
        place_ty(place, &tys, &self.d)
    }

    /// Scalar variables for a signature or a declaration: numbers, BOOLs
    /// and strings, each with a literal to start from.
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
    };

    for _ in 0..g.choices.below(3) {
        let n = 1 + g.choices.below(3);
        let fields = g.vars_named("f", n, Role::Plain);
        g.d.structs.push(fields);
    }

    for k in 0..g.choices.below(4) {
        let ty = g.scalar_type();
        let n = 1 + g.choices.below(3);
        // A string FUNCTION takes strings; any other, its own type.
        let inputs: Vec<Var> = (0..n)
            .map(|i| Var {
                name: format!("a{i}"),
                ty: match ty {
                    Ty::Str(_) => Ty::Str(1 + g.choices.below(8) as u8),
                    t => t,
                },
                init: Val::Int(0),
                role: Role::Input,
            })
            .collect();
        let readable: Vec<usize> = (0..n).collect();
        let (inputs, body) = g.with_frame(inputs, |g| g.expr(ty, 3, &readable));
        g.d.functions.push(Function {
            name: format!("g{k}"),
            ty,
            inputs,
            body,
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
        g.d.blocks.push(Block {
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

    for k in 0..1 + g.choices.below(8) {
        let ty = g.var_type(true);
        let init = g.literal(ty);
        g.vars.push(Var {
            name: format!("v{k}"),
            ty,
            init,
            role: Role::Plain,
        });
    }
    // Somewhere to put what each method returns.
    let results: Vec<Ty> =
        g.d.blocks
            .iter()
            .flat_map(|b| b.methods.iter().map(|m| m.sig.ty))
            .collect();
    for ty in results {
        let present = g.vars.iter().any(|v| match (v.ty, ty) {
            (Ty::Str(_), Ty::Str(_)) => true,
            (a, b) => a == b,
        });
        if !present {
            let init = g.literal(ty);
            g.vars.push(Var {
                name: format!("v{}", g.vars.len()),
                ty,
                init,
                role: Role::Plain,
            });
        }
    }
    for k in 0..g.choices.below(3) {
        let ty = g.var_type(true);
        let lo = g.choices.below(6) as i128 - 3;
        let init = (0..1 + g.choices.below(5)).map(|_| g.literal(ty)).collect();
        g.d.arrays.push(Array {
            name: format!("arr{k}"),
            ty,
            lo,
            init,
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
    // The program's own budget, whatever the blocks spent.
    g.budget = 28;
    let body = g.block(3, &readable, &counters, false, true);

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
    out.push_str("*)\n\n");

    let names = |vars: &[Var]| Names {
        vars: vars.iter().map(|v| v.name.clone()).collect(),
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

    for (k, fields) in d.structs.iter().enumerate() {
        let _ = writeln!(out, "TYPE S{k} : STRUCT");
        for f in fields {
            decl(&mut out, f, true);
        }
        out.push_str("END_STRUCT\nEND_TYPE\n\n");
    }
    for f in &d.functions {
        let _ = writeln!(out, "FUNCTION {} : {}\nVAR_INPUT", f.name, f.ty.name());
        for v in &f.inputs {
            decl(&mut out, v, false);
        }
        let _ = writeln!(
            out,
            "END_VAR\n    {} := {};\nEND_FUNCTION\n",
            f.name,
            names(&f.inputs).expr(&f.body)
        );
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
    for b in &d.blocks {
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
                // A reference has no initial value of its own.
                decl(&mut out, &b.members[m], role != Role::InOut);
            }
            out.push_str("END_VAR\n");
        }
        for m in &b.methods {
            signature(&mut out, &m.sig);
            let mut frame = b.members.clone();
            frame.extend(m.sig.params.iter().cloned());
            let n = names(&frame);
            n.block(&mut out, &m.body, 2);
            let _ = writeln!(
                out,
                "        {} := {};\n    END_METHOD",
                m.sig.name,
                n.expr(&m.result)
            );
        }
        names(&b.members).block(&mut out, &b.body, 1);
        out.push_str("END_FUNCTION_BLOCK\n\n");
    }
    out.push_str("PROGRAM P\nVAR\n");
    for v in vars {
        decl(&mut out, v, !matches!(v.ty, Ty::Struct(_)));
    }
    for a in &d.arrays {
        let range = format!("{}..{}", a.lo, a.lo + a.init.len() as i128 - 1);
        match a.ty {
            // Elements of a STRUCT type take the TYPE's defaults.
            Ty::Struct(_) => {
                let _ = writeln!(out, "    {} : ARRAY[{range}] OF {};", a.name, a.ty.name());
            }
            _ => {
                let values: Vec<String> = a.init.iter().map(|v| literal(a.ty, v)).collect();
                let _ = writeln!(
                    out,
                    "    {} : ARRAY[{range}] OF {} := [{}];",
                    a.name,
                    a.ty.name(),
                    values.join(", ")
                );
            }
        }
    }
    for i in &d.instances {
        let _ = writeln!(out, "    {} : {};", i.name, d.blocks[i.block].name);
    }
    out.push_str("END_VAR\n");
    names(vars).block(&mut out, body, 1);
    out.push_str(
        "END_PROGRAM\n\nCONFIGURATION C\n    RESOURCE R ON CPU\n        TASK T(INTERVAL := T#10ms, PRIORITY := 1);\n        PROGRAM Run WITH T : P;\n    END_RESOURCE\nEND_CONFIGURATION\n",
    );
    out
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

fn literal(ty: Ty, v: &Val) -> String {
    match (ty, v) {
        (Ty::Bool, _) => shown(ty, v),
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

/// What the names in a frame's code refer to.
struct Names<'a> {
    vars: Vec<String>,
    d: &'a Decls,
}

impl Names<'_> {
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
                format!("{}({})", f.name, self.args(&f.inputs, args))
            }
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

    fn block(&self, out: &mut String, block: &[Stmt], level: usize) {
        let pad = "    ".repeat(level);
        for stmt in block {
            match stmt {
                Stmt::Assign(p, e) => {
                    let _ = writeln!(out, "{pad}{} := {};", self.place(p), self.expr(e));
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
                                Label::Int(v) => v.to_string(),
                                Label::Range(a, b) => format!("{a}..{b}"),
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
                Stmt::For(v, from, to, by, body) => {
                    let _ = writeln!(
                        out,
                        "{pad}FOR {} := {from} TO {to} BY {by} DO",
                        self.vars[*v]
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
}

impl State {
    fn frame(vars: Vec<Val>) -> Self {
        Self {
            vars,
            arrays: Vec::new(),
            instances: Vec::new(),
        }
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
    }
}

fn write(p: &Place, v: Val, st: &mut State, tys: &[Ty], d: &Decls) {
    let ty = place_ty(p, tys, d);
    *slot(p, st, d) = store(ty, v);
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

fn run_block(block: &[Stmt], st: &mut State, tys: &[Ty], d: &Decls) -> Flow {
    for stmt in block {
        let flow = match stmt {
            Stmt::Assign(p, e) => {
                let v = eval(e, st, d);
                write(p, v, st, tys, d);
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
                        (Label::Int(v), Val::Int(x)) => x == v,
                        (Label::Range(a, b), Val::Int(x)) => a <= x && x <= b,
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
            Stmt::For(v, from, to, by, body) => {
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
        };
        if !matches!(flow, Flow::Next) {
            return flow;
        }
    }
    Flow::Next
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
            let inputs = f
                .inputs
                .iter()
                .zip(args)
                .map(|(v, a)| store(v.ty, ev(a)))
                .collect();
            store(f.ty, eval(&f.body, &State::frame(inputs), d))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
