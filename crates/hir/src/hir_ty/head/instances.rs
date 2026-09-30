use crate::{
    hir_def::{
        expressions::expression::InitExpr,
        interned::identifier::Ident,
        pous::{pou::Pou, variable::VariableDecl},
    },
    hir_ty::{infer::Infer, oop::instance_members},
};
use db::WorkspaceDataBase;

/// One step from an instance root toward an initialized member.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InstanceInitStep {
    /// Descend into the named member.
    Field(Ident),
    /// Descend into EVERY element of an array-typed member.
    ///
    /// HIR states that the traversal happens; how many elements there are and
    /// how far apart they sit are layout facts, so the consumer expands this.
    /// Keeping it a marker rather than one entry per element also keeps this
    /// query proportional to the number of members, not to the number of
    /// elements — an `ARRAY[0..9999] OF Cell` is one step, not ten thousand.
    AllElements,
}

/// One initializer that applies to a fresh instance of a POU.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InstanceInit<'db> {
    /// The route from the instance root down to the initialized member —
    /// `[Field(inner), Field(v)]` for an `Inner` instance's `v` held by an
    /// `Outer`. A direct member is a single step.
    pub path: Vec<InstanceInitStep>,
    pub init: InitValue<'db>,
}

/// What an initializer writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitValue<'db> {
    /// An initializer the source writes, to be resolved through
    /// [`infer_initialization`](crate::hir_ty::head::init_inference::infer_initialization).
    Written(InitExpr<'db>),
    /// The value a type starts at where the source writes none and it is not
    /// 0, which storage already holds (IEC 61131-3): an enumerated type's
    /// first value, a subrange's lower limit.
    Implied(i64),
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
    collect_instance_initializers(db, pou, &mut Vec::new(), &mut out, &mut Vec::new());
    out
}

fn collect_instance_initializers<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    prefix: &mut Vec<InstanceInitStep>,
    out: &mut Vec<InstanceInit<'db>>,
    pous: &mut Vec<Pou<'db>>,
) {
    // `pous` is the current *path*, not a global seen-set: a type reached
    // twice through different members must contribute twice (`a : Inner;
    // b : Inner;` initializes both), while a type reached through itself is a
    // cycle and stops here.
    if pous.contains(&pou) {
        return;
    }
    pous.push(pou);

    for member in instance_members(db, pou) {
        // A member located by VAR_CONFIG is a pointer to its channel; its
        // initial value goes to the channel, once the pointer is bound.
        if member.var.is_partly_located(db) {
            continue;
        }
        prefix.push(InstanceInitStep::Field(member.var.name(db)));

        // The member TYPE's own defaults first — an alias's, a STRUCT's
        // fields', an FB's or CLASS's members', each array element's — and an
        // explicit member init after, overlaying them by store order: a
        // partial one (`m : Motor := (speed := 9)`) keeps the others.
        collect_type_defaults(
            db,
            member.var.spec(db).infer(db),
            prefix,
            out,
            &mut Vec::new(),
            pous,
        );
        if let Some(init) = member.var.init(db) {
            out.push(InstanceInit {
                path: prefix.clone(),
                init: InitValue::Written(init),
            });
        }

        prefix.pop();
    }

    pous.pop();
}

/// Every member declared `AT %I*`, `%Q*` or `%M*` an instance of `pou`
/// holds, as the chain of members that reaches it: `[x]` for its own, `[fb,
/// x]` for one inside an instance it holds. VAR_CONFIG locates each of them,
/// for each instance (E1425).
///
/// An array of such instances is not followed: no VAR_CONFIG path reaches an
/// element, and E1425 refuses the array where it is declared. Nor is a
/// VAR_IN_OUT, which points at an instance located where it is declared, or
/// a VAR_INPUT, a copy of one, refused where it is declared.
#[salsa::tracked(returns(ref))]
pub fn partly_located_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Vec<Vec<VariableDecl<'db>>> {
    let mut out = Vec::new();
    collect_partly_located(
        db,
        &mut instance_members(db, pou).iter().map(|m| m.var),
        &mut Vec::new(),
        &mut out,
        &mut vec![pou],
    );
    out
}

/// A member declared `AT %I*`, `%Q*` or `%M*` held by an instance in a
/// field of `ty`, a STRUCT or an array of one, as the names that reach it
/// (the fields, then [`partly_located_members`]'s path) and the member
/// itself. A VAR_CONFIG path names instances, not fields, so no entry
/// reaches it (E1425).
pub fn partly_located_in_struct<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: crate::hir_ty::ty::Type<'db>,
    visited: &mut Vec<crate::hir_def::expressions::spec::Struct<'db>>,
) -> Option<(Vec<Ident>, VariableDecl<'db>)> {
    use crate::hir_ty::ty::Type;
    let element_of = |mut ty: Type<'db>| {
        while let Type::Array(array) = ty {
            ty = array.of_type(db).infer(db).normalize(db);
        }
        ty
    };
    let Type::Struct(strukt) = element_of(ty) else {
        return None;
    };
    if visited.contains(&strukt) {
        return None;
    }
    visited.push(strukt);
    let mut found = None;
    for element in &strukt.elements(db) {
        let field = element_of(element.spec(db).infer(db).normalize(db));
        let inner = match pou_of_type(db, field) {
            Some(pou) => partly_located_members(db, pou)
                .first()
                .and_then(|path| Some((path.iter().map(|v| v.name(db)).collect(), *path.last()?))),
            None => partly_located_in_struct(db, field, visited),
        };
        if let Some((mut names, var)) = inner {
            names.insert(0, element.name(db));
            found = Some((names, var));
            break;
        }
    }
    visited.pop();
    found
}

/// Whether a member of a retained instance is kept with it, as the retain
/// map decides: `NON_RETAIN` prunes it, a VAR_TEMP is per call and a
/// VAR_IN_OUT pointer is bound again by every call.
pub fn retained_with_instance<'db>(db: &'db dyn WorkspaceDataBase, var: VariableDecl<'db>) -> bool {
    use crate::hir_def::pous::variable::VariableKind;
    !var.qualifier(db).contains(crate::Qualifier::NON_RETAIN)
        && !matches!(
            var.kind(db),
            VariableKind::Temp | VariableKind::InOut | VariableKind::External
        )
}

/// The reference a retained value of type `ty` holds, as the route to it:
/// empty when the value is one (a REF_TO or an interface), else the STRUCT
/// fields and instance members, as written, down to the first one found.
/// Array elements add no name.
pub fn retained_reference<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: crate::hir_ty::ty::Type<'db>,
    visited: &mut Vec<crate::hir_ty::ty::Type<'db>>,
) -> Option<Vec<Ident>> {
    use crate::hir_ty::ty::Type;
    let ty = ty.normalize(db);
    let fields: Vec<(Ident, Type<'db>)> = match ty {
        Type::RefTo(_) | Type::Interface(_) => return Some(Vec::new()),
        Type::Array(array) => return retained_reference(db, array.of_type(db).infer(db), visited),
        Type::Struct(strukt) => strukt
            .elements(db)
            .iter()
            .map(|element| (element.name_with_case(db), element.spec(db).infer(db)))
            .collect(),
        Type::FunctionBlock(_) | Type::Class(_) => instance_members(db, pou_of_type(db, ty)?)
            .iter()
            .map(|member| member.var)
            .filter(|var| retained_with_instance(db, *var))
            .map(|var| (var.name_with_case(db), var.spec(db).infer(db)))
            .collect(),
        _ => return None,
    };
    if visited.contains(&ty) {
        return None;
    }
    visited.push(ty);
    let found = fields.into_iter().find_map(|(name, field)| {
        let mut route = retained_reference(db, field, visited)?;
        route.insert(0, name);
        Some(route)
    });
    visited.pop();
    found
}

/// [`partly_located_members`] from a list of members: a PROGRAM's variables
/// start it as a POU's members do.
pub fn collect_partly_located<'db>(
    db: &'db dyn WorkspaceDataBase,
    members: &mut dyn Iterator<Item = VariableDecl<'db>>,
    prefix: &mut Vec<VariableDecl<'db>>,
    out: &mut Vec<Vec<VariableDecl<'db>>>,
    visited: &mut Vec<Pou<'db>>,
) {
    for var in members {
        prefix.push(var);
        if var.is_partly_located(db) {
            out.push(prefix.clone());
        } else if !var.is_in_out(db)
            && !var.is_input(db)
            && let Some(inner) = pou_of_type(db, var.spec(db).infer(db).normalize(db))
            && !visited.contains(&inner)
        {
            // `visited` is the current path, as for the initializers: a type
            // reached through itself is a cycle, refused elsewhere.
            visited.push(inner);
            collect_partly_located(
                db,
                &mut instance_members(db, inner).iter().map(|m| m.var),
                prefix,
                out,
                visited,
            );
            visited.pop();
        }
        prefix.pop();
    }
}

/// The FB or CLASS `ty` is an instance of, if it is one.
///
/// Strictly the type itself — an `ARRAY OF Cell` is not a `Cell`, so this
/// answers `None` for it. Callers that mean "an instance may be nested in
/// here" peel the array layers first.
pub fn pou_of_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: crate::hir_ty::ty::Type<'db>,
) -> Option<Pou<'db>> {
    match ty {
        crate::hir_ty::ty::Type::FunctionBlock(fb) => Some(Pou::FunctionBlock(fb)),
        crate::hir_ty::ty::Type::Class(c) => Some(Pou::Class(c)),
        _ => None,
    }
}

/// The FB or CLASS a variable is an instance of, if it is one.
pub fn instance_pou_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Option<Pou<'db>> {
    pou_of_type(db, var.spec(db).infer(db).normalize(db))
}

/// Every initializer a TYPE contributes to a fresh value of it, flattened:
/// an alias's own `:= 5`, a STRUCT's field defaults, an array type's
/// `:= [1, 2, 3]` — and their compositions, `ARRAY OF Pt` included.
///
/// The declaration-site initializer is NOT here; it is emitted AFTER these by
/// every consumer, so an explicit `(y := 9)` overlays the type's `x := 3,
/// y := 4` by store order rather than by path arithmetic.
pub fn type_default_inits<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: crate::hir_ty::ty::Type<'db>,
) -> Vec<InstanceInit<'db>> {
    let mut out = Vec::new();
    collect_type_defaults(
        db,
        ty,
        &mut Vec::new(),
        &mut out,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    out
}

fn collect_type_defaults<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: crate::hir_ty::ty::Type<'db>,
    prefix: &mut Vec<InstanceInitStep>,
    out: &mut Vec<InstanceInit<'db>>,
    visited: &mut Vec<crate::hir_def::pous::data_type::DataType<'db>>,
    pous: &mut Vec<Pou<'db>>,
) {
    use crate::hir_ty::infer::const_eval::{enum_ordinals, subrange_bounds};
    use crate::hir_ty::ty::Type;
    match ty {
        Type::DataType(dt) => {
            // Same path-guard as the instance walk: recursion checks refuse a
            // cyclic TYPE, this only keeps erroneous input from hanging.
            if visited.contains(&dt) {
                return;
            }
            visited.push(dt);
            // What the type is made of first, its own init after: later
            // stores win, so `TYPE Origin : Point := (x := 7)` keeps Point's
            // `y := 1.5`. Taking the alias's init alone erased every member
            // it did not name.
            collect_type_defaults(db, dt.spec(db).infer(db), prefix, out, visited, pous);
            if let Some(init) = dt.init(db) {
                out.push(InstanceInit {
                    path: prefix.clone(),
                    init: InitValue::Written(init),
                });
            }
            visited.pop();
        }
        Type::Struct(s) => {
            for element in &s.elements(db) {
                prefix.push(InstanceInitStep::Field(element.name(db)));
                // The element type's own defaults first, the element's
                // explicit default after — later stores win, so a partial
                // struct-typed default overlays instead of erasing.
                collect_type_defaults(db, element.spec(db).infer(db), prefix, out, visited, pous);
                if let Some(init) = element.init(db) {
                    out.push(InstanceInit {
                        path: prefix.clone(),
                        init: InitValue::Written(init),
                    });
                }
                prefix.pop();
            }
        }
        Type::Array(array) => {
            prefix.push(InstanceInitStep::AllElements);
            collect_type_defaults(db, array.of_type(db).infer(db), prefix, out, visited, pous);
            prefix.pop();
        }
        // An instance starts from its members' defaults, wherever it is held:
        // a variable, a member, an array element, a STRUCT field.
        Type::FunctionBlock(_) | Type::Class(_) => {
            if let Some(pou) = pou_of_type(db, ty) {
                collect_instance_initializers(db, pou, prefix, out, pous);
            }
        }
        // IEC 61131-3: an enumerated type starts at its first value...
        Type::Enum(enm) => {
            if let Some((_, Some(first))) = enum_ordinals(db, enm).first()
                && *first != 0
            {
                out.push(InstanceInit {
                    path: prefix.clone(),
                    init: InitValue::Implied(*first),
                });
            }
        }
        // ...and a subrange at its lower limit, 0 being outside some.
        Type::SubRange(subrange) => {
            if let (Some(lower), _) = subrange_bounds(db, subrange)
                && lower != 0
            {
                out.push(InstanceInit {
                    path: prefix.clone(),
                    init: InitValue::Implied(lower),
                });
            }
        }
        _ => {}
    }
}
