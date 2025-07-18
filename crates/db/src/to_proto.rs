use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, SymbolKind},
};

use crate::{
    hir::{expression::Expr, variable::Spec},
    solver::fq_name::SpannedPath,
};

#[derive(bon::Builder, Debug, Clone)]
pub struct SymbolInfo<'a> {
    pub range: Span,
    pub name: String,
    pub name_range: Span,
    pub kind: Option<SymbolKind>,
    pub spec: Option<Spec<'a>>,
    pub init: Option<Expr<'a>>,
    pub implements: Option<Vec<SpannedPath>>,
    pub extends: Option<Extends<'a>>,
}

#[derive(Debug, Clone)]
pub enum Extends<'a> {
    Single(SpannedPath),
    Multiple(&'a Vec<SpannedPath>),
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
        match self.spec {
            Some(Spec::Target(target)) => target.to_string(db),
            Some(Spec::Array(_)) => "ARRAY".into(),
            Some(Spec::Subrange(_)) => "SUBRANGE".into(),
            Some(Spec::Enum) => "ENUM".into(),
            Some(Spec::Struct) => "STRUCT".into(),
            Some(Spec::Edge) => "EDGE".into(),
            Some(Spec::Bool) => "BOOL".into(),
            Some(Spec::Byte) => "BYTE (8 bits)".into(),
            Some(Spec::Word) => "WORD (16 bits)".into(),
            Some(Spec::DWord) => "DWORD (32 bits)".into(),
            Some(Spec::LWord) => "LWORD (64 bits)".into(),
            Some(Spec::SInt) => "SINT (8 bits)".into(),
            Some(Spec::USInt) => "USINT (8 bits)".into(),
            Some(Spec::UInt) => "UINT (16 bits)".into(),
            Some(Spec::Int) => "INT (16 bits)".into(),
            Some(Spec::DInt) => "DINT (32 bits)".into(),
            Some(Spec::UDInt) => "UDINT (32 bits)".into(),
            Some(Spec::LInt) => "LINT (64 bits)".into(),
            Some(Spec::ULInt) => "ULINT (64 bits)".into(),
            Some(Spec::Real) => "REAL (64 bits)".into(),
            Some(Spec::LReal) => "LREAL (128 bits)".into(),
            Some(Spec::String) => "STRING".into(),
            Some(Spec::WString) => "WSTRING".into(),
            Some(Spec::Char) => "CHAR".into(),
            Some(Spec::WChar) => "WCHAR".into(),
            Some(Spec::Date) => "DATE".into(),
            Some(Spec::LDate) => "LONG DATE".into(),
            Some(Spec::Dt) => "DATE AND TIME D".into(),
            Some(Spec::Ldt) => "LONG DATE AND TIME".into(),
            Some(Spec::Time) => "TIME".into(),
            Some(Spec::LTime) => "LONG TIME".into(),
            Some(Spec::Tod) => "TIME OF DAY".into(),
            Some(Spec::LTod) => "LONG TIME OF DAY".into(),
            _ => "unknown".into(),
        }
    }
}

pub trait ToProto<'db> {
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> usize;
    fn spanned(&'db self, db: &'db dyn crate::BaseDatabase) -> &'db Span;
    fn named_span(&'db self, db: &'db dyn crate::BaseDatabase) -> &'db Span;
    fn symbol_info(&'db self, _db: &'db dyn crate::BaseDatabase) -> Option<SymbolInfo<'db>> {
        None
    }
    fn completion_ctx(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        None
    }
}

pub fn self_iter<'db>(
    s: &'db impl ToProto<'db>,
) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
    std::iter::once::<&'db dyn ToProto<'db>>(s)
}

pub trait IterToProto<'db> {
    fn iter(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>>;

    fn descendant_at(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db) {
            let range = node.spanned(db);

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    if a.get_id(db) >= node.get_id(db) {
                        continue; // Keep the old best match
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
        db: &'db dyn crate::BaseDatabase,
        offset: usize,
    ) -> Option<&'db dyn ToProto<'db>> {
        let mut best_match: Option<&'db dyn ToProto<'db>> = None;

        for node in self.iter(db) {
            let range = node.named_span(db);

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    if a.get_id(db) >= node.get_id(db) {
                        continue; // Keep the old best match
                    }
                } else {
                    best_match = Some(node);
                }
            }
        }
        best_match
    }
}
