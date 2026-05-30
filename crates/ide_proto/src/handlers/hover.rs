#![allow(unused)]
use ast::generated::InitElem;
use auto_lsp::lsp_types::{Hover, HoverContents, MarkedString, MarkupContent, MarkupKind};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ProgConfig, ResourceDecl, TaskConfig},
        expressions::{
            expression::{
                BeginPathExpr, Elementary, Expr, ExprKind, InitExpr, InitExprKind, ParamAssign,
                PathExpr, PrimaryExpr, VariableAccess,
            },
            spec::{Spec, SpecKind, StructElement},
        },
        hir_node::HirNode,
        namespace::NamespaceDecl,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        program::ProgramDecl,
        scope::ScopeKind,
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        config::infer_config_result,
        head::{inheritance::MethodRef, signature::infer_signature},
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::{
    handlers::{
        HoverHandler,
        completions::{is_namespace_prefix, try_build_namespace_path},
    },
    hir_node::{HasComment, MaybeHirNode, get_param_start_pos},
};

impl<'db> HoverHandler<'db> for HirNode<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
            HirNode::Program(p) => p.hover(db, offset),
            HirNode::Namespace(n) => n.hover(db, offset),
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
            HirNode::Config(c) => c.hover(db, offset),
            HirNode::Resource(r) => r.hover(db, offset),
            HirNode::Task(t) => t.hover(db, offset),
            HirNode::ProgConfig(p) => p.hover(db, offset),
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
                value: format!("```iecst\n{path}PROGRAM {name}\n```\n{comment}"),
            }),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                    .unwrap_or_default(),
            ),
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
                value: format!("```iecst\n{path}{kind} {name}{return_type}\n```\n{comment}"),
            }),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                    .unwrap_or_default(),
            ),
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
                value: format!("```iecst\n({kind}) {name}: {type_name}\n```\n{comment}"),
            }),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                    .unwrap_or_default(),
            ),
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
                value: format!("```iecst\n{path}{kind} {name}{return_type}\n```\n{comment}"),
            }),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                    .unwrap_or_default(),
            ),
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
                value: format!("```iecst\n{path}{name}: {type_name}\n```\n{comment}"),
            }),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &self.get_name_span(db))
                    .unwrap_or_default(),
            ),
        })
    }
}

impl<'db> HoverHandler<'db> for InitExpr<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        if let InitExprKind::ConstantExpr(expr) = self.kind(db)
            && let ExprKind::PrimaryExpr(prim) = expr.expr(db)
        {
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                    "```iecst\n{}\n```",
                    prim.to_string(db)
                ))),
                range: None,
            });
        }
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
        let ty = self.infer(db);
        if ty.is_never() {
            // Check if this PathExpr is part of a namespace path (e.g. "System" in System.Math.Sin)
            if let Some(ns_path) = try_build_namespace_path(db, self)
                && is_namespace_prefix(db, ns_path)
            {
                return Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                        "\n```iecst\nNAMESPACE {}\n```\n",
                        ns_path.to_string(db)
                    ))),
                    range: None,
                });
            }
        }
        ty.hover(db, offset)
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
            let span = self
                .path(db)
                .get_fragment_ast_node(db, index)
                .get_range()
                .to_owned();
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

impl<'db> HoverHandler<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        if let SpecKind::Target(target) = self.kind(db)
            && let Some(path) = &target.path.namespace
        {
            let mut accumulated_path = vec![];

            for (index, fragment) in path.fragments(db).iter().enumerate() {
                let span = path.get_fragment_ast_node(db, index).get_range().to_owned();
                accumulated_path.push(fragment.text(db).to_string());

                if offset >= span.start_byte && offset <= span.end_byte {
                    let full_path = accumulated_path.join(".");
                    return Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                            r#"
```iecst
NAMESPACE {}
```
"#,
                            full_path
                        ))),
                        range: None,
                    });
                }
            }
        }

        self.infer(db).hover(db, offset)
    }
}

impl<'db> HoverHandler<'db> for Type<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
            Type::CallableType(cl) => cl.hover(db, cl.get_name_span(db).start_byte),
            Type::Program(p) => p.hover(db, p.get_name_span(db).start_byte),
            Type::Function(f) => Pou::Function(*f).hover(db, f.get_name_span(db).start_byte),
            Type::FunctionBlock(f) => {
                Pou::FunctionBlock(*f).hover(db, f.get_name_span(db).start_byte)
            }
            Type::Class(f) => Pou::Class(*f).hover(db, f.get_name_span(db).start_byte),
            Type::Interface(f) => Pou::Interface(*f).hover(db, f.get_name_span(db).start_byte),
            Type::DataType(f) => Pou::DataType(*f).hover(db, f.get_name_span(db).start_byte),
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

impl<'db> HoverHandler<'db> for CallableType<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        match self {
            CallableType::Function(f) => Pou::Function(*f).hover(db, offset),
            CallableType::FunctionBlock(fb) => Pou::FunctionBlock(*fb).hover(db, offset),
            CallableType::MethodDecl(m) => m.hover(db, offset),
        }
    }
}

impl<'db> HoverHandler<'db> for ConfigDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.get_name_span(db);
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let name = self.name(db).text(db);
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                "\n```iecst\nCONFIGURATION {name}\n```\n"
            ))),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &name_span)
                    .unwrap_or_default(),
            ),
        })
    }
}

impl<'db> HoverHandler<'db> for ResourceDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.name(db).get_span(db);
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let name = self.name(db).ident.text(db);
        let resource_type = self.resource_type_name(db).text(db);
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                "\n```iecst\nRESOURCE {name} ON {resource_type}\n```\n"
            ))),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &name_span)
                    .unwrap_or_default(),
            ),
        })
    }
}

impl<'db> HoverHandler<'db> for TaskConfig<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.name(db).get_span(db);
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let name = self.name(db).ident.text(db);
        let priority = self
            .priority(db)
            .map(|p| p.text(db).to_string())
            .unwrap_or_else(|| "?".to_string());
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                "\n```iecst\nTASK {name} (PRIORITY := {priority})\n```\n"
            ))),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &name_span)
                    .unwrap_or_default(),
            ),
        })
    }
}

impl<'db> HoverHandler<'db> for ProgConfig<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        // Check if cursor is on the WITH <task> reference
        if let Some(task_ref) = self.task(db) {
            let span = task_ref.get_span(db);
            if offset >= span.start_byte && offset < span.end_byte {
                if let ScopeKind::Config(config) = get_scope(db, self.scope_id(db)).kind
                    && let Some(task) = infer_config_result(db, config).task_of_prog.get(self)
                {
                    return task.hover(db, task.name(db).get_span(db).start_byte);
                }
                return None;
            }
        }

        let name_span = self.name(db).get_span(db);
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let name = self.name(db).ident.text(db);
        let prog_type = match self.prog_type(db).kind(db) {
            SpecKind::Target(t) => t.path.target.ident.text(db).to_string(),
            _ => "?".to_string(),
        };
        let task_part = match self.task(db) {
            Some(task) => format!(" WITH {}", task.ident.text(db)),
            None => String::new(),
        };
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                "\n```iecst\nPROGRAM {name}{task_part} : {prog_type}\n```\n"
            ))),
            range: Some(
                hir::denormalize(db, self.get_scope_id(db).file(db), &name_span)
                    .unwrap_or_default(),
            ),
        })
    }
}
