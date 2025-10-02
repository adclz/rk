use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    check::errors::path_error::PathResolveError,
    hir_def::{
        expressions::{
            expression::Expr,
            spec::{ElementarySpec, Spec, SpecKind, StructElement},
        },
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
    },
    hir_ty::{name_res::resolve_namespace_access, ty_path_expr_resolver::PathExprWalkStep},
    {AstId, HirNodeInfo, TypeInfo},
};

#[salsa::tracked(debug)]
pub struct Ty<'db> {
    // Where the type is declared (POU, Variable, Method)
    pub decl: TyDecl<'db>,
    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.decl(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.decl(db).scope_id(db)
    }
}

// Declaration of the type (POU, Variable, Method)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDecl<'db> {
    Pou(PouDecl<'db>),
    Variable(VariableDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
    StructElement(StructElement<'db>),
}

impl<'db> TyDecl<'db> {
    pub fn decl_as_ty(&self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        match self {
            TyDecl::Pou(pou) => ty_for_pou(db, *pou),
            TyDecl::Variable(variable) => ty_for_variable(db, *variable),
            TyDecl::Method(method) => ty_for_method_decl(db, *method),
            TyDecl::MethodProt(method) => ty_for_method_prot(db, *method),
            TyDecl::StructElement(element) => ty_for_struct_field(db, *element),
        }
    }

    pub fn span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_span(db),
            TyDecl::Variable(variable) => variable.get_span(db),
            TyDecl::Method(method) => method.get_span(db),
            TyDecl::MethodProt(method) => method.get_span(db),
            TyDecl::StructElement(element) => element.get_span(db),
        }
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_name_span(db),
            TyDecl::Variable(variable) => variable.get_name_span(db),
            TyDecl::Method(method) => method.get_name_span(db),
            TyDecl::MethodProt(method) => method.get_name_span(db),
            TyDecl::StructElement(element) => element.get_name_span(db),
        }
        .unwrap_or_else(|| panic!("All TyDecl variants should have a name span: {self:?}"))
    }

    pub fn name(&self, db: &'db dyn BaseDatabase) -> Ident {
        match self {
            TyDecl::Pou(pou) => *pou.name(db),
            TyDecl::Variable(variable) => *variable.name(db),
            TyDecl::Method(method) => *method.name(db),
            TyDecl::MethodProt(method) => *method.name(db),
            TyDecl::StructElement(element) => *element.name(db),
        }
    }

    pub fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDecl::Pou(pou) => pou.get_id(db),
            TyDecl::Variable(variable) => variable.get_id(db),
            TyDecl::Method(method) => method.get_id(db),
            TyDecl::MethodProt(method) => method.get_id(db),
            TyDecl::StructElement(element) => element.get_id(db),
        }
    }

    pub fn scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDecl::Pou(pou) => pou.scope_id(db),
            TyDecl::Variable(variable) => variable.scope_id(db),
            TyDecl::Method(method) => method.scope_id(db),
            TyDecl::MethodProt(method) => method.scope_id(db),
            TyDecl::StructElement(element) => element.scope_id(db),
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
    RefTo(Ty<'db>),
    Target(Ty<'db>),
    Array {
        ranges: Vec<(Expr<'db>, Expr<'db>)>,
        typ: Ty<'db>,
    },
    ArrayConformand {
        typ: Ty<'db>,
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
        variables: IndexMap<Ident, Ty<'db>>,
        methods: Vec<MethodDecl<'db>>,
    },

    Function {
        variables: IndexMap<Ident, Ty<'db>>,
        return_type: Option<Ty<'db>>,
    },

    FunctionBlock {
        extends: Option<Ty<'db>>,
        variables: IndexMap<Ident, Ty<'db>>,
        methods: Vec<MethodDecl<'db>>,
    },

    Method {
        is_prototype: bool,
        variables: IndexMap<Ident, Ty<'db>>,
    },

    // Error variants
    Unresolved(SpanNamespaceAccess<'db>),
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
            let mut variables = IndexMap::default();

            for v in func.variables(db) {
                let decl = TyDecl::Variable(*v);
                match v.kind(db) {
                    VariableKind::Input => {
                        variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
                    }
                    VariableKind::Output => {
                        variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
                    }
                    VariableKind::InOut => {
                        variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
                    }
                    _ => continue,
                };
            }

            Ty::new(
                db,
                decl,
                def,
                TyKind::Function {
                    variables,
                    return_type: func.return_type(db).map(|rt| rt.to_ty(db, decl)).copied(),
                },
            )
        }
        Pou::FunctionBlock(fb) => {
            let mut variables = IndexMap::default();

            for variable in fb.variables(db) {
                let decl = TyDecl::Variable(*variable);
                match variable.kind(db) {
                    VariableKind::Input => {
                        variables.insert(*variable.name(db), *variable.spec(db).to_ty(db, decl));
                    }
                    VariableKind::Output => {
                        variables.insert(*variable.name(db), *variable.spec(db).to_ty(db, decl));
                    }
                    VariableKind::InOut => {
                        variables.insert(*variable.name(db), *variable.spec(db).to_ty(db, decl));
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
                    None => Ty::new(db, decl, TyDef::Invalid, TyKind::Unresolved(*e)),
                }
            });

            let methods = fb.methods(db).to_vec();

            Ty::new(
                db,
                decl,
                def,
                TyKind::FunctionBlock {
                    extends,
                    variables,
                    methods,
                },
            )
        }
        Pou::DataType(dt) => *dt.spec(db).to_ty(db, TyDecl::Pou(pou)),
        Pou::Class(class) => {
            let mut class_variables = IndexMap::default();
            let mut class_methods = vec![];

            for v in class.variables(db) {
                let decl = TyDecl::Variable(*v);
                class_variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
            }

            for m in class.methods(db) {
                class_methods.push(*m);
            }

            let extends =
                class
                    .extends(db)
                    .map(|e| match resolve_namespace_access(db, e.scope_id, e.path) {
                        Some(pou) => {
                            let ty = ty_for_pou(db, pou);
                            Ty::new(db, decl, ty.def(db), TyKind::Target(ty))
                        }
                        None => Ty::new(db, decl, TyDef::Invalid, TyKind::Unresolved(*e)),
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
                        None => Ty::new(db, decl, TyDef::Invalid, TyKind::Unresolved(*interface)),
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
                                None => Ty::new(
                                    db,
                                    decl,
                                    TyDef::Invalid,
                                    TyKind::Unresolved(*interface),
                                ),
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

    let mut variables = IndexMap::default();

    for v in method.variables(db) {
        let decl = TyDecl::Variable(*v);
        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
            }
            VariableKind::Output => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
            }
            VariableKind::InOut => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
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
            variables,
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

    let mut variables = IndexMap::default();

    for v in method.variables(db) {
        let decl = TyDecl::Variable(*v);

        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
            }
            VariableKind::Output => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
            }
            VariableKind::InOut => {
                variables.insert(*v.name(db), *v.spec(db).to_ty(db, decl));
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
            variables,
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
    *variable.spec(db).to_ty(db, TyDecl::Variable(variable))
}

fn struct_field_ty_result<'db>(db: &'db dyn BaseDatabase, field: StructElement<'db>) -> Ty<'db> {
    Ty::new(
        db,
        TyDecl::StructElement(field),
        TyDef::Invalid,
        TyKind::Recursive,
    )
}

#[salsa::tracked(cycle_result = struct_field_ty_result)]
pub fn ty_for_struct_field<'db>(db: &'db dyn BaseDatabase, field: StructElement<'db>) -> Ty<'db> {
    *field.spec(db).to_ty(db, TyDecl::StructElement(field))
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn linear(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
    ) -> Result<Ty<'db>, PathResolveError<'db>> {
        match &step {
            PathExprWalkStep::Field { ident, expr } => match self.kind(db) {
                TyKind::Struct { elements, spec } => elements
                    .get(&ident.ident)
                    .ok_or(PathResolveError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    })
                    .cloned(),
                TyKind::Function { variables, .. } => variables
                    .get(&ident.ident)
                    .ok_or(PathResolveError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    })
                    .cloned(),
                TyKind::FunctionBlock {
                    extends, variables, ..
                } => variables
                    .get(&ident.ident)
                    .ok_or(PathResolveError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    })
                    .cloned(),
                TyKind::RefTo(inner) => inner.linear(db, step),
                _ => Err(PathResolveError::UnknownField {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array { typ, .. } => Ok(*typ),
                _ => Err(PathResolveError::NotAnArray {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Deref { expr } => match self.kind(db) {
                TyKind::RefTo(inner) => Ok(*inner),
                _ => Err(PathResolveError::NotAReference {
                    expr: *expr,
                    ty: *self,
                }),
            },
        }
    }

    pub fn variables(&self, db: &'db dyn BaseDatabase) -> Option<&'db IndexMap<Ident, Ty<'db>>> {
        match self.kind(db) {
            TyKind::Target(inner) => inner.variables(db),
            TyKind::Function { variables, .. } => Some(variables),
            TyKind::FunctionBlock { variables, .. } => Some(variables),
            TyKind::Class { variables, .. } => Some(variables),
            TyKind::Method { variables, .. } => Some(variables),
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
            TyKind::Function { .. } | TyKind::FunctionBlock { .. } | TyKind::Method { .. }
        )
    }

    pub fn is_direct_type(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Target(_))
    }

    pub fn is_method_prototype(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_method_prototype(db);
        };
        if let TyKind::Method { is_prototype, .. } = self.kind(db) {
            return *is_prototype;
        }
        false
    }

    pub fn is_unresolved(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_unresolved(db);
        };
        matches!(self.kind(db), TyKind::Unresolved(_))
    }

    pub fn is_recursive(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_recursive(db);
        };
        matches!(self.kind(db), TyKind::Recursive)
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn is_variable(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.decl(db), TyDecl::Variable(_))
    }

    pub fn is_variable_input(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::Input;
        }
        false
    }

    pub fn is_variable_inout(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::InOut;
        }
        false
    }

    pub fn is_variable_output(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::Output;
        }
        false
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.has_return_type(db);
        };
        if let TyKind::Function { return_type, .. } = self.kind(db) {
            return *return_type;
        }
        None
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn to_ty(self, db: &'db dyn BaseDatabase, origin: TyDecl<'db>) -> Ty<'db> {
        match self.kind(db) {
            SpecKind::Array(array) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::Array {
                    typ: *array.of_type.to_ty(db, origin),
                    ranges: array
                        .subranges
                        .iter()
                        .map(|(start, end)| (*start, *end))
                        .collect(),
                },
            ),
            SpecKind::Enum(enum_spec) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::Enum {
                    typ: enum_spec.typ.as_ref().map(|t| t.to_ty(db, origin)).copied(),
                    list: enum_spec.variants.iter().map(|v| v.name).collect(),
                },
            ),
            SpecKind::Subrange(subrange) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::SubRange(*subrange._type),
            ),
            SpecKind::Struct(fields) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::Struct {
                    spec: self,
                    elements: fields
                        .elements
                        .iter()
                        .map(|element| {
                            (*element.name(db), {
                                *element.spec(db).to_ty(db, TyDecl::StructElement(*element))
                            })
                        })
                        .collect(),
                },
            ),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), target.path) {
                    Some(pou) => {
                        let ty = ty_for_pou(db, pou);
                        Ty::new(db, origin, ty.def(db), TyKind::Target(ty))
                    }
                    None => Ty::new(db, origin, TyDef::Spec(self), TyKind::Unresolved(*target)),
                }
            }
            SpecKind::Simple(simple) => {
                Ty::new(db, origin, TyDef::Spec(self), TyKind::Simple(*simple))
            }
            SpecKind::ArrayConformand(array) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::ArrayConformand {
                    typ: *array.to_ty(db, origin),
                },
            ),
            SpecKind::Ref(_ref) => Ty::new(
                db,
                origin,
                TyDef::Spec(self),
                TyKind::RefTo(*_ref.to_ty(db, origin)),
            ),
        }
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> &'static str {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Target(t) => t.type_name(db),
            TyKind::Enum { .. } => "ENUM",
            TyKind::SubRange(_) => "SUBRANGE",
            TyKind::RefTo(_) => "REF_TO",
            TyKind::Array { .. } => "ARRAY",
            TyKind::ArrayConformand { .. } => "ARRAY*",
            TyKind::Struct { .. } => "STRUCT",
            TyKind::Interface { .. } => "INTERFACE",
            TyKind::Class { .. } => "CLASS",
            TyKind::Function { .. } => "FUNCTION",
            TyKind::FunctionBlock { .. } => "FUNCTION_BLOCK",
            TyKind::Method { .. } => "METHOD",
            TyKind::Unresolved(_) => "{unknown}",
            TyKind::Recursive => "{recursive}",
        }
    }
}
