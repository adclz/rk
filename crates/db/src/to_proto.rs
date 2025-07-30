use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, Hover, InlayHint, SymbolKind},
};

use crate::hir::{
    expressions::expression::Expr,
    interned::namespace::SpannedNamespaceAccess,
    expressions::spec::{Spec, SpecKind},
    semantic_index::SemanticIndex, 
};

#[derive(bon::Builder, Debug, Clone)]
pub struct SymbolInfo<'a> {
    pub range: Span,
    pub name: String,
    pub name_range: Span,
    pub kind: Option<SymbolKind>,
    pub spec: Option<Spec<'a>>,
    pub init: Option<Expr<'a>>,
    pub implements: Option<Vec<SpannedNamespaceAccess>>,
    pub extends: Option<Extends<'a>>,
}

#[derive(Debug, Clone)]
pub enum Extends<'a> {
    Single(SpannedNamespaceAccess),
    Multiple(&'a Vec<SpannedNamespaceAccess>),
}

impl SymbolInfo<'_> {
    pub fn kind_to_string(&self) -> &'static str {
        match self.kind {
            Some(SymbolKind::VARIABLE) => "var",
            Some(SymbolKind::FUNCTION) => "function",
            Some(SymbolKind::CLASS) => "class",
            Some(SymbolKind::INTERFACE) => "interface",
            Some(SymbolKind::TYPE_PARAMETER) => "type",
            Some(SymbolKind::NAMESPACE) => "namespace",
            Some(SymbolKind::METHOD) => "method",
            _ => "unknown",
        }
    }

    pub fn spec_to_string(&self, db: &dyn BaseDatabase) -> String {
        match &self.spec {
            Some(spec) => match spec.kind {
                SpecKind::Enum(_) => "ENUM".into(),
                SpecKind::StructLike(_) => "STRUCT".into(),
                SpecKind::Edge => "EDGE".into(),
                SpecKind::Bool => "BOOL".into(),
                SpecKind::Byte => "BYTE (8 bits)".into(),
                SpecKind::Word => "WORD (16 bits)".into(),
                SpecKind::DWord => "DWORD (32 bits)".into(),
                SpecKind::LWord => "LWORD (64 bits)".into(),
                SpecKind::SInt => "SINT (8 bits)".into(),
                SpecKind::USInt => "USINT (8 bits)".into(),
                SpecKind::UInt => "UINT (16 bits)".into(),
                SpecKind::Int => "INT (16 bits)".into(),
                SpecKind::DInt => "DINT (32 bits)".into(),
                SpecKind::UDInt => "UDINT (32 bits)".into(),
                SpecKind::LInt => "LINT (64 bits)".into(),
                SpecKind::ULInt => "ULINT (64 bits)".into(),
                SpecKind::Real => "REAL (64 bits)".into(),
                SpecKind::LReal => "LREAL (128 bits)".into(),
                SpecKind::String => "STRING".into(),
                SpecKind::WString => "WSTRING".into(),
                SpecKind::Char => "CHAR".into(),
                SpecKind::WChar => "WCHAR".into(),
                SpecKind::Date => "DATE".into(),
                SpecKind::LDate => "LONG DATE".into(),
                SpecKind::Dt => "DATE AND TIME D".into(),
                SpecKind::Ldt => "LONG DATE AND TIME".into(),
                SpecKind::Time => "TIME".into(),
                SpecKind::LTime => "LONG TIME".into(),
                SpecKind::Tod => "TIME OF DAY".into(),
                SpecKind::LTod => "LONG TIME OF DAY".into(),
                _ => "?".into(),
            },
            None => "?".into(),
        }
    }
}

pub trait ToProto<'db> {
    fn get_span(&'db self, db: &'db dyn crate::BaseDatabase) -> &'db Span;

    fn get_named_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<&'db Span> {
        None
    }

    fn symbol_info(&'db self, _db: &'db dyn crate::BaseDatabase) -> Option<SymbolInfo<'db>> {
        None
    }

    // LSP

    fn completion(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }

    fn inlay_hint(&'db self, _db: &'db dyn crate::BaseDatabase) -> Option<InlayHint> {
        None
    }

    fn hover(&'db self, _db: &'db dyn crate::BaseDatabase) -> Option<Hover> {
        None
    }
}

pub fn self_iter<'db>(s: &'db impl ToProto<'db>) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
    std::iter::once::<&'db dyn ToProto<'db>>(s)
}

pub trait IterToProto<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>>;

    fn descendant_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db, sema) {
            let range = node.get_span(db);
            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = a.get_span(db);
                    if a.start_byte >= range.start_byte {
                        continue;
                    } else {
                        best_match = Some(node);
                    }
                } else {
                    best_match = Some(node);
                }
            }
        }
        best_match
    }

    fn named_descendant_at(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db, sema) {
            let range = match node.get_named_span(db) {
                Some(span) => span,
                None => continue,
            };

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = match a.get_named_span(db) {
                        Some(span) => span,
                        None => continue,
                    };

                    if a.start_byte >= range.start_byte {
                        continue;
                    } else {
                        best_match = Some(node);
                    }
                } else {
                    best_match = Some(node);
                }
            }
        }
        best_match
    }
}
