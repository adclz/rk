#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use ast::generated::ConstantExpr;
use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase, file::File},
    lsp_types::DiagnosticRelatedInformation,
};
use db::{WorkspaceDataBase, configuration::Configuration};
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    HirNodeInfo,
    check::{
        check_inheritance::check_inheritance,
        check_namespaces::check_duplicate_namespaces,
        errors::{
            analysis_error::{AnalysisError, ToIdeDiagnostic},
            duplicates::DuplicateError,
            syntax::SyntaxError,
        },
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
        pous::{pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
        scope::ScopeId,
        semantic_index::{SemanticIndex, get_scope, semantic_index},
    },
    hir_ty::name_res::pou_index,
};

pub trait Check<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>);
}

pub trait DataTypeCheck<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>);
}

impl<'db> Check<'db> for SemanticIndex<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        // Check if this file should contains program declarations
        let config = Configuration::try_get(db);

        // Get syntax errors
        self.errors
            .iter()
            .for_each(|err| errors.push(err.to_diagnostic(db)));

        self.pous(db)
            .iter()
            .for_each(|pou| pou.get_scope_id(db).check(db, errors));

        // Programs
        self.programs.iter().for_each(|program| {
            program.get_scope_id(db).check(db, errors);
        });

        // Namespaces
        self.namespaces.iter().for_each(|ns| {
            check_duplicate_namespaces(db, *ns)
                .values()
                .for_each(|diags| errors.extend_from_slice(diags));
            ns.scope_id(db).check(db, errors)
        });
    }
}
