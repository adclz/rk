use auto_lsp::lsp_types::{GotoDefinitionResponse, Location};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
            spec::{Spec, SpecKind, StructElement},
        }, hir_node::HirNode, interned::namespace::NamespacePath, namespace::NamespaceDecl, pous::{pou::Pou, variable::VariableDecl}, using::Using
    },
    hir_ty::{index_graphs::namespace_index, infer::Infer, ty::Type},
};

use crate::{handlers::DefinitionHandler};

impl<'db> DefinitionHandler<'db> for HirNode<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        match self {
            HirNode::Namespace(ns) => ns.definition(db),
            HirNode::Using(u) => u.definition(db),
            HirNode::PouDecl(pou) => pou.definition(db),
            HirNode::VariableDecl(v) => v.definition(db),
            HirNode::StructElement(s) => s.definition(db),
            HirNode::InitExpr(i) => i.definition(db),
            HirNode::Spec(s) => s.definition(db),
            HirNode::PathExpr(p) => p.definition(db),
            HirNode::VariableAccess(v) => v.definition(db),
            HirNode::Expr(e) => e.definition(db),
            HirNode::Param(p) => p.definition(db),
            _ => None,
        }
    }
}

impl<'db> DefinitionHandler<'db> for NamespaceDecl<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        namespace_definitions(db, *self.path(db))
    }
}

impl<'db> DefinitionHandler<'db> for Using<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        namespace_definitions(db, self.path(db).path)
    }
}

impl<'db> DefinitionHandler<'db> for Pou<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}

impl<'db> DefinitionHandler<'db> for VariableDecl<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for StructElement<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for Spec<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for InitExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for BeginPathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for PathExpr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for VariableAccess<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for Expr<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for ParamAssign<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.infer(db).definition(db)
    }
}

impl<'db> DefinitionHandler<'db> for Type<'db> {
    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let loc: &'db dyn HirNodeInfo<'db> = match self {
            Type::Function(f) => f as _,
            Type::FunctionBlock(f) => f as _,
            Type::Class(c) => c as _,
            Type::Interface(i) => i as _,
            Type::StructElement(st) => st as _,
            Type::MethodDecl(m) => m as _,
            Type::DataType(dt) => match dt.spec(db).kind(db) {
                SpecKind::Target(_) => return dt.spec(db).infer(db).definition(db),
                _ => dt as _,
            },
            Type::Variable((var, _multibits)) => return var.spec(db).infer(db).definition(db),
            _ => None?,
        };

        Some(GotoDefinitionResponse::Scalar(Location::new(
            loc.get_scope_id(db).file(db).url(db).to_owned(),
            loc.get_span(db).into(),
        )))
    }
}

fn namespace_definitions<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Option<GotoDefinitionResponse> {
    let decls: Vec<_> = namespace_index(db, path)
        .iter()
        .map(|ns| {
            Location::new(
                ns.scope_id(db).file(db).url(db).to_owned(),
                ns.name_span(db).into(),
            )
        })
        .collect();
    if decls.is_empty() {
        None
    } else {
        Some(GotoDefinitionResponse::Array(decls))
    }
}
