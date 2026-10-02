// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::{
    AstId, HasModifiers, HasName, HasVisibility, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::spec::Spec,
        interned::identifier::Ident,
        pous::{class::MethodDecl, interface::MethodPrototype, variable::VariableDecl},
        scope::ScopeId,
    },
};

/// A method as a POU holds it: one it declares with a body, or an
/// INTERFACE's prototype, which a POU implementing the interface inherits
/// until it declares the method itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum MethodRef<'db> {
    Prototype(MethodPrototype<'db>),
    Declared(MethodDecl<'db>),
}

impl<'db> HasModifiers<'db> for MethodRef<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            MethodRef::Prototype(_) => Modifier::EMPTY,
            MethodRef::Declared(d) => d.modifier(db),
        }
    }
}

impl<'db> HasVisibility<'db> for MethodRef<'db> {
    fn get_visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility {
        match self {
            MethodRef::Prototype(_) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }
}

impl<'db> MethodRef<'db> {
    pub fn return_type(&self, db: &'db dyn WorkspaceDataBase) -> Option<&'db Spec<'db>> {
        match self {
            MethodRef::Prototype(p) => p.return_type(db),
            MethodRef::Declared(d) => d.return_type(db),
        }
    }

    pub fn variables(&self, db: &'db dyn WorkspaceDataBase) -> &'db Vec<VariableDecl<'db>> {
        match self {
            MethodRef::Prototype(p) => p.variables(db),
            MethodRef::Declared(d) => d.variables(db),
        }
    }

    pub fn is_prototype(&self) -> bool {
        matches!(self, MethodRef::Prototype(_))
    }

    pub fn is_declared(&self) -> bool {
        matches!(self, MethodRef::Declared(_))
    }
}

impl<'db> HirNodeInfo<'db> for MethodRef<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_id(db),
            MethodRef::Declared(d) => d.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            MethodRef::Prototype(p) => p.get_scope_id(db),
            MethodRef::Declared(d) => d.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for MethodRef<'db> {
    fn get_name_with_case(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> crate::hir_def::interned::identifier::Ident {
        match self {
            MethodRef::Prototype(p) => p.get_name_with_case(db),
            MethodRef::Declared(d) => d.get_name_with_case(db),
        }
    }

    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            MethodRef::Prototype(p) => p.get_name_ident(db),
            MethodRef::Declared(d) => d.get_name_ident(db),
        }
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_name_id(db),
            MethodRef::Declared(d) => d.get_name_id(db),
        }
    }
}

impl<'db> From<MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(value)
    }
}

impl<'db> From<MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: MethodDecl<'db>) -> Self {
        MethodRef::Declared(value)
    }
}

impl<'db> From<&MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: &MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(*value)
    }
}

impl<'db> From<&MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: &MethodDecl<'db>) -> Self {
        MethodRef::Declared(*value)
    }
}
