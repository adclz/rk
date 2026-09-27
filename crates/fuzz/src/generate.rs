//! Well-typed programs, written with their expected results.
//!
//! Every other oracle checks that the compiler behaves; none checks that
//! the program it emits computes the right values. This one builds a
//! program from the fuzzer's bytes (integer arithmetic at every width,
//! BOOL logic, IF, CASE, FOR with EXIT and CONTINUE, RETURN, and calls to
//! small FUNCTIONs), evaluates it here with the semantics the docs give
//! (docs/math-operations.md: every result wraps at its type's width,
//! division truncates, MOD takes the dividend's sign), and writes the
//! values each variable must hold after two scans into a header comment.
//! [`crate::semantics`] compiles the text, runs it, reads the variables
//! back through the debug symbols and compares.
//!
//! Division by zero is the one thing that stops a module, so a divisor is
//! always a literal other than 0 and -1 (`DINT#-2147483648 / -1` traps
//! too). Loops have literal bounds, so every program ends.

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
        let bits = self.bits();
        let modulus = 1i128 << bits;
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
    /// A variable, or in a FUNCTION one of its inputs.
    Var(usize),
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
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    Case(Expr, Vec<(Vec<Label>, Vec<Stmt>)>, Option<Vec<Stmt>>),
    For(usize, i128, i128, i128, Vec<Stmt>),
    Exit,
    Continue,
    Return,
}

struct Var {
    name: String,
    ty: Ty,
    init: i128,
    /// A FOR counter: read only inside its loop, and not compared, since
    /// its value after the loop is the implementation's.
    counter: bool,
}

struct Function {
    name: String,
    ty: Ty,
    inputs: usize,
    body: Expr,
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
    vars: Vec<Var>,
    functions: Vec<Function>,
    /// Statements left to write, so a program stays small.
    budget: usize,
}

impl Generator<'_> {
    fn integer(&mut self) -> Ty {
        INTEGERS[self.choices.below(INTEGERS.len())]
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
        let v = self.literal(ty);
        match v {
            0 | -1 => 3,
            _ => v,
        }
    }

    /// An expression of type `ty` over the variables in `readable`.
    fn expr(&mut self, ty: Ty, depth: u32, readable: &[usize], calls: bool) -> Expr {
        let of_ty: Vec<usize> = readable
            .iter()
            .copied()
            .filter(|&v| self.vars[v].ty == ty)
            .collect();
        let leaf = depth == 0 || self.choices.percent(30);
        if leaf {
            return match of_ty.is_empty() || self.choices.percent(35) {
                true => Expr::Lit(ty, self.literal(ty)),
                false => Expr::Var(of_ty[self.choices.below(of_ty.len())]),
            };
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
        let n = 1 + self.choices.below(4);
        for _ in 0..n {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            let assignable: Vec<usize> = (0..self.vars.len())
                .filter(|&v| !self.vars[v].counter)
                .collect();
            let stmt = match self.choices.below(12) {
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
                    let by = match self.choices.below(4) {
                        0 => -1,
                        1 => 2,
                        2 => -2,
                        _ => 1,
                    };
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

/// A program from `bytes`: ST text whose header comment holds the value of
/// every variable after [`SCANS`] scans.
pub fn program(bytes: &[u8]) -> String {
    let mut g = Generator {
        choices: Choices { bytes, at: 0 },
        vars: Vec::new(),
        functions: Vec::new(),
        budget: 24,
    };

    for k in 0..g.choices.below(4) {
        let ty = g.integer();
        let inputs = 1 + g.choices.below(3);
        // The inputs are the only variables a FUNCTION body reads.
        let saved = std::mem::take(&mut g.vars);
        g.vars = (0..inputs)
            .map(|i| Var {
                name: format!("a{i}"),
                ty,
                init: 0,
                counter: false,
            })
            .collect();
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

    for k in 0..1 + g.choices.below(8) {
        let ty = match g.choices.percent(20) {
            true => Ty::Bool,
            false => g.integer(),
        };
        let init = g.literal(ty);
        g.vars.push(Var {
            name: format!("v{k}"),
            ty,
            init,
            counter: false,
        });
    }
    let readable: Vec<usize> = (0..g.vars.len()).collect();
    let counters: Vec<usize> = (0..2)
        .map(|k| {
            g.vars.push(Var {
                name: format!("i{k}"),
                ty: Ty::Dint,
                init: 0,
                counter: true,
            });
            g.vars.len() - 1
        })
        .collect();
    let body = g.block(3, &readable, &counters, false);

    // What the program must leave behind.
    let mut state: Vec<i128> = g.vars.iter().map(|v| v.init).collect();
    for _ in 0..SCANS {
        let _ = run_block(&body, &mut state, &g.vars, &g.functions);
    }

    let mut out = format!("(* rk-fuzz expects, after {SCANS} scans:\n");
    for (v, value) in g.vars.iter().zip(&state) {
        if !v.counter {
            let _ = writeln!(out, "Run.{} = {}", v.name, shown(v.ty, *value));
        }
    }
    out.push_str("*)\n\n");
    for f in &g.functions {
        let _ = writeln!(out, "FUNCTION {} : {}\nVAR_INPUT", f.name, f.ty.name());
        for i in 0..f.inputs {
            let _ = writeln!(out, "    a{i} : {};", f.ty.name());
        }
        let names: Vec<String> = (0..f.inputs).map(|i| format!("a{i}")).collect();
        let _ = writeln!(
            out,
            "END_VAR\n    {} := {};\nEND_FUNCTION\n",
            f.name,
            text(&f.body, &names, &g.functions)
        );
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
    out.push_str("END_VAR\n");
    let names: Vec<String> = g.vars.iter().map(|v| v.name.clone()).collect();
    write_block(&mut out, &body, 1, &names, &g.functions);
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

fn text(e: &Expr, names: &[String], functions: &[Function]) -> String {
    let t = |e: &Expr| text(e, names, functions);
    match e {
        Expr::Lit(ty, v) => literal(*ty, *v),
        Expr::Var(v) => names[*v].clone(),
        Expr::Arith(op, _, a, b) => {
            let op = match op {
                Arith::Add => "+",
                Arith::Sub => "-",
                Arith::Mul => "*",
                Arith::Div => "/",
                Arith::Mod => "MOD",
            };
            format!("({} {op} {})", t(a), t(b))
        }
        Expr::Neg(_, a) => format!("(-{})", t(a)),
        Expr::Cmp(op, a, b) => {
            let op = match op {
                Cmp::Eq => "=",
                Cmp::Ne => "<>",
                Cmp::Lt => "<",
                Cmp::Gt => ">",
                Cmp::Le => "<=",
                Cmp::Ge => ">=",
            };
            format!("({} {op} {})", t(a), t(b))
        }
        Expr::Not(a) => format!("(NOT {})", t(a)),
        Expr::Logic(op, a, b) => {
            let op = match op {
                Logic::And => "AND",
                Logic::Or => "OR",
                Logic::Xor => "XOR",
            };
            format!("({} {op} {})", t(a), t(b))
        }
        Expr::Call(f, args) => {
            let args: Vec<String> = args
                .iter()
                .enumerate()
                .map(|(i, a)| format!("a{i} := {}", t(a)))
                .collect();
            format!("{}({})", functions[*f].name, args.join(", "))
        }
    }
}

fn write_block(
    out: &mut String,
    block: &[Stmt],
    level: usize,
    names: &[String],
    functions: &[Function],
) {
    let pad = "    ".repeat(level);
    let t = |e: &Expr| text(e, names, functions);
    for stmt in block {
        match stmt {
            Stmt::Assign(v, e) => {
                let _ = writeln!(out, "{pad}{} := {};", names[*v], t(e));
            }
            Stmt::If(arms, otherwise) => {
                for (i, (cond, body)) in arms.iter().enumerate() {
                    let kw = if i == 0 { "IF" } else { "ELSIF" };
                    let _ = writeln!(out, "{pad}{kw} {} THEN", t(cond));
                    write_block(out, body, level + 1, names, functions);
                }
                if let Some(body) = otherwise {
                    let _ = writeln!(out, "{pad}ELSE");
                    write_block(out, body, level + 1, names, functions);
                }
                let _ = writeln!(out, "{pad}END_IF;");
            }
            Stmt::Case(selector, arms, otherwise) => {
                let _ = writeln!(out, "{pad}CASE {} OF", t(selector));
                for (labels, body) in arms {
                    let labels: Vec<String> = labels
                        .iter()
                        .map(|l| match l {
                            Label::One(v) => v.to_string(),
                            Label::Range(a, b) => format!("{a}..{b}"),
                        })
                        .collect();
                    let _ = writeln!(out, "{pad}{}:", labels.join(", "));
                    write_block(out, body, level + 1, names, functions);
                }
                if let Some(body) = otherwise {
                    let _ = writeln!(out, "{pad}ELSE");
                    write_block(out, body, level + 1, names, functions);
                }
                let _ = writeln!(out, "{pad}END_CASE;");
            }
            Stmt::For(v, from, to, by, body) => {
                let _ = writeln!(out, "{pad}FOR {} := {from} TO {to} BY {by} DO", names[*v]);
                write_block(out, body, level + 1, names, functions);
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

/// How a statement ended.
enum Flow {
    Next,
    Exit,
    Continue,
    Return,
}

fn run_block(block: &[Stmt], state: &mut [i128], vars: &[Var], functions: &[Function]) -> Flow {
    for stmt in block {
        let flow = match stmt {
            Stmt::Assign(v, e) => {
                state[*v] = vars[*v].ty.wrap(eval(e, state, functions));
                Flow::Next
            }
            Stmt::If(arms, otherwise) => {
                match arms
                    .iter()
                    .find(|(cond, _)| eval(cond, state, functions) != 0)
                {
                    Some((_, body)) => run_block(body, state, vars, functions),
                    None => match otherwise {
                        Some(body) => run_block(body, state, vars, functions),
                        None => Flow::Next,
                    },
                }
            }
            Stmt::Case(selector, arms, otherwise) => {
                let s = eval(selector, state, functions);
                let hit = arms.iter().find(|(labels, _)| {
                    labels.iter().any(|l| match l {
                        Label::One(v) => s == *v,
                        Label::Range(a, b) => *a <= s && s <= *b,
                    })
                });
                match (hit, otherwise) {
                    (Some((_, body)), _) => run_block(body, state, vars, functions),
                    (None, Some(body)) => run_block(body, state, vars, functions),
                    (None, None) => Flow::Next,
                }
            }
            Stmt::For(v, from, to, by, body) => {
                state[*v] = *from;
                let mut flow = Flow::Next;
                while (*by > 0 && state[*v] <= *to) || (*by < 0 && state[*v] >= *to) {
                    match run_block(body, state, vars, functions) {
                        Flow::Exit => break,
                        Flow::Return => {
                            flow = Flow::Return;
                            break;
                        }
                        Flow::Next | Flow::Continue => {}
                    }
                    state[*v] += *by;
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
fn eval(e: &Expr, env: &[i128], functions: &[Function]) -> i128 {
    let ev = |e: &Expr| eval(e, env, functions);
    match e {
        Expr::Lit(_, v) => *v,
        Expr::Var(v) => env[*v],
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
            let inputs: Vec<i128> = args.iter().map(ev).collect();
            let f = &functions[*f];
            f.ty.wrap(eval(&f.body, &inputs, functions))
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
        let e = |op, a, b| {
            eval(
                &Expr::Arith(
                    op,
                    Ty::Dint,
                    Box::new(Expr::Lit(Ty::Dint, a)),
                    Box::new(Expr::Lit(Ty::Dint, b)),
                ),
                &[],
                &[],
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
