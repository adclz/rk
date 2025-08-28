use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{Location, request::GotoDeclarationResponse},
};
use rustc_hash::FxHashMap;

use crate::{
    def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::{
            class::MethodDecl,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
    },
    to_proto::{AstId, ToProto},
    ty::{
        name_res::resolve_namespace_access,
        ty_path_expr_resolver::{PathExprWalkError, PathExprWalkStep},
    },
};

#[salsa::tracked(debug)]
pub struct Ty<'db> {
    pub origin: TyOrigin<'db>,

    #[tracked]
    #[no_eq]
    pub kind: TyKind<'db>,
}

impl<'db> ToProto<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.origin(db).get_id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.origin(db).scope_id(db)
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
        _sema: &'db crate::def::semantic_index::SemanticIndex<'db>,
    ) -> Option<GotoDeclarationResponse> {
        let span = match self.origin(db) {
            TyOrigin::FromPou(pou) => Location::new(
                pou.scope_id(db).file().url(db).clone(),
                pou.get_span(db).into(),
            ),
            TyOrigin::FromVariable(variable) => Location::new(
                variable.scope_id(db).file().url(db).clone(),
                variable.get_span(db).into(),
            ),
            TyOrigin::FromMethod(method) => Location::new(
                method.scope_id(db).file().url(db).clone(),
                method.get_span(db).into(),
            ),
        };

        Some(GotoDeclarationResponse::Scalar(span))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyOrigin<'db> {
    FromPou(PouDecl<'db>),
    FromVariable(VariableDecl<'db>),
    FromMethod(MethodDecl<'db>),
}

impl<'db> TyOrigin<'db> {
    pub fn span(&'db self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyOrigin::FromPou(pou) => pou.get_span(db),
            TyOrigin::FromVariable(variable) => variable.get_span(db),
            TyOrigin::FromMethod(method) => method.get_span(db),
        }
    }

    pub fn name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        match self {
            TyOrigin::FromPou(pou) => pou.get_name_span(db),
            TyOrigin::FromVariable(variable) => variable.get_name_span(db),
            TyOrigin::FromMethod(method) => method.get_name_span(db),
        }
    }

    pub fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyOrigin::FromPou(pou) => pou.get_id(db),
            TyOrigin::FromVariable(variable) => variable.get_id(db),
            TyOrigin::FromMethod(method) => method.get_id(db),
        }
    }

    pub fn scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        match self {
            TyOrigin::FromPou(pou) => pou.get_scope_id(db),
            TyOrigin::FromVariable(variable) => variable.get_scope_id(db),
            TyOrigin::FromMethod(method) => method.get_scope_id(db),
        }
    }

    pub fn name(&'db self, db: &'db dyn BaseDatabase) -> Ident {
        match self {
            TyOrigin::FromPou(pou) => *pou.name(db),
            TyOrigin::FromVariable(variable) => *variable.name(db),
            TyOrigin::FromMethod(method) => *method.name(db),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyKind<'db> {
    // Literal types
    Simple(ElementarySpec),
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

    Class {
        extends: Option<Ty<'db>>,
        implements: FxHashMap<Ident, Ty<'db>>,
        variables: FxHashMap<Ident, Ty<'db>>,
        methods: FxHashMap<Ident, Ty<'db>>,
    },

    Interface {
        implements: FxHashMap<Ident, Ty<'db>>,
        methods: FxHashMap<Ident, Ty<'db>>,
    },

    RefTo(Ty<'db>),

    // Callable types
    Function {
        input: FxHashMap<Ident, Ty<'db>>,
        ztatic: FxHashMap<Ident, Ty<'db>>,
        output: FxHashMap<Ident, Ty<'db>>,
        in_out: FxHashMap<Ident, Ty<'db>>,
        temp: FxHashMap<Ident, Ty<'db>>,
        return_type: Option<Ty<'db>>,
    },

    FunctionBlock {
        extends: Option<Ty<'db>>,
        inputs: FxHashMap<Ident, Ty<'db>>,
        outputs: FxHashMap<Ident, Ty<'db>>,
        in_outs: FxHashMap<Ident, Ty<'db>>,
        temps: FxHashMap<Ident, Ty<'db>>,
        ztatic: FxHashMap<Ident, Ty<'db>>,
    },

    // Error variants
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
                    VariableKind::Var => {
                        ztatic.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
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
                TyKind::Function {
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
                    VariableKind::Var => {
                        ztatic.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    VariableKind::Temp => {
                        temp.insert(*v.name(db), v.spec(db).to_sig(db, origin));
                    }
                    _ => continue,
                };
            }

            let extends = fb.extends(db).map(|e| {
                match resolve_namespace_access(db, e.scope_id.file(), e.scope_id, e.path) {
                    Some(pou) => ty_for_pou(db, pou),
                    None => Ty::new(db, origin, TyKind::Unresolved(e.path)),
                }
            });

            if let Some(extends) = &extends {
                if let TyKind::FunctionBlock {
                    inputs: ext_inputs,
                    outputs: ext_outputs,
                    in_outs: ext_in_outs,
                    temps: ext_temps,
                    ztatic: ext_ztatic,
                    ..
                } = extends.kind(db)
                {
                    for (name, ty) in ext_inputs {
                        inputs.insert(name, ty);
                    }
                    for (name, ty) in ext_outputs {
                        outputs.insert(name, ty);
                    }
                    for (name, ty) in ext_in_outs {
                        in_outs.insert(name, ty);
                    }
                    for (name, ty) in ext_temps {
                        temp.insert(name, ty);
                    }
                    for (name, ty) in ext_ztatic {
                        ztatic.insert(name, ty);
                    }
                };
                // need to check in the case of class inheritance for function blocks
            };

            Ty::new(
                db,
                origin,
                TyKind::FunctionBlock {
                    extends,
                    inputs,
                    outputs,
                    in_outs,
                    ztatic,
                    temps: temp,
                },
            )
        }
        Pou::DataType(dt) => dt.spec(db).to_sig(db, TyOrigin::FromPou(pou)),
        Pou::Class(class) => {
            let origin = TyOrigin::FromPou(pou);
            let mut class_variables = FxHashMap::default();
            let mut class_methods = FxHashMap::default();

            for v in class.variables(db) {
                class_variables.insert(*v.name(db), v.spec(db).to_sig(db, origin));
            }

            for m in class.methods(db) {
                class_methods.insert(*m.name(db), ty_for_method(db, m));
            }

            let extends = class.extends(db).map(|e| {
                match resolve_namespace_access(db, e.scope_id.file(), e.scope_id, e.path) {
                    Some(pou) => ty_for_pou(db, pou),
                    None => Ty::new(db, origin, TyKind::Unresolved(e.path)),
                }
            });

            // Class can only extends another class
            if let Some(extends) = extends {
                if let TyKind::Class {
                    extends,
                    implements,
                    variables,
                    methods,
                } = extends.kind(db)
                {
                    for m in class.methods(db) {
                        class_methods.insert(*m.name(db), ty_for_method(db, m));
                    }

                    for v in variables {
                        class_variables.insert(v.0, v.1);
                    }
                }
            }

            // There can be any number of inherited interfaces
            let mut implements = FxHashMap::default();

            for imple in class.implements(db) {
                let ty = match resolve_namespace_access(
                    db,
                    imple.scope_id.file(),
                    imple.scope_id,
                    imple.path,
                ) {
                    Some(pou) => ty_for_pou(db, pou),
                    None => Ty::new(db, origin, TyKind::Unresolved(imple.path)),
                };

                let name = imple.path.target(db);

                implements.insert(name.ident, ty);
            }

            Ty::new(
                db,
                origin,
                TyKind::Class {
                    extends,
                    implements,
                    variables: class_variables,
                    methods: class_methods,
                },
            )
        }
        Pou::Interface(interface) => {
            let origin = TyOrigin::FromPou(pou);

            let mut implements = FxHashMap::default();
            let mut interface_methods = FxHashMap::default();

            if let Some(ext) = interface.extends(db) {
                for imple in ext {
                    let ty = match resolve_namespace_access(
                        db,
                        imple.scope_id.file(),
                        imple.scope_id,
                        imple.path,
                    ) {
                        Some(pou) => ty_for_pou(db, pou),
                        None => Ty::new(db, origin, TyKind::Unresolved(imple.path)),
                    };

                    if let TyKind::Interface {
                        implements: ext_implements,
                        methods: ext_methods,
                    } = ty.kind(db)
                    {
                        for (name, method) in ext_methods {
                            interface_methods.insert(name, method);
                        }

                        for (name, imple) in ext_implements {
                            implements.insert(name, imple);
                        }
                    }

                    let name = imple.path.target(db);

                    implements.insert(name.ident, ty);
                }
            }

            Ty::new(
                db,
                origin,
                TyKind::Interface {
                    implements,
                    methods: interface_methods,
                },
            )
        }
    }
}

fn method_ty_result<'db>(db: &'db dyn BaseDatabase, method: MethodDecl<'db>) -> Ty<'db> {
    Ty::new(db, TyOrigin::FromMethod(method), TyKind::Recursive)
}

#[salsa::tracked(cycle_result = method_ty_result)]
pub fn ty_for_method<'db>(db: &'db dyn BaseDatabase, method: MethodDecl<'db>) -> Ty<'db> {
    let origin = TyOrigin::FromMethod(method);
    let mut inputs = FxHashMap::default();
    let mut outputs = FxHashMap::default();
    let mut in_outs = FxHashMap::default();
    let mut temp = FxHashMap::default();
    let mut ztatic = FxHashMap::default();

    for v in method.variables(db) {
        match v.kind(db) {
            VariableKind::Var => {
                ztatic.insert(*v.name(db), v.spec(db).to_sig(db, origin));
            }
            VariableKind::Input => {
                inputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
            }
            VariableKind::Output => {
                outputs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
            }
            VariableKind::InOut => {
                in_outs.insert(*v.name(db), v.spec(db).to_sig(db, origin));
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
        TyKind::Function {
            input: inputs,
            output: outputs,
            in_out: in_outs,
            ztatic,
            temp,
            return_type: method.return_type(db).map(|rt| rt.to_sig(db, origin)),
        },
    )
}

fn variable_ty_result<'db>(db: &'db dyn BaseDatabase, variable: VariableDecl<'db>) -> Ty<'db> {
    Ty::new(db, TyOrigin::FromVariable(variable), TyKind::Recursive)
}
#[salsa::tracked(cycle_result = variable_ty_result)]
pub fn ty_for_variable<'db>(db: &'db dyn BaseDatabase, variable: VariableDecl<'db>) -> Ty<'db> {
    variable
        .spec(db)
        .to_sig(db, TyOrigin::FromVariable(variable))
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn linear(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
    ) -> Result<Ty<'db>, PathExprWalkError<'db>> {
        match &step {
            PathExprWalkStep::Field { ident, expr } => match self.kind(db) {
                TyKind::Struct { elements, spec } => elements
                    .get(&ident.ident)
                    .ok_or_else(|| PathExprWalkError::FieldNotFound {
                        expr: *expr,
                        origin: self.origin(db),
                    })
                    .cloned(),
                TyKind::Function {
                    input,
                    output,
                    in_out,
                    ..
                } => input
                    .get(&ident.ident)
                    .or_else(|| output.get(&ident.ident))
                    .or_else(|| in_out.get(&ident.ident))
                    .ok_or_else(|| PathExprWalkError::FieldNotFound {
                        expr: *expr,
                        origin: self.origin(db),
                    })
                    .cloned(),
                TyKind::FunctionBlock {
                    extends,
                    inputs,
                    outputs,
                    in_outs,
                    temps,
                    ztatic,
                } => inputs
                    .get(&ident.ident)
                    .or_else(|| outputs.get(&ident.ident))
                    .or_else(|| in_outs.get(&ident.ident))
                    .or_else(|| temps.get(&ident.ident))
                    .or_else(|| ztatic.get(&ident.ident))
                    .ok_or_else(|| PathExprWalkError::FieldNotFound {
                        expr: *expr,
                        origin: self.origin(db),
                    })
                    .cloned(),
                TyKind::RefTo(inner) => inner.linear(db, step),
                _ => Err(PathExprWalkError::FieldNotFound {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array { type_signature } => Ok(type_signature),
                _ => Err(PathExprWalkError::NotAnArray {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
            PathExprWalkStep::Deref { expr } => match self.kind(db) {
                TyKind::RefTo(inner) => Ok(inner),
                _ => Err(PathExprWalkError::NotAReference {
                    expr: *expr,
                    origin: self.origin(db),
                }),
            },
        }
    }

    #[salsa::tracked]
    pub fn as_callable_signature(
        self,
        db: &'db dyn BaseDatabase,
    ) -> Option<CallableSignature<'db>> {
        match self.kind(db) {
            TyKind::Function {
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
            TyKind::FunctionBlock {
                inputs,
                outputs,
                in_outs,
                temps,
                ztatic,
                ..
            } => {
                let all_inputs = inputs.clone();
                let all_outputs = outputs.clone();
                let all_in_outs = in_outs.clone();
                let all_temps = temps.clone();
                let all_ztatic = ztatic.clone();

                Some(CallableSignature {
                    inputs: all_inputs,
                    in_outs: all_in_outs,
                    outputs: all_outputs,
                    return_type: None,
                })
            }
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
        matches!(self.kind(db), TyKind::Function { .. })
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
        if let TyKind::Function { return_type, .. } = self.kind(db) {
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
                match resolve_namespace_access(
                    db,
                    self.scope_id(db).file(),
                    self.scope_id(db),
                    *target,
                ) {
                    Some(pou) => ty_for_pou(db, pou),
                    None => Ty::new(db, origin, TyKind::Unresolved(*target)),
                }
            }
            SpecKind::Simple(simple) => Ty::new(db, origin, TyKind::Simple(*simple)),
            _ => todo!(),
        }
    }
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
