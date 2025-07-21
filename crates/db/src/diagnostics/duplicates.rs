use std::{fmt::format, num::ParseIntError};

use auto_lsp::{
    core::span::Span, default::db::{file::File, BaseDatabase}, lsp_types::DiagnosticRelatedInformation
};
use rustc_hash::FxHashMap;
use salsa::Accumulator;

use crate::{
    diagnostics::{diagnostic_builder::diag, literals::check_date, DiagnosticAccumulator},
    hir::{
        expression::{AnyBit, AnyChars, AnyDate, AnyDuration, AnyElementary, AnyInt, AnyMagnitude, AnyNum, AnyReal, AnySigned, AnyUnsigned, Expr, ExprKind, Numeric, NumericKind, PrimaryExpr},
        namespace::{NamespaceResult, PouDecl, PouResult, Using},
        variable::Spec,
    },
    solver::{
        fq_name::SpannedPath,
        namespace::{namespace_path, namespaces_in_file, using_namespaces_in_scope, NamespacePath},
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
    if let Some(namespaces) = namespaces_in_file(db, file) {
        for namespace in namespaces.namespaces(db).iter() {
            let path = namespace.path(db);
            for using in namespace.using(db).iter() {
                using.check_with_visibility(db, file, *path);
            }

            for pou in namespace.pous(db).iter() {
                pou.check_with_visibility(db, file, *path);

                for other_decl in namespace_path(db, *path) {
                    if other_decl.file(db) == file {
                        continue;
                    }

                    match other_decl.get_pou(db, *path, *path, *pou.name(db)) {
                        PouResult::Found(other_pou) => {
                            let duplicate = pou.name(db).text(db);
                            DiagnosticAccumulator::accumulate(
                                diag()
                                    .file(file)
                                    .range(pou.name_span(db).clone())
                                    .message(format!("duplicate declaration of {duplicate} POU"))
                                    .source("IEC".into())
                                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                                    .related_information(vec![DiagnosticRelatedInformation {
                                        location: auto_lsp::lsp_types::Location {
                                            uri: other_decl.file(db).url(db).clone(),
                                            range: other_pou.name_span(db).into(),
                                        },
                                        message: format!(
                                            "'{duplicate}' is previously declared here"
                                        ),
                                    }])
                                    .call()
                                    .into(),
                                db,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl<'db> CheckWithVisibility<'db> for Using<'db> {
    fn check_with_visibility(&'db self, db: &'db dyn BaseDatabase, file: File, from: NamespacePath) {
        let to = self.path(db);
        let results = namespace_path(db, to);

        using_namespaces_in_scope(db, file, *self)
            .iter()
            .for_each(|(span, ns)| {
                let message = format!("namespace '{}' is already in scope", to.to_string(db));
                let diagnostic = diag()
                    .file(file)
                    .range(self.span(db).clone())
                    .message(message)
                    .source("IEC".into())
                    .related_information(
                        vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range: (*span).into(),
                            },
                            message: format!("namespace '{}' is already declared here", to.to_string(db)),
                        }],
                    )
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::WARNING)
                    .tags(vec![auto_lsp::lsp_types::DiagnosticTag::UNNECESSARY])
                    .call();
                DiagnosticAccumulator::accumulate(diagnostic.into(), db);
            });

        if results.is_empty() {
            let message = format!("unknown namespace: '{}'", to.to_string(db));
            let diagnostic = diag()
                .file(file)
                .range(self.span(db).clone())
                .message(message)
                .source("IEC".into())
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .call();
            DiagnosticAccumulator::accumulate(diagnostic.into(), db);
        } else {
            if results
                .iter()
                .all(|ns| match ns.get_namespace(db, from, to) {
                    NamespaceResult::Hidden(_) => true,
                    _ => false,
                })
            {
                let message = format!(
                    "All declarations of namespace '{}' are hidden.",
                    to.to_string(db)
                );
                let diagnostic = diag()
                    .file(file)
                    .range(self.span(db).clone())
                    .message(message)
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .call();
                DiagnosticAccumulator::accumulate(diagnostic.into(), db);
            }
        }
    }
}

impl<'db> CheckWithVisibility<'db> for PouDecl<'db> {
    fn check_with_visibility(&'db self, db: &'db dyn BaseDatabase, file: File, ns: NamespacePath) {
        match self.pou(db) {
            crate::hir::namespace::Pou::Function(func) => {
                func.variables(db).check(db, file);
                func.using(db)
                    .iter()
                    .for_each(|using| using.check_with_visibility(db, file, ns));
            }
            crate::hir::namespace::Pou::FunctionBlock(func) => {
                func.variables(db).check(db, file);
                func.extends(db)
                    .map(|extend| extend.check_with_visibility(db, file, ns));
                func.using(db)
                    .iter()
                    .for_each(|using| using.check_with_visibility(db, file,  ns));
                func.implements(db).map(|implements| {
                    implements.iter().for_each(|implement| {
                        implement.check_with_visibility(db, file, ns);
                    });
                });
            }
            crate::hir::namespace::Pou::DataType(_) => {}
            crate::hir::namespace::Pou::Class(class) => {
                class
                    .extends(db)
                    .map(|extend| extend.check_with_visibility(db,  file, ns));
                class
                    .using(db)
                    .iter()
                    .for_each(|using| using.check_with_visibility(db, file, ns));
            }
            crate::hir::namespace::Pou::Interface(interface) => {
                interface.extends(db).map(|extend| {
                    extend.iter().for_each(|extend| {
                        extend.check_with_visibility(db, file,  ns);
                    });
                });
                interface
                    .using(db)
                    .iter()
                    .for_each(|using| using.check_with_visibility(db, file, ns));
            }
        }
    }
}

impl<'db> CheckWithVisibility<'db> for SpannedPath {
    fn check_with_visibility(&'db self, db: &'db dyn BaseDatabase, file: File, from: NamespacePath) {
        let to = if let Some(to) = self.fq_name.namespace(db) {
            to
        } else {
            return;
        };
        let pou = self.fq_name.target(db);
        let results = namespace_path(db, to);

        eprintln!("results: {:?} -> {:?}", to.to_string(db), pou.ident.text(db));

        if results.is_empty() {
            let message = format!("unknown namespace: '{}'", to.to_string(db));
            let diagnostic = diag()
                .file(file)
                .range(self.span.clone())
                .message(message)
                .source("IEC".into())
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .call();
            DiagnosticAccumulator::accumulate(diagnostic.into(), db);
        } else {
            if let None = results
                .iter()
                .find_map(|ns| match ns.get_pou(db, from, to, pou.ident) {
                    PouResult::Hidden(_) => {
                        let message = format!(
                            "POU '{}' is hidden in namespace '{}'",
                            pou.ident.text(db),
                            to.to_string(db)
                        );
                        let diagnostic = diag()
                            .file(file)
                            .range(self.span.clone())
                            .message(message)
                            .source("IEC".into())
                            .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                            .call();
                        DiagnosticAccumulator::accumulate(diagnostic.into(), db);
                        None
                    }
                    PouResult::NotFound => None,
                    PouResult::Found(found) => Some(found),
                })
            {
                let message = format!(
                    "POU '{}' not found in namespace '{}'",
                    pou.ident.text(db),
                    to.to_string(db)
                );
                let diagnostic = diag()
                    .file(file)
                    .range(self.span.clone())
                    .message(message)
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .call();
                DiagnosticAccumulator::accumulate(diagnostic.into(), db);
            };
        }
    }
}

impl<'db> Check<'db> for &'db Vec<crate::hir::variable::Variable<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, file: File) {
        let mut seen = FxHashMap::default();

        for variable in self.iter() {
            if let Some(other) = seen.insert(variable.name(db), variable) {
                let message = format!(
                    "duplicate variable declaration: '{}'",
                    variable.name(db).text(db)
                );
                let diagnostic = diag()
                    .file(file)
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
            variable.check(db, file);
        }
    }
}

impl<'db> Check<'db> for crate::hir::variable::Variable<'db> {
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

fn create_type_error<'db>(db: &'db dyn BaseDatabase, file: File, span: Span, spec: &'db Spec<'db>, err: ParseIntError) {
    let diagnostic = diag()
        .file(file)
        .range(span.clone().into())
        .message(err.to_string())
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: file.url(db).clone(),
                range: span.clone().into(),
            },
            message: format!("because of type: '{:?}' declared here", spec),
        }])
        .call();
    DiagnosticAccumulator::accumulate(diagnostic.into(), db);
}

impl<'db> SpecCheck<'db> for Expr<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, file: File, spec: &Spec<'db>) {
        match self.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit))=> {
                let result = match spec {
                    Spec::Bool => match lit {
                        AnyElementary::AnyBit(AnyBit::Bool(_)) => true,
                        _ => false,
                    },
                    // bit string types
                    Spec::Byte => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            match infer.as_u8(db) {
                                Ok(_) => true,
                                Err(err) => {
                                    create_type_error(db, file, self.span(db).clone(), spec, err);
                                    return;
                                }
                            }
                        },
                        _ => false,
                    },
                    Spec::Word => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            infer.as_u16(db).is_some()
                        },
                        _ => false,
                    },
                    Spec::DWord => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                         AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            infer.as_u32(db).is_some()
                        },
                        _ => false,
                    },
                    Spec::LWord => match lit {
                        AnyElementary::AnyBit(AnyBit::Byte(_)) => true,
                        AnyElementary::AnyBit(AnyBit::Word(_)) => true,
                        AnyElementary::AnyBit(AnyBit::DWord(_)) => true,
                        AnyElementary::AnyBit(AnyBit::LWord(_)) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::Infer(infer)))) => {
                            infer.as_u64(db).is_some()
                        },
                        _ => false,
                    },
                    // signed integers
                    Spec::SInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        _ => false,
                    },
                    Spec::Int =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        _ => false,
                    },
                    Spec::DInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::DInt(_))))) => true,
                        _ => false,
                    }, 
                    Spec::LInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::SInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::Int(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::DInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnySigned(AnySigned::LInt(_))))) => true,
                        _ => false,
                    },
                    // unsigned integers
                    Spec::USInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        _ => false,
                    },
                    Spec::UInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        _ => false,
                    },
                    Spec::UDInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_))))) => true,
                        _ => false,
                    }, 
                    Spec::ULInt =>  match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::USInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::UDInt(_))))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyInt(AnyInt::AnyUnsigned(AnyUnsigned::ULInt(_))))) => true,
                        _ => false,
                    },
                    Spec::Real => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Real(_)))) => true,
                        _ => false,
                    },
                    Spec::LReal => match lit {
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::Real(_)))) => true,
                        AnyElementary::AnyMagnitude(AnyMagnitude::AnyNum(AnyNum::AnyReal(AnyReal::LReal(_)))) => true,
                        _ => false,
                    },
                    _ => false,
                };

                if !result {
                    let message = format!(
                        "value '{}' is not assignable to '{:?}'",
                        lit.to_string(db),
                        spec,
                    );
                    let diagnostic = diag()
                        .file(file)
                        .range(self.span(db).clone())
                        .message(message)
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .call();
                    DiagnosticAccumulator::accumulate(diagnostic.into(), db);
                }
            }
            _ => {}
        }
    }
}
