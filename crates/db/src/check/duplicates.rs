#![allow(unused_imports)]
#![allow(dead_code)]
use std::{error::Error, fmt::format, num::ParseIntError, str::ParseBoolError};

use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::DiagnosticRelatedInformation,
};
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    check::{
        diagnostic_builder::diag,
        errors::semantic_errors::{
            assign_direct_pou_to_a_variable, duplicate_variable_declaration, mismatch_type,
            no_item_in_scope, type_can_not_be_dereferenced, type_has_no_field,
            unexpected_index_expression, unknown_field,
        },
        hir::signature::ResolvePathExprCtx,
        literals::check_date,
        DiagnosticAccumulator,
    },
    hir::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, Numeric, NumericKind, PathExpr, PrimaryExpr,
                VariableAccessKind,
            },
            spec::{Spec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        interned::namespace::NamespacePath,
        pous::{pou::Pou, variable::Variable},
        scopes::solver::{pous_in_scope, resolve_namespace_access},
        semantic_index::{semantic_index, SemanticIndex},
        ty::{ty_for_pou, TyKind, WalkError},
    },
};

trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>);
}

#[salsa::tracked(no_eq)]
pub fn duplicate_declarations<'db>(db: &'db dyn BaseDatabase, file: File) {
    let sema = semantic_index(db, file);

    for scope in sema.scopes.values() {
        pous_in_scope(db, file, scope.id);
    }

    for ns in sema.namespaces.iter() {
        for pou in ns.pous(db).iter() {
            let pou_name = pou.name(db);

            match pou.pou(db) {
                Pou::Function(func) => {
                    func.variables(db).check(db, sema);
                    func.statements(db).check(db, sema);
                }
                Pou::FunctionBlock(fb) => {
                    fb.variables(db).check(db, sema);
                }
                Pou::Interface(interface) => {
                    interface.methods(db).iter().for_each(|method| {
                        method.variables(db).check(db, sema);
                    });
                }
                Pou::DataType(dt) => {}
                Pou::Class(class) => {}
            }
        }
    }
}

impl<'db> Check<'db> for Vec<Stmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>) {
        self.iter().for_each(|stmt| if let StmtKind::Assignment { var, target } = stmt.stmt(db) {
            if let VariableAccessKind::Symbolic(symbolic) = &var.kind {
                let ctx = ResolvePathExprCtx::new(db, sema.file, symbolic.kind);
                let r = ctx.resolve_path_expr();

                if let Some(err) = r.error {
                    match err {
                        WalkError::NoItemInScope { expr, scope } => {
                            no_item_in_scope(
                                db,
                                sema.file,
                                &expr.to_string(db),
                                expr.span(db),
                            );
                        }
                        WalkError::FieldNotFound { origin, expr } => {
                            unknown_field(db, sema.file, &expr.to_string(db), expr.span(db))
                        }
                        WalkError::NotAnArray { origin, expr } => {
                            unexpected_index_expression(db, sema.file, expr.span(db))
                        }
                        WalkError::NotAReference { origin, expr } => {
                            type_can_not_be_dereferenced(db, sema.file, expr.span(db));
                        }
                    }
                } else if let TyKind::Callable { .. } = r.elements[0].get_ty().kind(db) {
                    assign_direct_pou_to_a_variable(
                        db,
                        sema.file,
                        *r.elements[0].get_expr(),
                        r.elements[0].get_ty().origin(db),
                    );
                }
            }
        });
    }
}

impl<'db> Check<'db> for &'db Vec<Variable<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>) {
        let mut seen_variable = FxHashMap::default();
        let mut seen_spec = FxHashSet::default();

        for variable in self.iter() {
            if let Some(other) = seen_variable.insert(variable.name(db), variable) {
                duplicate_variable_declaration(db, variable.file(db), variable, other);
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
        if let Some(init) = self.init(db) {
            //init.check(db, sema, self.spec(db))
        }
    }
}

trait SpecCheck<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, spec: &Spec<'db>);
}

impl<'db> SpecCheck<'db> for Expr<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>, spec: &Spec<'db>) {
        match self.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, spec);
                right.check(db, sema, spec);
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, spec);
                right.check(db, sema, spec);
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, spec);
                right.check(db, sema, spec);
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                left.check(db, sema, spec);
                right.check(db, sema, spec);
            }
            ExprKind::PowerOperator { left, right } => {
                left.check(db, sema, spec);
                right.check(db, sema, spec);
            }
            ExprKind::UnaryOperator { expr, operator } => {
                expr.check(db, sema, spec);
            }
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => {
                let result = match spec.kind(db) {
                    SpecKind::Bool => match lit {
                        Elementary::Bool(_) => true,
                        Elementary::InferNumeric(infer) => match infer.as_bool(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::DWord => match lit {
                        Elementary::Byte(_) | Elementary::Word(_) | Elementary::DWord(_) => true,
                        Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::DInt => match lit {
                        Elementary::SInt(_) | Elementary::Int(_) | Elementary::DInt(_) => true,
                        Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::UDInt => match lit {
                        Elementary::USInt(_) | Elementary::UInt(_) | Elementary::UDInt(_) => true,
                        Elementary::InferNumeric(infer) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
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
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => true,
                    },
                    _ => true,
                };

                if !result {
                    mismatch_type(
                        db,
                        sema,
                        self.span(db).clone(),
                        spec,
                        format!(
                            "Literal '{}' does not match expected type '{}'",
                            lit.to_string(db),
                            spec.shorthand(db)
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}
