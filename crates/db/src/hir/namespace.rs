
use auto_lsp::default::db::{BaseDatabase, File};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{hir::{class::Class, data_type::DataType, function::Function, function_block::FunctionBlock, interface::Interface}, ident::Ident, solver::{NamespacePath}};

pub fn to_lsp_range(range: auto_lsp::tree_sitter::Range) -> auto_lsp::lsp_types::Range {
    auto_lsp::lsp_types::Range {
        start: auto_lsp::lsp_types::Position::new(range.start_point.row as u32, range.start_point.column as u32),
        end: auto_lsp::lsp_types::Position::new(range.end_point.row as u32, range.end_point.column as u32),
    }
}

/// Represents a group of namespaces in a file
#[salsa::tracked]
pub struct FileNamespaces<'db> {
    pub file: File,
    #[tracked]
    #[return_ref]
    pub namespaces: FxHashMap<NamespacePath, Namespace<'db>>,
}

#[salsa::tracked]
impl<'db> FileNamespaces<'db> {
    #[salsa::tracked(returns(as_ref))]
    pub fn get_pou(self, db: &'db dyn BaseDatabase, path: NamespacePath, key: Ident) -> Option<PouDecl<'db>> {
        self.namespaces(db).get(&path)?.pous(db).get(&key).copied()
    }
}

/// Represents a view of a namespace
#[salsa::tracked(debug)]
pub struct Namespace<'db> {
    pub internal: bool, 
    #[tracked]
    #[return_ref]
    pub in_scopes: FxHashSet<NamespacePath>,

    #[return_ref]
    pub span: auto_lsp::tree_sitter::Range,

    #[tracked]
    #[return_ref]
    pub pous: FxHashMap<Ident, PouDecl<'db>>,
}


#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[tracked]
    #[return_ref]
    pub pou: Pou<'db>,

    #[return_ref]
    pub span: auto_lsp::tree_sitter::Range,

    #[tracked]
    #[return_ref]
    pub name_span: auto_lsp::tree_sitter::Range,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}