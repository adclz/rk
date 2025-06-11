use std::{collections::HashMap, sync::Arc};

use ast::generated::NamespaceDecl;
use auto_lsp::default::db::File;

use crate::{builder::namespace::FileNamespacesBuilder, solver::{Ident, NamespacePath}};

/// Represents a group of namespaces in a file
#[derive(Clone, salsa::Update)]
pub struct FileNamespaces {
    pub namespaces: Arc<HashMap<NamespacePath, Namespace>>,
    pub file: File,
}

impl FileNamespaces {
    pub fn new(builder: FileNamespacesBuilder) -> Self {
        Self {
            namespaces: Arc::new(builder.paths),
            file: builder.file,
        }
    }
}

impl PartialEq for FileNamespaces {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.namespaces, &other.namespaces)
    }
}

/// Represents a view of a namespace
#[derive(Default, PartialEq, salsa::Update)]
pub struct Namespace {
    pub internal: bool, 
    pub in_scopes: Vec<NamespacePath>,
    pub functions: HashMap<Ident, Function>,
    pub classes: HashMap<Ident, Class>,
    pub interfaces: HashMap<Ident, Interface>,
    pub data_types: HashMap<Ident, DataType>,
    pub function_blocks: HashMap<Ident, FunctionBlock>,
}

impl Namespace {
    pub fn new(namespace: &NamespaceDecl) -> Self {
        Self {
            in_scopes: vec![],
            internal: namespace.internal.is_some(),
            functions: HashMap::default(),
            classes: HashMap::default(),
            interfaces: HashMap::default(),
            data_types: HashMap::default(),
            function_blocks: HashMap::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Function {}
#[derive(Debug, PartialEq)]
pub struct Class {}
#[derive(Debug, PartialEq)]
pub struct Interface {}
#[derive(Debug, PartialEq)]
pub struct DataType {}
#[derive(Debug, PartialEq)]
pub struct FunctionBlock {}