use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{
    def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpannedNamespaceAccess},
        pous::variable::VariableDecl,
        scope::FileScopeId,
        semantic_index::{semantic_index, SemanticIndex},
    },
    to_proto::{AstId, IterToProto, ToProto},
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
    pub name: Ident,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId,
}

impl<'db> ToProto<'db> for MethodPrototype<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::to_proto::AstId {
        self.id(db)       
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        let file = self.get_scope_id(db).file();
        Some(semantic_index(db, file).ast.get(self.name_id(db).0)?.get_span())
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
