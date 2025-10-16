use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    AstId, HirNodeInfo, TypeInfo,
    check::errors::path_error::PathResolveError,
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
    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    // The actual kind of the type
    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

impl<'db> Ty<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        self.def(db).name(db)
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self.def(db) {
            TyDef::Pou(pou) => pou.get_name_span(db).unwrap(),
            TyDef::MethodRef(m) => m.get_name_span(db).unwrap(),
            TyDef::Spec(spec) => spec.get_span(db),
        }
    }
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.def(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.def(db).get_scope_id(db)
    }
}

// Definition of the type (POU or Spec)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDef<'db> {
    Pou(PouDecl<'db>),
    MethodRef(MethodRef<'db>),
    Spec(Spec<'db>),
}

impl<'db> TyDef<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            TyDef::Pou(pou) => pou.name(db).text(db).to_string(),
            TyDef::MethodRef(m) => m.name(db).text(db).to_string(),
            TyDef::Spec(spec) => spec.shorthand(db),
        }
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self {
            TyDef::Pou(p) => p.modifier(db),
            _ => Modifier::empty(),
        }
    }

    pub fn is_pou(&self) -> bool {
        matches!(self, TyDef::Pou(_))
    }

    pub fn is_spec(&self) -> bool {
        matches!(self, TyDef::Spec(_))
    }

    pub fn get_span(&self, db: &'db dyn BaseDatabase) -> Option<Span> {
        match self {
            TyDef::Pou(pou) => Some(pou.get_span(db)),
            TyDef::MethodRef(m) => Some(m.get_span(db)),
            TyDef::Spec(spec) => Some(spec.get_span(db)),
        }
    }
}

impl<'db> HirNodeInfo<'db> for TyDef<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDef::Pou(pou) => pou.get_id(db),
            TyDef::MethodRef(m) => m.get_id(db),
            TyDef::Spec(spec) => spec.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDef::Pou(pou) => pou.get_scope_id(db),
            TyDef::MethodRef(m) => m.get_scope_id(db),
            TyDef::Spec(spec) => spec.get_scope_id(db),
        }
    }
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
    Pou(PouDecl<'db>),

    // Error variants
    Unresolved(SpanNamespaceAccess<'db>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchMode {
    Local,
    Global,
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
    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Option<Visibility> {
        match self.def(db) {
            TyDef::MethodRef(method) => Some(method.visibility(db)),
            _ => None,
        }
    }

    pub fn is_simple(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn is_unresolved(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Unresolved(_))
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked]
    pub fn spec_to_ty(self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        let kind = match self.kind(db) {
            SpecKind::Array(array) => TyKind::Array(array.clone()),
            SpecKind::Enum(enum_spec) => TyKind::Enum(enum_spec.clone()),
            SpecKind::Subrange(subrange) => TyKind::SubRange(subrange.clone()),
            SpecKind::Struct(ztruct) => TyKind::Struct(ztruct.clone()),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), target.path) {
                    Some(pou) => TyKind::Pou(pou),
                    None => TyKind::Unresolved(*target),
                }
            }
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
        };
        Ty::new(db, TyDef::Spec(self), kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Enum { .. } => "ENUM".into(),
            TyKind::SubRange { .. } => "SUBRANGE".into(),
            TyKind::RefTo(ref_) => format!("REF_TO {}", ref_.spec_to_ty(db).type_name(db)),
            TyKind::Array { .. } => "ARRAY".into(),
            TyKind::ArrayConformand { .. } => "ARRAY*".into(),
            TyKind::Struct { .. } => "STRUCT".into(),
            TyKind::Pou(pou) => pou.name(db).text(db).to_string(),
            TyKind::Unresolved(_) => "{unknown}".into(),
        }
    }
}
