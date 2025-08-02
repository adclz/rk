use auto_lsp::core::span::Span;
use auto_lsp::default::db::{file::File, BaseDatabase};
use auto_lsp::lsp_types::{
    CompletionItem, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind,
};

use crate::completions;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::scopes::scope::{PouId, ScopeId, Visibility};
use crate::hir::semantic_index::{semantic_index, SemanticIndex};
use crate::to_proto::{self_iter, IterToProto, SymbolInfo, ToProto};

#[salsa::tracked(debug)]
pub struct Namespace<'db> {
    #[tracked]
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub name_span: Span,

    #[tracked]
    #[returns(ref)]
    pub pous: Vec<PouId>,

    pub file: File,

    pub scope_id: ScopeId,
}

impl<'db> ToProto<'db> for Namespace<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn get_named_span(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::NAMESPACE)
                .name(self.path(db).to_string(db))
                .range(self.get_span(db).clone())
                .name_range(self.name_span(db).clone())
                .build(),
        )
    }

    fn hover(&'db self, db: &'db dyn crate::BaseDatabase, _sema: &'db SemanticIndex<'db>,) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Namespace `{}`", self.path(db).to_string(db)),
            }),
            range: Some(self.get_span(db).lsp()),
        })
    }

    fn inlay_hint(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>
    ) -> Option<auto_lsp::lsp_types::InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!("namespace {}", self.path(db).to_string(db))),
            position: self.span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let scope = sema.get_scope(self.scope_id(db));
        
        // Don't provide completions between the namespace keyword and the namespace name
        if self.name_span(db).end_byte > offset {
            if !scope.visibility == Visibility::PUBLIC {
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
            let pou = sema.get_pou(*pou);
            if pou.get_span(db).end_byte >= offset {
                completions.push(completions::snippets::using());
            }
        } else {
            completions.push(completions::snippets::using());
        }
        Some(completions)
    }
}

impl<'db> IterToProto<'db> for Namespace<'db> {
    fn iter(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        let scope = sema.get_scope(self.scope_id(db));

        self_iter(self)
            .chain(scope.usings.iter().map(move |using| using as _))
            .chain(
                self.sorted_pous(db)
                    .iter()
                    .map(move |pou| sema.get_pou(*pou).iter(db, sema))
                    .flatten(),
            )
    }
}

/// Pous need to be sorted by their order of appearance in the namespace.
///
/// Since FxHashMap does not guarantee order, we need to sort them explicitly.
///
/// This is only used when iterating at the namespace level.
#[salsa::tracked]
impl<'db> Namespace<'db> {
    #[salsa::tracked(returns(ref))]
    fn sorted_pous(self, db: &'db dyn BaseDatabase) -> Vec<PouId> {
        let mut sorted_pous: Vec<_> = self.pous(db).iter().cloned().collect();
        sorted_pous.sort();
        sorted_pous
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types};

    use super::*;
    use crate::{hir::pous::pou::Pou, RootDatabase};

    #[test]
    fn global_scope() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
    FUNCTION fn1
    END_FUNCTION    

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let sema = semantic_index(&db, file);

        sema.pou_keys
            .iter()
            .for_each(|(key, pou)| match pou.pou(&db) {
                Pou::Function(f) => {
                    assert_eq!(pou.name(&db).text(&db), "fn1");
                    assert_eq!(f.scope_id(&db), ScopeId::global());
                }
                Pou::FunctionBlock(fb) => {
                    assert_eq!(pou.name(&db).text(&db), "fn2");
                    assert_eq!(fb.scope_id(&db), ScopeId::global());
                }
                Pou::Class(c) => {
                    assert_eq!(pou.name(&db).text(&db), "cl");
                    assert_eq!(c.scope_id(&db), ScopeId::global());
                }
                Pou::Interface(i) => {
                    assert_eq!(pou.name(&db).text(&db), "in");
                    assert_eq!(i.scope_id(&db), ScopeId::global());
                }
                Pou::DataType(_) => {}
            });
    }

    #[test]
    fn scoped_pous() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns

    FUNCTION fn1
    END_FUNCTION

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE

END_NAMESPACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let sema = semantic_index(&db, file);

        let main_ns = sema.namespace_keys.values().next().unwrap();
        let scope = sema.get_scope(main_ns.scope_id(&db));

        for pou in sema.pou_keys.values() {
            match pou.pou(&db) {
                Pou::Function(f) => {
                    assert_eq!(pou.name(&db).text(&db), "fn1");
                    assert_eq!(f.scope_id(&db), scope.id);
                }
                Pou::FunctionBlock(fb) => {
                    assert_eq!(pou.name(&db).text(&db), "fn2");
                    assert_eq!(fb.scope_id(&db), scope.id);
                }
                Pou::Class(c) => {
                    assert_eq!(pou.name(&db).text(&db), "cl");
                    assert_eq!(c.scope_id(&db), scope.id);
                }
                Pou::Interface(i) => {
                    assert_eq!(pou.name(&db).text(&db), "in");
                    assert_eq!(i.scope_id(&db), scope.id);
                }
                Pou::DataType(_) => {}
            }
        }
    }

    #[test]
    fn scoped_nested_pous() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns

    NAMESPACE nss2
        FUNCTION fn1
        END_FUNCTION

        FUNCTION_BLOCK fn2
        END_FUNCTION_BLOCK

        CLASS cl
        END_CLASS

        INTERFACE in
        END_INTERFACE

    END_NAMESPACE

END_NAMESPACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let sema = semantic_index(&db, file);

        let nested_ns = sema
            .namespace_keys
            .iter()
            .find(|(k, n)| n.path(&db).to_string(&db) == "ns.nss2")
            .unwrap()
            .1;

        let scope = sema.get_scope(nested_ns.scope_id(&db));

        for pou in sema.pou_keys.values() {
            match pou.pou(&db) {
                Pou::Function(f) => {
                    assert_eq!(pou.name(&db).text(&db), "fn1");
                    assert_eq!(f.scope_id(&db), scope.id);
                }
                Pou::FunctionBlock(fb) => {
                    assert_eq!(pou.name(&db).text(&db), "fn2");
                    assert_eq!(fb.scope_id(&db), scope.id);
                }
                Pou::Class(c) => {
                    assert_eq!(pou.name(&db).text(&db), "cl");
                    assert_eq!(c.scope_id(&db), scope.id);
                }
                Pou::Interface(i) => {
                    assert_eq!(pou.name(&db).text(&db), "in");
                    assert_eq!(i.scope_id(&db), scope.id);
                }
                Pou::DataType(_) => {}
            }
        }
    }

    #[test]
    fn using_directives() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns
    USING ns2   
    USING ns3.nss
    FUNCTION fn1
    END_FUNCTION    
END_NAMESPACE
"#;
        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();
        let file = db.get_file(&url).unwrap();
        let sema = semantic_index(&db, file);

        let main_ns = sema.namespace_keys.values().next().unwrap();
        let scope = sema.get_scope(main_ns.scope_id(&db));

        assert_eq!(scope.usings.len(), 2);
    }
}
