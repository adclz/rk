use crate::{
    AstId, HasModifiers, HasName, HasVisibility, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::{expression::InitExpr, spec::{Spec, SpecKind}},
        interned::identifier::Ident,
        pous::{class::MethodDecl, interface::MethodPrototype, pou::Pou, variable::VariableDecl},
        scope::ScopeId,
    },
    hir_ty::resolver::name::resolve_namespace_access,
};
use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum MethodRef<'db> {
    Prototype(MethodPrototype<'db>),
    Declared(MethodDecl<'db>),
}

impl<'db> HasModifiers<'db> for MethodRef<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            MethodRef::Prototype(p) => Modifier::default(),
            MethodRef::Declared(d) => d.modifier(db),
        }
    }
}

impl<'db> HasVisibility<'db> for MethodRef<'db> {
    fn get_visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility {
        match self {
            MethodRef::Prototype(_) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }
}

impl<'db> MethodRef<'db> {
    pub fn return_type(&self, db: &'db dyn WorkspaceDataBase) -> Option<&'db Spec<'db>> {
        match self {
            MethodRef::Prototype(p) => p.return_type(db),
            MethodRef::Declared(d) => d.return_type(db),
        }
    }

    pub fn visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility {
        match self {
            MethodRef::Prototype(p) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }

    pub fn modifier(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            MethodRef::Prototype(p) => Modifier::EMPTY,
            MethodRef::Declared(d) => d.modifier(db),
        }
    }

    pub fn variables(&self, db: &'db dyn WorkspaceDataBase) -> &'db Vec<VariableDecl<'db>> {
        match self {
            MethodRef::Prototype(p) => p.variables(db),
            MethodRef::Declared(d) => d.variables(db),
        }
    }

    pub fn is_prototype(&self) -> bool {
        matches!(self, MethodRef::Prototype(_))
    }

    pub fn is_declared(&self) -> bool {
        matches!(self, MethodRef::Declared(_))
    }
}

impl<'db> HirNodeInfo<'db> for MethodRef<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_id(db),
            MethodRef::Declared(d) => d.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            MethodRef::Prototype(p) => p.get_scope_id(db),
            MethodRef::Declared(d) => d.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for MethodRef<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            MethodRef::Prototype(p) => p.get_name_ident(db),
            MethodRef::Declared(d) => d.get_name_ident(db),
        }
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_name_id(db),
            MethodRef::Declared(d) => d.get_name_id(db),
        }
    }
}

impl<'db> From<MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(value)
    }
}

impl<'db> From<MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: MethodDecl<'db>) -> Self {
        MethodRef::Declared(value)
    }
}

impl<'db> From<&MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: &MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(*value)
    }
}

impl<'db> From<&MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: &MethodDecl<'db>) -> Self {
        MethodRef::Declared(*value)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct InheritedMethodSet<'db> {
    pub methods: FxHashMap<Ident, InheritedMethod<'db>>,

    pub duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InheritedMethod<'db> {
    pub source: Pou<'db>,
    pub method: MethodRef<'db>,
}

impl<'db> InheritedMethod<'db> {
    fn new(source: Pou<'db>, method: MethodRef<'db>) -> Self {
        Self { source, method }
    }
}

/// Try to resolve a Spec to a Pou via SpecKind::Target.
fn resolve_spec_to_pou<'db>(db: &'db dyn WorkspaceDataBase, spec: &Spec<'db>) -> Option<Pou<'db>> {
    if let SpecKind::Target(target) = spec.kind(db) {
        resolve_namespace_access(db, &target.path).found()
    } else {
        None
    }
}

/// Every POU a POU inherits from DIRECTLY: its `EXTENDS` base (one for an
/// FB/CLASS, possibly several for an INTERFACE) plus every `IMPLEMENTS`
/// interface.
fn direct_bases<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Vec<Pou<'db>> {
    let mut bases = Vec::new();
    let mut push = |spec: &Spec<'db>| {
        if let Some(p) = resolve_spec_to_pou(db, spec) {
            bases.push(p);
        }
    };
    match pou {
        Pou::Class(class) => {
            if let Some(base) = class.extends(db) {
                push(base);
            }
            for iface in class.implements(db) {
                push(iface);
            }
        }
        Pou::FunctionBlock(fb) => {
            if let Some(base) = fb.extends(db) {
                push(base);
            }
            for iface in fb.implements(db) {
                push(iface);
            }
        }
        Pou::Interface(iface) => {
            if let Some(extends) = iface.extends(db) {
                for spec in extends {
                    push(spec);
                }
            }
        }
        _ => {}
    }
    bases
}

/// Every method visible ON `pou` — its own declarations plus everything it
/// inherits, with a NEARER declaration overriding a farther one.
///
/// Ancestors are collected first so a redeclaration closer to `pou` overwrites
/// it: that is method overriding, not a conflict, and must not be reported as a
/// duplicate.
fn chain_methods<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    visited: &mut Vec<Pou<'db>>,
) -> FxHashMap<Ident, InheritedMethod<'db>> {
    let mut out = FxHashMap::default();
    // Cyclic inheritance is reported separately (E05xx); stop so this
    // terminates regardless.
    if visited.contains(&pou) {
        return out;
    }
    visited.push(pou);

    for base in direct_bases(db, pou) {
        out.extend(chain_methods(db, base, visited));
    }
    for (name, method) in pou.get_scope_id(db).def_map(db).declared_methods.iter() {
        out.insert(*name, InheritedMethod::new(pou, *method));
    }
    out
}

/// Every method `pou` INHERITS (its own declarations excluded), resolved through
/// the whole inheritance graph.
///
/// A name declared at several depths of one chain is an override — the nearest
/// wins, silently. A name arriving from two INDEPENDENT bases (two interfaces,
/// or a base class and an interface) is a genuine conflict and is recorded in
/// `duplicates` for the E01xx diagnostic.
#[salsa::tracked(returns(ref))]
pub fn inherited_methods<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> InheritedMethodSet<'db> {
    let mut methods: FxHashMap<Ident, InheritedMethod<'db>> = FxHashMap::default();
    let mut duplicates = vec![];

    for base in direct_bases(db, pou) {
        let mut visited = vec![pou];
        for (name, m) in chain_methods(db, base, &mut visited) {
            match methods.insert(name, m) {
                None => {}
                // A prototype and a concrete method for the same name are not a
                // conflict: the concrete one IMPLEMENTS the prototype. This is
                // the ordinary `FB EXTENDS Base IMPLEMENTS Iface` shape, where
                // the inherited method satisfies the interface. Keep the
                // implementation, so conformance sees the name as implemented
                // rather than reporting it unimplemented.
                Some(prev) if prev.method.is_prototype() != m.method.is_prototype() => {
                    let concrete = if m.method.is_prototype() { prev } else { m };
                    methods.insert(name, concrete);
                }
                // Two declarations of the same kind from INDEPENDENT bases —
                // a genuine ambiguity.
                Some(prev) => duplicates.push((prev, m)),
            }
        }
    }

    InheritedMethodSet {
        methods,
        duplicates,
    }
}

/// One instance member (a field of an FB/CLASS instance), paired with the POU
/// that DECLARES it — which may be a base of the POU being queried.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InstanceMember<'db> {
    /// The POU this member is declared on. For an inherited member this is a
    /// base, not the queried POU.
    pub owner: Pou<'db>,
    pub var: VariableDecl<'db>,
}

/// Every instance member of a POU, in **layout order**: the base-most POU's
/// members first (the whole `EXTENDS` chain, recursively), each level in
/// declaration order.
///
/// This is the authoritative answer to "what state does an instance of this POU
/// hold?", so consumers never walk `EXTENDS` themselves. MIR in particular must
/// only compute offsets from this list: a derived instance has to be
/// layout-compatible with its base, because an inherited method is compiled
/// once against the base's offsets and then invoked with a derived instance
/// pointer — which the base-first ordering guarantees.
///
/// Sections that are not instance state are excluded here, once, rather than in
/// each consumer:
/// - `VAR_EXTERNAL` references a global; it resolves to the global's address.
/// - `VAR_TEMP` is per-invocation scratch and lives as a body local.
#[salsa::tracked(returns(ref))]
pub fn instance_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Vec<InstanceMember<'db>> {
    let mut out = Vec::new();
    let mut visited = Vec::new();
    collect_instance_members(db, pou, &mut out, &mut visited);
    out
}

/// One initializer that applies to a fresh instance of a POU.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InstanceInit<'db> {
    /// Member names from the instance root down to the initialized member —
    /// `["inner", "v"]` for an `Inner` instance's `v` held by an `Outer`. A
    /// direct member is a single-element path.
    pub path: Vec<Ident>,
    /// The initializer expression, to be resolved through
    /// [`infer_initialization`](crate::hir_ty::head::init_inference::infer_initialization).
    pub init: InitExpr<'db>,
}

/// Every initializer that applies to a fresh instance of `pou`, flattened.
///
/// An FB or CLASS instance carries no initializer at its declaration site — the
/// `:= 3` lives on the *type's* member declarations, one or more levels down.
/// This resolves both relationships that stand between an instance and its
/// initial state:
///
/// - **inheritance** — inherited members are included, via [`instance_members`];
/// - **composition** — a member that is itself an instance contributes its own
///   type's initializers, under a longer path.
///
/// Consumers therefore walk neither. A cyclic `EXTENDS` or a self-containing
/// FB is reported separately (E05xx / E09xx recursion checks); the `visited`
/// set here only guarantees this query terminates regardless.
///
/// Order is layout order — base-most POU first, then declaration order —
/// matching [`instance_members`], so writes land in a predictable sequence.
#[salsa::tracked(returns(ref))]
pub fn instance_initializers<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Vec<InstanceInit<'db>> {
    let mut out = Vec::new();
    let mut visited = Vec::new();
    collect_instance_initializers(db, pou, &mut Vec::new(), &mut out, &mut visited);
    out
}

fn collect_instance_initializers<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    prefix: &mut Vec<Ident>,
    out: &mut Vec<InstanceInit<'db>>,
    visited: &mut Vec<Pou<'db>>,
) {
    // `visited` is the current *path*, not a global seen-set: a type reached
    // twice through different members must contribute twice (`a : Inner;
    // b : Inner;` initializes both), while a type reached through itself is a
    // cycle and stops here.
    if visited.contains(&pou) {
        return;
    }
    visited.push(pou);

    for member in instance_members(db, pou) {
        prefix.push(member.var.name(db));

        if let Some(init) = member.var.init(db) {
            out.push(InstanceInit {
                path: prefix.clone(),
                init,
            });
        } else if let Some(inner) = instance_pou_of(db, member.var) {
            // A member that is itself an instance brings its own type's
            // initializers along, under this member's path.
            collect_instance_initializers(db, inner, prefix, out, visited);
        }

        prefix.pop();
    }

    visited.pop();
}

/// The FB or CLASS a variable is an instance of, if it is one.
pub fn instance_pou_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Option<Pou<'db>> {
    use crate::hir_ty::infer::Infer;
    match var.spec(db).infer(db).normalize(db) {
        crate::hir_ty::ty::Type::FunctionBlock(fb) => Some(Pou::FunctionBlock(fb)),
        crate::hir_ty::ty::Type::Class(c) => Some(Pou::Class(c)),
        _ => None,
    }
}

fn collect_instance_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    out: &mut Vec<InstanceMember<'db>>,
    visited: &mut Vec<Pou<'db>>,
) {
    // Cyclic EXTENDS is reported separately (E05xx); stop here so resolution
    // terminates regardless.
    if visited.contains(&pou) {
        return;
    }
    visited.push(pou);

    if let Some(base) = base_pou(db, pou) {
        collect_instance_members(db, base, out, visited);
    }

    let vars = match pou {
        Pou::FunctionBlock(fb) => fb.variables(db),
        Pou::Class(class) => class.variables(db),
        _ => return,
    };
    for var in vars {
        use crate::hir_def::pous::variable::VariableKind;
        match var.kind(db) {
            VariableKind::External | VariableKind::Temp => continue,
            _ => {}
        }
        out.push(InstanceMember {
            owner: pou,
            var: *var,
        });
    }
}

/// The POU a FUNCTION_BLOCK or CLASS directly `EXTENDS`, if any.
///
/// The single place `EXTENDS` is followed for base resolution — used by
/// [`instance_members`] and by consumers that need the base itself (`SUPER()`).
pub fn base_pou<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Option<Pou<'db>> {
    let spec = match pou {
        Pou::FunctionBlock(fb) => fb.extends(db)?,
        Pou::Class(class) => class.extends(db)?,
        _ => return None,
    };
    resolve_spec_to_pou(db, spec)
}

/// The concrete method an implementer provides for an inherited method NAME —
/// in particular, for an interface prototype it declares via `IMPLEMENTS`.
///
/// Conformance already establishes this pairing: `check_methods` matches each
/// inherited prototype against the implementer's declared method to verify the
/// signature. That pairing was only used for diagnostics and then discarded, so
/// consumers needing the implementation itself — devirtualizing an interface
/// call to `Worker#Run` when monomorphizing — had to re-derive it.
///
/// A method the implementer declares itself wins; otherwise one it inherits
/// from a base. Returns `None` when nothing concrete implements the name (an
/// unimplemented prototype, which conformance reports separately).
pub fn implementing_method<'db>(
    db: &'db dyn WorkspaceDataBase,
    implementer: Pou<'db>,
    name: Ident,
) -> Option<MethodDecl<'db>> {
    let own = implementer
        .get_scope_id(db)
        .def_map(db)
        .declared_methods
        .get(&name)
        .copied();
    let resolved = own.or_else(|| {
        inherited_methods(db, implementer)
            .methods
            .get(&name)
            .map(|m| m.method)
    })?;
    match resolved {
        // A prototype is a signature, not an implementation.
        MethodRef::Declared(decl) => Some(decl),
        MethodRef::Prototype(_) => None,
    }
}
