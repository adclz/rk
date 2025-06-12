use std::{sync::Arc};

use auto_lsp::default::db::File;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{hir::{class::Class, data_type::DataType, function::Function, function_block::FunctionBlock, interface::Interface}, parser::namespace::FileNamespacesBuilder, solver::{Ident, NamespacePath}};

/// Represents a group of namespaces in a file
#[derive(Clone, salsa::Update)]
pub struct FileNamespaces<'db> {
    pub namespaces: Arc<FxHashMap<NamespacePath, Namespace<'db>>>,
    pub file: File,
}

impl<'db> FileNamespaces<'db> {
    pub fn new(builder: FileNamespacesBuilder<'db>) -> Self {
        Self {
            namespaces: Arc::new(builder.paths),
            file: builder.file,
        }
    }
}

/// Represents a view of a namespace
#[derive(Default, Clone, PartialEq, Eq, salsa::Update)]
pub struct Namespace<'db> {
    pub internal: bool, 
    pub in_scopes: FxHashSet<NamespacePath>,
    pub functions: FxHashMap<Ident, Function<'db>>,
    pub function_blocks: FxHashMap<Ident, FunctionBlock<'db>>,
    pub classes: FxHashMap<Ident, Class<'db>>,
    pub interfaces: FxHashMap<Ident, Interface<'db>>,
    pub data_types: FxHashMap<Ident, DataType<'db>>,
}

impl<'db> Namespace<'db> {
    pub fn new(namespace: &ast::generated::NamespaceDecl) -> Self {
        Self {
            internal: namespace.internal.is_some(),
            ..Default::default()
        }
    }
}