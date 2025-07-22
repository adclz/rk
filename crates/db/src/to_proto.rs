use std::ops::Deref;

use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::{CompletionItem, SymbolKind},
};

use crate::{
    hir::{
        expression::Expr,
        namespace::{Namespace, PouDecl},
        variable::{Spec, SpecKind},
    },
    solver::{fq_name::SpannedPath, namespace::NamespacePath},
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
        match &self.spec {
            Some(spec) => match spec.kind {
                SpecKind::Enum => "ENUM".into(),
                SpecKind::Struct => "STRUCT".into(),
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

#[derive(Clone, Copy)]
pub struct HirCtx<'db> {
    pub db: &'db dyn BaseDatabase,
    pub file: File,
    pub namespace: Option<Namespace<'db>>,
    pub pou: Option<PouDecl<'db>>,
    pub path: Option<NamespacePath>,
    pub path2: Option<&'db SpannedPath>,
}

impl<'db> HirCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, file: File) -> Self {
        Self {
            db,
            file,
            namespace: None,
            pou: None,
            path: None,
            path2: None,
        }
    }

    pub fn with_namespace(mut self, namespace: Namespace<'db>) -> Self {
        self.namespace = Some(namespace);
        self
    }

    pub fn with_pou(mut self, pou: PouDecl<'db>) -> Self {
        self.pou = Some(pou);
        self
    }

    pub fn with_path(mut self, path: NamespacePath) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_path2(mut self, path: &'db SpannedPath) -> Self {
        self.path2 = Some(path);
        self
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

    fn completion_ctx(&'db self, _ctx: HirCtx<'db>, _offset: usize) -> Option<Vec<CompletionItem>> {
        None
    }
}

pub fn self_iter<'db>(
    s: &'db impl ToProto<'db>,
    ctx: HirCtx<'db>,
) -> impl Iterator<Item = ProtoAndCtx<'db>> {
    std::iter::once::<ProtoAndCtx<'db>>((ctx, s))
}

pub type ProtoAndCtx<'db> = (HirCtx<'db>, &'db dyn ToProto<'db>);

pub trait IterToProto<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>>;

    fn descendant_at(&'db self, ctx: HirCtx<'db>, offset: usize) -> Option<ProtoAndCtx<'db>> {
        let mut best_match: Option<ProtoAndCtx<'db>> = None;

        for node in self.iter(ctx) {
            let range = node.1.get_span(ctx.db).clone();

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = a.1.get_span(ctx.db);

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

    fn named_descendant_at(&'db self, ctx: HirCtx<'db>, offset: usize) -> Option<ProtoAndCtx<'db>> {
        let mut best_match: Option<ProtoAndCtx<'db>> = None;

        for node in self.iter(ctx) {
            let range = match node.1.get_named_span(ctx.db) {
                Some(span) => span,
                None => continue,
            };

            // Only consider nodes that contain the offset
            if range.start_byte <= offset && offset <= range.end_byte {
                // Compare old best match with new node
                if let Some(a) = best_match {
                    let a = match a.1.get_named_span(ctx.db) {
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
