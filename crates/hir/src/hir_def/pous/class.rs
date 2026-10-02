// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    AstId, HasModifiers, HasName, HasPragmas, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::{pragma::Pragma, variable::VariableDecl},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    /// The name as the author wrote it. Matching reads `name(db)`.
    pub name_with_case: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<Spec<'db>>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<Spec<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub methods: Vec<MethodDecl<'db>>,

    #[tracked]
    pub modifier: Modifier,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Class<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for Class<'db> {
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

impl<'db> HasModifiers<'db> for Class<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        self.modifier(db)
    }
}

#[salsa::tracked(debug)]
pub struct MethodDecl<'db> {
    /// The name as the author wrote it. Matching reads `name(db)`.
    pub name_with_case: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[tracked]
    pub modifier: Modifier,

    #[tracked]
    pub visibility: Visibility,

    #[tracked]
    pub _override: bool,

    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub stmts: Vec<Stmt<'db>>,

    #[tracked]
    #[returns(ref)]
    pub pragmas: Vec<Pragma<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for MethodDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasPragmas<'db> for MethodDecl<'db> {
    fn get_pragmas(&self, db: &'db dyn WorkspaceDataBase) -> &'db [Pragma<'db>] {
        self.pragmas(db)
    }
}

impl<'db> HasName<'db> for MethodDecl<'db> {
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

impl<'db> Class<'db> {
    /// The name, as names are matched: case folded. As written:
    /// [`Self::name_with_case`].
    pub fn name(self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name_with_case(db).folded(db)
    }
}

impl<'db> MethodDecl<'db> {
    /// The name, as names are matched: case folded. As written:
    /// [`Self::name_with_case`].
    pub fn name(self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name_with_case(db).folded(db)
    }
}
