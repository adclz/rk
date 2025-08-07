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
        errors::semantic_errors::{duplicate_variable_declaration, mismatch_type},
        literals::check_date,
        DiagnosticAccumulator,
    },
    hir::{
        expressions::{
            expression::{
                AnyBit, AnyChars, AnyDate, AnyDuration, AnyElementary, AnyInt, AnyMagnitude,
                AnyNum, AnyReal, AnySigned, AnyUnsigned, Expr, ExprKind, Numeric, NumericKind,
                PrimaryExpr,
            },
            spec::{SimpleSpecKind, Spec, SpecKind},
        },
        interned::namespace::NamespacePath,
        pous::{pou::Pou, variable::Variable},
        scopes::solver::{pous_in_scope, resolve_namespace_access},
        semantic_index::{semantic_index, SemanticIndex},
        signature::{type_signature, SignatureKind, TypeSignature},
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
                    func.variables(db).check(db, &sema);
                }
                Pou::FunctionBlock(fb) => {
                    fb.variables(db).check(db, &sema);
                }
                Pou::Interface(interface) => {
                    interface.methods(db).iter().for_each(|method| {
                        method.variables(db).check(db, &sema);
                    });
                }
                Pou::DataType(dt) => {}
                Pou::Class(class) => {}
            }
        }
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
            init.check(db, sema, self.spec(db))
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
                    SpecKind::Simple(SimpleSpecKind::Bool) => match lit {
                        AnyElementary::AnyBit(AnyBit::Bool(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_bool(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    // bit string types
                    SpecKind::Simple(SimpleSpecKind::Byte) => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u8(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::Word) => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u16(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::DWord) => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::LWord) => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                        AnyElementary::AnyBit(AnyBit::LWord(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u64(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    // signed integers
                    SpecKind::Simple(SimpleSpecKind::SInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::SInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u8(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::Int) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::SInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::Int(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u16(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::DInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::SInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::Int(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::DInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::LInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::SInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::Int(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::DInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnySigned(AnySigned::LInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u64(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    // unsigned integers
                    SpecKind::Simple(SimpleSpecKind::USInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::USInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u8(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::UInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::USInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::UInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u16(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::UDInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::USInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::UInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::ULInt) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::USInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::UInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::AnyUnsigned(AnyUnsigned::ULInt(_)),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(
                            AnyInt::Infer(infer),
                        ))) => match infer.as_u64(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    // floats
                    SpecKind::Simple(SimpleSpecKind::Real) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(
                            AnyReal::Real(_),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(
                            AnyReal::Infer(identifier),
                        ))) => match identifier.as_f32(db) {
                            Ok(_) => true,
                            Err(err) => {
                                mismatch_type(db, sema, self.span(db).clone(), spec, err);
                                return;
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Simple(SimpleSpecKind::LReal) => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(
                            AnyReal::Real(_),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(
                            AnyReal::LReal(_),
                        ))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(
                            AnyReal::Infer(identifier),
                        ))) => match identifier.as_f64(db) {
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
