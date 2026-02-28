#![allow(unused)]
use ast::generated::InitElem;
use auto_lsp::lsp_types::{Hover, HoverContents, MarkedString, MarkupContent, MarkupKind};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, InitExpr, InitExprKind, ParamAssign, PathExpr, VariableAccess,
            },
            spec::{Spec, SpecKind, StructElement},
        }, hir_node::HirNode, interned::namespace::{NamespaceAccess, SpanNamespaceAccess}, namespace::NamespaceDecl, pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        }, program::ProgramDecl, using::Using
    },
    hir_ty::{
        head::{inheritance::MethodRef, signature::infer_signature},
        infer::Infer,
        ty::Type,
    },
};

use crate::{
    handlers::HoverHandler,
    hir_node::{HasComment, MaybeHirNode, get_param_start_pos},
};

impl<'db> HoverHandler<'db> for HirNode<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
            HirNode::Program(p) => p.hover(db, offset),
            HirNode::Namespace(n) => n.hover(db, offset),
            HirNode::NamespaceAccess(a) => a.hover(db, offset),
            HirNode::PouDecl(p) => p.hover(db, offset),
            HirNode::VariableDecl(v) => v.hover(db, offset),
            HirNode::InitExpr(i) => i.hover(db, offset),
            HirNode::Spec(s) => s.hover(db, offset),
            HirNode::MethodRef(m) => m.hover(db, offset),
            HirNode::StructElement(st) => st.hover(db, offset),
            HirNode::PathExpr(p) => p.hover(db, offset),
            HirNode::VariableAccess(v) => v.hover(db, offset),
            HirNode::Expr(e) => e.hover(db, offset),
            HirNode::Using(u) => u.hover(db, offset),
            HirNode::Param(p) => p.hover(db, offset),
            _ => None,
        }
    }
}

impl<'db> HoverHandler<'db> for NamespaceDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let ns = self.path(db).to_string(db);
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(
                format!(
                    r#"
```iecst
NAMESPACE {ns}
```
                    "#
                )
                .to_string(),
            )),
            range: None,
        })
    }
}

impl<'db> HoverHandler<'db> for SpanNamespaceAccess<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for ProgramDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.get_name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let comment = self.get_comment(db).unwrap_or_default();
        let name = self.get_name_ident(db).text(db);

        let path = Type::Program(*self).path_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{path}PROGRAM {name}
```
                "#
                ),
            }),
            range: Some(self.get_name_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for Pou<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.get_name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self {
            Pou::Function(_) => "FUNCTION",
            Pou::FunctionBlock(_) => "FUNCTION_BLOCK",
            Pou::Class(_) => "CLASS",
            Pou::Interface(_) => "INTERFACE",
            Pou::DataType(dt) => "TYPE",
        };

        let name = self.get_name_ident(db).text(db);
        let return_type = match self {
            Pou::Function(f) => f
                .return_type(db)
                .map(|spec| format!(": {}", spec.infer(db).type_name(db)))
                .unwrap_or_default(),
            Pou::DataType(dt) => format!(": {}", dt.spec(db).infer(db).full_type_name(db)),
            _ => "".to_string(),
        };

        let path = Type::new_pou(db, *self).path_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{path}{kind} {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_name_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for VariableDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self.kind(db) {
            VariableKind::Input => "INPUT",
            VariableKind::Output => "OUTPUT",
            VariableKind::InOut => "IN_OUT",
            VariableKind::Var => "VAR",
            VariableKind::External => "EXTERNAL",
            VariableKind::Global => "GLOBAL",
            VariableKind::Access => "ACCESS",
            VariableKind::Config => "CONFIG",
            VariableKind::Temp => "TEMP",
        };

        let infer = self.spec(db).infer(db);
        let name = self.name(db).text(db);
        let type_name = infer.type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
({kind}) {name}: {type_name}
```
                "#
                ),
            }),
            range: Some(self.get_name_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = self.infer(db);

        let comment = infer
            .as_hir_node(db)
            .and_then(|n| n.get_comment(db))
            .unwrap_or_default();

        let desc = infer.full_type_name(db);
        let path = infer.path_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{path}{desc}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for MethodRef<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let kind = match self {
            MethodRef::Declared(_) => "METHOD",
            MethodRef::Prototype(_) => "METHOD PROTOTYPE",
        };

        let name = self.get_name_ident(db).text(db);
        let return_type = match self {
            MethodRef::Declared(decl) => match decl.return_type(db) {
                Some(ret_ty) => format!(": {}", ret_ty.infer(db).type_name(db)),
                None => "".to_string(),
            },
            MethodRef::Prototype(proto) => match proto.return_type(db) {
                Some(ret_ty) => format!(": {}", ret_ty.infer(db).type_name(db)),
                None => "".to_string(),
            },
        };
        let comment = self.get_comment(db).unwrap_or_default();
        let path = Type::MethodDecl(*self).path_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{path}{kind} {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_name_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for StructElement<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let name = self.name(db).text(db);
        let type_name = self.spec(db).infer(db).type_name(db);
        let path = Type::StructElement(*self).path_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{path}{name}: {type_name}
```
                "#
                ),
            }),
            range: Some(self.get_name_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for InitExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for BeginPathExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for PathExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for Expr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for VariableAccess<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for ParamAssign<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for Using<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let mut accumulated_path = vec![];

        for (index, fragment) in self.path(db).fragments(db).iter().enumerate() {
            let span = self.path(db).get_fragment_ast_node(db, index).get_span();
            accumulated_path.push(fragment.text(db).to_string());

            if offset >= span.start_byte && offset <= span.end_byte {
                let full_path = accumulated_path.join(".");
                return Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                        r#"
```iecst
(USING) NAMESPACE {}
```
"#,
                        full_path
                    ))),
                    range: None,
                });
            }
        }

        None
    }
}

impl<'db> HoverHandler<'db> for Type<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        eprintln!("Hovering type: {:?}", self);
        match self {
            Type::Function(f) => Pou::Function(*f).hover(db, offset),
            Type::FunctionBlock(f) => Pou::FunctionBlock(*f).hover(db, offset),
            Type::Class(f) => Pou::Class(*f).hover(db, offset),
            Type::Interface(f) => Pou::Interface(*f).hover(db, offset),
            Type::DataType(f) => Pou::DataType(*f).hover(db, offset),
            Type::StructElement(st) => st.hover(db, offset),
            Type::MethodDecl(m) => m.hover(db, offset),
            Type::Variable((var, _multibits)) => var.hover(db, offset),
            _ => Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: {
                        format!(
                            r#"
```iecst
{}
```"#,
                            self.full_type_name(db)
                        )
                    },
                }),
                range: None,
            }),
        }
    }
}
