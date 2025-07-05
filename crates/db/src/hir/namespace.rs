use auto_enums::auto_enum;
use auto_lsp::default::db::{BaseDatabase, File};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{diagnostics::diagnostic_builder::RangeKind, hir::{class::Class, data_type::DataType, function::Function, function_block::FunctionBlock, interface::Interface}, ident::Ident, solver::namespace::NamespacePath, to_proto::{Extends, IterToProto, SymbolInfo, ToProto}};

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
        self.namespaces(db).get(&path)?.pous(db).iter().find_map(|pou| {
            if pou.name(db) == &key {
                Some(*pou)
            } else {
                None
            }
        })
    }
}

impl<'db> IterToProto<'db> for FileNamespaces<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.namespaces(db).iter()
        .map(|(_, ns)| ns.iter(db))
        .flatten()
    }
}

impl<'db> FileNamespaces<'db> {
    pub fn namespace_at(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Namespace<'db>> {
        self.namespaces(db).iter().find_map(|(path, ns)| {
            if ns.spanned(db).start_byte <= offset && offset <= ns.spanned(db).end_byte {
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
    pub pous: Vec<PouDecl<'db>>,
}

#[salsa::tracked]
impl<'db> Namespace<'db> {
    pub fn pou_at(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<PouDecl<'db>> {
        self.pous(db).iter().find_map(|pou| {
            if pou.spanned(db).start_byte <= offset && offset <= pou.spanned(db).end_byte {
                Some(*pou)
            } else {
                None
            }
        })
    }

    #[salsa::tracked(returns(as_ref))]
    pub fn get_pou(self, db: &'db dyn BaseDatabase, key: Ident) -> Option<PouDecl<'db>> {
        self.pous(db).iter().find_map(|pou| {
            if pou.name(db) == &key {
                Some(*pou)
            } else {
                None
            }
        })
    }

}

impl<'db> ToProto<'db> for Namespace<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.span(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.name_span(db).into()
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> SymbolInfo<'db> {
        SymbolInfo::builder()
        .kind(auto_lsp::lsp_types::SymbolKind::NAMESPACE)
        .name(self.path(db).path(db).text(db))
        .range(self.spanned(db).into())
        .name_range(self.name_span(db).into())
        .build()
    }
}

impl<'db> IterToProto<'db> for Namespace<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::once::<&'db dyn ToProto<'db>>(self)
            .chain(self.pous(db).iter().flat_map(|pou| pou.iter(db)))
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
    #[auto_enum(Iterator)]
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.pou(db) {
            Pou::Function(f) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(f.iter(db)),
            Pou::FunctionBlock(fb) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(fb.iter(db)),
            Pou::Class(c) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(c.iter(db)),
            Pou::DataType(d) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(d.iter(db)),
            Pou::Interface(i) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(i.iter(db)),
        }
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.span(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.name_span(db).into()
    }

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
        .range(self.spanned(db).into())
        .maybe_spec(match self.pou(db) {
            Pou::DataType(d) => Some(d.spec(db)),
            _ => None,
        })
        .maybe_init(match self.pou(db) {
            Pou::DataType(d) => d.init(db),
            _ => None,
        })
        .name_range(self.name_span(db).into())
        .maybe_extends(match self.pou(db) {
            Pou::Class(c) => c.extends(db).map(|a| Extends::Single(a)),
            Pou::FunctionBlock(fb) => fb.extends(db).map(|a| Extends::Single(a)),
            Pou::Interface(i) => i.extends(db).map(|a| Extends::Multiple(a)),
            _ => None,
        })
        .maybe_implements(match self.pou(db) {
            Pou::Class(c) => c.implements(db),
            Pou::FunctionBlock(fb) => fb.implements(db),
            _ => None,
        })
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