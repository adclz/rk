use std::{error::Error, fmt::format, num::ParseIntError};

use auto_lsp::{
    core::span::Span, default::db::{file::File, BaseDatabase}, lsp_types::DiagnosticRelatedInformation
};
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Accumulator;

use crate::{
    diagnostics::{diagnostic_builder::diag, literals::check_date, DiagnosticAccumulator},
    hir::{
        expressions::expression::{AnyBit, AnyChars, 
            AnyDate, AnyDuration, AnyElementary, AnyInt, AnyMagnitude,
             AnyNum, AnyReal, AnySigned, AnyUnsigned, Expr, ExprKind, Numeric, NumericKind,
              PrimaryExpr}, interned::namespace::NamespacePath, pous::variable::{Spec, SpecKind, Variable}, using::Using
    },
};

trait Check<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, file: File);
}

trait CheckWithVisibility<'db> {
    fn check_with_visibility(&'db self, db: &'db dyn BaseDatabase, file: File, ns: NamespacePath);
}

#[salsa::tracked(no_eq)]
pub fn duplicate_declarations<'db>(db: &'db dyn BaseDatabase, file: File) {

}

impl<'db> CheckWithVisibility<'db> for Using<'db> {
    fn check_with_visibility(&'db self, db: &'db dyn BaseDatabase, file: File, from: NamespacePath) {
        let to = self.path(db);
    }
}

impl<'db> Check<'db> for &'db Vec<Variable<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, file: File) {
        let mut seen_variable = FxHashMap::default();
        let mut seen_spec = FxHashSet::default();

        for variable in self.iter() {
            if let Some(other) = seen_variable.insert(variable.name(db), variable) {
                let message = format!(
                    "duplicate variable declaration: '{}'",
                    variable.name(db).text(db)
                );
                let diagnostic = diag()
                    .range(variable.name_span(db).clone())
                    .message(message)
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: auto_lsp::lsp_types::Location {
                            uri: variable.file(db).url(db).clone(),
                            range: other.name_span(db).into(),
                        },
                        message: format!(
                            "variable '{}' is previously declared here",
                            other.name(db).text(db)
                        ),
                    }])
                    .call();
                DiagnosticAccumulator::accumulate(diagnostic.into(), db);
            }

            // Avoid checking the same spec multiple times
            if seen_spec.insert(variable.spec(db)) {
                variable.check(db, file);
            }
        }
    }
}

impl<'db> Check<'db> for Variable<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, file: File) {
        match self.init(db) {
            Some(init) => init.check(db, file, &self.spec(db)),
            None => {}
        }
    }
}
 
trait SpecCheck<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, file: File, spec: &Spec<'db>);
}

fn create_type_inference_error<'db>(db: &'db dyn BaseDatabase, file: File, span: Span, spec: &'db Spec<'db>, err: impl Error) {
    let diagnostic = diag()
        .range(span.clone().into())
        .message(err.to_string())
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: spec.span.clone().into(),
            },
            message: format!("because of type: '{:?}' declared here", spec.kind),
        }])
        .call();
    DiagnosticAccumulator::accumulate(diagnostic.into(), db);
}

fn create_mismatch_type_error<'db>(db: &'db dyn BaseDatabase, file: File, span: Span, spec: &'db Spec<'db>, message: String) {
    let diagnostic = diag()
        .range(span.clone().into())
        .message(message)
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: spec.span.clone().into(),
            },
            message: format!("because of type: '{}' declared here", spec.to_string(db)),
        }])
        .call();
    DiagnosticAccumulator::accumulate(diagnostic.into(), db);
}

impl<'db> SpecCheck<'db> for Expr<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, file: File, spec: &Spec<'db>) {
        match self.expr(db) {
            ExprKind::AddOperator { left, operator, right } => {
                left.check(db, file, spec);
                right.check(db, file, spec);
            },
            ExprKind::BooleanOperator { left, operator, right } => {
                if !matches!(spec.kind, SpecKind::Bool) {
                        create_mismatch_type_error(db, file, self.span(db).clone(), spec, 
                        format!("A boolean operator always returns a 'BOOL' but the expected type is '{}'", spec.to_string(db))
                    );
                }
                left.check(db, file, spec);
                right.check(db, file, spec);

            },
            ExprKind::ComparisonOperator { left, operator, right } => {
                if !matches!(spec.kind, SpecKind::Bool) {
                        create_mismatch_type_error(db, file, self.span(db).clone(), spec, 
                        format!("A comparison operator always returns a 'BOOL' but the expected type is '{}'", spec.to_string(db))
                    );
                }
                left.check(db, file, spec);
                right.check(db, file, spec);
            },
            ExprKind::MultOperator { left, operator, right } => {
                left.check(db, file, spec);
                right.check(db, file, spec);
            },
            ExprKind::PowerOperator { left, right } => {
                left.check(db, file, spec);
                right.check(db, file, spec);
            },
            ExprKind::UnaryOperator { expr, operator } => {
                expr.check(db, file, spec);
            },
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit))=> {
                let result = match spec.kind {
                    SpecKind::Bool => match lit {
                        AnyElementary::AnyBit(AnyBit::Bool(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_bool(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        }
                        _ => false,
                    },
                    // bit string types
                    SpecKind::Byte => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u8(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Word => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u16(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::DWord => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                         AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u32(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::LWord => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                        AnyElementary::AnyBit(AnyBit::LWord(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u64(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    // signed integers
                    SpecKind::SInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u8(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Int =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u16(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::DInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::DInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u32(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    }, 
                    SpecKind::LInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::DInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::LInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u64(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    // unsigned integers
                    SpecKind::USInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u8(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::UInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u16(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::UDInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u32(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    }, 
                    SpecKind::ULInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::ULInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u64(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::Real => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Real(_)))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Infer(identifier)))) => {
                            match identifier.as_f32(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    SpecKind::LReal => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Real(_)))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::LReal(_)))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Infer(identifier)))) => {
                            match identifier.as_f64(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_inference_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    _ => false,
                };

                if !result {
                    let message = format!(
                        "value '{}' is not assignable to '{}'",
                        lit.to_string(db),
                        spec.to_string(db),
                    );
                    let diagnostic = diag()
                        .range(self.span(db).clone())
                        .message(message)
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .related_information(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range: spec.span.clone().into(),
                            },
                            message: format!("because of type '{}' declared here", spec.to_string(db)),
                        }])
                        .call();
                    DiagnosticAccumulator::accumulate(diagnostic.into(), db);
                }
            }
            _ => {}
        }
    }
}
