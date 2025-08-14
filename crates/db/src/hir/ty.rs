use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    hir::{
        expressions::{
            expression::PathExpr,
            spec::{Spec, SpecKind},
        },
        interned::{
            identifier::{Ident, SpannedIdent},
            namespace::NamespaceAccess,
        },
        pous::{
            pou::{Pou, PouDecl},
            variable::{Variable, VariableKind},
        },
        scopes::{scope::FileScopeId, solver::resolve_namespace_access},
    },
};

#[salsa::tracked(debug)]
pub struct Ty<'db> {
    pub origin: TyOrigin<'db>,

    #[tracked]
    #[no_eq]
    pub kind: TyKind<'db>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyOrigin<'db> {
    FromPou(PouDecl<'db>),
    FromVariable(Variable<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyKind<'db> {
    // Base types
    Simple(Spec<'db>),
    Enum {
        typ: Option<Ty<'db>>,
        list: Vec<Ident>,
    },
    SubRange(Spec<'db>),
    Array {
        type_signature: Ty<'db>,
    },
    Struct {
        spec: Spec<'db>,
        elements: FxHashMap<Ident, Ty<'db>>,
    },

    Callable {
        input: FxHashMap<Ident, Ty<'db>>,
        ztatic: FxHashMap<Ident, Ty<'db>>,
        output: FxHashMap<Ident, Ty<'db>>,
        in_out: FxHashMap<Ident, Ty<'db>>,
        temp: FxHashMap<Ident, Ty<'db>>,
        return_type: Option<Ty<'db>>,
    },

    RefTo(Ty<'db>),

    // Meta / error
    Unresolved(NamespaceAccess),
    Recursive,
}

fn pou_ty_result<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    Ty::new(db, TyOrigin::FromPou(pou), TyKind::Recursive)
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked(cycle_result = pou_ty_result)]
pub fn ty_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    match pou.pou(db) {
        Pou::Function(func) => {
            let origin = TyOrigin::FromPou(pou);
            let mut inputs = FxHashMap::default();
            let mut ztatic = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();
            let mut temp = FxHashMap::default();

            for v in func.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Local => {
                        ztatic.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Temp => {
                        temp.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    _ => continue,
                };
            }

            Ty::new(
                db,
                origin,
                TyKind::Callable {
                    input: inputs,
                    output: outputs,
                    in_out: in_outs,
                    ztatic,
                    temp,
                    return_type: func.return_type(db).map(|rt| rt.to_sig(db, origin)),
                },
            )
        }
        Pou::FunctionBlock(fb) => {
            let origin = TyOrigin::FromPou(pou);
            let mut inputs = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();
            let mut temp = FxHashMap::default();
            let mut ztatic = FxHashMap::default();

            for v in fb.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Local => {
                        ztatic.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Temp => {
                        temp.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    _ => continue,
                };
            }

            Ty::new(
                db,
                origin,
                TyKind::Callable {
                    input: inputs,
                    output: outputs,
                    in_out: in_outs,
                    ztatic,
                    temp,
                    return_type: None, // Function blocks don't have a return type
                },
            )
        }
        Pou::DataType(dt) => dt.spec(db).to_sig(db, TyOrigin::FromPou(pou)),
        Pou::Class(class) => {
            todo!()
        }
        Pou::Interface(interface) => {
            todo!()
        }
    }
}

fn variable_ty_result<'db>(db: &'db dyn BaseDatabase, variable: Variable<'db>) -> Ty<'db> {
    Ty::new(db, TyOrigin::FromVariable(variable), TyKind::Recursive)
}
#[salsa::tracked(cycle_result = variable_ty_result)]
pub fn ty_for_variable<'db>(db: &'db dyn BaseDatabase, variable: Variable<'db>) -> Ty<'db> {
    variable
        .spec(db)
        .to_sig(db, TyOrigin::FromVariable(variable))
}

impl<'db> Ty<'db> {
    pub fn linear(
        &self,
        db: &'db dyn BaseDatabase,
        step: &TyStep<'db>,
    ) -> Result<Ty<'db>, WalkError<'db>> {
        match &step {
            TyStep::Field { ident, expr } => match self.kind(db) {
                TyKind::Struct { elements, spec } => elements
                    .get(&ident.ident)
                    .ok_or_else(|| WalkError::FieldNotFound {
                        expr: *expr,
                        origin: self.origin(db),
                    })
                    .cloned(),
                TyKind::Callable {
                    input,
                    output,
                    in_out,
                    ..
                } => input
                    .get(&ident.ident)
                    .or_else(|| output.get(&ident.ident))
                    .or_else(|| in_out.get(&ident.ident))
                    .ok_or_else(|| WalkError::FieldNotFound {
                        expr: *expr,
                        origin: self.origin(db),
                    })
                    .cloned(),
                TyKind::RefTo(inner) => inner.linear(db, step),
                _ => Err(WalkError::FieldNotFound {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
            TyStep::Index { expr } => match self.kind(db) {
                TyKind::Array { type_signature } => Ok(type_signature),
                _ => Err(WalkError::NotAnArray {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
            TyStep::Deref { expr } => match self.kind(db) {
                TyKind::RefTo(inner) => Ok(inner),
                _ => Err(WalkError::NotAReference {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
        }
    }

    pub fn as_callable_signature(
        self,
        db: &'db dyn BaseDatabase,
    ) -> Option<CallableSignature<'db>> {
        match self.kind(db) {
            TyKind::Callable {
                input,
                output,
                in_out,
                return_type,
                ..
            } => Some(CallableSignature {
                inputs: input.clone(),
                in_outs: in_out.clone(),
                outputs: output.clone(),
                return_type,
            }),
            _ => None,
        }
    }

    pub fn is_simple(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::RefTo(sig) = self.kind(db) {
            return sig.is_simple(db);
        };
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::RefTo(sig) = self.kind(db) {
            return sig.is_callable(db);
        };
        matches!(self.kind(db), TyKind::Callable { .. })
    }

    pub fn is_invalid(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Unresolved(_) | TyKind::Recursive)
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::RefTo(sig) = self.kind(db) {
            return sig.has_return_type(db);
        };
        if let TyKind::Callable { return_type, .. } = self.kind(db) {
            return return_type.is_some();
        }
        false
    }
}

impl<'db> Spec<'db> {
    pub fn to_sig(&self, db: &'db dyn BaseDatabase, origin: TyOrigin<'db>) -> Ty<'db> {
        match self.kind(db) {
            SpecKind::Array(array) => Ty::new(
                db,
                origin,
                TyKind::Array {
                    type_signature: array.of_type.to_sig(db, origin),
                },
            ),
            SpecKind::Enum(enum_spec) => Ty::new(
                db,
                origin,
                TyKind::Enum {
                    typ: enum_spec.typ.as_ref().map(|t| t.to_sig(db, origin)),
                    list: enum_spec.variants.iter().map(|v| v.name).collect(),
                },
            ),
            SpecKind::Subrange(subrange) => Ty::new(db, origin, TyKind::SubRange(*subrange._type)),
            SpecKind::Struct(fields) => Ty::new(
                db,
                origin,
                TyKind::Struct {
                    spec: *self,
                    elements: fields
                        .elements
                        .iter()
                        .map(|element| (element.name, element.spec.to_sig(db, origin)))
                        .collect(),
                },
            ),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.file(db), self.scope_id(db), *target) {
                    Some(pou) => ty_for_pou(db, pou),
                    None => Ty::new(db, origin, TyKind::Unresolved(*target)),
                }
            }
            _ => Ty::new(db, origin, TyKind::Simple(*self)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyStep<'db> {
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

impl TyStep<'_> {
    pub fn get_expr(&self) -> &PathExpr<'_> {
        match self {
            TyStep::Field { expr, .. } => expr,
            TyStep::Index { expr } => expr,
            TyStep::Deref { expr } => expr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum WalkError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: FileScopeId,
    },
    FieldNotFound {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
    NotAnArray {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
    NotAReference {
        expr: PathExpr<'db>,
        origin: TyOrigin<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CallableSignature<'db> {
    pub inputs: FxHashMap<Ident, Ty<'db>>,
    pub in_outs: FxHashMap<Ident, Ty<'db>>,
    pub outputs: FxHashMap<Ident, Ty<'db>>,
    pub return_type: Option<Ty<'db>>,
}

impl<'db> CallableSignature<'db> {
    pub fn get_param(&self, ident: &Ident) -> Option<&Ty<'db>> {
        self.inputs
            .get(ident)
            .or_else(|| self.outputs.get(ident))
            .or_else(|| self.in_outs.get(ident))
    }

    pub fn to_completion_string(&self, db: &'db dyn BaseDatabase) -> String {
        let mut params = Vec::new();
        for (name, ty) in &self.inputs {
            params.push(format!("{} := n", name.text(db)));
        }
        for (name, ty) in &self.in_outs {
            params.push(format!("{} := n", name.text(db)));
        }
        for (name, ty) in &self.outputs {
            params.push(format!("{} => n", name.text(db)));
        }
        format!("({})", params.join(",\n"))
    }
}
