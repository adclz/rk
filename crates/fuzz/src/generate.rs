//! Well-typed programs, written with their expected results.
//!
//! Every other oracle checks that the compiler behaves; none checks that
//! the program it emits computes the right values. This one builds a
//! program from the fuzzer's bytes, evaluates it here with the semantics
//! the docs give (docs/math-operations.md: every result wraps at its type's
//! width, division truncates, MOD takes the dividend's sign), and writes
//! the value of every variable, array element and FB member after two
//! scans into a header comment. [`crate::semantics`] compiles the text,
//! runs it, reads each one back through the debug symbols and compares.
//!
//! A program has integers at every width and BOOLs; arrays indexed by
//! computed expressions; IF, CASE, FOR with EXIT and CONTINUE, RETURN;
//! FUNCTIONs; and FUNCTION_BLOCKs with inputs, outputs, state and a
//! VAR_IN_OUT, called from the program and keeping their state between
//! scans. A `VAR_IN_OUT` is modelled as copy-in, copy-out, which is the
//! same thing here: nothing else runs while the block does.
//!
//! Nothing may stop the module: a divisor is a literal other than 0 and -1
//! (`DINT#-2147483648 / -1` traps), an index is wrapped into its array's
//! range by the expression itself, and loops have literal bounds.

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
    fn name(self) -> &'static str {
        match self {
            Ty::Bool => "BOOL",
            Ty::Sint => "SINT",
            Ty::Int => "INT",
            Ty::Dint => "DINT",
            Ty::Lint => "LINT",
            Ty::Usint => "USINT",
            Ty::Uint => "UINT",
            Ty::Udint => "UDINT",
            Ty::Ulint => "ULINT",
        }
    }

    fn bits(self) -> u32 {
        match self {
            Ty::Bool => 1,
            Ty::Sint | Ty::Usint => 8,
            Ty::Int | Ty::Uint => 16,
            Ty::Dint | Ty::Udint => 32,
            Ty::Lint | Ty::Ulint => 64,
        }
    }

    fn signed(self) -> bool {
        matches!(self, Ty::Sint | Ty::Int | Ty::Dint | Ty::Lint)
    }

    /// `v` wrapped into this type's range, as two's complement does.
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

#[derive(Clone, Debug)]
enum Expr {
    Lit(Ty, i128),
    /// A variable of the current frame: the program's, a FUNCTION's input,
    /// or a FUNCTION_BLOCK's member.
    Var(usize),
    /// An element of a program array.
    Index(usize, Box<Expr>),
    /// A member of a program's FB instance.
    Member(usize, usize),
    Arith(Arith, Ty, Box<Expr>, Box<Expr>),
    Neg(Ty, Box<Expr>),
    Cmp(Cmp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Logic(Logic, Box<Expr>, Box<Expr>),
    Call(usize, Vec<Expr>),
}

#[derive(Clone, Debug)]
enum Label {
    One(i128),
    Range(i128, i128),
}

#[derive(Clone, Debug)]
enum Stmt {
    Assign(usize, Expr),
    AssignIndex(usize, Expr, Expr),
    /// An FB instance called with its inputs, and the program variable
    /// bound to its `VAR_IN_OUT` if it has one.
    CallFb(usize, Vec<Expr>, Option<usize>),
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
    /// A FOR counter: read only inside its loop, and not compared, since
    /// its value after the loop is the implementation's.
    Counter,
    /// A FUNCTION's or an FB's input: read, never assigned.
    Input,
    Output,
    InOut,
}

#[derive(Clone)]
struct Var {
    name: String,
    ty: Ty,
    init: i128,
    role: Role,
}

struct Array {
    name: String,
    ty: Ty,
    lo: i128,
    init: Vec<i128>,
}

struct Function {
    name: String,
    ty: Ty,
    inputs: usize,
    body: Expr,
}

struct Block {
    name: String,
    /// Inputs first, then outputs, the in-out, and the state.
    members: Vec<Var>,
    body: Vec<Stmt>,
}

impl Block {
    fn inputs(&self) -> Vec<usize> {
        self.role(Role::Input)
    }

    fn inout(&self) -> Option<usize> {
        self.role(Role::InOut).first().copied()
    }

    fn role(&self, role: Role) -> Vec<usize> {
        (0..self.members.len())
            .filter(|&m| self.members[m].role == role)
            .collect()
    }
}

struct Instance {
    name: String,
    block: usize,
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
    /// FUNCTION's or FB's while its body is.
    vars: Vec<Var>,
    /// The program's arrays and FB instances; empty while a FUNCTION or
    /// FB body is written, since neither can see them.
    arrays: Vec<Array>,
    instances: Vec<Instance>,
    functions: Vec<Function>,
    blocks: Vec<Block>,
    /// Statements left to write, so a program stays small.
    budget: usize,
}

impl Generator<'_> {
    fn integer(&mut self) -> Ty {
        INTEGERS[self.choices.below(INTEGERS.len())]
    }

    fn any_type(&mut self) -> Ty {
        match self.choices.percent(20) {
            true => Ty::Bool,
            false => self.integer(),
        }
    }

    fn literal(&mut self, ty: Ty) -> i128 {
        if ty == Ty::Bool {
            return (self.choices.byte() & 1) as i128;
        }
        match self.choices.below(8) {
            0 => ty.min(),
            1 => ty.max(),
            2 => 0,
            3 => 1,
            4 if ty.signed() => -1,
            5 | 6 => ty.wrap((self.choices.byte() % 20) as i128 - 10),
            _ => ty.wrap(self.choices.u64() as i128),
        }
    }

    /// A divisor: never 0, and never -1, which traps at the minimum.
    fn divisor(&mut self, ty: Ty) -> i128 {
        match self.literal(ty) {
            0 | -1 => 3,
            v => v,
        }
    }

    /// An index into `array`: any DINT expression, wrapped into the range
    /// by `((e MOD n) + n) MOD n + lo`, which cannot leave it.
    fn index(&mut self, array: usize, readable: &[usize]) -> Expr {
        let (lo, n) = (self.arrays[array].lo, self.arrays[array].init.len() as i128);
        let e = self.expr(Ty::Dint, 1, readable, true);
        let lit = |v| Box::new(Expr::Lit(Ty::Dint, v));
        let wrapped = Expr::Arith(
            Arith::Mod,
            Ty::Dint,
            Box::new(Expr::Arith(
                Arith::Add,
                Ty::Dint,
                Box::new(Expr::Arith(Arith::Mod, Ty::Dint, Box::new(e), lit(n))),
                lit(n),
            )),
            lit(n),
        );
        Expr::Arith(Arith::Add, Ty::Dint, Box::new(wrapped), lit(lo))
    }

    /// Something of type `ty` to read: a variable, an array element or an
    /// FB output. `None` when there is none.
    fn place(&mut self, ty: Ty, readable: &[usize]) -> Option<Expr> {
        let vars: Vec<usize> = readable
            .iter()
            .copied()
            .filter(|&v| self.vars[v].ty == ty)
            .collect();
        let arrays: Vec<usize> = (0..self.arrays.len())
            .filter(|&a| self.arrays[a].ty == ty)
            .collect();
        let members: Vec<(usize, usize)> = self
            .instances
            .iter()
            .enumerate()
            .flat_map(|(i, inst)| {
                let block = &self.blocks[inst.block];
                block
                    .role(Role::Output)
                    .into_iter()
                    .filter(move |&m| block.members[m].ty == ty)
                    .map(move |m| (i, m))
            })
            .collect();
        let total = vars.len() + arrays.len() + members.len();
        if total == 0 {
            return None;
        }
        let pick = self.choices.below(total);
        Some(if pick < vars.len() {
            Expr::Var(vars[pick])
        } else if pick < vars.len() + arrays.len() {
            let a = arrays[pick - vars.len()];
            Expr::Index(a, Box::new(self.index(a, readable)))
        } else {
            let (i, m) = members[pick - vars.len() - arrays.len()];
            Expr::Member(i, m)
        })
    }

    /// An expression of type `ty` over the variables in `readable`.
    fn expr(&mut self, ty: Ty, depth: u32, readable: &[usize], calls: bool) -> Expr {
        if depth == 0 || self.choices.percent(30) {
            if !self.choices.percent(35)
                && let Some(place) = self.place(ty, readable)
            {
                return place;
            }
            return Expr::Lit(ty, self.literal(ty));
        }
        let d = depth - 1;
        if ty == Ty::Bool {
            return match self.choices.below(4) {
                0 => Expr::Not(Box::new(self.expr(ty, d, readable, calls))),
                1 => {
                    let op = [Logic::And, Logic::Or, Logic::Xor][self.choices.below(3)];
                    Expr::Logic(
                        op,
                        Box::new(self.expr(ty, d, readable, calls)),
                        Box::new(self.expr(ty, d, readable, calls)),
                    )
                }
                _ => {
                    let operands = self.integer();
                    let op = [Cmp::Eq, Cmp::Ne, Cmp::Lt, Cmp::Gt, Cmp::Le, Cmp::Ge]
                        [self.choices.below(6)];
                    Expr::Cmp(
                        op,
                        Box::new(self.expr(operands, d, readable, calls)),
                        Box::new(self.expr(operands, d, readable, calls)),
                    )
                }
            };
        }
        let callable: Vec<usize> = match calls {
            true => (0..self.functions.len())
                .filter(|&f| self.functions[f].ty == ty)
                .collect(),
            false => Vec::new(),
        };
        match self.choices.below(10) {
            0 if ty.signed() => Expr::Neg(ty, Box::new(self.expr(ty, d, readable, calls))),
            1 if !callable.is_empty() => {
                let f = callable[self.choices.below(callable.len())];
                let args = (0..self.functions[f].inputs)
                    .map(|_| self.expr(ty, d, readable, calls))
                    .collect();
                Expr::Call(f, args)
            }
            2 | 3 => {
                let op = [Arith::Div, Arith::Mod][self.choices.below(2)];
                let divisor = self.divisor(ty);
                Expr::Arith(
                    op,
                    ty,
                    Box::new(self.expr(ty, d, readable, calls)),
                    Box::new(Expr::Lit(ty, divisor)),
                )
            }
            _ => {
                let op = [Arith::Add, Arith::Sub, Arith::Mul][self.choices.below(3)];
                Expr::Arith(
                    op,
                    ty,
                    Box::new(self.expr(ty, d, readable, calls)),
                    Box::new(self.expr(ty, d, readable, calls)),
                )
            }
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
    ) -> Vec<Stmt> {
        let mut out = Vec::new();
        for _ in 0..1 + self.choices.below(4) {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            let assignable: Vec<usize> = (0..self.vars.len())
                .filter(|&v| matches!(self.vars[v].role, Role::Plain | Role::Output | Role::InOut))
                .collect();
            let stmt = match self.choices.below(14) {
                0 | 1 if depth > 0 => {
                    let mut arms = Vec::new();
                    for _ in 0..1 + self.choices.below(3) {
                        let cond = self.expr(Ty::Bool, 3, readable, true);
                        arms.push((cond, self.block(depth - 1, readable, free, in_loop)));
                    }
                    let otherwise = match self.choices.percent(50) {
                        true => Some(self.block(depth - 1, readable, free, in_loop)),
                        false => None,
                    };
                    Stmt::If(arms, otherwise)
                }
                2 if depth > 0 => {
                    let selector = self.expr(Ty::Dint, 2, readable, true);
                    let mut used = Vec::new();
                    let mut arms = Vec::new();
                    for _ in 0..1 + self.choices.below(4) {
                        let mut labels = Vec::new();
                        for _ in 0..1 + self.choices.below(2) {
                            let lo = self.choices.below(12) as i128 - 4;
                            let hi = lo + self.choices.below(3) as i128;
                            // Labels may not overlap.
                            if used.iter().any(|&(a, b)| lo <= b && a <= hi) {
                                continue;
                            }
                            used.push((lo, hi));
                            labels.push(match lo == hi {
                                true => Label::One(lo),
                                false => Label::Range(lo, hi),
                            });
                        }
                        if !labels.is_empty() {
                            arms.push((labels, self.block(depth - 1, readable, free, in_loop)));
                        }
                    }
                    let otherwise = match self.choices.percent(50) {
                        true => Some(self.block(depth - 1, readable, free, in_loop)),
                        false => None,
                    };
                    match arms.is_empty() {
                        true => continue,
                        false => Stmt::Case(selector, arms, otherwise),
                    }
                }
                3 if depth > 0 && !free.is_empty() => {
                    let counter = free[0];
                    let from = self.choices.below(9) as i128 - 4;
                    let to = self.choices.below(9) as i128 - 4;
                    let by = [1, -1, 2, -2][self.choices.below(4)];
                    let mut inner = readable.to_vec();
                    inner.push(counter);
                    Stmt::For(
                        counter,
                        from,
                        to,
                        by,
                        self.block(depth - 1, &inner, &free[1..], true),
                    )
                }
                4 if in_loop => match self.choices.percent(50) {
                    true => Stmt::Exit,
                    false => Stmt::Continue,
                },
                5 if self.choices.percent(20) => Stmt::Return,
                6 | 7 if !self.instances.is_empty() => {
                    let inst = self.choices.below(self.instances.len());
                    let block = &self.blocks[self.instances[inst].block];
                    let inputs: Vec<Ty> = block
                        .inputs()
                        .into_iter()
                        .map(|m| block.members[m].ty)
                        .collect();
                    let inout_ty = block.inout().map(|m| block.members[m].ty);
                    let bound = match inout_ty {
                        None => None,
                        Some(ty) => {
                            let fits: Vec<usize> = assignable
                                .iter()
                                .copied()
                                .filter(|&v| self.vars[v].ty == ty)
                                .collect();
                            match fits.is_empty() {
                                // Nothing to pass by reference.
                                true => continue,
                                false => Some(fits[self.choices.below(fits.len())]),
                            }
                        }
                    };
                    let args = inputs
                        .into_iter()
                        .map(|ty| self.expr(ty, 3, readable, true))
                        .collect();
                    Stmt::CallFb(inst, args, bound)
                }
                8 if !self.arrays.is_empty() => {
                    let a = self.choices.below(self.arrays.len());
                    let index = self.index(a, readable);
                    let value = self.expr(self.arrays[a].ty, 4, readable, true);
                    Stmt::AssignIndex(a, index, value)
                }
                _ if assignable.is_empty() => continue,
                _ => {
                    let target = assignable[self.choices.below(assignable.len())];
                    let ty = self.vars[target].ty;
                    Stmt::Assign(target, self.expr(ty, 4, readable, true))
                }
            };
            out.push(stmt);
        }
        out
    }
}

/// A program from `bytes`: ST text whose header comment holds, after
/// [`SCANS`] scans, the value of every variable, array element and FB
/// member.
pub fn program(bytes: &[u8]) -> String {
    let mut g = Generator {
        choices: Choices { bytes, at: 0 },
        vars: Vec::new(),
        arrays: Vec::new(),
        instances: Vec::new(),
        functions: Vec::new(),
        blocks: Vec::new(),
        budget: 28,
    };

    for k in 0..g.choices.below(4) {
        let ty = g.integer();
        let inputs = 1 + g.choices.below(3);
        // The inputs are the only variables a FUNCTION body reads.
        let saved = std::mem::replace(
            &mut g.vars,
            (0..inputs)
                .map(|i| Var {
                    name: format!("a{i}"),
                    ty,
                    init: 0,
                    role: Role::Input,
                })
                .collect(),
        );
        let readable: Vec<usize> = (0..inputs).collect();
        let body = g.expr(ty, 3, &readable, false);
        g.vars = saved;
        g.functions.push(Function {
            name: format!("g{k}"),
            ty,
            inputs,
            body,
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
            for i in 0..count {
                let ty = g.any_type();
                let init = g.literal(ty);
                let name = match role {
                    Role::InOut => prefix.to_string(),
                    _ => format!("{prefix}{i}"),
                };
                members.push(Var {
                    name,
                    ty,
                    init,
                    role,
                });
            }
        }
        // The body sees its members and nothing of the program's.
        let saved = std::mem::replace(&mut g.vars, members);
        let readable: Vec<usize> = (0..g.vars.len()).collect();
        let body = g.block(2, &readable, &[], false);
        let members = std::mem::replace(&mut g.vars, saved);
        g.blocks.push(Block {
            name: format!("fb{k}"),
            members,
            body,
        });
    }

    for k in 0..1 + g.choices.below(8) {
        let ty = g.any_type();
        let init = g.literal(ty);
        g.vars.push(Var {
            name: format!("v{k}"),
            ty,
            init,
            role: Role::Plain,
        });
    }
    for k in 0..g.choices.below(3) {
        let ty = g.any_type();
        let lo = g.choices.below(6) as i128 - 3;
        let init = (0..1 + g.choices.below(5)).map(|_| g.literal(ty)).collect();
        g.arrays.push(Array {
            name: format!("arr{k}"),
            ty,
            lo,
            init,
        });
    }
    if !g.blocks.is_empty() {
        for k in 0..g.choices.below(4) {
            let block = g.choices.below(g.blocks.len());
            g.instances.push(Instance {
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
                init: 0,
                role: Role::Counter,
            });
            g.vars.len() - 1
        })
        .collect();
    let body = g.block(3, &readable, &counters, false);

    // What the program must leave behind.
    let world = World {
        functions: &g.functions,
        blocks: &g.blocks,
        instances: &g.instances,
        arrays: &g.arrays,
    };
    let mut state = State {
        vars: g.vars.iter().map(|v| v.init).collect(),
        arrays: g.arrays.iter().map(|a| a.init.clone()).collect(),
        instances: g
            .instances
            .iter()
            .map(|i| g.blocks[i.block].members.iter().map(|m| m.init).collect())
            .collect(),
    };
    let tys: Vec<Ty> = g.vars.iter().map(|v| v.ty).collect();
    for _ in 0..SCANS {
        let _ = run_block(&body, &mut state, &tys, &world);
    }

    let mut out = format!("(* rk-fuzz expects, after {SCANS} scans:\n");
    for (v, value) in g.vars.iter().zip(&state.vars) {
        if v.role != Role::Counter {
            let _ = writeln!(out, "Run.{} = {}", v.name, shown(v.ty, *value));
        }
    }
    for (a, values) in g.arrays.iter().zip(&state.arrays) {
        for (k, value) in values.iter().enumerate() {
            let _ = writeln!(
                out,
                "Run.{}[{}] = {}",
                a.name,
                a.lo + k as i128,
                shown(a.ty, *value)
            );
        }
    }
    for (inst, values) in g.instances.iter().zip(&state.instances) {
        for (m, value) in g.blocks[inst.block].members.iter().zip(values) {
            // An in-out is a reference into the caller, not a value.
            if m.role != Role::InOut {
                let _ = writeln!(
                    out,
                    "Run.{}.{} = {}",
                    inst.name,
                    m.name,
                    shown(m.ty, *value)
                );
            }
        }
    }
    out.push_str("*)\n\n");

    let program_names = Names {
        vars: g.vars.iter().map(|v| v.name.clone()).collect(),
        arrays: g.arrays.iter().map(|a| a.name.clone()).collect(),
        instances: &g.instances,
        blocks: &g.blocks,
        functions: &g.functions,
    };
    for f in &g.functions {
        let _ = writeln!(out, "FUNCTION {} : {}\nVAR_INPUT", f.name, f.ty.name());
        for i in 0..f.inputs {
            let _ = writeln!(out, "    a{i} : {};", f.ty.name());
        }
        let names = Names {
            vars: (0..f.inputs).map(|i| format!("a{i}")).collect(),
            ..program_names.empty()
        };
        let _ = writeln!(
            out,
            "END_VAR\n    {} := {};\nEND_FUNCTION\n",
            f.name,
            names.expr(&f.body)
        );
    }
    for b in &g.blocks {
        let _ = writeln!(out, "FUNCTION_BLOCK {}", b.name);
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
                let m = &b.members[m];
                match role {
                    // A reference has no initial value of its own.
                    Role::InOut => {
                        let _ = writeln!(out, "    {} : {};", m.name, m.ty.name());
                    }
                    _ => {
                        let _ = writeln!(
                            out,
                            "    {} : {} := {};",
                            m.name,
                            m.ty.name(),
                            literal(m.ty, m.init)
                        );
                    }
                }
            }
            out.push_str("END_VAR\n");
        }
        let names = Names {
            vars: b.members.iter().map(|m| m.name.clone()).collect(),
            ..program_names.empty()
        };
        names.block(&mut out, &b.body, 1);
        out.push_str("END_FUNCTION_BLOCK\n\n");
    }
    out.push_str("PROGRAM P\nVAR\n");
    for v in &g.vars {
        let _ = writeln!(
            out,
            "    {} : {} := {};",
            v.name,
            v.ty.name(),
            literal(v.ty, v.init)
        );
    }
    for a in &g.arrays {
        let values: Vec<String> = a.init.iter().map(|v| literal(a.ty, *v)).collect();
        let _ = writeln!(
            out,
            "    {} : ARRAY[{}..{}] OF {} := [{}];",
            a.name,
            a.lo,
            a.lo + a.init.len() as i128 - 1,
            a.ty.name(),
            values.join(", ")
        );
    }
    for i in &g.instances {
        let _ = writeln!(out, "    {} : {};", i.name, g.blocks[i.block].name);
    }
    out.push_str("END_VAR\n");
    program_names.block(&mut out, &body, 1);
    out.push_str(
        "END_PROGRAM\n\nCONFIGURATION C\n    RESOURCE R ON CPU\n        TASK T(INTERVAL := T#10ms, PRIORITY := 1);\n        PROGRAM Run WITH T : P;\n    END_RESOURCE\nEND_CONFIGURATION\n",
    );
    out
}

fn shown(ty: Ty, v: i128) -> String {
    match ty {
        Ty::Bool => (if v != 0 { "TRUE" } else { "FALSE" }).to_string(),
        _ => v.to_string(),
    }
}

fn literal(ty: Ty, v: i128) -> String {
    match ty {
        Ty::Bool => shown(ty, v),
        _ => format!("{}#{v}", ty.name()),
    }
}

/// What the names in a frame's code refer to.
struct Names<'a> {
    vars: Vec<String>,
    arrays: Vec<String>,
    instances: &'a [Instance],
    blocks: &'a [Block],
    functions: &'a [Function],
}

impl<'a> Names<'a> {
    /// The program's view with no variables: what a FUNCTION or FB body
    /// starts from, before its own.
    fn empty(&self) -> Names<'a> {
        Names {
            vars: Vec::new(),
            arrays: Vec::new(),
            instances: &[],
            blocks: self.blocks,
            functions: self.functions,
        }
    }

    fn expr(&self, e: &Expr) -> String {
        match e {
            Expr::Lit(ty, v) => literal(*ty, *v),
            Expr::Var(v) => self.vars[*v].clone(),
            Expr::Index(a, index) => format!("{}[{}]", self.arrays[*a], self.expr(index)),
            Expr::Member(i, m) => {
                let inst = &self.instances[*i];
                format!("{}.{}", inst.name, self.blocks[inst.block].members[*m].name)
            }
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
                let args: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| format!("a{i} := {}", self.expr(a)))
                    .collect();
                format!("{}({})", self.functions[*f].name, args.join(", "))
            }
        }
    }

    fn block(&self, out: &mut String, block: &[Stmt], level: usize) {
        let pad = "    ".repeat(level);
        for stmt in block {
            match stmt {
                Stmt::Assign(v, e) => {
                    let _ = writeln!(out, "{pad}{} := {};", self.vars[*v], self.expr(e));
                }
                Stmt::AssignIndex(a, index, e) => {
                    let _ = writeln!(
                        out,
                        "{pad}{}[{}] := {};",
                        self.arrays[*a],
                        self.expr(index),
                        self.expr(e)
                    );
                }
                Stmt::CallFb(i, args, bound) => {
                    let inst = &self.instances[*i];
                    let block = &self.blocks[inst.block];
                    let mut parts: Vec<String> = block
                        .inputs()
                        .into_iter()
                        .zip(args)
                        .map(|(m, a)| format!("{} := {}", block.members[m].name, self.expr(a)))
                        .collect();
                    if let (Some(m), Some(v)) = (block.inout(), bound) {
                        parts.push(format!("{} := {}", block.members[m].name, self.vars[*v]));
                    }
                    let _ = writeln!(out, "{pad}{}({});", inst.name, parts.join(", "));
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
                                Label::One(v) => v.to_string(),
                                Label::Range(a, b) => format!("{a}..{b}"),
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

/// What the evaluator needs besides the state: the declarations.
struct World<'a> {
    functions: &'a [Function],
    blocks: &'a [Block],
    instances: &'a [Instance],
    arrays: &'a [Array],
}

/// A frame's values: its variables, and for the program its arrays and FB
/// instances (one value per member).
struct State {
    vars: Vec<i128>,
    arrays: Vec<Vec<i128>>,
    instances: Vec<Vec<i128>>,
}

/// How a statement ended.
enum Flow {
    Next,
    Exit,
    Continue,
    Return,
}

fn run_block(block: &[Stmt], st: &mut State, tys: &[Ty], w: &World) -> Flow {
    for stmt in block {
        let flow = match stmt {
            Stmt::Assign(v, e) => {
                st.vars[*v] = tys[*v].wrap(eval(e, st, w));
                Flow::Next
            }
            Stmt::AssignIndex(a, index, e) => {
                // The index first, then the value, as the code evaluates
                // them; neither has an effect on the other.
                let array = &w.arrays[*a];
                let k = (eval(index, st, w) - array.lo) as usize;
                let value = eval(e, st, w);
                st.arrays[*a][k] = array.ty.wrap(value);
                Flow::Next
            }
            Stmt::CallFb(i, args, bound) => {
                let block = &w.blocks[w.instances[*i].block];
                let values: Vec<i128> = args.iter().map(|a| eval(a, st, w)).collect();
                let mut frame = State {
                    vars: st.instances[*i].clone(),
                    arrays: Vec::new(),
                    instances: Vec::new(),
                };
                let member_tys: Vec<Ty> = block.members.iter().map(|m| m.ty).collect();
                for (m, v) in block.inputs().into_iter().zip(values) {
                    frame.vars[m] = member_tys[m].wrap(v);
                }
                let inout = block.inout().zip(*bound);
                if let Some((m, v)) = inout {
                    frame.vars[m] = st.vars[v];
                }
                let _ = run_block(&block.body, &mut frame, &member_tys, w);
                if let Some((m, v)) = inout {
                    st.vars[v] = frame.vars[m];
                }
                st.instances[*i] = frame.vars;
                Flow::Next
            }
            Stmt::If(arms, otherwise) => {
                match arms.iter().find(|(cond, _)| eval(cond, st, w) != 0) {
                    Some((_, body)) => run_block(body, st, tys, w),
                    None => match otherwise {
                        Some(body) => run_block(body, st, tys, w),
                        None => Flow::Next,
                    },
                }
            }
            Stmt::Case(selector, arms, otherwise) => {
                let s = eval(selector, st, w);
                let hit = arms.iter().find(|(labels, _)| {
                    labels.iter().any(|l| match l {
                        Label::One(v) => s == *v,
                        Label::Range(a, b) => *a <= s && s <= *b,
                    })
                });
                match (hit, otherwise) {
                    (Some((_, body)), _) => run_block(body, st, tys, w),
                    (None, Some(body)) => run_block(body, st, tys, w),
                    (None, None) => Flow::Next,
                }
            }
            Stmt::For(v, from, to, by, body) => {
                st.vars[*v] = *from;
                let mut flow = Flow::Next;
                while (*by > 0 && st.vars[*v] <= *to) || (*by < 0 && st.vars[*v] >= *to) {
                    match run_block(body, st, tys, w) {
                        Flow::Exit => break,
                        Flow::Return => {
                            flow = Flow::Return;
                            break;
                        }
                        Flow::Next | Flow::Continue => {}
                    }
                    st.vars[*v] += *by;
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

/// An expression's value, wrapped to its type. Every operation wraps, not
/// only the assignment: `(a * b) / c` in SINT is SINT arithmetic throughout.
fn eval(e: &Expr, st: &State, w: &World) -> i128 {
    let ev = |e: &Expr| eval(e, st, w);
    match e {
        Expr::Lit(_, v) => *v,
        Expr::Var(v) => st.vars[*v],
        Expr::Index(a, index) => st.arrays[*a][(ev(index) - w.arrays[*a].lo) as usize],
        Expr::Member(i, m) => st.instances[*i][*m],
        Expr::Arith(op, ty, a, b) => {
            let (a, b) = (ev(a), ev(b));
            ty.wrap(match op {
                Arith::Add => a.wrapping_add(b),
                Arith::Sub => a.wrapping_sub(b),
                Arith::Mul => a.wrapping_mul(b),
                // Rust's `/` and `%` truncate toward zero, as wasm's do.
                Arith::Div => a / b,
                Arith::Mod => a % b,
            })
        }
        Expr::Neg(ty, a) => ty.wrap(-ev(a)),
        Expr::Cmp(op, a, b) => {
            let (a, b) = (ev(a), ev(b));
            (match op {
                Cmp::Eq => a == b,
                Cmp::Ne => a != b,
                Cmp::Lt => a < b,
                Cmp::Gt => a > b,
                Cmp::Le => a <= b,
                Cmp::Ge => a >= b,
            }) as i128
        }
        Expr::Not(a) => (ev(a) == 0) as i128,
        Expr::Logic(op, a, b) => {
            let (a, b) = (ev(a) != 0, ev(b) != 0);
            (match op {
                Logic::And => a && b,
                Logic::Or => a || b,
                Logic::Xor => a ^ b,
            }) as i128
        }
        Expr::Call(f, args) => {
            let frame = State {
                vars: args.iter().map(ev).collect(),
                arrays: Vec::new(),
                instances: Vec::new(),
            };
            let f = &w.functions[*f];
            f.ty.wrap(eval(&f.body, &frame, w))
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
        let world = World {
            functions: &[],
            blocks: &[],
            instances: &[],
            arrays: &[],
        };
        let state = State {
            vars: Vec::new(),
            arrays: Vec::new(),
            instances: Vec::new(),
        };
        let e = |op, a, b| {
            eval(
                &Expr::Arith(
                    op,
                    Ty::Dint,
                    Box::new(Expr::Lit(Ty::Dint, a)),
                    Box::new(Expr::Lit(Ty::Dint, b)),
                ),
                &state,
                &world,
            )
        };
        assert_eq!(e(Arith::Div, -7, 2), -3);
        assert_eq!(e(Arith::Mod, -7, 2), -1);
        assert_eq!(e(Arith::Mod, 7, -2), 1);
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
