use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{
    hir::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpannedNamespaceAccess},
        pous::variable::Variable,
        scopes::scope::FileScopeId,
        semantic_index::SemanticIndex,
    },
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpannedNamespaceAccess>>,

    #[returns(ref)]
    pub methods: Vec<MethodPrototype<'db>>,

    pub scope_id: FileScopeId,
}

impl<'db> IterToProto<'db> for Interface<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.methods(db).iter().flat_map(move |m| m.iter(db, sema))
    }
}

#[salsa::tracked(debug)]
pub struct MethodPrototype<'db> {
    #[returns(ref)]
    pub range: Span,

    pub name: Ident,

    #[returns(ref)]
    pub name_span: Span,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}

impl<'db> ToProto<'db> for MethodPrototype<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db)
    }

    fn get_named_span(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }
}

impl<'db> IterToProto<'db> for MethodPrototype<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        Box::new(self.variables(db).iter().map(move |v| v as _))
    }
}
