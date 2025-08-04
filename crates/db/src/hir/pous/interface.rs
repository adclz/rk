use auto_lsp::{core::span::Span, default::db::{file::File, BaseDatabase}};

use crate::{
    hir::{expressions::spec::Spec, interned::{identifier::Ident, namespace::SpannedNamespaceAccess}, pous::variable::Variable, scopes::scope::{FilePouId, ScopeId}, semantic_index::SemanticIndex},
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpannedNamespaceAccess>>,

    #[returns(ref)]
    pub methods: Vec<Method<'db>>,

    pub scope_id: ScopeId,

}

impl<'db> IterToProto<'db> for Interface<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.methods(db)
            .iter()
            .map(move |m| m.iter(db, sema).map(|n| n))
            .flatten()
    }
}

#[salsa::tracked(debug)]
pub struct Method<'db> {
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

impl<'db> ToProto<'db> for Method<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db)
    }

    fn get_named_span(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }
}

impl<'db> IterToProto<'db> for Method<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        Box::new(self.variables(db).iter().map(move |v| v as _))
    }
}
