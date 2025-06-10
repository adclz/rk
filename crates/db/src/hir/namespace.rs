use std::{collections::HashMap, ops::Deref, sync::Arc};

use ast::generated::NamespaceDecl;

use crate::solver::{Ident, NamespacePath};

/// Represents a group of namespaces in a file
#[derive(Default, Clone, Debug, PartialEq, salsa::Update)]
pub struct FileNamespaces(pub(crate) Arc<HashMap<NamespacePath, Namespace>>);

impl Deref for FileNamespaces {
    type Target = HashMap<NamespacePath, Namespace>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Clone, Debug, PartialEq, salsa::Update)]
pub struct Namespace {
    pub internal: bool, 
    pub scopes: HashMap<Ident, Scope>,
}

impl Namespace {
    pub fn new(namespace: &NamespaceDecl, scopes: HashMap<Ident, Scope>) -> Self {
        Self {
            internal: namespace.internal.is_some(),
            scopes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Class,
    DataType,
    Fb,
    Func,
    Interface,
}