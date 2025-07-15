use auto_enums::auto_enum;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::{file::File, BaseDatabase};
use auto_lsp::lsp_types::CompletionItem;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::completions;
use crate::hir::COMPLETION_MARKER;
use crate::solver::namespace::{starts, starts_with};
use crate::{
    hir::{
        class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
        interface::Interface,
    },
    ident::Ident,
    solver::namespace::NamespacePath,
    to_proto::{Extends, IterToProto, SymbolInfo, ToProto},
};

/// Represents a group of namespaces in a file
#[salsa::tracked]
pub struct FileNamespaces<'db> {
    pub file: File,
    
    #[tracked]
    #[returns(ref)]
    pub globals: Vec<PouDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub namespaces: FxHashMap<NamespacePath, Namespace<'db>>,
}

#[derive(Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum NamespaceResult<'db> {
    NotFound,
    Hidden(Namespace<'db>),
    Found(Namespace<'db>),
}

#[derive(Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PouResult<'db> {
    NotFound,
    Hidden((Namespace<'db>, PouDecl<'db>)),
    Found(PouDecl<'db>)
    
}

#[salsa::tracked]
impl<'db> FileNamespaces<'db> {
    #[salsa::tracked]
    pub fn get_namespace(
        self,
        db: &'db dyn BaseDatabase,
        from: NamespacePath,
        to: NamespacePath,
    ) -> NamespaceResult<'db> {
        match self.namespaces(db).get(&to) {
            None => NamespaceResult::NotFound,
            Some(ns) => {
                if ns.internal(db) && from != to {
                    NamespaceResult::Hidden(*ns)
                } else {
                    NamespaceResult::Found(*ns)
                }
            }
        }
    }

    #[salsa::tracked]
    pub fn get_pou(
        self,
        db: &'db dyn BaseDatabase,
        from: NamespacePath,
        to: NamespacePath,
        key: Ident,
    ) -> PouResult<'db> {
        match self.get_namespace(db, from, to) {
            NamespaceResult::NotFound => PouResult::NotFound,
            NamespaceResult::Hidden(ns) => {
                match ns.get_pou(db, key) {
                    None => PouResult::NotFound,
                    Some(pou) => PouResult::Hidden((ns, *pou)),
                }
            }
            NamespaceResult::Found(ns) => {
                match ns.get_pou(db, key) {
                    None => PouResult::NotFound,
                    Some(pou) => PouResult::Found(*pou),
                }
            }
        }
    }
}

impl<'db> IterToProto<'db> for FileNamespaces<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.globals(db)
            .iter()
            .flat_map(|pou| pou.iter(db))
            .chain(self.namespaces(db).values().flat_map(|ns| ns.iter(db)))
    }
}

impl<'db> FileNamespaces<'db> {
    pub fn namespace_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Namespace<'db>> {
        self.namespaces(db).iter().find_map(|(_path, ns)| {
            if ns.spanned(db).start_byte <= offset && offset <= ns.spanned(db).end_byte {
                Some(*ns)
            } else {
                None
            }
        })
    }
}

/// Represents a view of a namespace
#[salsa::tracked]
pub struct Namespace<'db> {
    pub internal: bool,

    // from the standard: "A USING namespace directive enables the types contained in the given namespace, 
    // but specifically does not enable types contained in nested namespaces."

    // TLDR: Using directives are not recursive
    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[tracked]
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub name_span: Span,

    #[tracked]
    #[returns(ref)]
    pub pous: Vec<PouDecl<'db>>,
}

#[salsa::tracked(debug)]
pub struct Using<'db> {
    pub path: NamespacePath,
    #[returns(ref)]
    pub span: Span,
}

impl<'db> ToProto<'db> for Using<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn completion_ctx(
        &'db self,
        db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let fragments = self.path(db).fragments(db);
        let mut marker_index = None;

        for (i, fragment) in fragments.iter().enumerate() {
            if fragment.text(db).contains(COMPLETION_MARKER) {
                marker_index = Some(i);
                break;
            }
        }

        let marker_index = marker_index?;

        let mut seen = FxHashSet::default();

        // Case 1: marker is in the first fragment -> we can only prefix-match from root
        if marker_index == 0 {
            let prefix = fragments[0].text(db).replace(COMPLETION_MARKER, "");
            return Some(
                starts_with(db, Ident::new(db, prefix))
                    .iter()
                    .filter_map(|ns| ns.path(db).fragments(db).get(0))
                    .filter(|ident| seen.insert(*ident))
                    .map(|ident| CompletionItem::new_simple(ident.text(db), ident.text(db)))
                    .collect(),
            );
        }

        // Case 2: marker is in a deeper fragment -> walk through layers with exact match
        let mut matching = starts(db, fragments[0]).to_vec();

        for i in 1..marker_index {
            matching = matching
                .into_iter()
                .filter(|ns| {
                    ns.path(db)
                        .fragments(db)
                        .get(i)
                        .map_or(false, |frag| frag == &fragments[i])
                })
                .collect();
        }

        // Now suggest completions for the fragment at `marker_index`
        let completions = matching
            .iter()
            .filter_map(|ns| ns.path(db).fragments(db).get(marker_index))
            .filter(|ident| seen.insert(*ident))
            .map(|ident| CompletionItem::new_simple(ident.text(db), ident.text(db)))
            .collect();

        Some(completions)
    }
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
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.name_span(db)
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(SymbolInfo::builder()
            .kind(auto_lsp::lsp_types::SymbolKind::NAMESPACE)
            .name(self.path(db).to_string(db))
            .range(self.spanned(db).clone())
            .name_range(self.name_span(db).clone())
            .build())
    }

    fn completion_ctx(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        // Don't provide completions between the namespace keyword and the namespace name
        if self.name_span(db).end_byte > offset {
            if !self.internal(db) {
                return Some(vec![CompletionItem::new_simple(
                    "INTERNAL".into(),
                    "internal".into(),
                )]);
            } else {
                return None;
            }
        }

        let mut completions = vec![
            completions::snippets::namespace(),
            completions::snippets::function(),
            completions::snippets::function_block(),
            completions::snippets::type_(),
            completions::snippets::class(),
            completions::snippets::interface(),
        ];
        // Using directives can only be added before any POU declarations
        if let Some(pou) = self.pous(db).first() {
            if pou.spanned(db).end_byte >= offset {
                completions.push(completions::snippets::using());
            }
        } else {
            completions.push(completions::snippets::using());
        }
        Some(completions)
    }
}

impl<'db> IterToProto<'db> for Namespace<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::once::<&'db dyn ToProto<'db>>(self)
            .chain(self.using(db).iter().map(|using| using as _))
            .chain(self.pous(db).iter().flat_map(|pou| pou.iter(db)))
    }
}

#[salsa::tracked]
pub struct PouDecl<'db> {
    pub file: File,
    
    #[tracked]
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub name_span: Span,
}

impl<'db> IterToProto<'db> for PouDecl<'db> {
    #[auto_enum(Iterator)]
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.pou(db) {
            Pou::Function(f) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(f.iter(db)),
            Pou::FunctionBlock(fb) => {
                std::iter::once::<&'db dyn ToProto<'db>>(self).chain(fb.iter(db))
            }
            Pou::Class(c) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(c.iter(db)),
            Pou::DataType(d) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(d.iter(db)),
            Pou::Interface(i) => std::iter::once::<&'db dyn ToProto<'db>>(self).chain(i.iter(db)),
        }
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.name_span(db).into()
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(SymbolInfo::builder()
            .kind(match self.pou(db) {
                Pou::Function(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
                Pou::FunctionBlock(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
                Pou::Class(_) => auto_lsp::lsp_types::SymbolKind::CLASS,
                Pou::Interface(_) => auto_lsp::lsp_types::SymbolKind::INTERFACE,
                Pou::DataType(_) => auto_lsp::lsp_types::SymbolKind::TYPE_PARAMETER,
            })
            .name(self.name(db).text(db))
            .range(self.spanned(db).clone())
            .maybe_spec(match self.pou(db) {
                Pou::DataType(d) => Some(d.spec(db)),
                _ => None,
            })
            .maybe_init(match self.pou(db) {
                Pou::DataType(d) => d.init(db),
                _ => None,
            })
            .name_range(self.name_span(db).clone())
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
            .build())
    }

    fn completion_ctx(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.pou(db) {
            Pou::Function(f) => f.completion_ctx(db, offset),
            Pou::FunctionBlock(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Class(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Interface(_) => Some(vec![completions::snippets::var_input()]),
            Pou::DataType(_) => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>), 
}
