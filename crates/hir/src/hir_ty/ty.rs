use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Integer, MultibitsPart},
            spec::{Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange},
        },
        interned::identifier::Ident,
        pous::{
            class::Class,
            data_type::DataType,
            function::Function,
            function_block::FunctionBlock,
            interface::Interface,
            pou::Pou,
            variable::{DirectVariable, VariableDecl},
        },
        program::ProgramDecl,
        scope::ScopeId,
    },
    hir_ty::{
        def_map::LocalDefMap, signature::inheritance::MethodRef, name_res::resolve_namespace_access,
    },
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, salsa::Update, Default)]
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
    Variable((VariableDecl<'db>, Option<MultibitsPart>)),
    // HW bindings
    DirectVariable((DirectVariable<'db>, Option<MultibitsPart>)),
    // A reference created inside a body
    Infer(InferType),
    // Program (cannot be seen by other types, only used here for body inference)
    Program(ProgramDecl<'db>),
    // Func call - same as methods, functions, function blocks but we know it's being called
    CallableType(CallableType<'db>),
    // Void type, usually the result of a call that does not return anything
    Void,
    // Never type, represents an unresolvable type
    // Important: this type will stop propagation of errors and
    // therefore *requires* a diagnostic to be emitted when created
    #[default]
    Never,
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
    pub fn to_ty(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        Type::Elementary(self.to_spec(db))
    }

    pub fn to_spec(&self, db: &'db dyn WorkspaceDataBase) -> ElementarySpec {
        match self {
            InferType::Integer(i) => ElementarySpec::Int,
            InferType::Float(f) => ElementarySpec::Real,
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
    pub fn def_map(&self, db: &'db dyn WorkspaceDataBase) -> &'db LocalDefMap<'db> {
        match self {
            CallableType::Function(f) => f.scope_id(db).def_map(db),
            CallableType::FunctionBlock(fb) => fb.scope_id(db).def_map(db),
            CallableType::MethodDecl(m) => m.get_scope_id(db).def_map(db),
        }
    }

    pub fn var_len_params(&self, db: &'db dyn WorkspaceDataBase) -> usize {
        self.def_map(db).local_variables.len()
    }
}

impl<'db> HirNodeInfo<'db> for CallableType<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            CallableType::Function(f) => f.get_id(db),
            CallableType::FunctionBlock(f) => f.get_id(db),
            CallableType::MethodDecl(m) => m.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            CallableType::Function(f) => f.get_scope_id(db),
            CallableType::FunctionBlock(f) => f.get_scope_id(db),
            CallableType::MethodDecl(m) => m.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for CallableType<'db> {
    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            CallableType::Function(f) => f.get_name_id(db),
            CallableType::FunctionBlock(f) => f.get_name_id(db),
            CallableType::MethodDecl(m) => m.get_name_id(db),
        }
    }

    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            CallableType::Function(f) => f.get_name_ident(db),
            CallableType::FunctionBlock(f) => f.get_name_ident(db),
            CallableType::MethodDecl(m) => m.get_name_ident(db),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    Null,
    Size(usize),
}

impl PartialOrd for Size {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Size::Null, Size::Null) => Some(std::cmp::Ordering::Equal),
            (Size::Null, Size::Size(_)) => Some(std::cmp::Ordering::Less),
            (Size::Size(_), Size::Null) => Some(std::cmp::Ordering::Greater),
            (Size::Size(a), Size::Size(b)) => a.partial_cmp(b),
        }
    }
}

#[salsa::tracked]
impl<'db> Type<'db> {
    
    #[salsa::tracked]
    pub(crate) fn new_spec(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Self {
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

    pub const fn new_bool() -> Self {
        Type::Elementary(ElementarySpec::Bool)
    }

    pub fn new_pou(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Self {
        match pou {
            Pou::Class(cl) => Type::Class(cl),
            Pou::Function(f) => Type::Function(f),
            Pou::FunctionBlock(f) => Type::FunctionBlock(f),
            Pou::Interface(f) => Type::Interface(f),
            Pou::DataType(dt) => Type::DataType(dt),
        }
    }

    pub fn new_var(db: &'db dyn WorkspaceDataBase, var: VariableDecl<'db>) -> Self {
        Type::Variable((var, None))
    }

    pub fn new_var_with_multibits(
        db: &'db dyn WorkspaceDataBase,
        var: VariableDecl<'db>,
        multibits: Option<MultibitsPart>,
    ) -> Self {
        Type::Variable((var, multibits))
    }

    pub fn as_callable(&self, db: &'db dyn WorkspaceDataBase) -> Option<CallableType<'db>> {
        Some(match self {
            Type::Function(f) => CallableType::Function(*f),
            Type::FunctionBlock(fb) => CallableType::FunctionBlock(*fb),
            Type::MethodDecl(m) => CallableType::MethodDecl(*m),
            _ => None?,
        })
    }

    pub fn with_return_type(&self, db: &'db dyn WorkspaceDataBase) -> Option<Type<'db>> {
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

    pub fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }

    pub fn has_infer(&self) -> bool {
        matches!(self, Type::Infer(_))
    }

    pub fn is_variable(&self) -> bool {
        matches!(self, Type::Variable(_))
    }

    pub fn is_fb(&self) -> bool {
        matches!(self, Type::FunctionBlock(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(_))
    }
}
