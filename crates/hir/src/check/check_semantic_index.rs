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
    HirNodeInfo,
    check::{
        check_inheritance::check_inheritance,
        check_init_expr::check_init_expr,
        check_namespaces::check_duplicate_namespaces,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError, stmt::StmtError},
    },
    hir_def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExprKind, Integer, IntegerKind, PathExpr,
                PrimaryExpr, VariableAccessKind,
            },
            spec::{ElementarySpec, Spec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        interned::namespace::NamespacePath,
        namespace::NamespaceDecl,
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::ScopeId,
        semantic_index::{HirNode, SemanticIndex, semantic_index},
    },
    hir_ty::{
        init_expr_resolver::{ResolvedInitExpr, resolve_init_expr},
        name_res::{all_global_pous, all_local_pous},
        ty::{Ty, TyKind},
        ty_var_access_resolver::ResolvedAccess,
    },
    walk::WalkHir,
};

pub trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>);
}

pub trait DataTypeCheck<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        // Get syntax errors
        self.errors.iter().for_each(|err| errors.push(err.clone()));
        self.pous(db).iter().for_each(|pou| pou.check(db, errors));
        // Namespaces
        self.namespaces.iter().for_each(|ns| ns.check(db, errors));
    }
}

impl<'db> Check<'db> for NamespaceDecl<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        if let Some(ns_errors) =
            check_duplicate_namespaces(db, *self).get(&self.scope_id(db).file(db))
        {
            errors.extend(ns_errors.iter().cloned());
        }
        self.pous(db).iter().for_each(|p| p.check(db, errors));
    }
}

impl<'db> Check<'db> for PouDecl<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match self.pou(db) {
            Pou::Function(f) => {
                f.variables(db).check(db, errors);
                f.statements(db).check(db, errors);
            }
            Pou::FunctionBlock(fb) => {
                check_inheritance(db, *self, errors);
                fb.variables(db).check(db, errors);
                fb.statements(db).check(db, errors);
                fb.methods(db).iter().for_each(|m| {
                    m.variables(db).check(db, errors);
                    m.stmts(db).check(db, errors);
                });
            }
            Pou::Class(cl) => {
                check_inheritance(db, *self, errors);
                cl.variables(db).check(db, errors);
                cl.methods(db).iter().for_each(|m| {
                    m.variables(db).check(db, errors);
                    m.stmts(db).check(db, errors);
                });
            }
            Pou::Interface(it) => {
                check_inheritance(db, *self, errors);
            }
            Pou::DataType(typ) => match typ.spec(db).kind(db) {
                SpecKind::Array(arr) => {
                    arr.check(db, errors);
                }
                SpecKind::Struct(st) => {
                    st.check(db, errors);
                }
                SpecKind::Enum(en) => {
                    en.check(db, errors);
                }
                SpecKind::Subrange(sub) => {
                    sub.check(db, errors);
                }
                _ => {}
            },
        }
    }
}
