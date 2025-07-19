use auto_lsp::{
    default::db::{file::File, BaseDatabase},
    lsp_types::DiagnosticRelatedInformation,
};
use rustc_hash::FxHashMap;
use salsa::Accumulator;

use crate::{
    diagnostics::{diagnostic_builder::diag, literals::check_date, DiagnosticAccumulator},
    hir::{
        expression::{Expr, ExprKind, Literal, PrimaryExpr},
        namespace::{NamespaceResult, PouDecl, PouResult, Using},
        variable::Spec,
    },
    solver::{
        fq_name::SpannedPath,
        namespace::{namespace_path, namespaces_in_file, starts, starts_with, NamespacePath},
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
        for (path, namespace) in namespaces.namespaces(db).iter() {
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

impl<'db> SpecCheck<'db> for Expr<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, file: File, spec: &Spec<'db>) {
        use Literal::AnyNumeric;

        match self.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit))=> {
                let result = match spec {
                    Spec::Bool => matches!(lit, Literal::Bool(_)),
                    Spec::Byte => matches!(lit, Literal::Byte(_) | AnyNumeric(_)),
                    Spec::Word => matches!(lit, Literal::Word(_) | AnyNumeric(_)),
                    Spec::DWord => matches!(lit, Literal::DWord(_) | AnyNumeric(_)),
                    Spec::LWord => matches!(lit, Literal::LWord(_) | AnyNumeric(_)),
                    Spec::SInt => matches!(lit, Literal::SInt(_) | AnyNumeric(_)),
                    Spec::USInt => matches!(lit, Literal::USInt(_) | AnyNumeric(_)),
                    Spec::Int => matches!(lit, Literal::Int(_) | AnyNumeric(_)),
                    Spec::DInt => matches!(lit, Literal::DInt(_) | AnyNumeric(_)),
                    Spec::LInt => matches!(lit, Literal::LInt(_) | AnyNumeric(_)),
                    Spec::ULInt => matches!(lit, Literal::ULInt(_) | AnyNumeric(_)),
                    Spec::Real => matches!(lit, Literal::Real(_) | AnyNumeric(_)),
                    Spec::LReal => matches!(lit, Literal::LReal(_) | AnyNumeric(_)),
                    Spec::Time => matches!(lit, Literal::Time(_) | AnyNumeric(_)),
                    Spec::Date => matches!(lit, Literal::Date(_) | AnyNumeric(_)),
                    Spec::LDate => matches!(lit, Literal::LDate(_) | AnyNumeric(_)),
                    Spec::Tod => matches!(lit, Literal::Tod(_) | AnyNumeric(_)),
                    Spec::LTod => matches!(lit, Literal::LTod(_) | AnyNumeric(_)),
                    Spec::Dt => matches!(lit, Literal::DateTime(_)),
                    Spec::Ldt => matches!(lit, Literal::LDateTime(_)),
                    Spec::Char => matches!(lit, Literal::Char(_)),
                    Spec::WChar => matches!(lit, Literal::DChar(_)),
                    // Complex types
                    Spec::Enum => matches!(lit, AnyNumeric(_)),
                    Spec::Struct => matches!(lit, AnyNumeric(_)),
                    Spec::Array(_) => matches!(lit, AnyNumeric(_)),
                    Spec::String => matches!(lit, Literal::Char(_)),
                    Spec::WString => matches!(lit, Literal::Char(_)),
                    _ => false,
                };

                lit.self_check(db, file);

                if !result {
                    let message = format!(
                        "invalid literal for spec '{:?}': {}",
                        spec,
                        lit.to_string(db)
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

impl Literal {
    fn self_check(&self, db: &dyn BaseDatabase, file: File) {
        match self {
            Literal::Date(ident) => {
                if let Err(err) = check_date(db, &ident.text(db)) {
                    let diagnostic = err.to_diag();
                    DiagnosticAccumulator::accumulate((file, diagnostic).into(), db);
                };
            }
            _ => {}
        }
    }
}
