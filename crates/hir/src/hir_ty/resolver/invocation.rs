use db::WorkspaceDataBase;

use crate::{
    CallSite,
    check::errors::{analysis_error::ToIdeDiagnostic, e5_inheritance::InheritanceError},
    hir_def::{
        expressions::invocation::{Invocation, InvocationKind},
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body_inference::BodyInferenceResult, ty::Type},
};

pub fn resolve_invocation<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    invocation: Invocation<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Option<Pou<'db>> {
    match get_scope(db, scope).kind {
        ScopeKind::MethodDecl(m) => {
            let scope = get_scope(db, m.scope_id(db));
            let parent = scope
                .parent
                .expect("A method scope always has a parent scope");
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
                        ctx.type_of_invocation
                            .insert(invocation, Type::new_pou(db, pou));
                        return Some(pou);
                    }
                    _ => {
                        ctx.errors.push(
                            InheritanceError::SuperBodyOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            }
                            .to_diagnostic(db),
                        );
                    }
                },
                InvocationKind::Super => match pou {
                    // fixme: SUPER only gives access to base methods from EXTENDS, not all implemented interfaces
                    Pou::FunctionBlock(fb) => {
                        if let Some(extend) = fb.extends(db) {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        }
                    }
                    Pou::Class(class) => {
                        if let Some(extend) = class.extends(db) {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        }
                    }
                    _ => {
                        ctx.errors.push(
                            InheritanceError::SuperOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            }
                            .to_diagnostic(db),
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
                            .to_diagnostic(db),
                        );
                    }
                },
            }
        }
        _ => unreachable!("An invocation will always be in a POU scope"),
    }
    None
}
