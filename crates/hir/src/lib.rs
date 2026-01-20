#![recursion_limit = "256"]
#![allow(unused_variables)]

use std::ops::Deref;

use auto_lsp::{
    core::{ast::AstNode, document::Encoding, span::Span},
    lsp_types,
};
use bitflags::bitflags;
use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::{hir_def::{
    expressions::{
        expression::{Expr, InitExpr, VariableAccess},
        spec::{Spec, StructElement},
    },
    interned::{identifier::Ident, namespace::SpanNamespaceAccess},
    pous::{class::MethodDecl, pou::Pou, variable::VariableDecl},
    scope::ScopeId,
    semantic_index::semantic_index,
}, hir_ty::inheritance_solver::MethodRef};

pub mod builder;
pub mod check;
pub mod hir_def;
pub mod hir_ty;
pub mod query_string;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AstId(pub(crate) usize);

impl<T: AstNode> From<&T> for AstId {
    fn from(node: &T) -> Self {
        AstId(node.get_id())
    }
}

impl AstId {
    pub fn id(&self) -> usize {
        self.0
    }
}

impl Deref for AstId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallSite<'db> {
    pub scope: ScopeId<'db>,
    pub id: AstId,
}

impl<'db> CallSite<'db> {
    pub fn new(scope: ScopeId<'db>, id: AstId) -> Self {
        Self { scope, id }
    }

    pub fn from_expr(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> Self {
        Self {
            scope: expr.get_scope_id(db),
            id: expr.get_id(db),
        }
    }

    pub fn from_init_expr(db: &'db dyn WorkspaceDataBase, expr: InitExpr<'db>) -> Self {
        Self {
            scope: expr.get_scope_id(db),
            id: expr.get_id(db),
        }
    }

    pub fn from_var_access(
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> Self {
        Self {
            scope: var_access.get_scope_id(db),
            id: var_access.get_id(db),
        }
    }

    pub fn from_namespace_access(
        db: &'db dyn WorkspaceDataBase,
        access: &SpanNamespaceAccess<'db>,
    ) -> Self {
        Self {
            scope: access.get_scope_id(db),
            id: access.get_id(db),
        }
    }

    pub fn from_var_decl(db: &'db dyn WorkspaceDataBase, var_access: VariableDecl<'db>) -> Self {
        Self {
            scope: var_access.get_scope_id(db),
            id: var_access.get_id(db),
        }
    }

    pub fn from_spec(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Self {
        Self {
            scope: spec.get_scope_id(db),
            id: spec.get_id(db),
        }
    }

    pub fn from_struct_element(
        db: &'db dyn WorkspaceDataBase,
        element: StructElement<'db>,
    ) -> Self {
        Self {
            scope: element.get_scope_id(db),
            id: element.get_id(db),
        }
    }

    pub fn from_method_ref(
        db: &'db dyn WorkspaceDataBase,
        method: MethodRef<'db>,
    ) -> Self {
        Self {
            scope: method.get_scope_id(db),
            id: method.get_id(db),
        }
    }

    pub fn from_pou(
        db: &'db dyn WorkspaceDataBase,
        pou: Pou<'db>,
    ) -> Self {
        Self {
            scope: pou.get_scope_id(db),
            id: pou.get_id(db),
        }
    }

    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> CompactString {
        let file = self.scope.file(db);
        let node = semantic_index(db, file)
            .ast
            .get(self.id.0)
            .unwrap_or_else(|| panic!("Invalid ID {} when attempting to retrieve text", self.id.0));
        CompactString::from(
            node.get_text(file.document(db).as_bytes())
                .expect("Failed to get text"),
        )
    }
}

impl<'db> HirNodeInfo<'db> for CallSite<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope
    }
}

fn tree_sitter_position_adjusted(
    encoding: Encoding,
    line_str: &str,
    point: auto_lsp::tree_sitter::Point,
) -> lsp_types::Position {
    let mut utf16_offset = 0;
    let mut u8_offset = 0;
    let mut u32_offset = 0;

    for c in line_str.chars() {
        if u8_offset >= point.column {
            break;
        }
        utf16_offset += c.len_utf16();
        u8_offset += c.len_utf8();
        u32_offset += 1;
    }

    let character = match encoding {
        Encoding::UTF8 => u8_offset,
        Encoding::UTF16 => utf16_offset,
        Encoding::UTF32 => u32_offset,
    };

    lsp_types::Position {
        line: point.row as u32,
        character: character as u32,
    }
}

pub trait HirNodeInfo<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId;

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db>;

    fn as_call_site(&self, db: &'db dyn WorkspaceDataBase) -> CallSite<'db> {
        CallSite {
            scope: self.get_scope_id(db),
            id: self.get_id(db),
        }
    }

    fn get_span(&self, db: &'db dyn WorkspaceDataBase) -> Span {
        let mut ts_range = *semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "invalid ID {} when attempting to retrieve span",
                    *self.get_id(db)
                )
            })
            .get_range();

        let document = self.get_scope_id(db).file(db).document(db);

        let line_str = self
            .get_scope_id(db)
            .file(db)
            .document(db)
            .texter
            .get_row(ts_range.start_point.row)
            .expect("Failed to get line string for start point; this is a bug!");

        let start =
            tree_sitter_position_adjusted(document.encoding, line_str, ts_range.start_point);

        let line_str = self
            .get_scope_id(db)
            .file(db)
            .document(db)
            .texter
            .get_row(ts_range.end_point.row)
            .expect("Failed to get line string for end point; this is a bug!");

        let end = tree_sitter_position_adjusted(document.encoding, line_str, ts_range.end_point);

        ts_range.start_point.column = start.character as usize;
        ts_range.start_point.row = start.line as usize;
        ts_range.end_point.column = end.character as usize;
        ts_range.end_point.row = end.line as usize;

        ts_range.into()
    }
}

pub trait HasName<'db>: HirNodeInfo<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident;

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId;

    fn get_name_span(&'db self, db: &'db dyn WorkspaceDataBase) -> Span {
        let mut ts_range = *semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_name_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "invalid name ID {} when attempting to retrieve name span",
                    *self.get_name_id(db)
                )
            })
            .get_range();

        let document = self.get_scope_id(db).file(db).document(db);

        let line_str = self
            .get_scope_id(db)
            .file(db)
            .document(db)
            .texter
            .get_row(ts_range.start_point.row)
            .expect("Failed to get line string for start point; this is a bug!");

        let start =
            tree_sitter_position_adjusted(document.encoding, line_str, ts_range.start_point);

        let line_str = self
            .get_scope_id(db)
            .file(db)
            .document(db)
            .texter
            .get_row(ts_range.end_point.row)
            .expect("Failed to get line string for end point; this is a bug!");

        let end = tree_sitter_position_adjusted(document.encoding, line_str, ts_range.end_point);

        ts_range.start_point.column = start.character as usize;
        ts_range.start_point.row = start.line as usize;
        ts_range.end_point.column = end.character as usize;
        ts_range.end_point.row = end.line as usize;

        ts_range.into()
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifier: u16 {
        const ABSTRACT = 1 << 0;
        const FINAL = 1 << 1;
        const OVERRIDE = 1 << 2;
    }
}

impl Modifier {
    pub const EMPTY: Modifier = Modifier::empty();
}

pub trait HasModifiers<'db>: HirNodeInfo<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier;
}

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Visibility: u16 {
        const PUBLIC = 1 << 0;
        const PROTECTED = 1 << 1;
        const INTERNAL = 1 << 2;
        const PRIVATE = 1 << 3;
    }
}

impl<'db> Visibility {
    pub const EMPTY: Visibility = Visibility::empty();
}

pub trait HasVisibility<'db>: HirNodeInfo<'db> {
    fn get_visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility;
}
