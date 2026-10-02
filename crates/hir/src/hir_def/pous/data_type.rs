// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::{expression::InitExpr, spec::Spec},
        interned::identifier::Ident,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    /// The name as the author wrote it. Matching reads `name(db)`.
    pub name_with_case: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for DataType<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for DataType<'db> {
    fn get_name_with_case(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> crate::hir_def::interned::identifier::Ident {
        self.name_with_case(db)
    }

    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

impl<'db> DataType<'db> {
    /// The name, as names are matched: case folded. As written:
    /// [`Self::name_with_case`].
    pub fn name(self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name_with_case(db).folded(db)
    }
}
