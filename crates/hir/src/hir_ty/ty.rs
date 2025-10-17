use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    AstId, HirNodeInfo, TypeInfo,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::spec::{
            Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange,
        },
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::Class,
            function::Function,
            function_block::FunctionBlock,
            interface::Interface,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
        visibility::Visibility,
    },
    hir_ty::{
        inheritance_solver::{MethodRef, method_table},
        name_res::resolve_namespace_access,
        ty_var_access_resolver::PathExprWalkStep,
    },
};

/// The  resolved type of a variable, POU, or method
/// [`Ty`] is the most fundamental unit of type information in the HIR.
#[salsa::tracked(debug)]
pub struct Ty<'db> {
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
    Target(SpanNamespaceAccess<'db>)
    //Pou(PouDecl<'db>),
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
            SpecKind::Target(target) => TyKind::Target(*target),
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
        };
        Ty::new(db, kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Enum { .. } => "ENUM".into(),
            TyKind::SubRange { .. } => "SUBRANGE".into(),
            TyKind::RefTo(ref_) => format!("REF_TO {}", ref_.to_ty(db).type_name(db)),
            TyKind::Array { .. } => "ARRAY".into(),
            TyKind::ArrayConformand { .. } => "ARRAY*".into(),
            TyKind::Struct { .. } => "STRUCT".into(),
            TyKind::Target(_) => "{unknown}".into(),
        }
    }
}
