use auto_lsp::default::db::{BaseDatabase, File};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{hir::{class::Class, data_type::DataType, function::Function, function_block::FunctionBlock, interface::Interface}, ident::Ident, solver::NamespacePath, to_proto::{IterToProto, SymbolInfo, ToProto}};

/// Represents a group of namespaces in a file
#[salsa::tracked]
pub struct FileNamespaces<'db> {
    pub file: File,
    #[tracked]
    #[returns(ref)]
    pub namespaces: FxHashMap<NamespacePath, Namespace<'db>>,
}

#[salsa::tracked]
impl<'db> FileNamespaces<'db> {
    #[salsa::tracked(returns(as_ref))]
    pub fn get_pou(self, db: &'db dyn BaseDatabase, path: NamespacePath, key: Ident) -> Option<PouDecl<'db>> {
        self.namespaces(db).get(&path)?.pous(db).get(&key).copied()
    }
}

impl<'db> IterToProto<'db> for FileNamespaces<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        self.namespaces(db).iter()
        .map(|(_, ns)| ns.iter(db))
        .flatten()
    }
}

impl<'db> FileNamespaces<'db> {
    pub fn namespace_at(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Namespace<'db>> {
        self.namespaces(db).iter().find_map(|(path, ns)| {
            if ns.span(db).start_byte <= offset && offset <= ns.span(db).end_byte {
                Some(*ns)
            } else {
                None
            }
        })
    }
}

/// Represents a view of a namespace
#[salsa::tracked(debug)]
pub struct Namespace<'db> {
    pub internal: bool, 
    #[tracked]
    #[returns(ref)]
    pub in_scopes: FxHashSet<NamespacePath>,

    #[tracked]
    #[returns(ref)]
    pub span: auto_lsp::tree_sitter::Range,

    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub name_span: auto_lsp::tree_sitter::Range,

    #[tracked]
    #[returns(ref)]
    pub pous: FxHashMap<Ident, PouDecl<'db>>,
}

impl<'db> Namespace<'db> {
    pub fn pou_at(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<PouDecl<'db>> {
        self.pous(db).iter().find_map(|(name, pou)| {
            if pou.span(db).start_byte <= offset && offset <= pou.span(db).end_byte {
                Some(*pou)
            } else {
                None
            }
        })
    }
}

impl<'db> IterToProto<'db> for Namespace<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        let namespace_symbol = SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::NAMESPACE)
                .range(self.span(db).into())
                .name(self.path(db).display(db))
                .name_range(self.name_span(db).into())
                .build();

        std::iter::once(namespace_symbol)
            .chain(self.pous(db).iter().flat_map(|(_, pou)| pou.iter(db)))
    }
}


#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub span: auto_lsp::tree_sitter::Range,

    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub name_span: auto_lsp::tree_sitter::Range,
}

impl<'db> IterToProto<'db> for PouDecl<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        std::iter::once(self.symbol_info(db))
        .chain(match self.pou(db) {
            Pou::Function(f) => f.iter(db),
            _ => todo!(),
        })
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> SymbolInfo<'db> {
        SymbolInfo::builder()
        .kind(match self.pou(db) {
            Pou::Function(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
            Pou::FunctionBlock(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
            Pou::Class(_) => auto_lsp::lsp_types::SymbolKind::CLASS,
            Pou::Interface(_) => auto_lsp::lsp_types::SymbolKind::INTERFACE,
            Pou::DataType(_) => auto_lsp::lsp_types::SymbolKind::TYPE_PARAMETER,
        })
        .name(self.name(db).text(db))
        .range(self.span(db).into())
        .name_range(self.name_span(db).into())
        .build()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}