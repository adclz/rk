use auto_lsp::default::db::BaseDatabase;

use crate::{HirNodeInfo, hir_def::scope::ScopeId, hir_ty::ty::Type};

pub struct TypeErr<'db> {
    pub expected: Type<'db>,
    pub actual: Type<'db>,
}

pub type CoerceResult<'db> = Result<(), TypeErr<'db>>;

impl<'db> Type<'db> {
    // Type coercion check
    #[must_use]
    pub fn coerce_with(
        &self,
        db: &'db dyn BaseDatabase,
        to: Type<'db>,
        scope: ScopeId<'db>,
    ) -> CoerceResult<'db> {
        // We return true if the lhs or rhs is of type never.
        // that's because Never variants are already reported by the resolver and we don't want to propagate too many errors

        if self.is_never() || to.is_never() {
            return Ok(());
        }

        match (self, &to) {
            // allow coercion between Type and its Spec
            (Type::DataType(typ), typ2) => {
                Type::new_spec(db, typ.spec(db)).coerce_with(db, *typ2, scope)
            }
            (typ1, Type::DataType(typ)) => {
                typ1.coerce_with(db, Type::new_spec(db, typ.spec(db)), scope)
            }
            // same with variable declarations
            (Type::Variable(var), var2) => {
                Type::new_spec(db, var.spec(db)).coerce_with(db, *var2, scope)
            }
            (var1, Type::Variable(var)) => {
                var1.coerce_with(db, Type::new_spec(db, var.spec(db)), scope)
            }
            // variant is already solved by the resolver
            (Type::Enum(e1), Type::EnumVariant(e2)) => Ok(()),
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => {
                return match s1.eq(s2) {
                    true => Ok(()),
                    false => Err(TypeErr {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check element spec equality
            (Type::StructElement(elem), rhs) => {
                Type::new_spec(db, elem.spec(db)).coerce_with(db, *rhs, scope)
            }
            // same types are assignable
            (Type::Array(a1), Type::Array(a2)) => {
                return match a1.eq(a2) {
                    true => Ok(()),
                    false => Err(TypeErr {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check element spec equality
            (Type::Array(a1), rhs) => {
                Type::new_spec(db, a1.of_type(db)).coerce_with(db, *rhs, scope)
            }
            // check subrange base type equality
            (Type::SubRange(sub), rhs) => {
                Type::new_spec(db, sub._type(db)).coerce_with(db, *rhs, scope)
            }
            (Type::Elementary(lhs), Type::Elementary(rhs)) => {
                if lhs == rhs {
                    return Ok(());
                }
                // try implicit conversions in both directions
                match !lhs.implicit_cast(*rhs).is_never() || !rhs.implicit_cast(*lhs).is_never() {
                    true => Ok(()),
                    false => Err(TypeErr {
                        expected: *self,
                        actual: to,
                    }),
                }
            }
            (Type::Infer(infer), rhs) => infer.infer_default(db).coerce_with(db, *rhs, scope),
            (lhs, Type::Infer(infer)) => infer.infer_default(db).coerce_with(db, *lhs, scope),
            // Assignments to function /method are allowed *only inside their body*
            (Type::Function(f), rhs) => {
                /*if f.scope_id(db) != scope {
                    return false;
                }*/
                if let Some(ret_ty) = f.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, scope)
                } else {
                    Err(TypeErr {
                        expected: *self,
                        actual: to,
                    })
                }
            }
            (Type::MethodDecl(m), rhs) => {
                if m.get_scope_id(db) != scope {
                    return Err(TypeErr {
                        expected: *self,
                        actual: to,
                    });
                }
                if let Some(ret_ty) = m.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, scope)
                } else {
                    Err(TypeErr {
                        expected: *self,
                        actual: to,
                    })
                }
            }
            (Type::RefTo(_), Type::Null) => Ok(()),
            _ => Err(TypeErr {
                expected: *self,
                actual: to,
            }),
        }
    }
}
