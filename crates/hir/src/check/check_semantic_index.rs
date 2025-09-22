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
        check_inheritance::check_methods,
        check_init_expr::check_init_expr,
        check_ty::check_ty,
        errors::{duplicates::DuplicateError, sem_errors::AnalysisError, stmt::StmtError},
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
        scope::FileScopeId,
        semantic_index::{HirNode, SemanticIndex, semantic_index},
    },
    hir_ty::{
        TyInfo,
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        init_expr_resolver::{ResolvedInitExpr, resolve_init_expr},
        name_res::{
            all_global_pous, all_local_pous, shared_namespaces,
        },
        stmt_resolver::{ResolveStmtCtx, ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, TyKind, ty_for_pou, ty_for_struct_field, ty_for_variable},
        ty_path_expr_resolver::{ResolvePathExprCtx, ResolvedPathElementKind, ResolvedPathResult},
        ty_var_access_resolver::{ResolvedVarKind, ResolvedVarResult},
    },
    walk::WalkHir,
};

pub trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        // Get syntax errors
        self.errors.iter().for_each(|err| errors.push(err.clone()));
        let mut seen = FxHashMap::default();

        // Global pous
        self.global_pous.iter().for_each(|p| {
            match all_global_pous(db).get(p.name(db)) {
                Some(prev) => {
                    if prev.scope_id(db).file(db) != p.scope_id(db).file(db) {
                        errors.push(
                            DuplicateError::Pou {
                                pou1: ty_for_pou(db, *p),
                                pou2: ty_for_pou(db, *prev),
                            }
                            .into(),
                        );
                    } else {
                        match seen.get(p.name(db)) {
                            Some(prev) => {
                                errors.push(
                                    DuplicateError::Pou {
                                        pou1: ty_for_pou(db, *p),
                                        pou2: ty_for_pou(db, *prev),
                                    }
                                    .into(),
                                );
                            }
                            None => {
                                seen.insert(p.name(db), *p);
                            }
                        }
                    }
                }
                None => {
                    seen.insert(p.name(db), *p);
                }
            }
            p.check(db, errors)
        });

        // Namespaces
        self.namespaces.iter().for_each(|n| {
            let mut seen = FxHashMap::default();
            n.pous(db).iter().for_each(|p| {
                seen.insert(p.name(db), *p);
            });

            shared_namespaces(db, *n.path(db)).iter().for_each(|n| {
                n.pous(db)
                    .into_iter()
                    .for_each(|p| match seen.get(p.name(db)) {
                        Some(prev) => {
                            if prev.scope_id(db).file(db) == p.scope_id(db).file(db) {
                                return;
                            }
                            errors.push(
                                DuplicateError::Pou {
                                    pou1: ty_for_pou(db, *prev),
                                    pou2: ty_for_pou(db, *p),
                                }
                                .into(),
                            );
                        }
                        None => {
                            seen.insert(p.name(db), *p);
                        }
                    })
            });
            n.check(db, errors)
        });
    }
}

impl<'db> Check<'db> for NamespaceDecl<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let mut seen = FxHashMap::default();
        self.pous(db).iter().for_each(|p| {
            match seen.get(p.name(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::Pou {
                            pou1: ty_for_pou(db, *p),
                            pou2: ty_for_pou(db, *prev),
                        }
                        .into(),
                    );
                }
                None => {
                    seen.insert(p.name(db), *p);
                }
            }
            p.check(db, errors)
        });
    }
}

impl<'db> Check<'db> for PouDecl<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match self.pou(db) {
            Pou::Function(f) => {
                check_ty(db, ty_for_pou(db, *self), errors);
                f.variables(db).check(db, errors);
                f.statements(db).check(db, errors);
            }
            Pou::FunctionBlock(fb) => {
                check_ty(db, ty_for_pou(db, *self), errors);
                fb.variables(db).check(db, errors);
                fb.statements(db).check(db, errors);
            }
            Pou::DataType(dt) => {
                check_ty(db, ty_for_pou(db, *self), errors);
            }
            Pou::Class(cl) => {
                check_ty(db, ty_for_pou(db, *self), errors);
                check_methods(db, ty_for_pou(db, *self), errors);
                cl.variables(db).check(db, errors);
            }
            Pou::Interface(it) => {
                check_ty(db, ty_for_pou(db, *self), errors);
                check_methods(db, ty_for_pou(db, *self), errors);
            }
        }

        if let Pou::DataType(typ) = self.pou(db) {
            match typ.spec(db).kind(db) {
                SpecKind::Struct(st) => {
                    st.check(db, errors);
                }
                SpecKind::Array(arr) => {
                    arr.check(db, errors);
                }
                _ => {}
            }
        }
    }
}
