use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, HoverContents, Location, MarkupContent, MarkupKind,
        request::GotoDeclarationResponse,
    },
};
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{path_expr::PathExprError, sem_errors::AnalysisError, stmt::StmtError}, hir_def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        modifier::Modifier,
        pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
    }, hir_ty::{name_res::resolve_namespace_access, ty_path_expr_resolver::PathExprWalkStep}, to_proto::{AstId, ToProto}
};

#[salsa::tracked(debug)]
pub struct Ty<'db> {
    // Where the type is declared (POU, Variable, Method)
    pub decl: TyDecl<'db>,
    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    #[tracked]
    #[no_eq]
    pub kind: TyKind<'db>,
}

impl<'db> ToProto<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.decl(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.decl(db).scope_id(db)
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.decl(db).scope_id(db).file(db).url(db).clone(),
            self.decl(db).span(db).into(),
        )))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.def(db) {
            TyDef::Pou(pou) => Some(GotoDefinitionResponse::Scalar(Location::new(
                pou.get_scope_id(db).file(db).url(db).clone(),
                pou.get_span(db).into(),
            ))),
            TyDef::Method(method) => Some(GotoDefinitionResponse::Scalar(Location::new(
                method.get_scope_id(db).file(db).url(db).clone(),
                method.get_span(db).into(),
            ))),
            TyDef::MethodProt(method) => Some(GotoDefinitionResponse::Scalar(Location::new(
                method.get_scope_id(db).file(db).url(db).clone(),
                method.get_span(db).into(),
            ))),
            TyDef::Spec(spec) => Some(GotoDefinitionResponse::Scalar(Location::new(
                spec.scope_id(db).file(db).url(db).clone(),
                spec.get_span(db).into(),
            ))),
            TyDef::Invalid => None,
        }
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("{:?}", self.kind(db)),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}

impl<'db> Ty<'db> {
    pub fn as_err(&self, db: &'db dyn BaseDatabase) -> Option<AnalysisError<'db>> {
        match self.kind(db) {
            TyKind::Unresolved(path) => todo!(),
            TyKind::Recursive => Some(AnalysisError::StmtError(StmtError::RecursiveType {
                ty: *self,
            })),
            _ => None,
        }
    }
}

// Declaration of the type (POU, Variable, Method)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDecl<'db> {
    Pou(PouDecl<'db>),
    Variable(VariableDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
}

impl<'db> TyDecl<'db> {
    pub fn decl_as_ty(&self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        match self {
            TyDecl::Pou(pou) => ty_for_pou(db, *pou),
            TyDecl::Variable(variable) => ty_for_variable(db, *variable),
            TyDecl::Method(method) => ty_for_method_decl(db, *method),
            TyDecl::MethodProt(method) => ty_for_method_prot(db, *method),
        }
    }

    pub fn span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_span(db),
            TyDecl::Variable(variable) => variable.get_span(db),
            TyDecl::Method(method) => method.get_span(db),
            TyDecl::MethodProt(method) => method.get_span(db),
        }
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_name_span(db),
            TyDecl::Variable(variable) => variable.get_name_span(db),
            TyDecl::Method(method) => method.get_name_span(db),
            TyDecl::MethodProt(method) => method.get_name_span(db),
        }
        .expect(&format!(
            "All TyDecl variants should have a name span: {:?}",
            self
        ))
    }

    pub fn name(&self, db: &'db dyn BaseDatabase) -> Ident {
        match self {
            TyDecl::Pou(pou) => *pou.name(db),
            TyDecl::Variable(variable) => *variable.name(db),
            TyDecl::Method(method) => *method.name(db),
            TyDecl::MethodProt(method) => *method.name(db),
        }
    }

    pub fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDecl::Pou(pou) => pou.get_id(db),
            TyDecl::Variable(variable) => variable.get_id(db),
            TyDecl::Method(method) => method.get_id(db),
            TyDecl::MethodProt(method) => method.get_id(db),
        }
    }

    pub fn scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDecl::Pou(pou) => pou.scope_id(db),
            TyDecl::Variable(variable) => variable.scope_id(db),
            TyDecl::Method(method) => method.scope_id(db),
            TyDecl::MethodProt(method) => method.scope_id(db),
        }
    }
}

// Definition of the type (POU or Spec)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDef<'db> {
    Pou(PouDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
    Spec(Spec<'db>),
    Invalid,
}

impl<'db> TyDef<'db> {
    pub fn def_as_ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self {
            Self::Pou(pou) => Some(ty_for_pou(db, *pou)),
            Self::Method(method) => Some(ty_for_method_decl(db, *method)),
            Self::MethodProt(method) => Some(ty_for_method_prot(db, *method)),
            _ => None,
        }
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self {
            TyDef::Method(m) => m.modifier(db),
            TyDef::Pou(p) => p.modifier(db),
            _ => Modifier::empty(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !matches!(self, TyDef::Invalid)
    }

    pub fn is_pou(&self) -> bool {
        matches!(self, TyDef::Pou(_))
    }

    pub fn is_method(&self) -> bool {
        matches!(self, TyDef::Method(_))
    }

    pub fn is_spec(&self) -> bool {
        matches!(self, TyDef::Spec(_))
    }

    pub fn get_span(&self, db: &'db dyn BaseDatabase) -> Option<Span> {
        match self {
            TyDef::Pou(pou) => Some(pou.get_span(db)),
            TyDef::Method(method) => Some(method.get_span(db)),
            TyDef::MethodProt(method) => Some(method.get_span(db)),
            TyDef::Spec(spec) => Some(spec.get_span(db)),
            TyDef::Invalid => None,
        }
    }

    pub fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> Option<FileScopeId<'db>> {
        match self {
            TyDef::Pou(pou) => Some(pou.scope_id(db)),
            TyDef::Method(method) => Some(method.scope_id(db)),
            TyDef::MethodProt(method) => Some(method.scope_id(db)),
            TyDef::Spec(spec) => Some(spec.scope_id(db)),
            TyDef::Invalid => None,
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

    Interface {
        implements: Vec<Ty<'db>>,
        methods: Vec<MethodPrototype<'db>>,
    },

    Class {
        extends: Option<Ty<'db>>,
        implements: Vec<Ty<'db>>,
        variables: FxHashMap<Ident, Ty<'db>>,
        methods: Vec<MethodDecl<'db>>,
    },

    RefTo(Ty<'db>),
    Target(Ty<'db>),

    // Could either be Function or Method
    Function {
        input: FxHashMap<Ident, Ty<'db>>,
        output: FxHashMap<Ident, Ty<'db>>,
        in_out: FxHashMap<Ident, Ty<'db>>,
        return_type: Option<Ty<'db>>,
    },

    FunctionBlock {
        extends: Option<Ty<'db>>,
        inputs: FxHashMap<Ident, Ty<'db>>,
        outputs: FxHashMap<Ident, Ty<'db>>,
        in_outs: FxHashMap<Ident, Ty<'db>>,
    },

    Method {
        is_prototype: bool,
        input: FxHashMap<Ident, Ty<'db>>,
        output: FxHashMap<Ident, Ty<'db>>,
        in_out: FxHashMap<Ident, Ty<'db>>,
    },

    // Error variants
    Unresolved(NamespaceAccess),
    Recursive,
}

fn pou_ty_result<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    Ty::new(db, TyDecl::Pou(pou), TyDef::Invalid, TyKind::Recursive)
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked(cycle_result = pou_ty_result)]
pub fn ty_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    let decl = TyDecl::Pou(pou);
    let def = TyDef::Pou(pou);

    match pou.pou(db) {
        Pou::Function(func) => {
            let mut inputs = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();

            for v in func.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
                    }
                    VariableKind::Output => {
                        outputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
                    }
                    _ => continue,
                };
            }

            Ty::new(
                db,
                decl,
                def,
                TyKind::Function {
                    input: inputs,
                    output: outputs,
                    in_out: in_outs,
                    return_type: func.return_type(db).map(|rt| rt.to_ty(db, decl)),
                },
            )
        }
        Pou::FunctionBlock(fb) => {
            let mut inputs = FxHashMap::default();
            let mut outputs = FxHashMap::default();
            let mut in_outs = FxHashMap::default();

            for variable in fb.variables(db) {
                match variable.kind(db) {
                    VariableKind::Input => {
                        inputs.insert(*variable.name(db), variable.spec(db).to_ty(db, decl));
                    }
                    VariableKind::Output => {
                        outputs.insert(*variable.name(db), variable.spec(db).to_ty(db, decl));
                    }
                    VariableKind::InOut => {
                        in_outs.insert(*variable.name(db), variable.spec(db).to_ty(db, decl));
                    }
                    _ => continue,
                };
            }

            let extends = fb.extends(db).map(|e| {
                match resolve_namespace_access(db, e.get_scope_id(db), e.path) {
                    Some(pou) => {
                        let ty = ty_for_pou(db, pou);
                        Ty::new(db, decl, ty.def(db), TyKind::Target(ty))
                    }
                    None => Ty::new(db, decl, def, TyKind::Unresolved(e.path)),
                }
            });

            Ty::new(
                db,
                decl,
                def,
                TyKind::FunctionBlock {
                    extends,
                    inputs,
                    outputs,
                    in_outs,
                },
            )
        }
        Pou::DataType(dt) => dt.spec(db).to_ty(db, TyDecl::Pou(pou)),
        Pou::Class(class) => {
            let mut class_variables = FxHashMap::default();
            let mut class_methods = vec![];

            for v in class.variables(db) {
                class_variables.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }

            for m in class.methods(db) {
                class_methods.push(m);
            }

            let extends =
                class
                    .extends(db)
                    .map(|e| match resolve_namespace_access(db, e.scope_id, e.path) {
                        Some(pou) => {
                            let ty = ty_for_pou(db, pou);
                            Ty::new(db, decl, ty.def(db), TyKind::Target(ty))
                        }
                        None => Ty::new(db, decl, def, TyKind::Unresolved(e.path)),
                    });

            let implements = class
                .implements(db)
                .iter()
                .map(|interface| {
                    match resolve_namespace_access(db, interface.scope_id, interface.path) {
                        Some(pou) => {
                            let ty = ty_for_pou(db, pou);
                            Ty::new(db, decl, ty.def(db), TyKind::Target(ty))
                        }
                        None => Ty::new(db, decl, def, TyKind::Unresolved(interface.path)),
                    }
                })
                .collect();

            Ty::new(
                db,
                decl,
                def,
                TyKind::Class {
                    extends,
                    implements,
                    variables: class_variables,
                    methods: class_methods,
                },
            )
        }
        Pou::Interface(interface) => {
            let mut interface_methods = vec![];

            for m in interface.methods(db) {
                interface_methods.push(*m);
            }

            let implements = interface
                .extends(db)
                .map(|e| {
                    e.iter()
                        .map(|interface| {
                            match resolve_namespace_access(db, interface.scope_id, interface.path) {
                                Some(pou) => {
                                    let ty = ty_for_pou(db, pou);
                                    Ty::new(db, decl, ty.def(db), TyKind::Target(ty))
                                }
                                None => Ty::new(db, decl, def, TyKind::Unresolved(interface.path)),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ty::new(
                db,
                decl,
                def,
                TyKind::Interface {
                    implements,
                    methods: interface_methods,
                },
            )
        }
    }
}

fn method_ty_result<'db>(db: &'db dyn BaseDatabase, method: MethodDecl<'db>) -> Ty<'db> {
    Ty::new(
        db,
        TyDecl::Method(method),
        TyDef::Invalid,
        TyKind::Recursive,
    )
}

#[salsa::tracked(cycle_result = method_ty_result)]
pub fn ty_for_method_decl<'db>(db: &'db dyn BaseDatabase, method: MethodDecl<'db>) -> Ty<'db> {
    let decl = TyDecl::Method(method);
    let def = TyDef::Method(method);

    let mut inputs = FxHashMap::default();
    let mut outputs = FxHashMap::default();
    let mut in_outs = FxHashMap::default();

    for v in method.variables(db) {
        match v.kind(db) {
            VariableKind::Input => {
                inputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            VariableKind::Output => {
                outputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            VariableKind::InOut => {
                in_outs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            _ => continue,
        };
    }

    Ty::new(
        db,
        decl,
        def,
        TyKind::Method {
            is_prototype: false,
            input: inputs,
            output: outputs,
            in_out: in_outs,
        },
    )
}

fn method_prot_ty_result<'db>(db: &'db dyn BaseDatabase, method: MethodPrototype<'db>) -> Ty<'db> {
    Ty::new(
        db,
        TyDecl::MethodProt(method),
        TyDef::Invalid,
        TyKind::Recursive,
    )
}

#[salsa::tracked(cycle_result = method_prot_ty_result)]
pub fn ty_for_method_prot<'db>(db: &'db dyn BaseDatabase, method: MethodPrototype<'db>) -> Ty<'db> {
    let decl = TyDecl::MethodProt(method);
    let def = TyDef::MethodProt(method);

    let mut inputs = FxHashMap::default();
    let mut outputs = FxHashMap::default();
    let mut in_outs = FxHashMap::default();

    for v in method.variables(db) {
        match v.kind(db) {
            VariableKind::Input => {
                inputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            VariableKind::Output => {
                outputs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            VariableKind::InOut => {
                in_outs.insert(*v.name(db), v.spec(db).to_ty(db, decl));
            }
            _ => continue,
        };
    }

    Ty::new(
        db,
        decl,
        def,
        TyKind::Method {
            is_prototype: true,
            input: inputs,
            output: outputs,
            in_out: in_outs,
        },
    )
}

fn variable_ty_result<'db>(db: &'db dyn BaseDatabase, variable: VariableDecl<'db>) -> Ty<'db> {
    Ty::new(
        db,
        TyDecl::Variable(variable),
        TyDef::Invalid,
        TyKind::Recursive,
    )
}
#[salsa::tracked(cycle_result = variable_ty_result)]
pub fn ty_for_variable<'db>(db: &'db dyn BaseDatabase, variable: VariableDecl<'db>) -> Ty<'db> {
    variable.spec(db).to_ty(db, TyDecl::Variable(variable))
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn linear(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
    ) -> Result<Ty<'db>, PathExprError<'db>> {
        match &step {
            PathExprWalkStep::Field { ident, expr } => match self.kind(db) {
                TyKind::Struct { elements, spec } => elements
                    .get(&ident.ident)
                    .ok_or(PathExprError::UnknownField {
                        expr: *expr,
                        ty: *self,
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
                    .ok_or(PathExprError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    })
                    .cloned(),
                TyKind::FunctionBlock {
                    extends,
                    inputs,
                    outputs,
                    in_outs,
                } => inputs
                    .get(&ident.ident)
                    .or_else(|| outputs.get(&ident.ident))
                    .or_else(|| in_outs.get(&ident.ident))
                    .ok_or(PathExprError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    })
                    .cloned(),
                TyKind::RefTo(inner) => inner.linear(db, step),
                _ => Err(PathExprError::UnknownField {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array { type_signature } => Ok(type_signature),
                _ => Err(PathExprError::NotAnArray {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Deref { expr } => match self.kind(db) {
                TyKind::RefTo(inner) => Ok(inner),
                _ => Err(PathExprError::NotAReference {
                    expr: *expr,
                    ty: *self,
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
                ..
            } => {
                let all_inputs = inputs.clone();
                let all_outputs = outputs.clone();
                let all_in_outs = in_outs.clone();

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
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_simple(db);
        };
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self.decl(db) {
            TyDecl::Method(m) => m.modifier(db),
            TyDecl::Pou(f) => f.modifier(db),
            _ => Modifier::default(),
        }
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_callable(db);
        };
        matches!(
            self.kind(db),
            TyKind::Function { .. } | TyKind::FunctionBlock { .. }
        )
    }

    pub fn is_method_prototype(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_method_prototype(db);
        };
        if let TyKind::Method { is_prototype, .. } = self.kind(db) {
            return is_prototype;
        }
        false
    }

    pub fn is_invalid(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Unresolved(_) | TyKind::Recursive)
    }

    pub fn is_recursive(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Recursive)
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn is_variable(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.decl(db), TyDecl::Variable(_))
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.has_return_type(db);
        };
        if let TyKind::Function { return_type, .. } = self.kind(db) {
            return return_type;
        }
        None
    }
}

impl<'db> Spec<'db> {
    pub fn to_ty(&self, db: &'db dyn BaseDatabase, origin: TyDecl<'db>) -> Ty<'db> {
        match self.kind(db) {
            SpecKind::Array(array) => Ty::new(
                db,
                origin,
                TyDef::Spec(*self),
                TyKind::Array {
                    type_signature: array.of_type.to_ty(db, origin),
                },
            ),
            SpecKind::Enum(enum_spec) => Ty::new(
                db,
                origin,
                TyDef::Spec(*self),
                TyKind::Enum {
                    typ: enum_spec.typ.as_ref().map(|t| t.to_ty(db, origin)),
                    list: enum_spec.variants.iter().map(|v| v.name).collect(),
                },
            ),
            SpecKind::Subrange(subrange) => Ty::new(
                db,
                origin,
                TyDef::Spec(*self),
                TyKind::SubRange(*subrange._type),
            ),
            SpecKind::Struct(fields) => Ty::new(
                db,
                origin,
                TyDef::Spec(*self),
                TyKind::Struct {
                    spec: *self,
                    elements: fields
                        .elements
                        .iter()
                        .map(|element| (element.name, element.spec.to_ty(db, origin)))
                        .collect(),
                },
            ),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), *target) {
                    Some(pou) => {
                        let ty = ty_for_pou(db, pou);
                        Ty::new(db, origin, ty.def(db), TyKind::Target(ty))
                    }
                    None => Ty::new(db, origin, TyDef::Spec(*self), TyKind::Unresolved(*target)),
                }
            }
            SpecKind::Simple(simple) => {
                Ty::new(db, origin, TyDef::Spec(*self), TyKind::Simple(*simple))
            }
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
