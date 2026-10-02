// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic,
        e01_duplicates::DuplicateError,
        e14_config::{ConfigError, UnlocatableAddress},
    },
    hir_def::expressions::{
        expression::{MultibitsPart, VariableAccessKind},
        spec::Struct,
    },
    hir_ty::{head::init_inference::InitInference, infer::Infer},
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen = FxHashMap::default();

        for field in &strukt.elements(db) {
            match seen.get(&field.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::StructField {
                            field1: *field,
                            field2: *prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
                None => {
                    seen.insert(field.get_name_ident(db), *field);
                }
            }

            // A field is part of every variable of its type: an address on
            // it was accepted and ignored.
            if let Some(located) = field.located(db)
                && let VariableAccessKind::Direct(dv) = located.kind(db)
            {
                // A bit's number is kept apart from the address: `%IX0.0`.
                let mut address = compact_str::CompactString::from(dv.to_address(db));
                if let Some(MultibitsPart::Offset(bit)) = field.multibits(db) {
                    address.push('.');
                    address.push_str(bit.ident(db).text(db));
                }
                self.errors.push(
                    ConfigError::DirectVariableUnsupported {
                        site: field.as_call_site(db),
                        address,
                        why: if dv.partly(db) {
                            UnlocatableAddress::InStructPartly
                        } else {
                            UnlocatableAddress::InStruct
                        },
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }

            let element_type = field.spec(db).infer(db);
            if let Some(init_expr) = field.init(db) {
                self.init_expr_result.resolve_init_expr(
                    db,
                    init_expr,
                    &mut self.body_infer_result,
                    element_type,
                );
                self.check_string_init(db, field.spec(db), init_expr);
            }
        }
    }
}
