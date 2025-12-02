use auto_lsp::default::db::BaseDatabase;

use crate::{
    HirNodeInfo,
    check::errors::{body_inference::BodyInferenceError, init_inference::InitInferenceError},
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Elementary, Expr, ExprKind, FuncCall, InitExpr, InitExprKind,
                Integer, ParamAssign, ParamAssignKind, PathExpr, PrimaryExpr, VariableAccess,
                VariableAccessKind,
            },
            invocation::{self, Invocation, InvocationKind},
            spec::{Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange},
            statement::{Stmt, StmtKind},
        },
        interned::identifier::{Ident, SpanIdent},
        pous::{
            class::{Class, MethodDecl}, data_type::DataType, function::Function, function_block::FunctionBlock, interface::{Interface, MethodPrototype}, pou::{Pou, PouDecl}, variable::VariableDecl
        },
        scope::{Scope, ScopeId, ScopeKind},
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult},
        def_map::LocalDefMap,
        infer::ctx::InferCtx,
        inheritance_solver::MethodRef,
        init_inference::{InitExprInferenceResult, InitExprWalkStep},
        name_res::{pou_names_res, resolve_namespace_access},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    // Primitive types
    Elementary(ElementarySpec),
    RefTo(Spec<'db>),
    Null,
    // Structured types
    Struct(Struct<'db>),
    StructElement(StructElement<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>),
    Enum(Enum<'db>),
    EnumVariant(Ident),
    SubRange(SubRange<'db>),
    // Pous
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
    // Methods
    MethodDecl(MethodRef<'db>),
    Variable(VariableDecl<'db>),
    Infer(InferType),
    Never,
}

impl Default for Type<'_> {
    fn default() -> Self {
        Type::Never
    }
}

impl From<Elementary> for Type<'_> {
    fn from(elem: Elementary) -> Self {
        match elem {
            Elementary::Bool(_) => Type::Elementary(ElementarySpec::Bool),
            Elementary::Byte(_) => Type::Elementary(ElementarySpec::Byte),
            Elementary::Word(_) => Type::Elementary(ElementarySpec::Word),
            Elementary::DWord(_) => Type::Elementary(ElementarySpec::DWord),
            Elementary::LWord(_) => Type::Elementary(ElementarySpec::LWord),
            Elementary::SInt(_) => Type::Elementary(ElementarySpec::SInt),
            Elementary::Int(_) => Type::Elementary(ElementarySpec::Int),
            Elementary::DInt(_) => Type::Elementary(ElementarySpec::DInt),
            Elementary::LInt(_) => Type::Elementary(ElementarySpec::LInt),
            Elementary::USInt(_) => Type::Elementary(ElementarySpec::USInt),
            Elementary::UInt(_) => Type::Elementary(ElementarySpec::UInt),
            Elementary::UDInt(_) => Type::Elementary(ElementarySpec::UDInt),
            Elementary::ULInt(_) => Type::Elementary(ElementarySpec::ULInt),
            Elementary::Real(_) => Type::Elementary(ElementarySpec::Real),
            Elementary::LReal(_) => Type::Elementary(ElementarySpec::LReal),
            Elementary::Time(_) => Type::Elementary(ElementarySpec::Time),
            Elementary::LTime(_) => Type::Elementary(ElementarySpec::LTime),
            Elementary::TimeOfDay(_) => Type::Elementary(ElementarySpec::Tod),
            Elementary::LTod(_) => Type::Elementary(ElementarySpec::LTod),
            Elementary::Date(_) => Type::Elementary(ElementarySpec::Date),
            Elementary::LDate(_) => Type::Elementary(ElementarySpec::LDate),
            Elementary::DateAndTime(_) => Type::Elementary(ElementarySpec::DateAndTime),
            Elementary::LDateTime(_) => Type::Elementary(ElementarySpec::LDateTime),
            Elementary::AnyChar(_) => Type::Elementary(ElementarySpec::Char),
            Elementary::AnyString(_) => Type::Elementary(ElementarySpec::String),
            Elementary::InferInteger(integer) => Type::Infer(InferType::Integer(integer)),
            Elementary::InferFloat(ident) => Type::Infer(InferType::Float(ident)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum InferType {
    Integer(Integer),
    Float(Ident),
}

impl<'db> InferType {
    pub fn infer_default(&self, db: &'db dyn BaseDatabase) -> Type<'db> {
        match self {
            InferType::Integer(integer) => integer
                .as_i32(db)
                .map(|_| Type::Elementary(ElementarySpec::Int))
                .unwrap_or_else(|_| Type::Never),
            InferType::Float(ident) => ident
                .as_f32(db)
                .map(|_| Type::Elementary(ElementarySpec::Real))
                .unwrap_or_else(|_| Type::Never),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum CallableType<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    MethodDecl(MethodRef<'db>),
}

impl<'db> CallableType<'db> {
    pub fn def_map(&self, db: &'db dyn BaseDatabase) -> &'db LocalDefMap<'db> {
        match self {
            CallableType::Function(f) => f.scope_id(db).def_map(db),
            CallableType::FunctionBlock(fb) => fb.scope_id(db).def_map(db),
            CallableType::MethodDecl(m) => m.get_scope_id(db).def_map(db),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    Null,
    Size(usize),
}

#[salsa::tracked]
impl<'db> Type<'db> {
    pub const fn new_bool() -> Self {
        Type::Elementary(ElementarySpec::Bool)
    }

    pub fn new_pou(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Self {
        match pou.pou(db) {
            Pou::Class(cl) => Type::Class(*cl),
            Pou::Function(f) => Type::Function(*f),
            Pou::FunctionBlock(f) => Type::FunctionBlock(*f),
            Pou::Interface(f) => Type::Interface(*f),
            Pou::DataType(dt) => Type::DataType(*dt),
        }
    }

    pub fn new_var(db: &'db dyn BaseDatabase, var: VariableDecl<'db>) -> Self {
        Type::Variable(var)
    }

    #[salsa::tracked]
    pub fn new_spec(db: &'db dyn BaseDatabase, spec: Spec<'db>) -> Self {
        match spec.kind(db) {
            SpecKind::Simple(elem) => Type::Elementary(*elem),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => match resolve_namespace_access(db, &t.path) {
                Some(pou) => Type::new_pou(db, pou),
                None => Type::Never,
            },
        }
    }

    pub fn as_callable(&self) -> Option<CallableType<'db>> {
        Some(match self {
            Type::Function(f) => CallableType::Function(*f),
            Type::FunctionBlock(fb) => CallableType::FunctionBlock(*fb),
            Type::MethodDecl(m) => CallableType::MethodDecl(*m),
            _ => None?,
        })
    }

    pub fn with_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Type<'db>> {
        match self {
            Type::Function(f) => f.return_type(db),
            Type::MethodDecl(m) => m.return_type(db),
            _ => None?,
        }
        .map(|rt| Type::new_spec(db, *rt))
    }

    pub const fn get_size(&self) -> Size {
        match self {
            Type::Elementary(elem) => match elem {
                ElementarySpec::Bool | ElementarySpec::FEDGEBool | ElementarySpec::REDGEBool => {
                    Size::Size(1)
                }
                ElementarySpec::Byte | ElementarySpec::SInt | ElementarySpec::USInt => {
                    Size::Size(8)
                }
                ElementarySpec::Word | ElementarySpec::Int | ElementarySpec::UInt => Size::Size(16),
                ElementarySpec::DWord
                | ElementarySpec::DInt
                | ElementarySpec::UDInt
                | ElementarySpec::Real => Size::Size(32),
                ElementarySpec::LWord
                | ElementarySpec::LInt
                | ElementarySpec::ULInt
                | ElementarySpec::LReal => Size::Size(64),
                ElementarySpec::Time
                | ElementarySpec::Tod
                | ElementarySpec::Date
                | ElementarySpec::DateAndTime => Size::Size(32),
                ElementarySpec::LTime
                | ElementarySpec::LTod
                | ElementarySpec::LDate
                | ElementarySpec::LDateTime => Size::Size(64),
                _ => Size::Null,
            },
            _ => Size::Null,
        }
    }

    // Type coercion check
    #[must_use]
    pub fn coerce_with(
        &self,
        db: &'db dyn BaseDatabase,
        to: Type<'db>,
        scope: ScopeId<'db>,
    ) -> bool {
        // We return true if the lhs or rhs is of type never.
        // that's because Never variants are already reported by the resolver and we don't want to propagate too many errors

        if self.is_never() || to.is_never() {
            return true;
        }

        match (self, &to) {
            // allow coercion between Type and its Spec
            (Type::DataType(typ), typ2) => {
                Type::new_spec(db, typ.spec(db)).coerce_with(db, *typ2, scope)
            }
            (typ1, Type::DataType(typ)) => {
                typ1.coerce_with(db, Type::new_spec(db, typ.spec(db)), scope)
            }
            // same with variable declarations
            (Type::Variable(var), var2) => {
                Type::new_spec(db, var.spec(db)).coerce_with(db, *var2, scope)
            }
            (var1, Type::Variable(var)) => {
                var1.coerce_with(db, Type::new_spec(db, var.spec(db)), scope)
            }
            // variant is already solved by the resolver
            (Type::Enum(e1), Type::EnumVariant(e2)) => true,
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => return s1 == s2,
            // check element spec equality
            (Type::StructElement(elem), rhs) => {
                Type::new_spec(db, elem.spec(db)).coerce_with(db, *rhs, scope)
            }
            // same types are assignable
            (Type::Array(a1), Type::Array(a2)) => return a1 == a2,
            // check element spec equality
            (Type::Array(a1), rhs) => {
                Type::new_spec(db, a1.of_type(db)).coerce_with(db, *rhs, scope)
            }
            // check subrange base type equality
            (Type::SubRange(sub), rhs) => {
                Type::new_spec(db, sub._type(db)).coerce_with(db, *rhs, scope)
            }
            (Type::Elementary(lhs), Type::Elementary(rhs)) => {
                if lhs == rhs {
                    return true;
                }
                // try implicit conversions in both directions
                return !lhs.implicit_cast(*rhs).is_never() || !rhs.implicit_cast(*lhs).is_never();
            }
            (Type::Infer(infer), rhs) => infer.infer_default(db).coerce_with(db, *rhs, scope),
            (lhs, Type::Infer(infer)) => infer.infer_default(db).coerce_with(db, *lhs, scope),
            // Assignments to function /method are allowed *only inside their body*
            (Type::Function(f), rhs) => {
                if f.scope_id(db) != scope {
                    return false;
                }
                if let Some(ret_ty) = f.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, scope)
                } else {
                    false
                }
            }
            (Type::MethodDecl(m), rhs) => {
                if m.get_scope_id(db) != scope {
                    return false;
                }
                if let Some(ret_ty) = m.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, scope)
                } else {
                    false
                }
            }
            (Type::RefTo(_), Type::Null) => true,
            _ => false,
        }
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Type::Elementary(ElementarySpec::Bool))
    }

    pub fn is_numeric(&self) -> bool {
        self.is_binary_integer()
            || self.is_signed_integer()
            || self.is_unsigned_integer()
            || self.is_float()
    }

    pub fn is_binary_integer(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::Byte)
                | Type::Elementary(ElementarySpec::Word)
                | Type::Elementary(ElementarySpec::DWord)
                | Type::Elementary(ElementarySpec::LWord)
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::SInt)
                | Type::Elementary(ElementarySpec::Int)
                | Type::Elementary(ElementarySpec::DInt)
                | Type::Elementary(ElementarySpec::LInt)
        )
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::USInt)
                | Type::Elementary(ElementarySpec::UInt)
                | Type::Elementary(ElementarySpec::UDInt)
                | Type::Elementary(ElementarySpec::ULInt)
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::Real) | Type::Elementary(ElementarySpec::LReal)
        )
    }

    pub fn is_time(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::Time) | Type::Elementary(ElementarySpec::LTime)
        )
    }

    pub fn is_tod(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::Tod) | Type::Elementary(ElementarySpec::LTod)
        )
    }

    pub fn is_date(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::Date) | Type::Elementary(ElementarySpec::LDate)
        )
    }

    pub fn is_dt(&self) -> bool {
        matches!(
            self,
            Type::Elementary(ElementarySpec::DateAndTime)
                | Type::Elementary(ElementarySpec::LDateTime)
        )
    }

    pub fn is_never(&self) -> bool {
        matches!(self, Type::Never)
    }
}
