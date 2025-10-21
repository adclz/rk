use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use ide_diagnostic::{IdeDiagnostic, Related};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::path_error::AccessError, hir_def::{
        expressions::spec::{
            Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange,
        },
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        pous::{class::Class, function::Function, function_block::FunctionBlock, interface::Interface, pou::{Pou, PouDecl}}, scope::FileScopeId,
    }, hir_ty::name_res::resolve_namespace_access, AstId, HirNodeInfo, TypeInfo
};

/// Resolved type of a [`Spec`]
///
/// For now Ty just wraps [`Spec`], but in the future it can represent more complex types.
/// 
/// Ty serves as a '
#[salsa::tracked(debug)]
pub struct Ty<'db> {
    pub spec: Spec<'db>,

    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
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

    Err(AccessError<'db>)
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.spec(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.spec(db).get_scope_id(db)
    }
}

impl<'db> Struct<'db> {
    pub fn resolve_elements(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> FxHashMap<Ident, StructElement<'db>> {
        self.elements
            .iter()
            .map(|element| (*element.name(db), *element))
            .collect()
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
            SpecKind::Array(array) => TyKind::Array(array.clone()),
            SpecKind::Enum(enum_spec) => TyKind::Enum(enum_spec.clone()),
            SpecKind::Subrange(subrange) => TyKind::SubRange(subrange.clone()),
            SpecKind::Struct(ztruct) => TyKind::Struct(ztruct.clone()),
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
            SpecKind::Target(target) => match resolve_namespace_access(db, target.scope_id, target.path) {
                Some(pou) => match pou.pou(db) {
                    Pou::Function(f) => TyKind::Function(*f),
                    Pou::FunctionBlock(fb) => TyKind::FunctionBlock(*fb),
                    Pou::Class(c) => TyKind::Class(*c),
                    Pou::Interface(i) => TyKind::Interface(*i),
                    Pou::DataType(dt) => return dt.spec(db).to_ty(db),
                },
                None => {
                    TyKind::Err(AccessError::NoItemInScope {
                        access: target.clone(),
                    })
                }
            },
        };
        Ty::new(db, self, kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        self.spec(db).type_name(db)
    }
}
