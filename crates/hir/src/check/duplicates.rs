#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use ast::generated::ConstantExpr;
use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
    lsp_types::DiagnosticRelatedInformation,
};
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    check::literals::check_date,
    def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExprKind, Integer, IntegerKind, PathExpr,
                PrimaryExpr, VariableAccessKind,
            },
            spec::{Spec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        interned::namespace::NamespacePath,
        pous::{pou::Pou, variable::VariableDecl},
        scope::FileScopeId,
        semantic_index::{SemanticIndex, semantic_index},
    },
    to_proto::ToProto,
    ty::{
        name_res::pous_in_scope,
        stmt_resolver::resolve_stmt,
        ty::{Ty, TyKind, ty_for_variable},
        ty_path_expr_resolver::ResolvePathExprCtx,
    },
};

trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>);
}

#[salsa::tracked]
pub fn duplicate_declarations<'db>(db: &'db dyn BaseDatabase, file: File) {
    let sema = semantic_index(db, file);
}
/*
trait CheckStmts<'db> {
    fn check(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        scope_id: FileScopeId,
    );
}

impl<'db> CheckStmts<'db> for Vec<Stmt<'db>> {
    fn check(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        scope_id: FileScopeId,
    ) {
        resolve_stmts(db, self, scope_id);
    }
}

impl<'db> Check<'db> for &'db Vec<Variable<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>) {
        let mut seen_variable = FxHashMap::default();
        let mut seen_spec = FxHashSet::default();

        for variable in self.iter() {
            if let Some(other) = seen_variable.insert(variable.name(db), variable) {
                duplicate_variable_declaration(db, variable.scope_id(db).file(), variable, other);
            }

            // Avoid checking the same spec multiple times
            if seen_spec.insert(variable.spec(db)) {
                variable.check(db, sema);
            }
        }
    }
}

impl<'db> Check<'db> for Variable<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>) {
        let signature = ty_for_variable(db, *self);
        if let Some(init) = self.init(db) {
            match init.kind {
                InitExprKind::ConstantExpr(expr) => expr.check(db, sema, &signature),
                _ => {}
            }
        }
    }
}
*/