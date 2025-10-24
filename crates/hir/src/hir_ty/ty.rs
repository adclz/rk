use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};
use rustc_hash::FxHashMap;

use crate::{
    AstId, HirNodeInfo, TypeInfo,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::spec::{
            Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange,
        },
        interned::identifier::Ident,
        pous::{
            class::Class,
            data_type::DataType,
            function::Function,
            function_block::FunctionBlock,
            interface::Interface,
            pou::{Pou, PouDecl},
        },
        scope::FileScopeId,
    },
    hir_ty::name_res::resolve_namespace_access,
};

/// [`Ty`] represents a type in the HIR.
/// 
/// It can be created from a [`Spec`] using the [`Spec::to_ty`] method.
/// 
/// Ty encapsulates both simple types (like elementary types, arrays, enums, structs)
/// and complex types (like POUs: functions, function blocks, classes, interfaces).
/// 
/// Each Ty has a source, which is either a Spec or a POU declaration.
/// 
/// The purpose of Ty is to provide a unified representation of types in the HIR,
/// allowing for easy type checking, coercion, and error reporting.
#[salsa::tracked(debug)]
pub struct Ty<'db> {
    pub spec: TySource<'db>,

    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TySource<'db> {
    Spec(Spec<'db>),
    Pou((Spec<'db>, PouDecl<'db>)),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyKind<'db> {
    // Specs
    Simple(ElementarySpec),
    Enum(Enum<'db>),
    SubRange(SubRange<'db>),
    RefTo(Spec<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>), // todo
    Struct(Struct<'db>),

    // POUs
    // there is no DataType because a Type is a spec and thus belongs to the variants above
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),

    Err(AccessError<'db>),
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self.spec(db) {
            TySource::Spec(spec) => spec.get_id(db),
            TySource::Pou((spec, pou)) => pou.name_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self.spec(db) {
            TySource::Spec(spec) => spec.get_scope_id(db),
            TySource::Pou((spec, pou)) => spec.get_scope_id(db),
        }
    }
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn is_simple(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn is_pou(&self, db: &'db dyn BaseDatabase) -> bool {
        match self.kind(db) {
            TyKind::Function(f) => true,
            TyKind::FunctionBlock(fb) => true,
            TyKind::Class(c) => true,
            TyKind::Interface(i) => true,
            _ => false,
        }
    }

    pub fn diag_with_location(
        &self,
        db: &'db dyn BaseDatabase,
        diag: &mut IdeDiagnostic,
        message_closure: Option<fn(String) -> String>,
    ) {
        diag.with_related(Related::new(
            match message_closure {
                Some(closure) => closure(self.type_name(db)),
                None => format!("type '{}' defined here", self.type_name(db)),
            },
            self.get_scope_id(db).file(db),
            self.get_span(db),
        ));
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked]
    pub fn to_ty(self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        let kind = match self.kind(db) {
            SpecKind::Array(array) => TyKind::Array(*array),
            SpecKind::Enum(enum_spec) => TyKind::Enum(*enum_spec),
            SpecKind::Subrange(subrange) => TyKind::SubRange(*subrange),
            SpecKind::Struct(ztruct) => TyKind::Struct(*ztruct),
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, target.path) {
                    Some(pou) => {
                        let kind = match pou.pou(db) {
                            Pou::Function(f) => TyKind::Function(*f),
                            Pou::FunctionBlock(fb) => TyKind::FunctionBlock(*fb),
                            Pou::Class(c) => TyKind::Class(*c),
                            Pou::Interface(i) => TyKind::Interface(*i),
                            Pou::DataType(dt) => dt.spec(db).to_ty_kind(db),
                        };
                        return Ty::new(db, TySource::Pou((self, pou)), kind);
                    }
                    None => TyKind::Err(AccessError::NoItemInScope { access: *target }),
                }
            }
        };
        Ty::new(db, TySource::Spec(self), kind)
    }

    fn to_ty_kind(&self, db: &'db dyn BaseDatabase) -> TyKind<'db> {
        match self.kind(db) {
            SpecKind::Array(array) => TyKind::Array(*array),
            SpecKind::Enum(enum_spec) => TyKind::Enum(*enum_spec),
            SpecKind::Subrange(subrange) => TyKind::SubRange(*subrange),
            SpecKind::Struct(ztruct) => TyKind::Struct(*ztruct),
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, target.path) {
                    Some(pou) => {
                        match pou.pou(db) {
                            Pou::Function(f) => TyKind::Function(*f),
                            Pou::FunctionBlock(fb) => TyKind::FunctionBlock(*fb),
                            Pou::Class(c) => TyKind::Class(*c),
                            Pou::Interface(i) => TyKind::Interface(*i),
                            Pou::DataType(dt) => dt.spec(db).to_ty_kind(db),
                        }
                    }
                    None => TyKind::Err(AccessError::NoItemInScope { access: *target }),
                }
            }
        }
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.spec(db) {
            TySource::Spec(spec) => spec.type_name(db),
            TySource::Pou((_, pou,)) => format!(
                "{}: {}",
                pou.name(db).text(db),
                match pou.pou(db) {
                    Pou::Function(_) => "FUNCTION".into(),
                    Pou::FunctionBlock(_) => "FUNCTION_BLOCK".into(),
                    Pou::Class(_) => "CLASS".into(),
                    Pou::Interface(_) => "INTERFACE".into(),
                    Pou::DataType(dt) => dt.spec(db).type_name(db),
                }
            ),
        }
    }
}
