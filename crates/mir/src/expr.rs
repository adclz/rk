use hir::hir_def::interned::identifier::Ident;

use crate::types::{MirElementary, MirType};

/// An expression in MIR — fully resolved, no type inference needed.
#[derive(Debug, Clone)]
pub enum MirExpr {
    /// A constant literal value.
    Constant(MirConstant),

    /// Read from a place (variable, field, array element, deref).
    Load(MirPlace, MirType),

    /// Binary operation. Both operands and the result have the same type
    /// (casts are inserted explicitly during lowering).
    BinOp {
        op: MirBinOp,
        lhs: Box<MirExpr>,
        rhs: Box<MirExpr>,
        /// The concrete type at which this operation executes.
        ty: MirElementary,
    },

    /// Unary operation.
    UnaryOp {
        op: MirUnaryOp,
        expr: Box<MirExpr>,
        ty: MirElementary,
    },

    /// Explicit type cast (inserted by lowering for implicit casts).
    Cast {
        expr: Box<MirExpr>,
        from: MirElementary,
        to: MirElementary,
    },

    /// Function or method call that produces a value.
    Call(MirCall),

    /// Take the address of a place (REF operator).
    AddrOf(MirPlace),

    /// Copy `size` bytes from the address `src` yields into the scratch local
    /// `scratch`, then yield the scratch's address: the call-entry snapshot an
    /// aggregate `VAR_INPUT` argument is passed as (see `ExprLowerCtx::call_scratch`).
    CopyIntoScratch {
        scratch: Ident,
        src: Box<MirExpr>,
        size: u32,
    },

    /// String literal reference.
    StringLiteral {
        /// Index into MirModule::string_literals.
        id: u32,
        /// Pre-computed offset in the data section.
        offset: u32,
        /// Length in bytes.
        len: u32,
    },
}

/// A call expression (also usable as statement via MirStmt::Call).
#[derive(Debug, Clone)]
pub struct MirCall {
    /// Callee name (salsa-interned).
    pub callee: Ident,
    /// Pre-resolved function index.
    pub callee_index: u32,
    /// Arguments in parameter order.
    pub args: Vec<MirCallArg>,
    /// Return type.
    pub return_type: MirType,
    /// After the call, copy each callee `VAR_OUTPUT` to the caller's place.
    pub output_bindings: Vec<MirOutputBinding>,
    /// Extern-import results: scalar `VAR_OUTPUT`s come back on the stack, in
    /// declaration order, the return value last; each pops into a scratch and
    /// is stored to its bound place (`None` = discarded). Empty for other calls.
    pub extern_results: Vec<ExternResultBind>,
    /// Holds the return value while the outputs above pop; set only when the
    /// callee is extern with BOTH outputs and a return type.
    pub extern_ret_scratch: Option<Ident>,
}

/// One extern result: where it pops, and where it goes.
#[derive(Debug, Clone)]
pub struct ExternResultBind {
    /// The scalar scratch local the result pops into (`$extret$N`).
    pub scratch: Ident,
    /// The `o => dest` place, or `None` when the output was not bound.
    pub dest: Option<MirPlace>,
    pub ty: MirType,
    /// `Some` when the destination is a WIDER scalar than the result: the
    /// store converts to it instead of writing the result's bytes raw.
    pub target_lane: Option<MirElementary>,
}

/// A store the call makes once the callee has returned, from the scratch a
/// `VAR_OUTPUT` was received in, because its `=>` destination could not take
/// it directly: it is wider than the output, and the caller converts, or it
/// is part of a wider address, and the caller rebuilds that address's cell
/// with the output's bits. Made after an extern's results are stored, so it
/// can read theirs.
#[derive(Debug, Clone)]
pub struct MirOutputBinding {
    /// Where the store goes.
    pub target: MirPlace,
    /// What is stored; it reads the scratch.
    pub value: MirExpr,
    /// The lane of the store.
    pub ty: MirElementary,
}

#[derive(Debug, Clone)]
pub struct MirCallArg {
    pub value: MirExpr,
    pub kind: MirArgKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirArgKind {
    /// Pass by value.
    ByValue,
    /// Pass by reference (address of the argument).
    ByRef,
}

/// A place designates a memory location that can be read or assigned to.
#[derive(Debug, Clone)]
pub enum MirPlace {
    /// A named local variable or parameter.
    Local(Ident),

    /// Field access: base.field_name, with the field offset pre-computed.
    Field {
        base: Box<MirPlace>,
        field_name: Ident,
        field_offset: u32,
        field_type: MirType,
    },

    /// Array index: base[index], with element size pre-computed.
    Index {
        base: Box<MirPlace>,
        index: Box<MirExpr>,
        element_size: u32,
        element_type: MirType,
        /// Lower bound of the array dimension (for offset calculation).
        lower_bound: i64,
    },

    /// Pointer dereference: base^
    Deref {
        base: Box<MirPlace>,
        pointee_type: MirType,
        /// Whether the pointer needs a runtime null check: true for a dereference
        /// the user wrote, false for the transparent `VAR_IN_OUT` dereference,
        /// which always addresses real storage.
        checked: bool,
    },

    /// Instance variable access through 'this' pointer (for methods).
    ThisField {
        field_name: Ident,
        field_offset: u32,
        field_type: MirType,
    },

    /// A configuration VAR_GLOBAL, or the `VAR_EXTERNAL` naming one.
    Global {
        /// Set when the body referenced this global by name: `address` and `ty`
        /// are placeholders `lower_module` fills once the layout is final.
        name: Option<Ident>,
        address: u32,
        ty: MirType,
    },
}

impl MirExpr {
    /// Whether evaluating this runs a call, which evaluating it twice would
    /// run twice.
    pub fn has_call(&self) -> bool {
        match self {
            MirExpr::Call(_) => true,
            MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => false,
            MirExpr::Load(place, _) | MirExpr::AddrOf(place) => place.has_call(),
            MirExpr::BinOp { lhs, rhs, .. } => lhs.has_call() || rhs.has_call(),
            MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => expr.has_call(),
            MirExpr::CopyIntoScratch { src, .. } => src.has_call(),
        }
    }
}

impl MirExpr {
    /// Whether this reaches a place whose path runs a call, which addressing
    /// the place twice would run twice.
    pub fn reaches_place_with_call(&self) -> bool {
        match self {
            MirExpr::Load(place, _) | MirExpr::AddrOf(place) => place.has_call(),
            MirExpr::Call(call) => call.reaches_place_with_call(),
            MirExpr::BinOp { lhs, rhs, .. } => {
                lhs.reaches_place_with_call() || rhs.reaches_place_with_call()
            }
            MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
                expr.reaches_place_with_call()
            }
            MirExpr::CopyIntoScratch { src, .. } => src.reaches_place_with_call(),
            MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => false,
        }
    }
}

impl MirExpr {
    /// Whether `f` holds for this expression or one inside it, the subscripts
    /// of its places included.
    pub fn any<F: FnMut(&MirExpr) -> bool>(&self, f: &mut F) -> bool {
        if f(self) {
            return true;
        }
        match self {
            MirExpr::Load(place, _) | MirExpr::AddrOf(place) => place.any_expr(f),
            MirExpr::Call(call) => call.any_expr(f),
            MirExpr::BinOp { lhs, rhs, .. } => lhs.any(f) || rhs.any(f),
            MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => expr.any(f),
            MirExpr::CopyIntoScratch { src, .. } => src.any(f),
            MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => false,
        }
    }
}

impl MirCall {
    /// [`MirExpr::any`] over the call's arguments and the stores of its
    /// outputs.
    pub fn any_expr<F: FnMut(&MirExpr) -> bool>(&self, f: &mut F) -> bool {
        self.args.iter().any(|a| a.value.any(f))
            || self
                .output_bindings
                .iter()
                .any(|b| b.target.any_expr(f) || b.value.any(f))
            || self
                .extern_results
                .iter()
                .any(|r| r.dest.as_ref().is_some_and(|p| p.any_expr(f)))
    }

    /// [`MirExpr::reaches_place_with_call`] for the call's arguments and the
    /// places its outputs are stored in.
    pub fn reaches_place_with_call(&self) -> bool {
        self.args.iter().any(|a| a.value.reaches_place_with_call())
            || self
                .output_bindings
                .iter()
                .any(|b| b.target.has_call() || b.value.reaches_place_with_call())
            || self
                .extern_results
                .iter()
                .any(|r| r.dest.as_ref().is_some_and(MirPlace::has_call))
    }
}

impl MirPlace {
    /// Whether reaching this place runs a call: one in a subscript.
    pub fn has_call(&self) -> bool {
        match self {
            MirPlace::Local(_) | MirPlace::ThisField { .. } | MirPlace::Global { .. } => false,
            MirPlace::Field { base, .. } | MirPlace::Deref { base, .. } => base.has_call(),
            MirPlace::Index { base, index, .. } => base.has_call() || index.has_call(),
        }
    }

    /// [`MirExpr::any`] over the expressions in this place's path: its
    /// subscripts.
    pub fn any_expr<F: FnMut(&MirExpr) -> bool>(&self, f: &mut F) -> bool {
        match self {
            MirPlace::Local(_) | MirPlace::ThisField { .. } | MirPlace::Global { .. } => false,
            MirPlace::Field { base, .. } | MirPlace::Deref { base, .. } => base.any_expr(f),
            MirPlace::Index { base, index, .. } => base.any_expr(f) || index.any(f),
        }
    }

    /// The type of what the place holds, which a bare `Local` does not say.
    pub fn ty(&self) -> Option<&MirType> {
        match self {
            MirPlace::Local(_) => None,
            MirPlace::Field { field_type, .. } | MirPlace::ThisField { field_type, .. } => {
                Some(field_type)
            }
            MirPlace::Index { element_type, .. } => Some(element_type),
            MirPlace::Deref { pointee_type, .. } => Some(pointee_type),
            MirPlace::Global { ty, .. } => Some(ty),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,
    // Boolean / Bitwise
    And,
    Or,
    Xor,
    /// Logical shift left, used to build partial (bit/byte/word) accesses.
    Shl,
    /// Logical (zero-filling) shift right — see [`MirBinOp::Shl`].
    Shr,
    // Comparison (result is always Bool/i32)
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    /// Arithmetic negation.
    Neg,
    /// Boolean/bitwise not.
    Not,
}

#[derive(Debug, Clone)]
pub enum MirConstant {
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    /// Null pointer.
    Null,
}
