use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    AstId, HirNodeInfo, TypeInfo,
    check::errors::path_error::PathResolveError,
    hir_def::{
        expressions::{
            spec::{Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange},
        },
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::{Class},
            function::Function,
            function_block::FunctionBlock,
            interface::{Interface},
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
        visibility::Visibility,
    },
    hir_ty::{
        inheritance_solver::{MethodRef, method_table},
        name_res::resolve_namespace_access,
        ty_var_access_resolver::PathExprWalkStep,
    },
};

/// The  resolved type of a variable, POU, or method
/// [`Ty`] is the most fundamental unit of type information in the HIR.
#[salsa::tracked(debug)]
pub struct Ty<'db> {
    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    // The actual kind of the type
    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

impl<'db> Ty<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        self.def(db).name(db)
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self.def(db) {
            TyDef::Pou(pou) => pou.get_name_span(db).unwrap(),
            TyDef::MethodRef(m) => m.get_name_span(db).unwrap(),
            TyDef::Spec(spec) => spec.get_span(db),
        }
    }
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.def(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.def(db).get_scope_id(db)
    }
}

// Definition of the type (POU or Spec)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDef<'db> {
    Pou(PouDecl<'db>),
    MethodRef(MethodRef<'db>),
    Spec(Spec<'db>),
}

impl<'db> TyDef<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            TyDef::Pou(pou) => pou.name(db).text(db).to_string(),
            TyDef::MethodRef(m) => m.name(db).text(db).to_string(),
            TyDef::Spec(spec) => spec.shorthand(db),
        }
    }
    pub fn def_as_ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self {
            Self::Pou(pou) => Some(ty_for_pou(db, *pou)),
            _ => None,
        }
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self {
            TyDef::Pou(p) => p.modifier(db),
            _ => Modifier::empty(),
        }
    }

    pub fn is_pou(&self) -> bool {
        matches!(self, TyDef::Pou(_))
    }

    pub fn is_spec(&self) -> bool {
        matches!(self, TyDef::Spec(_))
    }

    pub fn get_span(&self, db: &'db dyn BaseDatabase) -> Option<Span> {
        match self {
            TyDef::Pou(pou) => Some(pou.get_span(db)),
            TyDef::MethodRef(m) => Some(m.get_span(db)),
            TyDef::Spec(spec) => Some(spec.get_span(db)),
        }
    }
}

impl<'db> HirNodeInfo<'db> for TyDef<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDef::Pou(pou) => pou.get_id(db),
            TyDef::MethodRef(m) => m.get_id(db),
            TyDef::Spec(spec) => spec.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDef::Pou(pou) => pou.get_scope_id(db),
            TyDef::MethodRef(m) => m.get_scope_id(db),
            TyDef::Spec(spec) => spec.get_scope_id(db),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyKind<'db> {
    // Specs
    Simple(ElementarySpec),
    Enum(Enum<'db>),
    SubRange(SubRange<'db>),
    RefTo(Spec<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>), // todo
    Struct(Struct<'db>),

    // Pous
    Interface(Interface<'db>),
    Class(Class<'db>),
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    MethodRef(MethodRef<'db>),

    // Error variants
    Unresolved(SpanNamespaceAccess<'db>),
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked]
pub fn ty_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    let def = TyDef::Pou(pou);

    match pou.pou(db) {
        Pou::Function(func) => Ty::new(db, def, TyKind::Function(*func)),
        Pou::FunctionBlock(fb) => Ty::new(db, def, TyKind::FunctionBlock(*fb)),
        Pou::DataType(dt) => dt.spec(db).spec_to_ty(db),
        Pou::Class(class) => Ty::new(db, def, TyKind::Class(*class)),
        Pou::Interface(interface) => Ty::new(db, def, TyKind::Interface(*interface)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchMode {
    Local,
    Global,
}

impl<'db> Struct<'db> {
    pub fn resolve_elements(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> FxHashMap<Ident, StructElement<'db>> {
        self.elements
            .iter()
            .map(|element| (*element.name(db), *element))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
        search_mode: SearchMode,
    ) -> Result<Ty<'db>, PathResolveError<'db>> {
        match &step {
            PathExprWalkStep::Field { ident, expr } => {
                // Try different lookup strategies in order, falling through to the next if not found

                // first, try struct fields (highest priority)
                if let TyKind::Struct(ztruct) = self.kind(db) {
                    let elements = ztruct.resolve_elements(db);
                    if let Some(field) = elements.get(&ident.ident) {
                        return Ok(field.spec(db).spec_to_ty(db));
                    }
                }

                // try methods for Class and FunctionBlock types
                match self.kind(db) {
                    TyKind::Class { .. } | TyKind::FunctionBlock { .. } => {
                        if let Some(method) =
                            method_table(db, *self).declared_methods.get(&ident.ident)
                        {
                            return Ok(method.to_ty(db));
                        }
                    }
                    _ => {}
                }

                // arrays cannot have fields (return error immediately)
                if matches!(self.kind(db), TyKind::Array { .. }) {
                    return Err(PathResolveError::TypeHasNoField {
                        ty: *self,
                        expr: *expr,
                    });
                }

                if matches!(self.kind(db), TyKind::RefTo(_)) {
                    return Err(PathResolveError::MissingDeref {
                        ty: *self,
                        expr: *expr,
                    });
                }

                // last resort: look for variables
                match search_mode {
                    SearchMode::Global => self.all_variables(db).get(&ident.ident).cloned(),
                    SearchMode::Local => self.local_variables(db).get(&ident.ident).cloned(),
                }
                .ok_or(PathResolveError::UnknownField {
                    expr: *expr,
                    ty: *self,
                })
            }
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array(array) => Ok(array.of_type.spec_to_ty(db)),
                _ => Err(PathResolveError::NotAnArray {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Deref { expr, target } => {
                let target = match self.kind(db) {
                    // Look for a struct field
                    TyKind::Struct(ztruct) => ztruct
                        .resolve_elements(db)
                        .get(&target.ident)
                        .map(|f| f.spec(db).spec_to_ty(db))
                        .ok_or(PathResolveError::UnknownField {
                            expr: *expr,
                            ty: *self,
                        }),
                    // Otherwise, look for a variable
                    _ => match search_mode {
                        SearchMode::Global => self.all_variables(db).get(&target.ident).cloned(),
                        SearchMode::Local => self.local_variables(db).get(&target.ident).cloned(),
                    }
                    .ok_or(PathResolveError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    }),
                }?;

                match target.kind(db) {
                    TyKind::RefTo(inner) => Ok(inner.spec_to_ty(db)),
                    _ => Err(PathResolveError::NotAReference {
                        expr: *expr,
                        ty: *self,
                    }),
                }
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn inheritors(self, db: &'db dyn BaseDatabase) -> Vec<Ty<'db>> {
        match self.kind(db) {
            TyKind::Class(class) => {
                let mut inheritors = vec![];
                if let Some(base) = class.extends(db)
                    && let Some(base) = resolve_namespace_access(db, base.scope_id, base.path)
                {
                    inheritors.push(ty_for_pou(db, base));
                }
                for iface in class.implements(db) {
                    resolve_namespace_access(db, iface.scope_id, iface.path).map(|iface| {
                        inheritors.push(ty_for_pou(db, iface));
                    });
                }
                inheritors
            }
            TyKind::Interface(interface) => {
                let mut inheritors = vec![];
                if let Some(extends) = interface.extends(db) {
                    for iface in extends {
                        resolve_namespace_access(db, iface.scope_id, iface.path).map(|iface| {
                            inheritors.push(ty_for_pou(db, iface));
                        });
                    }
                }
                inheritors
            }
            TyKind::FunctionBlock(fb) => {
                let mut inheritors = vec![];
                if let Some(base) = fb.extends(db)
                    && let Some(base) = resolve_namespace_access(db, base.scope_id, base.path)
                {
                    inheritors.push(ty_for_pou(db, base));
                }
                inheritors
            }
            _ => vec![],
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn all_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        match self.kind(db) {
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_all_variables(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_all_variables(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                _ => Default::default(),
            },
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        match self.kind(db) {
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_local_variables(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_local_variables(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                _ => Default::default(),
            },
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn local_variables2(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self.kind(db) {
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_local_variables_2(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_local_variables_2(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                _ => Default::default(),
            },
        }
    }

    fn fetch_local_variables_2(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, VariableDecl<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            match v.kind(db) {
                VariableKind::Input => {
                    variables.insert(*v.name(db), *v);
                }
                VariableKind::Output => {
                    variables.insert(*v.name(db), *v);
                }
                VariableKind::InOut => {
                    variables.insert(*v.name(db), *v);
                }
                _ => continue,
            };
        }
        variables
    }

    fn fetch_local_variables(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, Ty<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            match v.kind(db) {
                VariableKind::Input => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                VariableKind::Output => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                VariableKind::InOut => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                _ => continue,
            };
        }
        variables
    }

    fn fetch_all_variables(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, Ty<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
        }
        variables
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Option<Visibility> {
        match self.def(db) {
            TyDef::MethodRef(method) => Some(method.visibility(db)),
            _ => None,
        }
    }

    pub fn is_simple(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind(db),
            TyKind::Function { .. } | TyKind::FunctionBlock { .. }
        )
    }

    pub fn is_unresolved(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Unresolved(_))
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self.kind(db) {
            TyKind::Function(f) => f.return_type(db).map(|rt| rt.spec_to_ty(db)),
            TyKind::MethodRef(m) => m.return_type(db).map(|rt| rt.spec_to_ty(db)),
            _ => None,
        }
    }
}

// temporary, until we have proper method declarations
#[salsa::tracked]
impl<'db> MethodRef<'db> {
    pub fn to_ty(self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        Ty::new(db, TyDef::MethodRef(self), TyKind::MethodRef(self))
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked]
    pub fn spec_to_ty(self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        let kind = match self.kind(db) {
            SpecKind::Array(array) => TyKind::Array(array.clone()),
            SpecKind::Enum(enum_spec) => TyKind::Enum(enum_spec.clone()),
            SpecKind::Subrange(subrange) => TyKind::SubRange(subrange.clone()),
            SpecKind::Struct(ztruct) => TyKind::Struct(ztruct.clone()),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), target.path) {
                    Some(pou) => ty_for_pou(db, pou).kind(db).clone(),
                    None => TyKind::Unresolved(*target),
                }
            }
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand(*array),
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
        };
        Ty::new(db, TyDef::Spec(self), kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Enum { .. } => "ENUM".into(),
            TyKind::SubRange { .. } => "SUBRANGE".into(),
            TyKind::RefTo(ref_) => format!("REF_TO {}", ref_.spec_to_ty(db).type_name(db)),
            TyKind::Array { .. } => "ARRAY".into(),
            TyKind::ArrayConformand { .. } => "ARRAY*".into(),
            TyKind::Struct { .. } => "STRUCT".into(),
            TyKind::Interface { .. } => "INTERFACE".into(),
            TyKind::Class { .. } => "CLASS".into(),
            TyKind::Function { .. } => "FUNCTION".into(),
            TyKind::FunctionBlock { .. } => "FUNCTION_BLOCK".into(),
            TyKind::MethodRef(m) => "METHOD".to_string(),
            TyKind::Unresolved(_) => "{unknown}".into(),
        }
    }
}
