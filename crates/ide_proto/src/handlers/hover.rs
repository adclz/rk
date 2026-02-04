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
        },
        namespace::NamespaceDecl,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        using::Using,
    },
    hir_ty::{
        body::infer_body,
        signature::{infer_signature, inheritance::MethodRef},
        ty::Type,
    },
};

use crate::{
    handlers::HoverHandler,
    hir_node::{HasComment, MaybeHirNode, get_param_start_pos},
};

impl<'db> HoverHandler<'db> for NamespaceDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
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

impl<'db> HoverHandler<'db> for Pou<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.get_name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let infer = infer_signature(db, self.get_scope_id(db));

        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self {
            Pou::Function(_) => "FUNCTION".into(),
            Pou::FunctionBlock(_) => "FUNCTION_BLOCK".into(),
            Pou::Class(_) => "CLASS".into(),
            Pou::Interface(_) => "INTERFACE".into(),
            Pou::DataType(dt) => infer.type_of_specs[&dt.spec(db)].type_name(db),
        };

        let name = self.get_name_ident(db).text(db);
        let return_type = match self {
            Pou::Function(f) => f
                .return_type(db)
                .map(|spec| format!(": {}", infer.type_of_specs[spec].type_name(db)))
                .unwrap_or_default(),
            _ => "".to_string(),
        };

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
[{kind}] {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
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

        let infer = infer_signature(db, self.get_scope_id(db));
        let name = self.name(db).text(db);
        let type_name = infer.type_of_specs[&self.spec(db)].type_name(db);

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
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_signature(db, self.scope_id(db));

        let comment = infer.type_of_specs[self]
            .as_hir_node(db)
            .and_then(|n| n.get_comment(db))
            .unwrap_or_default();

        let desc = infer.type_of_specs[self].full_type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{desc}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for InitExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_signature(db, self.scope_id(db));
        let typ = infer.init_expr_result.type_of_init_expr.get(self)?;

        let comment = typ
            .as_hir_node(db)
            .and_then(|n| n.get_comment(db))
            .unwrap_or_default();

        let desc = typ.full_type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{desc}
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
        let infer = infer_signature(db, self.get_scope_id(db));
        let name = self.get_name_ident(db).text(db);
        let return_type = match self {
            MethodRef::Declared(decl) => match decl.return_type(db) {
                Some(ret_ty) => format!(": {}", infer.type_of_specs[ret_ty].type_name(db)),
                None => "".to_string(),
            },
            MethodRef::Prototype(proto) => match proto.return_type(db) {
                Some(ret_ty) => format!(": {}", infer.type_of_specs[ret_ty].type_name(db)),
                None => "".to_string(),
            },
        };
        let comment = self.get_comment(db).unwrap_or_default();

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
[{kind}] {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for StructElement<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let name = self.name(db).text(db);
        let infer = infer_signature(db, self.get_scope_id(db));
        let type_name = infer.type_of_specs[&self.spec(db)].type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{name}: {type_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> HoverHandler<'db> for BeginPathExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_begin_path_expr(db, *self)
            .and_then(|typ| {
                typ.hover(db, offset).map(|mut hover| {
                    hover.range = Some(self.get_span(db).lsp());
                    hover
                })
            })
    }
}

impl<'db> HoverHandler<'db> for PathExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body(db, self.scope_id(db));
        infer.get_type_of_path_expr(db, *self).and_then(|typ| {
            typ.hover(db, offset).map(|mut hover| {
                hover.range = Some(self.get_span(db).lsp());
                hover
            })
        })
    }
}

impl<'db> HoverHandler<'db> for Expr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        if let Some(r) = infer_signature(db, self.scope_id(db))
            .body_infer_result
            .get_type_of_expr(*self)
            .and_then(|typ| typ.hover(db, offset))
        {
            return Some(r);
        }

        infer_body(db, self.scope_id(db))
            .get_type_of_expr(*self)
            .and_then(|typ| typ.hover(db, offset))
    }
}

impl<'db> HoverHandler<'db> for VariableAccess<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body(db, self.scope_id(db));
        infer
            .get_type_of_variable_access(db, *self)
            .and_then(|typ| {
                typ.hover(db, offset).map(|mut hover| {
                    hover.range = Some(self.get_span(db).lsp());
                    hover
                })
            })
    }
}

impl<'db> HoverHandler<'db> for ParamAssign<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body(db, self.scope_id(db));
        infer.variable_of_param.get(self).and_then(|var| {
            Type::new_var(db, *var).hover(db, offset).map(|mut hover| {
                hover.range = Some(get_param_start_pos(db, self).get_span(db).lsp());
                hover
            })
        })
    }
}

impl<'db> HoverHandler<'db> for Type<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
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
(using) NAMESPACE {}
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
