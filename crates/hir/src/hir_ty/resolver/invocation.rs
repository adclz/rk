use db::WorkspaceDataBase;

use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e5_inheritance::InheritanceError},
    hir_def::{
        expressions::invocation::{Invocation, InvocationKind},
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};

pub fn resolve_invocation<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    invocation: Invocation<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Option<Pou<'db>> {
    match get_scope(db, scope).kind {
        ScopeKind::MethodDecl(m) => {
            let method_scope = get_scope(db, m.scope_id(db));
            let parent = method_scope
                .parent
                .expect("A method scope always has a parent scope");
            // Rule 5 (IEC 6.6.7.2.9): `SUPER()` (the base-body call) may only
            // appear in the function block BODY, not in a method. `THIS` and
            // `SUPER.<method>` ARE valid in methods, so reject only `SuperBody`.
            // Only for FB methods — in a CLASS method `SUPER()` is invalid for a
            // different reason (a class has no body, E0501), which the recursion
            // below produces.
            if invocation.kind(db) == InvocationKind::SuperBody
                && matches!(get_scope(db, parent).kind, ScopeKind::Pou(Pou::FunctionBlock(_)))
            {
                ctx.errors.push(
                    InheritanceError::SuperBodyInMethod {
                        call_site: CallSite::new(scope, invocation.keyword_id(db)),
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                return None;
            }
            return resolve_invocation(db, parent, invocation, ctx);
        }
        ScopeKind::Pou(pou) => {
            /*
            CLASS:

            7Access reference
            9a THIS: Reference to own methods
            9b SUPER: Access reference to method in base class

            FUNCTION BLOCKS:

            Access reference
            10a THIS:  Reference to own methods
            10b SUPER:  Access reference to method in base function block
            10c SUPER():  Access reference to body in base function block
            */
            match invocation.kind(db) {
                InvocationKind::SuperBody => match pou {
                    Pou::FunctionBlock(fb) => {
                        // `SUPER()` executes the BASE FB's body, so the FB must
                        // EXTEND one (mirror the `SUPER.<method>` check, E0513).
                        if fb.extends(db).is_some() {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        } else {
                            ctx.errors.push(
                                InheritanceError::SuperButNoExtends {
                                    pou,
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                    _ => {
                        ctx.errors.push(
                            InheritanceError::SuperBodyOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }
                },
                InvocationKind::Super => match pou {
                    Pou::FunctionBlock(fb) => {
                        if let Some(extend) = fb.extends(db) {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        } else {
                            ctx.errors.push(
                                InheritanceError::SuperButNoExtends {
                                    pou,
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                    Pou::Class(class) => {
                        if let Some(extend) = class.extends(db) {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        } else {
                            ctx.errors.push(
                                InheritanceError::SuperButNoExtends {
                                    pou,
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                    _ => {
                        ctx.errors.push(
                            InheritanceError::SuperOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }
                },
                InvocationKind::This => match pou {
                    Pou::FunctionBlock(_) | Pou::Class(_) => {
                        ctx.type_of_invocation
                            .insert(invocation, Type::new_pou(db, pou));
                        return Some(pou);
                    }
                    _ => {
                        ctx.errors.push(
                            InheritanceError::ThisOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }
                },
            }
        }
        _ => unreachable!("An invocation will always be in a POU scope"),
    }
    None
}
