#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use ast::generated::ConstantExpr;
use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::DiagnosticRelatedInformation,
};
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    check::{
        literals::check_date,
    },
    def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExprKind, Numeric, NumericKind, PathExpr,
                PrimaryExpr, VariableAccessKind,
            },
            spec::{Spec, SpecKind},
            statement::{Stmt, StmtKind}, 
        },
        interned::namespace::NamespacePath,
        pous::{pou::Pou, variable::Variable},
        scope::FileScopeId,
        semantic_index::{semantic_index, SemanticIndex},
    },
    ty::{
        name_res::pous_in_scope,
        stmt_resolver::resolve_stmts,
        ty::{ty_for_variable, Ty, TyKind},
        ty_path_expr_resolver::ResolvePathExprCtx,
    },
    to_proto::ToProto,
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

trait SpecCheck<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, spec: &Ty<'db>);
}

impl<'db> SpecCheck<'db> for Expr<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, ty: &Ty<'db>) {
        match self.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, ty);
                right.check(db, sema, ty);
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, ty);
                right.check(db, sema, ty);
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, ty);
                right.check(db, sema, ty);
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, ty);
                right.check(db, sema, ty);
            }
            ExprKind::PowerOperator { left, right } => {
                left.check(db, sema, ty);
                right.check(db, sema, ty);
            }
            ExprKind::UnaryOperator { expr, operator } => {
                expr.check(db, sema, ty);
            }
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => {
                let result = match ty.kind(db) {
                    TyKind::Simple(spec) => {
                        match spec.kind(db) {
                            SpecKind::Bool => match lit {
                                Elementary::Bool(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_bool(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            // bit string types
                            SpecKind::Byte => match lit {
                                Elementary::Byte(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u8(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::Word => match lit {
                                Elementary::Byte(_) | Elementary::Word(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u16(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::DWord => match lit {
                                Elementary::Byte(_)
                                | Elementary::Word(_)
                                | Elementary::DWord(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::LWord => match lit {
                                Elementary::Byte(_)
                                | Elementary::Word(_)
                                | Elementary::DWord(_)
                                | Elementary::LWord(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u64(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            // signed integers
                            SpecKind::SInt => match lit {
                                Elementary::SInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u8(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::Int => match lit {
                                Elementary::SInt(_) | Elementary::Int(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u16(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::DInt => match lit {
                                Elementary::SInt(_) | Elementary::Int(_) | Elementary::DInt(_) => {
                                    true
                                }
                                Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::LInt => match lit {
                                Elementary::SInt(_)
                                | Elementary::Int(_)
                                | Elementary::DInt(_)
                                | Elementary::LInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u64(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            // unsigned integers
                            SpecKind::USInt => match lit {
                                Elementary::USInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u8(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::UInt => match lit {
                                Elementary::USInt(_) | Elementary::UInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u16(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::UDInt => match lit {
                                Elementary::USInt(_)
                                | Elementary::UInt(_)
                                | Elementary::UDInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            SpecKind::ULInt => match lit {
                                Elementary::USInt(_)
                                | Elementary::UInt(_)
                                | Elementary::UDInt(_)
                                | Elementary::ULInt(_) => true,
                                Elementary::InferNumeric(infer) => match infer.as_u64(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => false,
                            },
                            // floats
                            SpecKind::Real => match lit {
                                Elementary::Real(_) => true,
                                Elementary::InferIdent(identifier) => match identifier.as_f32(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => true,
                            },
                            SpecKind::LReal => match lit {
                                Elementary::Real(_) | Elementary::LReal(_) => true,
                                Elementary::InferIdent(identifier) => match identifier.as_f64(db) {
                                    Ok(_) => true,
                                    Err(err) => {
                                        mismatch_type(db, sema, self.get_span(db).clone(), ty, err);
                                        return;
                                    }
                                },
                                _ => true,
                            },
                            _ => true,
                        }
                    }
                    _ => true,
                };
                if !result {
                    mismatch_type(
                        db,
                        sema,
                        self.get_span(db).clone(),
                        ty,
                        format!(
                            "Literal '{}' does not match expected type '{}'",
                            lit.to_string(db),
                            ""
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}
*/