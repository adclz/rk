// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Instances that point at each other, lowered: what a `REF_TO` back to an
//! instance being lowered names, and when a pass lowers an instance again.

use db::RootDatabase;
use mir::types::{MirRefTarget, MirType};
use rstest::rstest;

use crate::tests::utils::{add_source, lower_workspace, with_db};

/// The struct a pointer field of `instance` points at, and the name the
/// struct it points at names instead, when it is being lowered.
fn pointee<'m>(
    module: &'m mir::MirModule,
    db: &RootDatabase,
    instance: &str,
    field: &str,
) -> &'m MirRefTarget {
    let instance = module
        .instance_types
        .iter()
        .find(|t| t.name.text(db) == instance)
        .unwrap_or_else(|| panic!("no instance type {instance}"));
    let field = instance
        .fields
        .iter()
        .find(|f| f.name.text(db) == field)
        .unwrap_or_else(|| panic!("no field {field} in {}", instance.name.text(db)));
    match &field.ty {
        MirType::Pointer(target) => match target.as_ref() {
            MirType::RefTarget(target) => target,
            other => panic!("the pointer points at {other:?}"),
        },
        other => panic!("{} is {other:?}", field.name.text(db)),
    }
}

/// The field `name` of the struct `target` points at.
fn field_of<'m>(db: &RootDatabase, target: &'m MirRefTarget, name: &str) -> &'m MirType {
    let MirRefTarget::Type(ty) = target else {
        panic!("{target:?} is no type");
    };
    let MirType::Struct(strukt) = ty.as_ref() else {
        panic!("{ty:?} is no struct");
    };
    &strukt
        .fields
        .iter()
        .find(|f| f.name_with_case.text(db) == name)
        .unwrap_or_else(|| panic!("no field {name}"))
        .ty
}

/// Two blocks pointing at each other: from each, the other's pointer names
/// the first as the one being lowered. A pass keeps each instance it
/// lowered for its next use, but not under an instance its lowering
/// reached: the other's copy, kept from its own lowering, holds the first
/// whole, and the first would then hold itself.
#[rstest]
fn a_pointer_back_to_the_instance_being_lowered_names_it(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK A
VAR
    r : REF_TO B;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK B
VAR
    r : REF_TO A;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    a : A;
    b : B;
END_VAR
END_PROGRAM
"#;
    add_source(&mut with_db, source);
    let module = lower_workspace(&with_db);
    for (instance, other) in [("A", "B"), ("B", "A")] {
        let target = pointee(&module, &with_db, instance, "r");
        let back = match field_of(&with_db, target, "r") {
            MirType::Pointer(back) => back.as_ref(),
            other => panic!("{other:?}"),
        };
        match back {
            MirType::RefTarget(MirRefTarget::Enclosing(name)) => {
                assert_eq!(
                    name.text(&with_db),
                    instance,
                    "from {instance} through {other}"
                );
            }
            other => panic!("{instance}'s pointer, through {other:?}, does not name {instance}"),
        }
    }
}
