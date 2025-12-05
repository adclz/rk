#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use ast::generated::ConstantExpr;
use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
    lsp_types::DiagnosticRelatedInformation,
};
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    HirNodeInfo,
    check::{
        check_inheritance::check_inheritance,
        check_namespaces::check_duplicate_namespaces,
        errors::{analysis_error::ToIdeDiagnostic, duplicates::DuplicateError},
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
            pou::{Pou},
            variable::VariableDecl,
        },
        scope::ScopeId,
        semantic_index::{SemanticIndex, get_scope, semantic_index},
    },
    hir_ty::{
        name_res::pou_index,
    },
};

pub trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>);
}

pub trait DataTypeCheck<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        // Get syntax errors
        self.errors
            .iter()
            .for_each(|err| errors.push(err.to_diagnostic(db)));

        self.pous(db).iter().for_each(|pou| pou.get_scope_id(db).check(db, errors));
        // Namespaces
        self.namespaces.iter().for_each(|ns| ns.scope_id(db).check(db, errors));
    }
}
