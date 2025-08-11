use std::sync::Arc;

use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use rustc_hash::FxHashMap;

use crate::hir::{
    expressions::{
        expression::{PathExpr, PathExprKind, VarAccess},
        spec::{Enum, Spec, SpecKind},
    },
    interned::{
        identifier::{Ident, SpannedIdent},
        namespace::{NamespaceAccess, SpannedNamespaceAccess},
    },
    pous::{
        pou::{Pou, PouDecl},
        variable::{Variable, VariableKind},
    },
    scopes::{scope::ScopeId, solver::resolve_namespace_access},
    semantic_index::SemanticIndex,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Signature<'db> {
    // Base types
    Simple(Spec<'db>),
    Enum {
        spec: Spec<'db>,
        list: Vec<Ident>,
    },
    SubRange,
    Array {
        spec: Arc<Signature<'db>>,
    },
    Struct(FxHashMap<Ident, Arc<Signature<'db>>>),

    Callable {
        decl: PouDecl<'db>,
        input: FxHashMap<Ident, Arc<Signature<'db>>>,
        output: FxHashMap<Ident, Arc<Signature<'db>>>,
        in_out: FxHashMap<Ident, Arc<Signature<'db>>>,
        return_type: Option<Arc<Signature<'db>>>,
    },

    RefTo(Arc<Signature<'db>>),

    Variable(Arc<Signature<'db>>),

    // Meta / error
    Unresolved(NamespaceAccess),
    RecursivePou(PouDecl<'db>),
    RecursiveVariable(Variable<'db>),
}

fn pou_result<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Arc<Signature<'db>> {
    Arc::new(Signature::RecursivePou(pou))
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked(cycle_result = pou_result)]
pub fn signature_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Arc<Signature<'db>> {
    match pou.pou(db) {
        Pou::Function(func) => {
            let mut inputs = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();

            for v in func.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    _ => continue,
                };
            }

            Arc::new(Signature::Callable {
                decl: pou,
                input: inputs,
                output: outputs,
                in_out: in_outs,
                return_type: func.return_type(db).map(|rt| rt.to_sig(db)),
            })
        }
        Pou::FunctionBlock(fb) => {
            let mut inputs = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();

            for v in fb.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db));
                    }
                    _ => continue,
                };
            }

            Arc::new(Signature::Callable {
                decl: pou,
                input: inputs,
                output: outputs,
                in_out: in_outs,
                return_type: None, // Function blocks don't have a return type in this context
            })
        }
        Pou::DataType(dt) => dt.spec(db).to_sig(db),
        Pou::Class(class) => {
            todo!()
        }
        Pou::Interface(interface) => {
            todo!()
        }
    }
}

fn signature_result<'db>(
    db: &'db dyn BaseDatabase,
    variable: Variable<'db>,
) -> Arc<Signature<'db>> {
    Arc::new(Signature::RecursiveVariable(variable))
}
#[salsa::tracked(cycle_result = signature_result)]
pub fn signature_for_variable<'db>(
    db: &'db dyn BaseDatabase,
    variable: Variable<'db>,
) -> Arc<Signature<'db>> {
    Arc::new(Signature::Variable(variable.spec(db).to_sig(db)))
}

impl<'db> Signature<'db> {
    pub fn is_simple(&self) -> bool {
        if let Signature::RefTo(sig) = self {
            return sig.is_simple();
        };
        matches!(self, Signature::Simple(_))
    }

    pub fn is_callable(&self) -> bool {
        if let Signature::RefTo(sig) = self {
            return sig.is_callable();
        };
        matches!(self, Signature::Callable { .. })
    }

    pub fn is_invalid(&self) -> bool {
        matches!(self, Signature::Unresolved(_) | Signature::RecursivePou(_))
    }

    pub fn is_reference(&self) -> bool {
        matches!(self, Signature::RefTo(_))
    }

    pub fn has_return_type(&self) -> bool {
        if let Signature::RefTo(sig) = self {
            return sig.has_return_type();
        };
        if let Signature::Callable { return_type, .. } = self {
            return return_type.is_some();
        }
        false
    }
}

impl<'db> Spec<'db> {
    pub fn to_sig(&self, db: &'db dyn BaseDatabase) -> Arc<Signature<'db>> {
        match self.kind(db) {
            SpecKind::Array(array) => Arc::new(Signature::Array {
                spec: array.of_type.to_sig(db),
            }),
            SpecKind::Enum(enum_spec) => match enum_spec {
                Enum::Named(list) => Arc::new(Signature::Enum {
                    spec: self.clone(),
                    list: list.iter().map(|(name, expr)| (*name)).collect(),
                }),
                Enum::Anonymous(list) => Arc::new(Signature::Enum {
                    spec: self.clone(),
                    list: list.iter().map(|name| *name).collect(),
                }),
            },
            SpecKind::Subrange(_) => Arc::new(Signature::SubRange),
            SpecKind::Struct(fields) => Arc::new(Signature::Struct(
                fields
                    .elements
                    .iter()
                    .map(|elem| (elem.name, elem.spec.to_sig(db)))
                    .collect(),
            )),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.file(db), self.scope_id(db), *target) {
                    Some(pou) => signature_for_pou(db, pou),
                    None => Arc::new(Signature::Unresolved(*target)),
                }
            }
            _ => Arc::new(Signature::Simple(self.clone())),
        }
    }
}

pub trait WalkSignature<'db> {
    fn linear(&self, step: SignatureStep<'db>) -> Result<Arc<Signature<'db>>, LinearError<'db>>;
    fn as_callable(&'db self) -> Option<Callable<'db>>;
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SignatureStep<'db> {
    Field {
        ident: SpannedIdent,
        expr: PathExpr<'db>,
    }, // By name
    Index {
        expr: PathExpr<'db>,
    }, // By index
    Deref {
        expr: PathExpr<'db>,
    }, // For pointers or references
}

pub struct Callable<'db> {
    pub inputs: FxHashMap<Ident, Arc<Signature<'db>>>,
    pub in_outs: FxHashMap<Ident, Arc<Signature<'db>>>,
    pub outputs: FxHashMap<Ident, Arc<Signature<'db>>>,
    pub return_type: Option<Arc<Signature<'db>>>,
}

impl<'db> Callable<'db> {
    pub fn get_param(&self, ident: &Ident) -> Option<&Arc<Signature<'db>>> {
        self.inputs
            .get(ident)
            .or_else(|| self.outputs.get(ident))
            .or_else(|| self.in_outs.get(ident))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearError<'db> {
    NoItemInScope {
        ident: SpannedIdent,
        scope: ScopeId,
    },
    FieldNotFound {
        ident: SpannedIdent,
    },
    SpecNotHaveField {
        ident: SpannedIdent,
        spec: Spec<'db>,
    },
    CanNotHaveField {
        ident: SpannedIdent,
    },
    NotAnArray {
        expr: PathExpr<'db>,
    },
    NotAReference,
}

impl<'db> WalkSignature<'db> for Signature<'db> {
    fn linear(&self, step: SignatureStep<'db>) -> Result<Arc<Signature<'db>>, LinearError<'db>> {
        match &step {
            SignatureStep::Field { ident, expr } => match self {
                Signature::Struct(fields) => fields
                    .get(&ident.ident)
                    .ok_or_else(|| LinearError::FieldNotFound {
                        ident: ident.clone(),
                    })
                    .cloned(),
                Signature::Callable {
                    input,
                    output,
                    in_out,
                    ..
                } => input
                    .get(&ident.ident)
                    .or_else(|| output.get(&ident.ident))
                    .or_else(|| in_out.get(&ident.ident))
                    .ok_or_else(|| LinearError::FieldNotFound {
                        ident: ident.clone(),
                    })
                    .cloned(),
                Signature::RefTo(inner) => inner.linear(step),
                Signature::Variable(var) => var.linear(step),
                Signature::Simple(spec) => Err(LinearError::SpecNotHaveField {
                    ident: ident.clone(),
                    spec: *spec,
                }),
                _ => Err(LinearError::CanNotHaveField {
                    ident: ident.clone(),
                }),
            },
            SignatureStep::Index { expr } => match self {
                Signature::Array { spec } => Ok(spec.clone()),
                Signature::Variable(var) => var.linear(step),
                _ => Err(LinearError::NotAnArray { expr: *expr }),
            },
            SignatureStep::Deref { .. } => match self {
                Signature::RefTo(inner) => Ok(inner).cloned(),
                _ => Err(LinearError::NotAReference),
            },
        }
    }

    fn as_callable(&'db self) -> Option<Callable<'db>> {
        match self {
            Signature::Callable {
                input,
                output,
                in_out,
                return_type,
                ..
            } => Some(Callable {
                inputs: input.clone(),
                in_outs: in_out.clone(),
                outputs: output.clone(),
                return_type: return_type.clone(),
            }),
            _ => None,
        }
    }
}
