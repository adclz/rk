use db::WorkspaceDataBase;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    CallSite, HasName,
    hir_def::{
        expressions::spec::{Enum, EnumVariant, SpecKind, Struct, StructElement},
        interned::identifier::FoldedIdent,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{head::inheritance::MethodRef, resolver::name::resolve_namespace_access},
};

pub type FxIndexMap<K, V> = IndexMap<K, V, rustc_hash::FxBuildHasher>;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
///
/// Every map is keyed by [`FoldedIdent`], because case is not significant in
/// IEC identifiers. The key type is what enforces it: an `Ident` will not open
/// these maps, so a lookup that forgot to fold does not compile.
pub struct LocalDefMap<'db> {
    /// Local POUs accessible in this scope
    pub local_pous: FxHashMap<FoldedIdent, Pou<'db>>,
    /// Local variables accessible in this scope (VARIABLES with Input, Output, InOut specifiers)
    ///
    /// We use [`IndexMap`] here to preserve the order of declaration
    pub local_variables: FxIndexMap<FoldedIdent, VariableDecl<'db>>,
    /// Global variables accessible in this scope (all VARIABLES)
    pub global_variables: FxHashMap<FoldedIdent, VariableDecl<'db>>,
    /// Methods declared in this scope (for CLASSes, INTERFACEs, FUNCTION BLOCKs)
    pub declared_methods: FxHashMap<FoldedIdent, MethodRef<'db>>,
}

#[salsa::tracked]
impl<'db> ScopeId<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn def_map(self, db: &'db dyn WorkspaceDataBase) -> LocalDefMap<'db> {
        LocalDefMap {
            local_pous: self.local_pous(db).clone(),
            local_variables: self.local_variables(db),
            global_variables: self.global_variables(db),
            declared_methods: self.declared_methods(db),
        }
    }

    fn local_pous(&self, db: &'db dyn WorkspaceDataBase) -> FxHashMap<FoldedIdent, Pou<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) => {
                let mut result = FxHashMap::default();
                ns.pous(db).iter().for_each(|pou| {
                    result.insert(pou.get_name_ident(db).fold(db), *pou);
                });
                result
            }
            _ => FxHashMap::default(),
        }
    }

    pub fn can_have_local_variables(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => false,
            ScopeKind::Pou(pou) => matches!(
                pou,
                Pou::Function(_) | Pou::FunctionBlock(_) | Pou::Class(_)
            ),
            ScopeKind::Program(_) => true,
            ScopeKind::MethodDecl(m) => true,
            _ => false,
        }
    }

    fn local_variables(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> FxIndexMap<FoldedIdent, VariableDecl<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) | ScopeKind::Program(_) => {
                IndexMap::default()
            }
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => local_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => local_variables(db, fb.variables(db)),
                Pou::Class(cl) => local_variables(db, cl.variables(db)),
                _ => IndexMap::default(),
            },
            ScopeKind::MethodDecl(m) => local_variables(db, m.variables(db)),
            ScopeKind::MethodProt(m) => local_variables(db, m.variables(db)),
            _ => IndexMap::default(),
        }
    }

    fn global_variables(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> FxHashMap<FoldedIdent, VariableDecl<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => global_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => global_variables(db, fb.variables(db)),
                Pou::Class(cl) => global_variables(db, cl.variables(db)),
                _ => FxHashMap::default(),
            },
            ScopeKind::MethodDecl(m) => global_variables(db, m.variables(db)),
            ScopeKind::MethodProt(m) => global_variables(db, m.variables(db)),
            ScopeKind::Program(program) => global_variables(db, program.variables(db)),
            ScopeKind::Config(config) => global_variables(db, config.variables(db)),
            _ => FxHashMap::default(),
        }
    }

    fn declared_methods(&self, db: &'db dyn WorkspaceDataBase) -> FxHashMap<FoldedIdent, MethodRef<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Global
            | ScopeKind::Namespace(_)
            | ScopeKind::MethodDecl(_)
            | ScopeKind::Program(_) => FxHashMap::default(),
            ScopeKind::Pou(pou) => match pou {
                Pou::Class(class) => class
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db).fold(db), m.into()))
                    .collect(),
                Pou::Interface(interface) => interface
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db).fold(db), m.into()))
                    .collect(),
                Pou::FunctionBlock(fb) => fb
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db).fold(db), m.into()))
                    .collect(),
                _ => Default::default(),
            },
            _ => FxHashMap::default(),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn inheritors(self, db: &'db dyn WorkspaceDataBase) -> FxHashMap<CallSite<'db>, Pou<'db>> {
        let resolve_spec =
            |spec: &crate::hir_def::expressions::spec::Spec<'db>| -> Option<Pou<'db>> {
                if let SpecKind::Target(target) = spec.kind(db) {
                    resolve_namespace_access(db, &target.path).found()
                } else {
                    None
                }
            };

        match get_scope(db, self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Interface(interface) => {
                    let mut inheritors = FxHashMap::default();
                    if let Some(extends) = interface.extends(db) {
                        for base in extends {
                            if let Some(iface) = resolve_spec(base) {
                                inheritors.insert(CallSite::from_scoped(db, base), iface);
                            }
                        }
                    }
                    inheritors
                }
                Pou::Class(class) => {
                    let mut inheritors = FxHashMap::default();
                    if let Some(base) = class.extends(db)
                        && let Some(pou) = resolve_spec(base)
                    {
                        inheritors.insert(CallSite::from_scoped(db, base), pou);
                    }
                    for base in class.implements(db) {
                        if let Some(iface) = resolve_spec(base) {
                            inheritors.insert(CallSite::from_scoped(db, base), iface);
                        }
                    }
                    inheritors
                }
                Pou::FunctionBlock(fb) => {
                    let mut inheritors = FxHashMap::default();
                    if let Some(base) = fb.extends(db)
                        && let Some(pou) = resolve_spec(base)
                    {
                        inheritors.insert(CallSite::from_scoped(db, base), pou);
                    }

                    for base in fb.implements(db) {
                        if let Some(iface) = resolve_spec(base) {
                            inheritors.insert(CallSite::from_scoped(db, base), iface);
                        }
                    }
                    inheritors
                }
                _ => FxHashMap::default(),
            },
            _ => FxHashMap::default(),
        }
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn struct_elements(
        self,
        db: &'db dyn WorkspaceDataBase,
    ) -> FxHashMap<FoldedIdent, StructElement<'db>> {
        self.elements(db)
            .iter()
            .map(|element| (element.get_name_ident(db).fold(db), *element))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn enum_variants(
        self,
        db: &'db dyn WorkspaceDataBase,
    ) -> FxHashMap<FoldedIdent, EnumVariant<'db>> {
        self.variants(db)
            .iter()
            .map(|element| (element.name.fold(db), *element))
            .collect()
    }
}

fn global_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[VariableDecl<'db>],
) -> FxHashMap<FoldedIdent, VariableDecl<'db>> {
    let mut variables = FxHashMap::default();
    for v in vars {
        variables.insert(v.get_name_ident(db).fold(db), *v);
    }
    variables
}

fn local_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: &[VariableDecl<'db>],
) -> FxIndexMap<FoldedIdent, VariableDecl<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(v.get_name_ident(db).fold(db), *v);
            }
            VariableKind::Output => {
                variables.insert(v.get_name_ident(db).fold(db), *v);
            }
            VariableKind::InOut => {
                variables.insert(v.get_name_ident(db).fold(db), *v);
            }
            _ => continue,
        };
    }
    variables
}
